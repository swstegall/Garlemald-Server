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
use crate::data::{ClientHandle, Session, SessionChannel};
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
    // closes or the peer stops accepting bytes.
    tokio::spawn(async move {
        while let Some(bytes) = rx.recv().await {
            let len = bytes.len();
            if write.write_all(&bytes).await.is_err() {
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
