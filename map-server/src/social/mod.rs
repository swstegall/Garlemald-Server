//! Phase 7 — chat, social (friendlist/blacklist), recruitment, support
//! desk, GM commands. Port of the `Packets/Receive/Social|Recruitment|
//! SupportDesk/` handlers from `PacketProcessor.cs` + the very thin
//! `Actors/Command/` and `Actors/Debug/` types.
//!
//! Dispatch follows the project-wide outbox pattern: mutations emit
//! `SocialEvent`s; `dispatcher::dispatch_social_event` turns each into
//! the right packet send (chat broadcast, friendlist delta, GM ticket
//! response, etc). The processor layer parses incoming packets, reaches
//! for the outbox, pushes the corresponding event, then drains to
//! dispatch.

#![allow(dead_code, unused_imports)]

pub mod chat;
pub mod dispatcher;
pub mod friendlist;
pub mod outbox;
pub mod recruitment;
pub mod support;

pub use chat::{
    CHAT_LS, CHAT_PARTY, CHAT_SAY, CHAT_SHOUT, CHAT_SYSTEM, CHAT_SYSTEM_ERROR, CHAT_TELL,
    CHAT_YELL, ChatKind, message_type_from_u32,
};
pub use dispatcher::dispatch_social_event;
pub use friendlist::{BlacklistEntry, FriendlistEntry};
pub use outbox::{SocialEvent, SocialOutbox};

// ---------------------------------------------------------------------------
// Integration tests.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::actor::Character;
    use crate::data::ClientHandle;
    use crate::runtime::actor_registry::{ActorHandle, ActorKindTag, ActorRegistry};
    use crate::world_manager::WorldManager;
    use crate::zone::navmesh::StubNavmeshLoader;
    use crate::zone::zone::Zone;
    use common::Vector3;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    fn character_named(actor_id: u32, name: &str) -> Character {
        let mut c = Character::new(actor_id);
        c.base.actor_name = name.to_string();
        c.base.custom_display_name = name.to_string();
        c
    }

    #[tokio::test]
    async fn chat_broadcast_reaches_nearby_player_client() {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        let zone = Zone::new(
            100,
            "test",
            1,
            "/Area/Zone/Test",
            0,
            0,
            0,
            false,
            false,
            false,
            false,
            false,
            Some(&StubNavmeshLoader),
        );
        world.register_zone(zone).await;

        // Two Players in the same zone close to each other.
        let mut ob = crate::zone::outbox::AreaOutbox::new();
        {
            let z = world.zone(100).await.unwrap();
            let mut z = z.write().await;
            z.core.add_actor(
                crate::zone::area::StoredActor {
                    actor_id: 1,
                    kind: crate::zone::area::ActorKind::Player,
                    position: Vector3::ZERO,
                    grid: (0, 0),
                    is_alive: true,
                },
                &mut ob,
            );
            z.core.add_actor(
                crate::zone::area::StoredActor {
                    actor_id: 2,
                    kind: crate::zone::area::ActorKind::Player,
                    position: Vector3::new(10.0, 0.0, 0.0),
                    grid: (0, 0),
                    is_alive: true,
                },
                &mut ob,
            );
        }
        registry
            .insert(ActorHandle::new(
                1,
                ActorKindTag::Player,
                100,
                11,
                character_named(1, "Sender"),
            ))
            .await;
        registry
            .insert(ActorHandle::new(
                2,
                ActorKindTag::Player,
                100,
                22,
                character_named(2, "Nearby"),
            ))
            .await;

        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(16);
        world.register_client(22, ClientHandle::new(22, tx)).await;

        let event = SocialEvent::ChatBroadcast {
            source_actor_id: 1,
            kind: ChatKind::Say,
            sender_name: "Sender".to_string(),
            message: "hello world".to_string(),
        };
        dispatch_social_event(&event, &registry, &world).await;
        let got = rx.recv().await.expect("chat should reach nearby player");
        assert!(!got.is_empty());
    }

    #[tokio::test]
    async fn blacklist_add_queues_packet() {
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

        let event = SocialEvent::BlacklistAdded {
            actor_id: 1,
            name: "Griefer".to_string(),
            success: true,
        };
        dispatch_social_event(&event, &registry, &world).await;
        let got = rx.recv().await.expect("blacklist-added packet on queue");
        assert!(!got.is_empty());
    }

    #[tokio::test]
    async fn support_desk_faq_list_queues_packet() {
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

        let event = SocialEvent::FaqListRequested {
            actor_id: 1,
            faqs: vec!["Faq 1".into(), "Faq 2".into()],
        };
        dispatch_social_event(&event, &registry, &world).await;
        let got = rx.recv().await.expect("faq-list packet on queue");
        assert!(!got.is_empty());
    }
}
