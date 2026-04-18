//! Broadcast fan-out. Port of the C# `Area.BroadcastPacketAroundActor`.
//!
//! Given a source actor, a SubPacket payload, and a zone, fan the packet
//! out to every Player within `BROADCAST_RADIUS` yalms — using the zone's
//! spatial grid (via the `ActorArena` impl) so we only pay for neighbours.
//!
//! The caller is responsible for supplying the raw SubPacket bytes.

#![allow(dead_code)]

use std::sync::Arc;

use tokio::sync::RwLock;

use crate::world_manager::WorldManager;
use crate::zone::area::{ActorKind, BROADCAST_RADIUS};
use crate::zone::zone::Zone;

use super::actor_registry::ActorRegistry;

/// Fan a SubPacket out to every Player within `BROADCAST_RADIUS` of the
/// source actor. `source_actor_id` is excluded from the fan-out (it won't
/// receive a copy of its own broadcast); pass the same actor in
/// `source_actor_id` regardless of whether it's a Player or an NPC.
///
/// Returns the number of clients the packet was queued to.
pub async fn broadcast_around_actor(
    world: &WorldManager,
    registry: &ActorRegistry,
    zone: &Arc<RwLock<Zone>>,
    source_actor_id: u32,
    packet_bytes: Vec<u8>,
) -> usize {
    let nearby = {
        let zone = zone.read().await;
        if zone.core.is_isolated {
            return 0;
        }
        zone.core
            .actors_around(source_actor_id, BROADCAST_RADIUS)
            .into_iter()
            .filter(|a| a.kind == ActorKind::Player)
            .map(|a| a.actor_id)
            .collect::<Vec<_>>()
    };

    let mut sent = 0usize;
    for player_actor_id in nearby {
        let Some(handle) = registry.get(player_actor_id).await else {
            continue;
        };
        let Some(client) = world.client(handle.session_id).await else {
            continue;
        };
        client.send_bytes(packet_bytes.clone()).await;
        sent += 1;
    }
    sent
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::Character;
    use crate::data::ClientHandle;
    use crate::runtime::actor_registry::{ActorHandle, ActorKindTag};
    use crate::zone::area::StoredActor;
    use crate::zone::navmesh::StubNavmeshLoader;
    use common::Vector3;
    use tokio::sync::mpsc;

    fn character() -> Character {
        Character::new(0)
    }

    #[tokio::test]
    async fn broadcast_queues_to_nearby_player_client() {
        let world = WorldManager::new();
        let registry = ActorRegistry::new();

        let mut zone = Zone::new(
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

        // Source BattleNpc at origin.
        let mut ob = crate::zone::outbox::AreaOutbox::new();
        zone.core.add_actor(
            StoredActor {
                actor_id: 1,
                kind: ActorKind::BattleNpc,
                position: Vector3::ZERO,
                grid: (0, 0),
                is_alive: true,
            },
            &mut ob,
        );
        // Nearby player at 10 yalms.
        zone.core.add_actor(
            StoredActor {
                actor_id: 10,
                kind: ActorKind::Player,
                position: Vector3::new(10.0, 0.0, 0.0),
                grid: (0, 0),
                is_alive: true,
            },
            &mut ob,
        );

        let zone_arc = Arc::new(RwLock::new(zone));
        world
            .register_zone({
                let z = zone_arc.read().await;
                Zone::new(
                    z.core.actor_id,
                    z.core.zone_name.clone(),
                    z.core.region_id,
                    z.core.class_path.clone(),
                    0,
                    0,
                    0,
                    false,
                    false,
                    false,
                    false,
                    false,
                    Some(&StubNavmeshLoader),
                )
            })
            .await;

        // Register the player handle + client socket.
        registry
            .insert(ActorHandle::new(
                10,
                ActorKindTag::Player,
                100,
                42,
                character(),
            ))
            .await;
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(4);
        world.register_client(42, ClientHandle::new(42, tx)).await;

        let sent = broadcast_around_actor(
            &world,
            &registry,
            &zone_arc,
            /* source */ 1,
            vec![1, 2, 3],
        )
        .await;
        assert_eq!(sent, 1);
        let got = rx.recv().await.unwrap();
        assert_eq!(got, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn broadcast_skips_isolated_zones() {
        let world = WorldManager::new();
        let registry = ActorRegistry::new();

        let zone = Zone::new(
            100,
            "inst",
            1,
            "/Area/Zone/Inst",
            0,
            0,
            0,
            /* is_isolated */ true,
            false,
            false,
            false,
            false,
            Some(&StubNavmeshLoader),
        );
        let zone_arc = Arc::new(RwLock::new(zone));
        let sent = broadcast_around_actor(&world, &registry, &zone_arc, 1, vec![]).await;
        assert_eq!(sent, 0);
    }
}
