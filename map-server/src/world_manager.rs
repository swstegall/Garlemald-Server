//! Zone/actor registry. The C# `WorldManager` is 2000+ lines and owns:
//!   - zone templates (from DB)
//!   - per-zone actor lists
//!   - spawn lifecycle (respawn timers, seamless boundaries)
//!   - zone-change routing
//!   - the StaticActors singletons
//!
//! Phase 4 lands the public API surface so the packet processor can sit on
//! top of it; deep spawn/AI logic is TODO.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::actor::{BattleNpc, Npc, Player};
use crate::data::{SeamlessBoundary, Session, ZoneConnection};

/// In-memory zone descriptor. The real server populates this from
/// `zone_list` + `server_zones_seamless` + `server_zones_instance` on startup.
#[derive(Debug, Clone, Default)]
pub struct Zone {
    pub zone_id: u32,
    pub name: String,
    pub region_id: u32,
    pub is_isolated: bool,
    pub is_inn: bool,
    pub can_ride_chocobo: bool,
    pub can_stealth: bool,
    pub is_instance_raid: bool,
    pub is_private: bool,

    pub zone_connections: Vec<ZoneConnection>,
    pub seamless_boundaries: Vec<SeamlessBoundary>,

    pub players: HashMap<u32, Player>,
    pub npcs: HashMap<u32, Npc>,
    pub battle_npcs: HashMap<u32, BattleNpc>,
}

/// Top-level zone registry. Each operation acquires the per-zone lock, so
/// PvE actions in different zones don't contend with each other.
pub struct WorldManager {
    zones: RwLock<HashMap<u32, Arc<RwLock<Zone>>>>,
    sessions: RwLock<HashMap<u32, Session>>,
}

impl WorldManager {
    pub fn new() -> Self {
        Self { zones: RwLock::new(HashMap::new()), sessions: RwLock::new(HashMap::new()) }
    }

    /// Register (or replace) a zone template. Called once per zone during
    /// startup by the equivalent of `WorldManager.LoadZones()`.
    pub async fn register_zone(&self, zone: Zone) {
        let id = zone.zone_id;
        self.zones.write().await.insert(id, Arc::new(RwLock::new(zone)));
    }

    pub async fn zone(&self, zone_id: u32) -> Option<Arc<RwLock<Zone>>> {
        self.zones.read().await.get(&zone_id).cloned()
    }

    pub async fn add_player(&self, zone_id: u32, player: Player) {
        if let Some(zone) = self.zone(zone_id).await {
            let mut z = zone.write().await;
            z.players.insert(player.character.base.actor_id, player);
        }
    }

    pub async fn remove_player(&self, zone_id: u32, actor_id: u32) -> Option<Player> {
        if let Some(zone) = self.zone(zone_id).await {
            let mut z = zone.write().await;
            return z.players.remove(&actor_id);
        }
        None
    }

    pub async fn upsert_session(&self, session: Session) {
        self.sessions.write().await.insert(session.id, session);
    }

    pub async fn session(&self, id: u32) -> Option<Session> {
        self.sessions.read().await.get(&id).cloned()
    }

    pub async fn remove_session(&self, id: u32) -> Option<Session> {
        self.sessions.write().await.remove(&id)
    }
}

impl Default for WorldManager {
    fn default() -> Self {
        Self::new()
    }
}
