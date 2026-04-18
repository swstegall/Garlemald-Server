//! TCP listener + per-connection tasks. Each accepted socket splits into two
//! halves: a read-loop that decodes `BasePacket`s and hands them to the
//! `PacketProcessor`, and a write-loop that drains an mpsc channel of
//! already-serialized frames.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

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
    tokio::spawn(async move {
        while let Some(bytes) = rx.recv().await {
            let frame = common::wrap_subpackets_in_basepacket(bytes);
            let len = frame.len();
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

/// Load `server_zones` and open one TCP connection per unique map-server
/// endpoint. Each established connection is registered in `WorldMaster`
/// against every zone id that endpoint claims to own, and a reader task
/// is spawned to relay inbound subpackets back to their target session.
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

    // Group by endpoint so we open one socket per map server.
    let mut by_endpoint: HashMap<(String, u16), Vec<u32>> = HashMap::new();
    for (zone_id, ip, port) in rows {
        by_endpoint.entry((ip, port)).or_default().push(zone_id);
    }

    if by_endpoint.is_empty() {
        tracing::warn!("no zone servers configured in server_zones");
        return;
    }

    for ((ip, port), zone_ids) in by_endpoint {
        let addr = format!("{ip}:{port}");
        tracing::info!(%addr, zones = zone_ids.len(), "connecting to zone server");
        let socket = match TcpStream::connect(&addr).await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, %addr, "zone-server connect failed");
                continue;
            }
        };

        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(SEND_QUEUE_DEPTH);
        let (mut read_half, mut write_half) = tokio::io::split(socket);

        // Writer — wraps each outbound SubPacket payload in a BasePacket
        // frame, matching the server-to-server wire protocol used by the
        // map server's inbound reader.
        let addr_write = addr.clone();
        tokio::spawn(async move {
            while let Some(bytes) = rx.recv().await {
                let frame = common::wrap_subpackets_in_basepacket(bytes);
                if write_half.write_all(&frame).await.is_err() {
                    tracing::warn!(addr = %addr_write, "zone-server write failed");
                    break;
                }
            }
        });

        let handle = Arc::new(ZoneServerHandle {
            address: ip.clone(),
            port,
            owned_zone_ids: zone_ids.clone(),
            outbound: tx,
        });

        for zid in &zone_ids {
            world.register_zone_server(*zid, handle.clone()).await;
        }

        // Reader — parses BasePackets from the zone server, fans the
        // inner SubPackets out to the session identified by
        // `target_id`, and forwards them to that session's client
        // connection. Closes silently on zone-server disconnect.
        let sessions_r = sessions.clone();
        let addr_read = addr.clone();
        tokio::spawn(async move {
            let mut buffer = vec![0u8; BUFFER_SIZE];
            let mut pending = 0usize;
            loop {
                let n = match read_half.read(&mut buffer[pending..]).await {
                    Ok(0) => {
                        tracing::warn!(addr = %addr_read, "zone server disconnected");
                        return;
                    }
                    Ok(n) => n,
                    Err(e) => {
                        tracing::warn!(addr = %addr_read, error = %e, "zone-server read err");
                        return;
                    }
                };
                let bytes_in = pending + n;

                let mut offset = 0usize;
                while let Some(mut packet) = BasePacket::try_from_buffer(
                    &buffer[..bytes_in],
                    &mut offset,
                    bytes_in,
                ) {
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
                        let session = sessions_r.get(SessionChannel::Zone, target).await;
                        let session = match session {
                            Some(s) => Some(s),
                            None => sessions_r.get(SessionChannel::Chat, target).await,
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
        });
    }
}
