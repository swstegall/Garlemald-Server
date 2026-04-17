//! Zone + session registry. Phase 1 trims the placeholder down and swaps it
//! onto the real `zone::Zone` so the game loop can drive it each tick.
//!
//! Character state (Players, Npcs, BattleNpcs) no longer lives on
//! `WorldManager` — it moves to `ActorRegistry` (`crate::runtime::actor_registry`).
//! The WorldManager now owns only:
//!
//! * The zone table — canonical `zone::Zone` instances keyed by zone id.
//! * The session table — `ClientHandle`s keyed by session id.
//!
//! Both are `RwLock<HashMap<_,_>>` so independent zones / sessions don't
//! contend during PvE ticks.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::data::{ClientHandle, Session};
use crate::zone::Zone;

/// Top-level zone + session registry.
pub struct WorldManager {
    zones: RwLock<HashMap<u32, Arc<RwLock<Zone>>>>,
    /// Sessions keyed by session id (from the packet source id).
    sessions: RwLock<HashMap<u32, Session>>,
    /// Live socket handles keyed by session id. Used by packet dispatchers
    /// to fan outbound SubPackets to the right clients.
    clients: RwLock<HashMap<u32, ClientHandle>>,
}

impl WorldManager {
    pub fn new() -> Self {
        Self {
            zones: RwLock::new(HashMap::new()),
            sessions: RwLock::new(HashMap::new()),
            clients: RwLock::new(HashMap::new()),
        }
    }

    // -----------------------------------------------------------------
    // Zones
    // -----------------------------------------------------------------

    /// Register (or replace) a zone. Called once per zone during startup.
    pub async fn register_zone(&self, zone: Zone) {
        let id = zone.core.actor_id;
        self.zones.write().await.insert(id, Arc::new(RwLock::new(zone)));
    }

    pub async fn zone(&self, zone_id: u32) -> Option<Arc<RwLock<Zone>>> {
        self.zones.read().await.get(&zone_id).cloned()
    }

    /// Snapshot of all zone ids — used by the game ticker to drive each
    /// zone's `update()` without holding a global lock.
    pub async fn zone_ids(&self) -> Vec<u32> {
        self.zones.read().await.keys().copied().collect()
    }

    pub async fn zone_count(&self) -> usize {
        self.zones.read().await.len()
    }

    // -----------------------------------------------------------------
    // Sessions
    // -----------------------------------------------------------------

    pub async fn upsert_session(&self, session: Session) {
        self.sessions.write().await.insert(session.id, session);
    }

    pub async fn session(&self, id: u32) -> Option<Session> {
        self.sessions.read().await.get(&id).cloned()
    }

    pub async fn remove_session(&self, id: u32) -> Option<Session> {
        self.clients.write().await.remove(&id);
        self.sessions.write().await.remove(&id)
    }

    // -----------------------------------------------------------------
    // Client handles (outbound-packet channels)
    // -----------------------------------------------------------------

    pub async fn register_client(&self, id: u32, handle: ClientHandle) {
        self.clients.write().await.insert(id, handle);
    }

    pub async fn client(&self, id: u32) -> Option<ClientHandle> {
        self.clients.read().await.get(&id).cloned()
    }

    pub async fn all_clients(&self) -> Vec<ClientHandle> {
        self.clients.read().await.values().cloned().collect()
    }
}

impl Default for WorldManager {
    fn default() -> Self {
        Self::new()
    }
}
