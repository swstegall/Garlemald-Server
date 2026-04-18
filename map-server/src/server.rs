//! Tokio TCP server. Accepts zone-server connections (inbound from the World
//! Server) and bridges them to the packet processor.

use std::sync::Arc;

use anyhow::{Context, Result};
use common::BasePacket;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use crate::config::Config;
use crate::data::ClientHandle;
use crate::database::Database;
use crate::lua::LuaEngine;
use crate::processor::PacketProcessor;
use crate::runtime::ActorRegistry;
use crate::world_manager::WorldManager;

const BUFFER_SIZE: usize = 0xFFFF;
const SEND_QUEUE_DEPTH: usize = 1000;

pub async fn run(
    config: Config,
    db: Arc<Database>,
    world: Arc<WorldManager>,
    registry: Arc<ActorRegistry>,
    lua: Arc<LuaEngine>,
) -> Result<()> {
    let addr = format!("{}:{}", config.bind_ip(), config.port());
    let listener = TcpListener::bind(&addr)
        .await
        .with_context(|| format!("bind {addr}"))?;
    tracing::info!(%addr, "map server listening");

    let processor = Arc::new(PacketProcessor {
        db,
        world,
        registry,
        lua: Some(lua),
    });

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
    peer: std::net::SocketAddr,
    processor: Arc<PacketProcessor>,
) -> Result<()> {
    let (mut read, mut write) = tokio::io::split(socket);

    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(SEND_QUEUE_DEPTH);
    // The write task wraps each outbound SubPacket in a BasePacket frame so
    // the world-server (our only inbound peer) can decode it via the same
    // `BasePacket::try_from_buffer` reader it uses for client traffic.
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

    // Session id isn't known until the first subpacket; start with 0 and let
    // the processor notice via `sub.header.source_id`.
    let client = ClientHandle::new(0, tx);

    let mut buffer = vec![0u8; BUFFER_SIZE];
    let mut pending = 0usize;
    loop {
        let n = read.read(&mut buffer[pending..]).await?;
        if n == 0 {
            tracing::info!(%peer, client_id = client.session_id, "disconnected");
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
                client_id = client.session_id,
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
