//! Global actor registry. Every live `Character` in the server lives
//! behind one `ActorHandle`. The game ticker walks this map each tick to
//! drive `StatusEffectContainer::update` and `AIContainer::update`.
//!
//! Ownership model:
//!
//! * The `Character` (stat/mod/status/AI state) lives in the registry
//!   inside an `Arc<RwLock<Character>>`.
//! * Zones hold only lightweight `StoredActor` projections for spatial
//!   queries. The `actor_id` is the canonical foreign key.
//! * When an actor despawns, we remove it from both the registry and
//!   whichever zone held its projection.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::actor::Character;

/// Kind tag so dispatchers can tell Players apart from Npcs/BattleNpcs
/// without locking the Character first.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorKindTag {
    Player,
    Npc,
    BattleNpc,
    Ally,
    Pet,
}

#[derive(Clone)]
pub struct ActorHandle {
    pub actor_id: u32,
    pub kind: ActorKindTag,
    pub zone_id: u32,
    /// Session id for Players (0 for NPCs). Used by dispatchers to find
    /// the `ClientHandle` that should receive a packet.
    pub session_id: u32,
    pub character: Arc<RwLock<Character>>,
}

impl ActorHandle {
    pub fn new(
        actor_id: u32,
        kind: ActorKindTag,
        zone_id: u32,
        session_id: u32,
        character: Character,
    ) -> Self {
        Self {
            actor_id,
            kind,
            zone_id,
            session_id,
            character: Arc::new(RwLock::new(character)),
        }
    }

    pub fn is_player(&self) -> bool {
        self.kind == ActorKindTag::Player
    }
}

/// Central registry — every live Character is addressable by id.
pub struct ActorRegistry {
    actors: RwLock<HashMap<u32, ActorHandle>>,
    /// Reverse index: session_id → actor_id. Populated only for Players
    /// (Npcs don't carry a session). Used by the processor when it knows
    /// the session id but needs the actor.
    by_session: RwLock<HashMap<u32, u32>>,
}

impl ActorRegistry {
    pub fn new() -> Self {
        Self {
            actors: RwLock::new(HashMap::new()),
            by_session: RwLock::new(HashMap::new()),
        }
    }

    /// Insert / replace the handle for this actor.
    pub async fn insert(&self, handle: ActorHandle) {
        let id = handle.actor_id;
        let session_id = handle.session_id;
        let is_player = handle.is_player();
        self.actors.write().await.insert(id, handle);
        if is_player && session_id != 0 {
            self.by_session.write().await.insert(session_id, id);
        }
    }

    pub async fn remove(&self, actor_id: u32) -> Option<ActorHandle> {
        let removed = self.actors.write().await.remove(&actor_id);
        if let Some(h) = &removed
            && h.is_player()
            && h.session_id != 0
        {
            self.by_session.write().await.remove(&h.session_id);
        }
        removed
    }

    pub async fn remove_session(&self, session_id: u32) -> Option<ActorHandle> {
        let actor_id = self.by_session.write().await.remove(&session_id)?;
        self.actors.write().await.remove(&actor_id)
    }

    pub async fn get(&self, actor_id: u32) -> Option<ActorHandle> {
        self.actors.read().await.get(&actor_id).cloned()
    }

    pub async fn by_session(&self, session_id: u32) -> Option<ActorHandle> {
        let actor_id = *self.by_session.read().await.get(&session_id)?;
        self.actors.read().await.get(&actor_id).cloned()
    }

    pub async fn len(&self) -> usize {
        self.actors.read().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.actors.read().await.is_empty()
    }

    /// Snapshot of all handles — used by the ticker each frame to walk
    /// actors without holding the registry lock while state mutates.
    pub async fn snapshot(&self) -> Vec<ActorHandle> {
        self.actors.read().await.values().cloned().collect()
    }

    /// Snapshot of every actor in the given zone.
    pub async fn actors_in_zone(&self, zone_id: u32) -> Vec<ActorHandle> {
        self.actors
            .read()
            .await
            .values()
            .filter(|h| h.zone_id == zone_id)
            .cloned()
            .collect()
    }

    /// Move an actor from one zone to another — used by zone transitions.
    /// The caller still has to update the zone's spatial grid separately.
    pub async fn reassign_zone(&self, actor_id: u32, new_zone_id: u32) -> bool {
        let mut actors = self.actors.write().await;
        let Some(handle) = actors.get_mut(&actor_id) else {
            return false;
        };
        handle.zone_id = new_zone_id;
        true
    }
}

impl Default for ActorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn character() -> Character {
        Character::new(100)
    }

    #[tokio::test]
    async fn insert_and_lookup() {
        let reg = ActorRegistry::new();
        let h = ActorHandle::new(100, ActorKindTag::Player, 1, 42, character());
        reg.insert(h.clone()).await;
        assert_eq!(reg.len().await, 1);
        assert!(reg.get(100).await.is_some());
        assert!(reg.by_session(42).await.is_some());
    }

    #[tokio::test]
    async fn remove_clears_session_index() {
        let reg = ActorRegistry::new();
        let h = ActorHandle::new(100, ActorKindTag::Player, 1, 42, character());
        reg.insert(h).await;
        reg.remove(100).await;
        assert!(reg.by_session(42).await.is_none());
        assert_eq!(reg.len().await, 0);
    }

    #[tokio::test]
    async fn remove_session_removes_actor() {
        let reg = ActorRegistry::new();
        let h = ActorHandle::new(100, ActorKindTag::Player, 1, 42, character());
        reg.insert(h).await;
        reg.remove_session(42).await;
        assert!(reg.get(100).await.is_none());
    }

    #[tokio::test]
    async fn actors_in_zone_filters() {
        let reg = ActorRegistry::new();
        reg.insert(ActorHandle::new(
            100,
            ActorKindTag::Player,
            1,
            42,
            character(),
        ))
        .await;
        reg.insert(ActorHandle::new(
            200,
            ActorKindTag::BattleNpc,
            1,
            0,
            character(),
        ))
        .await;
        reg.insert(ActorHandle::new(300, ActorKindTag::Npc, 2, 0, character()))
            .await;
        let in_zone_1 = reg.actors_in_zone(1).await;
        assert_eq!(in_zone_1.len(), 2);
    }

    #[tokio::test]
    async fn npcs_skip_session_index() {
        let reg = ActorRegistry::new();
        reg.insert(ActorHandle::new(500, ActorKindTag::Npc, 1, 0, character()))
            .await;
        assert_eq!(reg.by_session(0).await.map(|h| h.actor_id), None);
    }

    #[tokio::test]
    async fn reassign_zone_updates_filter() {
        let reg = ActorRegistry::new();
        reg.insert(ActorHandle::new(
            100,
            ActorKindTag::BattleNpc,
            1,
            0,
            character(),
        ))
        .await;
        assert_eq!(reg.actors_in_zone(1).await.len(), 1);
        assert!(reg.reassign_zone(100, 2).await);
        assert_eq!(reg.actors_in_zone(1).await.len(), 0);
        assert_eq!(reg.actors_in_zone(2).await.len(), 1);
    }
}
