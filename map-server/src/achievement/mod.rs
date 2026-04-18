//! Achievements + titles runtime. Port of the "long tail" host-side
//! state that drives the achievement pop-up, the title dropdown, and
//! the achievement progress panel.
//!
//! The retail `PacketProcessor` treats achievements as a tiny surface:
//! read `AchievementProgressRequestPacket`, respond with a progress
//! packet from the DB. Earning/equipping mutations happen via Lua
//! scripts calling into the host. Our port matches that with the
//! outbox-first pattern already established in Phases 4/5/6/7.

#![allow(dead_code, unused_imports)]

pub mod dispatcher;
pub mod outbox;

pub use dispatcher::dispatch_achievement_event;
pub use outbox::{AchievementEvent, AchievementOutbox};

/// Retail achievement bit array length — matches
/// `SetCompletedAchievementsPacket` which packs 0x480 bits (0x240 bytes)
/// but the Map-Server emits only the first 0x500 bits in the send
/// builder we already have.
pub const COMPLETED_ACHIEVEMENTS_BITS: usize = 0x240 * 8;

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::actor::Character;
    use crate::actor::player::PlayerHelperState;
    use crate::data::ClientHandle;
    use crate::runtime::actor_registry::{ActorHandle, ActorKindTag, ActorRegistry};
    use crate::world_manager::WorldManager;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    fn character_named(actor_id: u32, name: &str) -> Character {
        let mut c = Character::new(actor_id);
        c.base.actor_name = name.to_string();
        c.base.custom_display_name = name.to_string();
        c
    }

    #[tokio::test]
    async fn earn_achievement_emits_three_events_and_queues_packets() {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        registry
            .insert(ActorHandle::new(
                1,
                ActorKindTag::Player,
                100,
                11,
                character_named(1, "Sender"),
            ))
            .await;
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(16);
        world.register_client(11, ClientHandle::new(11, tx)).await;

        let mut helper = PlayerHelperState::default();
        let mut outbox = AchievementOutbox::new();
        let earned =
            helper.earn_achievement(1, /* id */ 42, /* points */ 10, &mut outbox);
        assert!(earned);
        assert!(helper.has_achievement(42));
        assert_eq!(helper.achievement_points, 10);
        assert_eq!(helper.latest_achievements[0], 42);
        assert_eq!(outbox.len(), 3);

        for e in outbox.drain() {
            dispatch_achievement_event(&e, &registry, &world).await;
        }
        for _ in 0..3 {
            let got = rx.recv().await.expect("achievement packet on queue");
            assert!(!got.is_empty());
        }
    }

    #[tokio::test]
    async fn earning_same_achievement_twice_is_noop() {
        let mut helper = PlayerHelperState::default();
        let mut ob = AchievementOutbox::new();
        assert!(helper.earn_achievement(1, 42, 10, &mut ob));
        ob.drain();
        assert!(!helper.earn_achievement(1, 42, 10, &mut ob));
        assert!(ob.is_empty(), "dupe earn should be silent");
        assert_eq!(helper.achievement_points, 10);
    }

    #[tokio::test]
    async fn set_title_queues_packet() {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        registry
            .insert(ActorHandle::new(
                1,
                ActorKindTag::Player,
                100,
                11,
                character_named(1, "Sender"),
            ))
            .await;
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(4);
        world.register_client(11, ClientHandle::new(11, tx)).await;

        let mut helper = PlayerHelperState::default();
        let mut ob = AchievementOutbox::new();
        helper.set_title(1, /* title */ 777, &mut ob);
        assert_eq!(helper.current_title_id, 777);
        for e in ob.drain() {
            dispatch_achievement_event(&e, &registry, &world).await;
        }
        let got = rx.recv().await.expect("title packet on queue");
        assert!(!got.is_empty());
    }

    #[test]
    fn completed_bits_encode_earned_ids() {
        let mut helper = PlayerHelperState::default();
        let mut ob = AchievementOutbox::new();
        helper.earn_achievement(1, 5, 1, &mut ob);
        helper.earn_achievement(1, 100, 1, &mut ob);
        let bits = helper.completed_achievement_bits();
        assert_eq!(bits.len(), COMPLETED_ACHIEVEMENTS_BITS);
        assert!(bits[5]);
        assert!(bits[100]);
        assert!(!bits[6]);
    }

    #[test]
    fn retainer_slot_tracking_round_trip() {
        let mut helper = PlayerHelperState::default();
        assert!(!helper.has_spawned_retainer());
        helper.set_spawned_retainer(0x4000_0099);
        assert!(helper.has_spawned_retainer());
        assert_eq!(helper.current_spawned_retainer_id, 0x4000_0099);
        helper.clear_spawned_retainer();
        assert!(!helper.has_spawned_retainer());
    }
}
