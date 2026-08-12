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

//! Data objects ported from World Server/DataObjects.
//!
//! The Rust port splits the original C#'s `Session` + `ClientConnection`
//! tangle into two concerns:
//!   - `Session`   — stable per-player metadata (id, zone, active linkshell)
//!   - `ClientConn` — async I/O handle (owned by the `server` module)
//!
//! The channel on which a session arrived (ZONE vs CHAT) is kept as an enum.
//!
//! Several fields (zone-server address, session channel tag) aren't yet read
//! from any code path but are part of the wire schema the Map Server will
//! consume in Phase 4 — keep them around.
#![allow(dead_code)]

use std::sync::Arc;

use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionChannel {
    Zone,
    Chat,
}

/// Outbound-packet handle for a connected player. Cloneable and Send so
/// managers can stash them in shared state.
#[derive(Clone)]
pub struct ClientHandle {
    pub id: u32,
    /// Stable per-connection identity assigned at accept. Distinguishes a
    /// freshly-reconnected session from a superseded one so a stale teardown
    /// can't wipe the live connection. (`id` is seeded 0 and never overwritten,
    /// so it is unusable for connection identity — mirrors the C# reliance on
    /// `Object.ReferenceEquals(clientConnection, ...)`.)
    pub conn_seq: u64,
    pub tx: mpsc::Sender<Vec<u8>>,
    /// `(channel, session_id)` pairs this connection created, so its read-loop
    /// can tear them down on disconnect (mirrors C# `HandleClientDisconnect`).
    pub owned: Arc<Mutex<Vec<(SessionChannel, u32)>>>,
    /// Signalled to make this connection's read-loop exit early when it is
    /// evicted by a duplicate login (mirrors C# `ClientConnection.Disconnect()`).
    pub shutdown: Arc<Notify>,
}

impl ClientHandle {
    pub fn new(id: u32, conn_seq: u64, tx: mpsc::Sender<Vec<u8>>) -> Self {
        Self {
            id,
            conn_seq,
            tx,
            owned: Arc::new(Mutex::new(Vec::new())),
            shutdown: Arc::new(Notify::new()),
        }
    }

    /// Best-effort send. Drops if the channel is closed (client has
    /// disconnected).
    pub async fn send_bytes(&self, bytes: Vec<u8>) {
        let _ = self.tx.send(bytes).await;
    }

    /// Record a `(channel, session_id)` this connection owns so the read-loop
    /// can tear it down when the socket drops.
    pub async fn note_owned(&self, channel: SessionChannel, session_id: u32) {
        self.owned.lock().await.push((channel, session_id));
    }
}

/// Server-side record of a logical session. One per active zone / chat
/// connection. Mutable fields are behind an async Mutex so the PacketProcessor
/// can mutate routing info without cloning.
pub struct Session {
    pub session_id: u32,
    pub channel: SessionChannel,
    pub client: ClientHandle,
    pub state: Mutex<SessionState>,
}

#[derive(Debug, Clone, Default)]
pub struct SessionState {
    pub character_name: String,
    pub current_zone_id: u32,
    pub active_linkshell_name: String,
    pub routing1: Option<Arc<ZoneServerHandle>>,
    pub routing2: Option<Arc<ZoneServerHandle>>,
}

impl Session {
    pub fn new(session_id: u32, channel: SessionChannel, client: ClientHandle) -> Self {
        Self {
            session_id,
            channel,
            client,
            state: Mutex::new(SessionState::default()),
        }
    }
}

/// Opaque handle to a downstream zone server connection. Phase 3 populates
/// just enough of this to route session lifecycle packets; Phase 4 fills in
/// the rest from the map-server side.
#[derive(Debug)]
pub struct ZoneServerHandle {
    pub address: String,
    pub port: u16,
    pub owned_zone_ids: Vec<u32>,
    pub outbound: mpsc::Sender<Vec<u8>>,
}

impl ZoneServerHandle {
    pub async fn send_bytes(&self, bytes: Vec<u8>) {
        let _ = self.outbound.send(bytes).await;
    }
}
