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
//! Several fields (DBWorld extras, zone-server address, session channel tag)
//! aren't yet read from any code path but are part of the wire schema the
//! Map Server will consume in Phase 4 — keep them around.
#![allow(dead_code)]

use std::sync::Arc;

use tokio::sync::Mutex;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionChannel {
    Zone,
    Chat,
}

#[derive(Debug, Clone, Default)]
pub struct DBWorld {
    pub id: u32,
    pub address: String,
    pub port: u16,
    pub list_position: u16,
    pub population: u16,
    pub name: String,
    pub is_active: bool,
    pub motd: String,
}

/// Outbound-packet handle for a connected player. Cloneable and Send so
/// managers can stash them in shared state.
#[derive(Clone)]
pub struct ClientHandle {
    pub id: u32,
    pub tx: mpsc::Sender<Vec<u8>>,
}

impl ClientHandle {
    pub fn new(id: u32, tx: mpsc::Sender<Vec<u8>>) -> Self {
        Self { id, tx }
    }

    /// Best-effort send. Drops if the channel is closed (client has
    /// disconnected).
    pub async fn send_bytes(&self, bytes: Vec<u8>) {
        let _ = self.tx.send(bytes).await;
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
        Self { session_id, channel, client, state: Mutex::new(SessionState::default()) }
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
