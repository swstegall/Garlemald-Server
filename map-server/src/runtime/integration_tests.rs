//! End-to-end game-loop integration tests. Exercises the full pipeline:
//! Actor + Zone → Battle engine → BattleOutbox → dispatcher → SubPacket
//! → SessionRegistry → ClientHandle → test-side mpsc receiver.

#![cfg(test)]

use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::sync::RwLock;

use crate::actor::Character;
use crate::battle::command::{CommandResult, CommandType};
use crate::battle::effects::{ActionProperty, ActionType, HitType};
use crate::battle::outbox::BattleEvent;
use crate::data::ClientHandle;
use crate::runtime::actor_registry::{ActorHandle, ActorKindTag, ActorRegistry};
use crate::runtime::dispatcher::dispatch_battle_event;
use crate::world_manager::WorldManager;
use crate::zone::area::{ActorKind, StoredActor};
use crate::zone::navmesh::StubNavmeshLoader;
use crate::zone::outbox::AreaOutbox;
use crate::zone::zone::Zone;
use common::Vector3;

#[tokio::test]
async fn do_battle_action_reaches_player_client_queue() {
    // Scene: Zone 100 contains a BattleNpc (attacker, id=1) at origin and
    // a Player (victim, id=10) at (5, 0, 0) with session_id=42.
    let world = Arc::new(WorldManager::new());
    let registry = Arc::new(ActorRegistry::new());

    // Build zone + its in-memory replica so we can snapshot it before
    // registering.
    let mut canonical = Zone::new(
        100, "test", 1, "/Area/Zone/Test", 0, 0, 0, false, false, false, false, false,
        Some(&StubNavmeshLoader),
    );
    let mut ob = AreaOutbox::new();
    canonical.core.add_actor(
        StoredActor {
            actor_id: 1,
            kind: ActorKind::BattleNpc,
            position: Vector3::ZERO,
            grid: (0, 0),
            is_alive: true,
        },
        &mut ob,
    );
    canonical.core.add_actor(
        StoredActor {
            actor_id: 10,
            kind: ActorKind::Player,
            position: Vector3::new(5.0, 0.0, 0.0),
            grid: (0, 0),
            is_alive: true,
        },
        &mut ob,
    );
    world.register_zone(canonical).await;

    // Register the attacker Character and the victim Player handle.
    registry
        .insert(ActorHandle::new(
            1,
            ActorKindTag::BattleNpc,
            100,
            0,
            Character::new(1),
        ))
        .await;
    registry
        .insert(ActorHandle::new(
            10,
            ActorKindTag::Player,
            100,
            42,
            Character::new(10),
        ))
        .await;

    // Attach a ClientHandle for session 42 with a test-side receiver.
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(16);
    world.register_client(42, ClientHandle::new(42, tx)).await;

    // Build a DoBattleAction event: one hit against the player.
    let mut result = CommandResult::for_target(10, 30301, 0);
    result.amount = 120;
    result.action_type = ActionType::Physical;
    result.action_property = ActionProperty::Slashing;
    result.command_type = CommandType::AUTO_ATTACK;
    result.hit_type = HitType::Hit;

    let event = BattleEvent::DoBattleAction {
        owner_actor_id: 1,
        skill_handler: 0x765D,
        battle_animation: 0x1100_0001,
        results: vec![result],
    };

    let zone_arc = world.zone(100).await.unwrap();
    dispatch_battle_event(&event, &registry, &world, &zone_arc).await;

    // The player's ClientHandle should have received at least one SubPacket.
    let got = rx
        .recv()
        .await
        .expect("DoBattleAction should have produced a packet");
    assert!(!got.is_empty(), "packet payload should be non-empty");
}

#[tokio::test]
async fn hate_add_event_updates_attacker_hate_container() {
    let world = Arc::new(WorldManager::new());
    let registry = Arc::new(ActorRegistry::new());

    let zone = Zone::new(
        100, "test", 1, "/Area/Zone/Test", 0, 0, 0, false, false, false, false, false,
        Some(&StubNavmeshLoader),
    );
    world.register_zone(zone).await;
    registry
        .insert(ActorHandle::new(
            1,
            ActorKindTag::BattleNpc,
            100,
            0,
            Character::new(1),
        ))
        .await;

    let event = BattleEvent::HateAdd {
        owner_actor_id: 1,
        target_actor_id: 10,
        amount: 250,
    };
    let zone_arc = world.zone(100).await.unwrap();
    dispatch_battle_event(&event, &registry, &world, &zone_arc).await;

    let handle = registry.get(1).await.unwrap();
    let chara = handle.character.read().await;
    assert_eq!(chara.hate.most_hated(), Some(10));
    assert!(chara.hate.get(10).unwrap().cumulative_enmity >= 250);
    drop(chara);
    // Silence the unused-import warning on Arc/RwLock when the test above
    // doesn't reach for them.
    let _ = Arc::new(RwLock::new(()));
}
