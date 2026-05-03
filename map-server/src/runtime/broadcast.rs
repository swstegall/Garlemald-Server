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
/// Private-area filtering: looks up the source's session and, if they're
/// in a named private area (per `Session::current_private_area_name`,
/// set by `WorldManager::do_zone_change_with_private_area` in commit
/// `c624570`), iterates that `PrivateArea`'s `core` actor pool instead
/// of the parent zone's `core`. Recipients in different private-area
/// instances (or the parent zone) are inherently excluded because they
/// live in a different actor list. Sources without a session (NPCs)
/// always read from the parent zone — NPC routing into private areas
/// isn't yet wired beyond the CreateContentArea path. Same isolation
/// model as C# `Player.CurrentArea`-driven `BroadcastPacketAroundActor`.
///
/// Returns the number of clients the packet was queued to.
pub async fn broadcast_around_actor(
    world: &WorldManager,
    registry: &ActorRegistry,
    zone: &Arc<RwLock<Zone>>,
    source_actor_id: u32,
    packet_bytes: Vec<u8>,
) -> usize {
    // Resolve the source's private-area routing (if any). NPCs and
    // sourceless broadcasts fall through to the parent zone's `core`.
    let source_private_area: Option<(String, u32)> = match registry.get(source_actor_id).await {
        Some(handle) if handle.session_id != 0 => match world.session(handle.session_id).await {
            Some(s) => s
                .current_private_area_name
                .clone()
                .map(|n| (n, s.current_private_area_level)),
            None => None,
        },
        _ => None,
    };
    let nearby = {
        let zone = zone.read().await;
        if zone.core.is_isolated {
            return 0;
        }
        // Pick the right `AreaCore` based on private-area routing.
        // `unwrap_or(&zone.core)` covers the edge case where the
        // source's session references a private area that isn't
        // installed for this zone (warn-and-fallback in
        // `do_zone_change_with_private_area` handles the routing
        // side; here we just match its parent-zone fallback).
        let core = if let Some((ref name, level)) = source_private_area {
            zone.private_areas
                .get(name)
                .and_then(|m| m.get(&level))
                .map(|pa| &pa.core)
                .unwrap_or(&zone.core)
        } else {
            &zone.core
        };
        core.actors_around(source_actor_id, BROADCAST_RADIUS)
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

    /// Private-area isolation contract: a player whose session is
    /// flagged into a named PrivateArea should only see broadcasts
    /// from sources also in that area. The parent-zone copy of the
    /// recipient list is invisible to them, and vice versa.
    #[tokio::test]
    async fn broadcast_filters_by_private_area_routing() {
        use crate::data::Session;
        use crate::zone::private_area::PrivateArea;

        let world = WorldManager::new();
        let registry = ActorRegistry::new();

        // Zone with one named PrivateArea ("Past", level=0).
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
        let mut past = PrivateArea::new(
            100,
            "test",
            1,
            200,
            "/Area/Zone/Test/Past",
            "Past",
            0,
            0,
            0,
            0,
            false,
            false,
            false,
            false,
        );

        // Source player (id=1) sits in the PrivateArea.
        let mut ob = crate::zone::outbox::AreaOutbox::new();
        past.core.add_actor(
            StoredActor {
                actor_id: 1,
                kind: ActorKind::Player,
                position: Vector3::ZERO,
                grid: (0, 0),
                is_alive: true,
            },
            &mut ob,
        );
        // Co-located player (id=2) ALSO in the PrivateArea — should
        // receive the broadcast.
        past.core.add_actor(
            StoredActor {
                actor_id: 2,
                kind: ActorKind::Player,
                position: Vector3::new(5.0, 0.0, 0.0),
                grid: (0, 0),
                is_alive: true,
            },
            &mut ob,
        );
        zone.add_private_area(past);
        // Player (id=3) in the parent zone (NOT in the PrivateArea)
        // — should NOT receive the broadcast.
        zone.core.add_actor(
            StoredActor {
                actor_id: 3,
                kind: ActorKind::Player,
                position: Vector3::new(5.0, 0.0, 0.0),
                grid: (0, 0),
                is_alive: true,
            },
            &mut ob,
        );

        let zone_arc = Arc::new(RwLock::new(zone));

        // Register handles + client sockets for both recipients.
        registry
            .insert(ActorHandle::new(
                1,
                ActorKindTag::Player,
                100,
                /* session */ 1,
                character(),
            ))
            .await;
        registry
            .insert(ActorHandle::new(
                2,
                ActorKindTag::Player,
                100,
                /* session */ 2,
                character(),
            ))
            .await;
        registry
            .insert(ActorHandle::new(
                3,
                ActorKindTag::Player,
                100,
                /* session */ 3,
                character(),
            ))
            .await;
        let (tx2, mut rx2) = mpsc::channel::<Vec<u8>>(4);
        let (tx3, mut rx3) = mpsc::channel::<Vec<u8>>(4);
        world.register_client(2, ClientHandle::new(2, tx2)).await;
        world.register_client(3, ClientHandle::new(3, tx3)).await;

        // Source's session is flagged into the PrivateArea — that's
        // what the broadcast filter keys off.
        let mut source_session = Session::new(1);
        source_session.current_zone_id = 100;
        source_session.current_private_area_name = Some("Past".to_string());
        source_session.current_private_area_level = 0;
        world.upsert_session(source_session).await;

        let sent = broadcast_around_actor(
            &world,
            &registry,
            &zone_arc,
            /* source */ 1,
            vec![0xAA, 0xBB],
        )
        .await;

        // Only the co-located player in the PrivateArea should get
        // the packet.
        assert_eq!(sent, 1, "exactly one recipient (the co-located player)");
        let got = rx2.try_recv().expect("co-located player got the packet");
        assert_eq!(got, vec![0xAA, 0xBB]);
        // Parent-zone player must NOT have received anything.
        assert!(
            rx3.try_recv().is_err(),
            "parent-zone player should NOT see PrivateArea broadcast",
        );
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
