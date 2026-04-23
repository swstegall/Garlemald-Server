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

//! Turn `DirectorEvent`s into the matching packet bundles on the
//! right player's socket. Same outbox-first pattern as the rest of
//! the server.
//!
//! Full director-actor-spawn packets (ActorInstantiate + Init) need
//! the director's Lua class path + params — those come from the Phase 4
//! event dispatcher when the script runs. Here we hand off the
//! member-facing side effects: music, game messages, remove packets on
//! end, property packets for guildleve-work.

#![allow(dead_code)]

use std::sync::Arc;

use crate::database::Database;
use crate::packets::send as tx;
use crate::runtime::actor_registry::ActorRegistry;
use crate::world_manager::WorldManager;

use super::guildleve::{GL_TEXT_ABANDON, GL_TEXT_COMPLETE, GL_TEXT_START, GL_TEXT_TIME_LIMIT};
use super::outbox::DirectorEvent;

/// The game loop drains the DirectorOutbox and hands each event to
/// `dispatch_director_event` along with the director's current player
/// roster (which lives on the `Director::player_members` set on the
/// in-memory struct — see `zone::area::AreaCore::director_mut`).
///
/// `db` is required by the GC seal-reward branch on `GuildleveEnded` —
/// `Option`-wrapped so test harnesses (and any future caller that's
/// purely packet-side) can pass `None` without standing up a database.
pub async fn dispatch_director_event(
    event: &DirectorEvent,
    player_members: &[u32],
    registry: &ActorRegistry,
    world: &WorldManager,
    db: Option<&Database>,
) {
    match event {
        DirectorEvent::DirectorStarted {
            director_id,
            zone_id,
            class_path,
            class_name,
            actor_name,
            spawn_immediate,
        } => {
            tracing::debug!(
                director = director_id,
                zone = zone_id,
                class = %class_path,
                name = %class_name,
                actor = %actor_name,
                spawn = spawn_immediate,
                "director started — actor-instantiate dispatch pending",
            );
        }
        DirectorEvent::DirectorEnded { director_id, .. } => {
            // Tell every current member the director actor went away.
            for actor_id in player_members {
                let Some(handle) = registry.get(*actor_id).await else {
                    continue;
                };
                let _ = handle.session_id;
                let sub = tx::actor::build_remove_actor(*director_id);
                if let Some(client) = world.client(handle.session_id).await {
                    client.send_bytes(sub.to_bytes()).await;
                }
            }
        }
        DirectorEvent::MainCoroutine { director_id } => {
            tracing::debug!(director = director_id, "director main() (Lua hook pending)");
        }
        DirectorEvent::EventStarted {
            director_id,
            player_actor_id,
        } => {
            tracing::debug!(
                director = director_id,
                player = ?player_actor_id,
                "director onEventStarted (Lua hook pending)",
            );
        }
        DirectorEvent::MemberAdded { .. } | DirectorEvent::MemberRemoved { .. } => {
            // Director's own member list is maintained in Rust. The
            // ContentGroup side (Phase 6) handles the client-visible
            // roster; the Director-level event is logging-only.
        }
        DirectorEvent::GuildleveStarted {
            director_id,
            guildleve_id,
            difficulty,
            location,
            time_limit_seconds,
            start_time_unix: _,
        } => {
            for actor_id in player_members {
                let Some(handle) = registry.get(*actor_id).await else {
                    continue;
                };
                let client = match world.client(handle.session_id).await {
                    Some(c) => c,
                    None => continue,
                };
                // Music swap based on location bucket.
                if let Some(music_id) = super::guildleve::GuildleveLocationMusic::pick(*location) {
                    let sub = tx::build_set_music(handle.session_id, music_id, 0);
                    client.send_bytes(sub.to_bytes()).await;
                }
                // "You have begun guildleve X" text.
                let start_msg = tx::build_game_message(
                    handle.session_id,
                    tx::GameMessageOptions {
                        sender_actor_id: 0,
                        receiver_actor_id: handle.session_id,
                        text_id: GL_TEXT_START,
                        log: 0x20,
                        display_id: None,
                        custom_sender: None,
                        lua_params: vec![
                            common::luaparam::LuaParam::UInt32(*guildleve_id),
                            common::luaparam::LuaParam::UInt32(*difficulty as u32),
                        ],
                    },
                );
                client.send_bytes(start_msg.to_bytes()).await;
                // Time-limit hint.
                let limit_msg = tx::build_game_message(
                    handle.session_id,
                    tx::GameMessageOptions {
                        sender_actor_id: 0,
                        receiver_actor_id: handle.session_id,
                        text_id: GL_TEXT_TIME_LIMIT,
                        log: 0x20,
                        display_id: None,
                        custom_sender: None,
                        lua_params: vec![common::luaparam::LuaParam::UInt32(*time_limit_seconds)],
                    },
                );
                client.send_bytes(limit_msg.to_bytes()).await;
                let _ = director_id;
            }
        }
        DirectorEvent::GuildleveEnded {
            director_id,
            guildleve_id,
            was_completed,
            completion_time_seconds: _,
            difficulty,
        } => {
            for actor_id in player_members {
                let Some(handle) = registry.get(*actor_id).await else {
                    continue;
                };
                let client = match world.client(handle.session_id).await {
                    Some(c) => c,
                    None => continue,
                };
                if *was_completed {
                    // Victory music + "You've completed the guildleve" text.
                    let music = tx::build_set_music(
                        handle.session_id,
                        super::guildleve::GuildleveLocationMusic::VICTORY,
                        0,
                    );
                    client.send_bytes(music.to_bytes()).await;
                    let msg = tx::build_game_message(
                        handle.session_id,
                        tx::GameMessageOptions {
                            sender_actor_id: 0,
                            receiver_actor_id: handle.session_id,
                            text_id: GL_TEXT_COMPLETE,
                            log: 0x20,
                            display_id: None,
                            custom_sender: None,
                            lua_params: vec![common::luaparam::LuaParam::UInt32(*guildleve_id)],
                        },
                    );
                    client.send_bytes(msg.to_bytes()).await;
                    // GC seal accrual — every enlisted member of the
                    // leve earns a per-difficulty seal payout. No-op
                    // for unenlisted players, capped at the rank seal
                    // ceiling, and silently skipped when no DB handle
                    // is wired in (test harnesses).
                    if let Some(db) = db {
                        crate::runtime::dispatcher::award_leve_completion_seals(
                            &handle,
                            *difficulty,
                            db,
                        )
                        .await;
                    }
                }
                let _ = director_id;
            }
        }
        DirectorEvent::GuildleveAbandoned {
            director_id,
            guildleve_id,
        } => {
            for actor_id in player_members {
                let Some(handle) = registry.get(*actor_id).await else {
                    continue;
                };
                let client = match world.client(handle.session_id).await {
                    Some(c) => c,
                    None => continue,
                };
                let msg = tx::build_game_message(
                    handle.session_id,
                    tx::GameMessageOptions {
                        sender_actor_id: 0,
                        receiver_actor_id: handle.session_id,
                        text_id: GL_TEXT_ABANDON,
                        log: 0x20,
                        display_id: None,
                        custom_sender: None,
                        lua_params: vec![common::luaparam::LuaParam::UInt32(*guildleve_id)],
                    },
                );
                client.send_bytes(msg.to_bytes()).await;
                let _ = director_id;
            }
        }
        DirectorEvent::GuildleveAimUpdated {
            director_id,
            index,
            value,
        }
        | DirectorEvent::GuildleveUiUpdated {
            director_id,
            index,
            value,
        } => {
            tracing::debug!(
                director = director_id,
                index,
                value,
                "guildleve property update (ActorProperty packet pending)",
            );
        }
        DirectorEvent::GuildleveMarkerUpdated {
            director_id,
            index,
            x,
            y,
            z,
        } => {
            tracing::debug!(
                director = director_id,
                index,
                x,
                y,
                z,
                "guildleve marker move (ActorProperty packet pending)",
            );
        }
        DirectorEvent::GuildleveSyncAll { director_id } => {
            tracing::debug!(
                director = director_id,
                "guildleve full-sync (ActorPropertyPacketUtil pending)",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::Character;
    use crate::data::ClientHandle;
    use crate::runtime::actor_registry::{ActorHandle, ActorKindTag};
    use std::sync::Arc;
    use tokio::sync::mpsc;

    fn chara(id: u32) -> Character {
        Character::new(id)
    }

    #[tokio::test]
    async fn guildleve_started_emits_music_and_two_messages() {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        registry
            .insert(ActorHandle::new(1, ActorKindTag::Player, 0, 11, chara(1)))
            .await;
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(16);
        world.register_client(11, ClientHandle::new(11, tx)).await;

        let event = DirectorEvent::GuildleveStarted {
            director_id: 0x6000_0001,
            guildleve_id: 10801,
            difficulty: 1,
            location: 1,
            time_limit_seconds: 600,
            start_time_unix: 0,
        };
        dispatch_director_event(&event, &[1], &registry, &world, None).await;
        // Music + start msg + time-limit msg = 3 packets.
        for _ in 0..3 {
            let got = rx.recv().await.expect("guildleve start packet");
            assert!(!got.is_empty());
        }
    }

    #[tokio::test]
    async fn guildleve_ended_completed_emits_victory_music_and_message() {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        registry
            .insert(ActorHandle::new(1, ActorKindTag::Player, 0, 11, chara(1)))
            .await;
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(16);
        world.register_client(11, ClientHandle::new(11, tx)).await;

        let event = DirectorEvent::GuildleveEnded {
            director_id: 0x6000_0001,
            guildleve_id: 10801,
            was_completed: true,
            completion_time_seconds: 300,
            difficulty: 3,
        };
        dispatch_director_event(&event, &[1], &registry, &world, None).await;
        for _ in 0..2 {
            let got = rx.recv().await.expect("guildleve end packet");
            assert!(!got.is_empty());
        }
    }

    #[tokio::test]
    async fn director_ended_broadcasts_remove_actor() {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        registry
            .insert(ActorHandle::new(1, ActorKindTag::Player, 0, 11, chara(1)))
            .await;
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(4);
        world.register_client(11, ClientHandle::new(11, tx)).await;

        let event = DirectorEvent::DirectorEnded {
            director_id: 0x6000_0001,
            zone_id: 100,
        };
        dispatch_director_event(&event, &[1], &registry, &world, None).await;
        let got = rx.recv().await.expect("remove-actor packet");
        assert!(!got.is_empty());
    }
}
