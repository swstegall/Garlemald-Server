// garlemald-server — Rust port of a FINAL FANTASY XIV v1.23b server emulator (lobby/world/map)
// Copyright (C) 2026  Samuel Stegall
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: AGPL-3.0-or-later

//! TCP listener + per-connection tasks. Each accepted socket splits into two
//! halves: a read-loop that decodes `BasePacket`s and hands them to the
//! `PacketProcessor`, and a write-loop that drains an mpsc channel of
//! already-serialized frames.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use common::BasePacket;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, mpsc};

use crate::config::Config;
use crate::data::{ClientHandle, Session, SessionChannel, ZoneServerHandle};
use crate::database::Database;
use crate::processor::PacketProcessor;
use crate::world_master::WorldMaster;

const BUFFER_SIZE: usize = 0xFFFF;
const SEND_QUEUE_DEPTH: usize = 1000;

/// Thread-safe registry of live sessions, keyed by channel + id. Matches the
/// C# pair of `mZoneSessionList` / `mChatSessionList` dictionaries.
pub struct SessionRegistry {
    zone: Mutex<HashMap<u32, Arc<Session>>>,
    chat: Mutex<HashMap<u32, Arc<Session>>>,
    id_to_name: Mutex<HashMap<u32, String>>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self {
            zone: Mutex::new(HashMap::new()),
            chat: Mutex::new(HashMap::new()),
            id_to_name: Mutex::new(HashMap::new()),
        }
    }

    pub async fn add(&self, channel: SessionChannel, id: u32, session: Arc<Session>) {
        let map = match channel {
            SessionChannel::Zone => &self.zone,
            SessionChannel::Chat => &self.chat,
        };
        map.lock().await.insert(id, session.clone());

        // Remember the character name for debug/chat lookups.
        let name = session.state.lock().await.character_name.clone();
        if !name.is_empty() {
            self.id_to_name.lock().await.insert(id, name);
        }
    }

    pub async fn get(&self, channel: SessionChannel, id: u32) -> Option<Arc<Session>> {
        let map = match channel {
            SessionChannel::Zone => &self.zone,
            SessionChannel::Chat => &self.chat,
        };
        map.lock().await.get(&id).cloned()
    }

    pub async fn remove(&self, channel: SessionChannel, id: u32) {
        let map = match channel {
            SessionChannel::Zone => &self.zone,
            SessionChannel::Chat => &self.chat,
        };
        map.lock().await.remove(&id);
    }

    #[allow(dead_code)]
    pub async fn get_name(&self, id: u32) -> Option<String> {
        self.id_to_name.lock().await.get(&id).cloned()
    }

    pub async fn preload_names(&self, names: Vec<(u32, String)>) {
        let mut map = self.id_to_name.lock().await;
        for (id, name) in names {
            map.insert(id, name);
        }
    }
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn run(config: Config, db: Arc<Database>, world: Arc<WorldMaster>) -> Result<()> {
    let addr = format!("{}:{}", config.bind_ip(), config.port());
    let listener = TcpListener::bind(&addr).await.with_context(|| format!("bind {addr}"))?;
    tracing::info!(%addr, "world server listening");

    let sessions = Arc::new(SessionRegistry::new());

    // Preload chara names like the C# `LoadCharaNames()` startup step.
    if let Ok(all) = db.get_all_chara_names().await {
        sessions.preload_names(all).await;
    }

    // Open connections to every map (zone) server advertised in
    // `server_zones`, one per unique (ip, port). Mirrors the C#
    // `WorldMaster.ConnectToZoneServers()` startup step.
    connect_zone_servers(&db, &world, &sessions).await;

    let processor = Arc::new(PacketProcessor { db: db.clone(), world, sessions: sessions.clone() });

    loop {
        let (socket, peer) = match listener.accept().await {
            Ok(pair) => pair,
            Err(e) => {
                tracing::warn!(error = %e, "accept failed");
                continue;
            }
        };
        tracing::info!(%peer, "accepted connection");

        let proc = Arc::clone(&processor);
        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket, peer, proc).await {
                tracing::warn!(%peer, error = %e, "connection dropped");
            }
        });
    }
}

async fn handle_connection(
    socket: TcpStream,
    peer: SocketAddr,
    processor: Arc<PacketProcessor>,
) -> Result<()> {
    let (mut read, mut write) = tokio::io::split(socket);

    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(SEND_QUEUE_DEPTH);
    // Dedicated write task — drains the outbound queue until the receiver
    // closes or the peer stops accepting bytes. Callers hand this task raw
    // SubPacket bytes; we wrap each chunk in a BasePacket frame so the
    // client's framing parser sees a well-formed packet_size.
    let writer_peer = peer;
    tokio::spawn(async move {
        while let Some(bytes) = rx.recv().await {
            let frame = common::wrap_subpackets_in_basepacket(bytes);
            let len = frame.len();
            common::packet_log::log_outbound(writer_peer, &frame);
            if write.write_all(&frame).await.is_err() {
                break;
            }
            tracing::trace!(bytes = len, "reply sent");
        }
    });

    // Before the first hello we don't know the player's id; seed with 0 and
    // let the hello handler overwrite `ClientHandle.id` via a new struct.
    let client = ClientHandle::new(0, tx);

    let mut buffer = vec![0u8; BUFFER_SIZE];
    let mut pending = 0usize;
    loop {
        let n = read.read(&mut buffer[pending..]).await?;
        if n == 0 {
            tracing::info!(%peer, client_id = client.id, "disconnected");
            return Ok(());
        }
        common::packet_log::log_inbound(peer, &buffer[pending..pending + n]);
        let bytes_in = pending + n;
        tracing::trace!(%peer, bytes = n, total = bytes_in, "socket read");

        let mut offset = 0usize;
        while let Some(packet) =
            BasePacket::try_from_buffer(&buffer[..bytes_in], &mut offset, bytes_in)
        {
            tracing::debug!(
                %peer,
                client_id = client.id,
                size = packet.header.packet_size,
                subpackets = packet.header.num_subpackets,
                "packet in"
            );
            if let Err(e) = processor.process_packet(&client, packet).await {
                tracing::warn!(error = %e, "packet processing error");
            }
        }

        if offset < bytes_in {
            buffer.copy_within(offset..bytes_in, 0);
        }
        pending = bytes_in - offset;
        buffer[pending..].fill(0);
    }
}

/// Load `server_zones` and spawn one supervisor task per unique map-server
/// endpoint. Each supervisor loops: connect → register handle against every
/// zone id it owns → drive reader/writer halves → on disconnect, deregister
/// and retry with backoff. This decouples startup ordering: the world server
/// can boot before the map server is listening, and transient map-server
/// restarts automatically reattach.
async fn connect_zone_servers(
    db: &Arc<Database>,
    world: &Arc<WorldMaster>,
    sessions: &Arc<SessionRegistry>,
) {
    let rows = match db.get_server_zones().await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!(error = %e, "server_zones query failed; zones unrouted");
            return;
        }
    };

    let mut by_endpoint: HashMap<(String, u16), Vec<u32>> = HashMap::new();
    for (zone_id, ip, port) in rows {
        by_endpoint.entry((ip, port)).or_default().push(zone_id);
    }

    if by_endpoint.is_empty() {
        tracing::warn!("no zone servers configured in server_zones");
        return;
    }

    for ((ip, port), zone_ids) in by_endpoint {
        let world = world.clone();
        let sessions = sessions.clone();
        tokio::spawn(async move {
            supervise_zone_endpoint(ip, port, zone_ids, world, sessions).await;
        });
    }
}

/// Retry-forever connection supervisor for a single map-server endpoint.
async fn supervise_zone_endpoint(
    ip: String,
    port: u16,
    zone_ids: Vec<u32>,
    world: Arc<WorldMaster>,
    sessions: Arc<SessionRegistry>,
) {
    let addr = format!("{ip}:{port}");
    let mut backoff = Duration::from_secs(1);
    let max_backoff = Duration::from_secs(30);

    loop {
        tracing::info!(%addr, zones = zone_ids.len(), "connecting to zone server");
        let socket = match TcpStream::connect(&addr).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, %addr, retry_in = ?backoff, "zone-server connect failed");
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(max_backoff);
                continue;
            }
        };
        tracing::info!(%addr, "zone server connected");
        backoff = Duration::from_secs(1);

        run_zone_connection(socket, &ip, port, &zone_ids, &world, &sessions).await;

        tracing::warn!(%addr, retry_in = ?backoff, "zone server disconnected; will retry");
        tokio::time::sleep(backoff).await;
    }
}

/// Drive a single live connection to a map-server endpoint until it drops.
/// Registers the handle against every owned zone id on entry; the caller
/// deregisters on return.
async fn run_zone_connection(
    socket: TcpStream,
    ip: &str,
    port: u16,
    zone_ids: &[u32],
    world: &Arc<WorldMaster>,
    sessions: &Arc<SessionRegistry>,
) {
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(SEND_QUEUE_DEPTH);
    let (mut read_half, mut write_half) = tokio::io::split(socket);

    let addr = format!("{ip}:{port}");

    // Writer — wraps each outbound SubPacket payload in a BasePacket frame,
    // matching the server-to-server wire protocol used by the map server's
    // inbound reader.
    let addr_write = addr.clone();
    let writer = tokio::spawn(async move {
        while let Some(bytes) = rx.recv().await {
            let frame = common::wrap_subpackets_in_basepacket(bytes);
            common::packet_log::log_outbound_named(&addr_write, &frame);
            if write_half.write_all(&frame).await.is_err() {
                tracing::warn!(addr = %addr_write, "zone-server write failed");
                break;
            }
        }
    });

    let handle = Arc::new(ZoneServerHandle {
        address: ip.to_string(),
        port,
        owned_zone_ids: zone_ids.to_vec(),
        outbound: tx,
    });
    for zid in zone_ids {
        world.register_zone_server(*zid, handle.clone()).await;
    }

    // Reader — parses BasePackets from the zone server, fans the inner
    // SubPackets out to the session identified by `target_id`, and forwards
    // them to that session's client connection.
    let mut buffer = vec![0u8; BUFFER_SIZE];
    let mut pending = 0usize;
    loop {
        let n = match read_half.read(&mut buffer[pending..]).await {
            Ok(0) => {
                tracing::warn!(%addr, "zone server disconnected");
                break;
            }
            Ok(n) => n,
            Err(e) => {
                tracing::warn!(%addr, error = %e, "zone-server read err");
                break;
            }
        };
        common::packet_log::log_inbound_named(&addr, &buffer[pending..pending + n]);
        let bytes_in = pending + n;

        let mut offset = 0usize;
        while let Some(mut packet) =
            BasePacket::try_from_buffer(&buffer[..bytes_in], &mut offset, bytes_in)
        {
            if packet.header.is_compressed == 0x01
                && let Err(e) = packet.decompress()
            {
                tracing::warn!(error = %e, "zone reply decompress failed");
                continue;
            }
            let subs = match packet.get_subpackets() {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(error = %e, "zone reply subpacket parse failed");
                    continue;
                }
            };
            for sub in subs {
                let target = sub.header.target_id;
                if target == 0 {
                    continue;
                }
                // Try Zone channel first; Chat sessions also receive
                // peer-to-peer forwards, so fall back there.
                let session = sessions.get(SessionChannel::Zone, target).await;
                let session = match session {
                    Some(s) => Some(s),
                    None => sessions.get(SessionChannel::Chat, target).await,
                };
                if let Some(session) = session {
                    session.client.send_bytes(sub.to_bytes()).await;
                } else {
                    tracing::debug!(target, "zone reply to unknown session");
                }
            }
        }

        if offset < bytes_in {
            buffer.copy_within(offset..bytes_in, 0);
        }
        pending = bytes_in - offset;
        buffer[pending..].fill(0);
    }

    // Deregister so new work stops landing on the dying handle, then drop
    // our own reference. Active sessions may still hold clones via
    // `routing1`/`routing2`; the writer task exits whenever those finally
    // drop. We don't await it — blocking on orphaned sessions would stall
    // reconnect.
    world.unregister_zone_server(zone_ids).await;
    drop(handle);
    drop(writer);
}
