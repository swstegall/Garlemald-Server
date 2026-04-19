//! TCP server loop. One tokio task per accepted connection; each task owns a
//! `LobbySession` and iteratively drains `BasePacket`s from the socket buffer.
//!
//! Replaces the BeginAccept/BeginReceive AsyncCallback chain of the C# original.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use common::BasePacket;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::config::Config;
use crate::processor::{LobbySession, PacketProcessor, Reply};

const BUFFER_SIZE: usize = 0xFFFF;

pub async fn run(config: Config, processor: PacketProcessor) -> Result<()> {
    let addr = format!("{}:{}", config.bind_ip(), config.port());
    let listener = TcpListener::bind(&addr).await.with_context(|| format!("bind {addr}"))?;
    tracing::info!(%addr, "lobby server listening");

    let processor = Arc::new(processor);

    loop {
        let (socket, peer) = match listener.accept().await {
            Ok(pair) => pair,
            Err(e) => {
                tracing::warn!(error = %e, "accept failed");
                continue;
            }
        };
        tracing::info!(%peer, "accepted connection");

        let processor = Arc::clone(&processor);
        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket, peer, processor).await {
                tracing::warn!(%peer, error = %e, "connection dropped");
            }
        });
    }
}

async fn handle_connection(
    mut socket: TcpStream,
    peer: SocketAddr,
    processor: Arc<PacketProcessor>,
) -> Result<()> {
    let mut session = LobbySession::default();
    let mut buffer = vec![0u8; BUFFER_SIZE];
    let mut pending = 0usize;

    loop {
        let n = socket.read(&mut buffer[pending..]).await?;
        if n == 0 {
            tracing::info!(%peer, user_id = session.current_user_id, "disconnected");
            return Ok(());
        }
        common::packet_log::log_inbound(peer, &buffer[pending..pending + n]);
        let bytes_in = pending + n;
        tracing::trace!(%peer, bytes = n, total = bytes_in, "socket read");

        let mut offset = 0usize;
        while let Some(packet) = BasePacket::try_from_buffer(&buffer[..bytes_in], &mut offset, bytes_in) {
            tracing::debug!(
                %peer,
                size = packet.header.packet_size,
                subpackets = packet.header.num_subpackets,
                "packet in"
            );
            let replies = processor.process(&mut session, packet).await?;
            tracing::trace!(%peer, replies = replies.len(), "packet processed");
            for reply in replies {
                send_reply(&mut socket, &session, reply).await?;
            }
        }

        // Move any leftover bytes to the front, like the C# `Array.Copy`.
        if offset < bytes_in {
            buffer.copy_within(offset..bytes_in, 0);
        }
        pending = bytes_in - offset;
        // Zero the tail so stale bytes never leak into a future parse.
        buffer[pending..].fill(0);
    }
}

async fn send_reply(
    socket: &mut TcpStream,
    session: &LobbySession,
    reply: Reply,
) -> Result<()> {
    let bytes = match reply {
        Reply::Raw(bytes) => bytes,
        Reply::Encrypted(subs) => {
            let mut packet = BasePacket::create_from_subpackets(&subs, true, false)?;
            if let Some(bf) = session.blowfish.as_ref() {
                packet.encrypt(bf)?;
            }
            packet.to_bytes()
        }
    };
    let len = bytes.len();
    if let Ok(peer) = socket.peer_addr() {
        common::packet_log::log_outbound(peer, &bytes);
    }
    socket.write_all(&bytes).await?;
    tracing::trace!(bytes = len, "reply sent");
    Ok(())
}
