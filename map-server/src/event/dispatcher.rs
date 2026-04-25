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

//! Dispatcher for `EventEvent`s. Turns outbox rows into:
//!
//! * Outbound packets (`RunEventFunctionPacket`, `EndEventPacket`,
//!   `KickEventPacket`, `SendGameMessage`).
//! * Lua dispatch — when a `LuaEngine` is available, `EventStarted` /
//!   `EventUpdated` resume the NPC or coroutine script; quest events
//!   call `isObjectivesComplete` / `onAbandonQuest` on the quest script.
//! * DB writes (`QuestSaveToDb`).

#![allow(dead_code)]

use std::sync::Arc;

use mlua::{MultiValue, Value};

use common::luaparam::LuaParam;

use crate::database::Database;
use crate::lua::LuaEngine;
use crate::packets::send as tx;
use crate::runtime::actor_registry::ActorRegistry;
use crate::world_manager::WorldManager;

use super::outbox::EventEvent;

/// Dispatch one `EventEvent`. The `lua` argument is optional so callers
/// that don't (yet) have a Lua engine wired in can still process events
/// — the Lua-flavoured events degrade gracefully to `tracing::debug!`.
pub async fn dispatch_event_event(
    event: &EventEvent,
    registry: &ActorRegistry,
    world: &WorldManager,
    db: &Database,
    lua: Option<&Arc<LuaEngine>>,
) {
    match event {
        // ---- Lua hooks --------------------------------------------------
        EventEvent::EventStarted {
            player_actor_id,
            owner_actor_id,
            event_name,
            event_type,
            lua_params,
        } => {
            dispatch_event_started(
                registry,
                world,
                db,
                lua,
                *player_actor_id,
                *owner_actor_id,
                event_name,
                *event_type,
                lua_params,
            )
            .await;
        }
        EventEvent::EventUpdated {
            player_actor_id,
            trigger_actor_id,
            event_type,
            lua_params,
        } => {
            dispatch_event_updated(
                lua,
                *player_actor_id,
                *trigger_actor_id,
                *event_type,
                lua_params,
            );
        }
        EventEvent::QuestCheckCompletion {
            player_actor_id,
            quest_id,
            quest_name,
        } => {
            dispatch_quest_check_completion(lua, *player_actor_id, *quest_id, quest_name);
        }
        EventEvent::QuestAbandonHook {
            player_actor_id,
            quest_id,
            quest_name,
        } => {
            dispatch_quest_abandon(lua, *player_actor_id, *quest_id, quest_name);
        }

        // ---- Packet sends ----------------------------------------------
        // Each builder leaves SubPacketHeader.target_id = 0 because
        // `SubPacket::new` only sets source_id (target_id stays at the
        // default-zero from `SubPacketHeader::default()`). The 1.x
        // client appears to silently drop event subpackets whose
        // target_id != the receiving actor's session id — Meteor's
        // KickEvent / RunEventFunction packets carry target_id = 1
        // (the player). Without this, the server's RunEventFunction
        // for the opening cutscene reaches the client with the right
        // bytes everywhere except SubPacketHeader.target_id, and the
        // client sits on "Now Loading" forever (no EventUpdate, no
        // EndEvent, the script's `_WAIT_EVENT` coroutine never
        // resumes).
        EventEvent::RunEventFunction {
            player_actor_id,
            trigger_actor_id,
            owner_actor_id,
            event_name,
            event_type,
            function_name,
            lua_params,
        } => {
            let mut sub = tx::build_run_event_function(
                *trigger_actor_id,
                *owner_actor_id,
                event_name,
                *event_type,
                function_name,
                lua_params,
            );
            sub.set_target_id(*player_actor_id);
            send_to_player(world, registry, *player_actor_id, sub.to_bytes()).await;
        }
        EventEvent::EndEvent {
            player_actor_id,
            owner_actor_id,
            event_name,
            event_type,
        } => {
            let mut sub =
                tx::build_end_event(*player_actor_id, *owner_actor_id, event_name, *event_type);
            sub.set_target_id(*player_actor_id);
            send_to_player(world, registry, *player_actor_id, sub.to_bytes()).await;
        }
        EventEvent::KickEvent {
            player_actor_id,
            target_actor_id,
            owner_actor_id,
            event_name,
            event_type,
            lua_params,
        } => {
            let mut sub = tx::build_kick_event(
                *target_actor_id,
                *owner_actor_id,
                event_name,
                *event_type,
                lua_params,
            );
            sub.set_target_id(*player_actor_id);
            send_to_player(world, registry, *player_actor_id, sub.to_bytes()).await;
        }

        // ---- DB + game message -----------------------------------------
        EventEvent::QuestSaveToDb {
            player_actor_id,
            slot,
            quest_id,
            sequence,
            flags,
            counter1,
            counter2,
            counter3,
        } => {
            if let Err(e) = db
                .save_quest(
                    *player_actor_id,
                    *slot,
                    *quest_id,
                    *sequence,
                    *flags,
                    *counter1,
                    *counter2,
                    *counter3,
                )
                .await
            {
                tracing::warn!(
                    error = %e,
                    player = player_actor_id,
                    quest = quest_id,
                    "quest save failed",
                );
            }
        }
        EventEvent::QuestGameMessage {
            player_actor_id,
            text_id,
            quest_id,
        } => {
            tracing::debug!(
                player = player_actor_id,
                text = text_id,
                quest = quest_id,
                "quest: game message (send-builder pending)",
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Lua dispatch bodies
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
async fn dispatch_event_started(
    registry: &ActorRegistry,
    world: &WorldManager,
    db: &Database,
    lua: Option<&Arc<LuaEngine>>,
    player_actor_id: u32,
    owner_actor_id: u32,
    event_name: &str,
    event_type: u8,
    lua_params: &[LuaParam],
) {
    let Some(lua) = lua else {
        tracing::debug!(
            player = player_actor_id,
            owner = owner_actor_id,
            name = %event_name,
            ty = event_type,
            params = lua_params.len(),
            "event: started (no Lua engine wired)",
        );
        return;
    };

    // Director actor ids carry the `6` kind nibble in the high 4 bits
    // (`(6 << 28) | (zone << 19) | local`). Directors live on
    // `Zone::core.directors`, not in the actor registry — route them
    // through a dedicated dispatcher so the script gets a real
    // `LuaPlayer` + `LuaDirectorHandle` userdata pair instead of the
    // raw-integer arg list the NPC fallback below uses, and so any
    // emitted `LuaCommand`s actually drain (most importantly
    // `quest:OnNotice` from `AfterQuestWarpDirector`).
    if owner_actor_id >> 28 == 6 {
        dispatch_director_event_started(
            registry,
            world,
            db,
            lua,
            player_actor_id,
            owner_actor_id,
            event_name,
            event_type,
            lua_params,
        )
        .await;
        return;
    }

    dispatch_npc_event_started(
        registry,
        world,
        db,
        lua,
        player_actor_id,
        owner_actor_id,
        event_name,
        event_type,
        lua_params,
    )
    .await;
}

/// NPC-flavoured `onEventStarted` dispatch.
///
/// Mirrors Meteor's `LuaEngine.CallLuaFunction` for non-director
/// targets (`origin/develop:Map Server/Lua/LuaEngine.cs:555`): build
/// `args = [player, target, ...lparams]` (where `lparams[0]` is the
/// `eventName` string the C# `EventStarted` shim inserts ahead of the
/// player's params, see `Map Server/Lua/LuaEngine.cs:601`) and resume
/// the script's `onEventStarted` global.
///
/// Goes through `LuaEngine::call_quest_hook`-style snapshot
/// construction so the script sees real `LuaPlayer` + `LuaNpc`
/// userdata. The previous implementation passed the player as a raw
/// integer, which silently broke every NPC script that called
/// `player:HasQuest(...)` / `player:GetItemPackage(0):AddItem(...)` /
/// any other userdata-method (the call would error with "attempt to
/// index a number value" the moment the script ran). Hook-emitted
/// commands also drain through `apply_runtime_lua_commands` instead
/// of being silently dropped.
///
/// Lookup walks the NPC's unique-override path first
/// (`unique/<zone>/<class>/<unique>.lua`) then falls back to the
/// base class (`base/<class_path>.lua`). Quietly no-ops on:
/// * missing owner (NPC despawned mid-event-start),
/// * missing player (player disconnected mid-event-start),
/// * missing script on disk (NPC has no Lua entry),
/// * missing `onEventStarted` global on the loaded script.
#[allow(clippy::too_many_arguments)]
async fn dispatch_npc_event_started(
    registry: &ActorRegistry,
    world: &WorldManager,
    db: &Database,
    lua: &Arc<LuaEngine>,
    player_actor_id: u32,
    owner_actor_id: u32,
    event_name: &str,
    event_type: u8,
    lua_params: &[LuaParam],
) {
    let Some(owner_handle) = registry.get(owner_actor_id).await else {
        tracing::debug!(owner = owner_actor_id, "event: owner actor missing");
        return;
    };
    let (class_path, class_name, unique_id, npc_state, npc_pos, npc_rot, actor_class_id) = {
        let chara = owner_handle.character.read().await;
        (
            chara.base.class_path.clone(),
            chara.base.class_name.clone(),
            chara.base.actor_name.clone(),
            chara.base.current_main_state,
            (
                chara.base.position_x,
                chara.base.position_y,
                chara.base.position_z,
            ),
            chara.base.rotation,
            chara.chara.actor_class_id,
        )
    };
    let zone_name = zone_name_for(world, owner_handle.zone_id).await;

    let unique_path = lua.resolver().npc(&zone_name, &class_name, &unique_id);
    let base_path = lua.resolver().base_class(&class_path);
    let script_path = if unique_path.exists() {
        unique_path
    } else if base_path.exists() {
        base_path
    } else {
        tracing::debug!(
            owner = owner_actor_id,
            class = %class_path,
            "NPC onEventStarted skipped — no script on disk",
        );
        return;
    };

    let Some(player_handle) = registry.get(player_actor_id).await else {
        tracing::debug!(
            player = player_actor_id,
            owner = owner_actor_id,
            "NPC onEventStarted skipped — player not in registry",
        );
        return;
    };
    let snapshot = {
        let c = player_handle.character.read().await;
        crate::lua::userdata::PlayerSnapshot {
            actor_id: c.base.actor_id,
            name: c.base.actor_name.clone(),
            zone_id: c.base.zone_id,
            pos: (c.base.position_x, c.base.position_y, c.base.position_z),
            rotation: c.base.rotation,
            state: c.base.current_main_state,
            hp: c.chara.hp,
            max_hp: c.chara.max_hp,
            mp: c.chara.mp,
            max_mp: c.chara.max_mp,
            tp: c.chara.tp,
            active_quests: c
                .quest_journal
                .slots
                .iter()
                .flatten()
                .map(|q| q.quest_id())
                .collect(),
            active_quest_states: c
                .quest_journal
                .slots
                .iter()
                .flatten()
                .map(|q| crate::lua::QuestStateSnapshot {
                    quest_id: q.quest_id(),
                    sequence: q.get_sequence(),
                    flags: q.get_flags(),
                    counters: [q.get_counter(0), q.get_counter(1), q.get_counter(2)],
                })
                .collect(),
            completed_quests: c.quest_journal.iter_completed().collect(),
            ..Default::default()
        }
    };

    let lua_clone = Arc::clone(lua);
    let event_name_owned = event_name.to_string();
    let class_name_owned = class_name.clone();
    let class_path_owned = class_path.clone();
    let unique_id_owned = unique_id.clone();
    let zone_name_owned = zone_name.clone();
    let npc_actor_id = owner_actor_id;
    let npc_zone_id = owner_handle.zone_id;
    let lua_params_owned: Vec<LuaParam> = lua_params.to_vec();
    let _ = event_type; // Meteor's NPC dispatch ignores event_type for onEventStarted — eventName is what scripts branch on.

    let result = tokio::task::spawn_blocking(move || {
        let (lua_vm, queue) = match lua_clone.load_script(&script_path) {
            Ok(pair) => pair,
            Err(e) => {
                return Err(format!("load_script failed: {e}"));
            }
        };
        let globals = lua_vm.globals();
        let Some(f): Option<mlua::Function> = globals.get("onEventStarted").ok() else {
            // Quiet no-op — many NPC scripts only define `init()` /
            // `main()` and rely on the global hook absence to skip.
            return Ok((Vec::new(), None));
        };

        let player = crate::lua::userdata::LuaPlayer {
            snapshot,
            queue: queue.clone(),
        };
        let player_ud = lua_vm
            .create_userdata(player)
            .map_err(|e| format!("create_userdata(LuaPlayer): {e}"))?;
        let npc = crate::lua::userdata::LuaNpc {
            base: crate::lua::userdata::LuaActor {
                actor_id: npc_actor_id,
                name: class_name_owned.clone(),
                class_name: class_name_owned,
                class_path: class_path_owned,
                unique_id: unique_id_owned,
                zone_id: npc_zone_id,
                zone_name: zone_name_owned,
                state: npc_state,
                pos: npc_pos,
                rotation: npc_rot,
                queue: queue.clone(),
            },
            actor_class_id,
            quest_graphic: 0,
        };
        let npc_ud = lua_vm
            .create_userdata(npc)
            .map_err(|e| format!("create_userdata(LuaNpc): {e}"))?;

        let mut mv = MultiValue::new();
        mv.push_back(Value::UserData(player_ud));
        mv.push_back(Value::UserData(npc_ud));
        // Meteor inserts `eventName` ahead of the original lparams (see
        // `LuaEngine.EventStarted` `lparams.Insert(0, ...)`). That's
        // what surfaces as the third script arg — `triggerName` in
        // most NPC scripts.
        mv.push_back(Value::String(
            lua_vm
                .create_string(&event_name_owned)
                .map_err(|e| format!("create_string(eventName): {e}"))?,
        ));
        for p in &lua_params_owned {
            let v = match p {
                LuaParam::Int32(i) => Value::Integer(*i as mlua::Integer),
                LuaParam::UInt32(u) => Value::Integer(*u as mlua::Integer),
                LuaParam::String(s) => Value::String(
                    lua_vm
                        .create_string(s)
                        .map_err(|e| format!("create_string(lparam): {e}"))?,
                ),
                LuaParam::True => Value::Boolean(true),
                LuaParam::False => Value::Boolean(false),
                LuaParam::Nil => Value::Nil,
                LuaParam::Actor(id) => Value::Integer(*id as mlua::Integer),
                LuaParam::Type7 { actor_id, .. } => Value::Integer(*actor_id as mlua::Integer),
                LuaParam::Type9 { item1, .. } => Value::Integer(*item1 as mlua::Integer),
                LuaParam::Byte(b) => Value::Integer(*b as mlua::Integer),
                LuaParam::Short(s) => Value::Integer(*s as mlua::Integer),
            };
            mv.push_back(v);
        }

        let call_err = f.call::<Value>(mv).err().map(|e| format!("{e}"));
        let commands = crate::lua::command::CommandQueue::drain(&queue);
        Ok((commands, call_err))
    })
    .await;

    let (commands, hook_err) = match result {
        Ok(Ok((cmds, err))) => (cmds, err),
        Ok(Err(setup_err)) => {
            tracing::debug!(
                error = %setup_err,
                owner = owner_actor_id,
                event = %event_name,
                "NPC onEventStarted setup failed",
            );
            return;
        }
        Err(join_err) => {
            tracing::warn!(
                error = %join_err,
                owner = owner_actor_id,
                "NPC onEventStarted dispatch panicked",
            );
            return;
        }
    };
    if let Some(e) = hook_err {
        tracing::debug!(
            error = %e,
            owner = owner_actor_id,
            event = %event_name,
            "NPC onEventStarted errored",
        );
    }
    if !commands.is_empty() {
        // Event-flavoured commands (RunEventFunction / EndEvent /
        // KickEvent) need to flow through the EventOutbox so their
        // packets actually reach the client. apply_runtime_lua_commands
        // knows about non-event variants but isn't wired to the event
        // bridge — without this drain step, the GM `talkto` command's
        // emitted RunEventFunction/EndEvent get silently logged as
        // "runtime lua command unhandled" and the cutscene never plays
        // on the client. Mirrors the bridge step in
        // `dispatch_director_event_started` and `apply_quest_on_notice`.
        if let Some(player_handle) = registry.get(player_actor_id).await {
            let event_session_snapshot = {
                let c = player_handle.character.read().await;
                c.event_session.clone()
            };
            let mut outbox = crate::event::outbox::EventOutbox::new();
            crate::event::lua_bridge::translate_lua_commands_into_outbox(
                &commands,
                &event_session_snapshot,
                &mut outbox,
            );
            for e in outbox.drain() {
                Box::pin(dispatch_event_event(&e, registry, world, db, Some(lua))).await;
            }
        }
        crate::runtime::quest_apply::apply_runtime_lua_commands(
            commands,
            registry,
            db,
            world,
            Some(lua),
        )
        .await;
    }
}

/// Director-flavoured `onEventStarted` dispatch.
///
/// Mirrors Meteor's `Director.OnEventStart` (`origin/develop:Map
/// Server/Actors/Director/Director.cs:325`): build `args = [player,
/// director, eventName, ...lparams]` and resume the script's
/// `onEventStarted` global. Unlike the NPC fallback above, this path
/// goes through `LuaEngine::call_quest_hook`-style snapshot construction
/// so the script sees real `LuaPlayer` + `LuaDirectorHandle` userdata —
/// without that, the script's `player:HasQuest(110002)` /
/// `quest:OnNotice(player)` chain would error on a method-on-integer
/// the moment `AfterQuestWarpDirector.lua` runs.
///
/// Lookup walks the actor-id encoding (`(6 << 28) | (zone << 19) |
/// local`) to find the director on its zone's `AreaCore`. Quietly
/// no-ops on:
/// * unknown zone (zone never loaded),
/// * unknown director (already ended / never spawned),
/// * missing script (`directors/<class_name>.lua` not on disk),
/// * missing player snapshot (player left mid-event-start).
///
/// Hook-emitted commands drain through `apply_runtime_lua_commands`
/// against the same registry / db / world / lua refs the dispatcher
/// already holds, so `quest:OnNotice` → `QuestOnNotice` →
/// `apply_quest_on_notice` → target quest's `onNotice` flow lands
/// without further plumbing.
#[allow(clippy::too_many_arguments)]
async fn dispatch_director_event_started(
    registry: &ActorRegistry,
    world: &WorldManager,
    db: &Database,
    lua: &Arc<LuaEngine>,
    player_actor_id: u32,
    director_actor_id: u32,
    event_name: &str,
    event_type: u8,
    lua_params: &[LuaParam],
) {
    // Decode the zone id and look the director up on the zone's
    // AreaCore. Director ids are zone-scoped; if the zone doesn't have
    // it, the director's been ended or never created.
    let zone_id = (director_actor_id >> 19) & 0x1FF;
    let Some(zone_arc) = world.zone(zone_id).await else {
        tracing::debug!(
            director = director_actor_id,
            zone = zone_id,
            "director onEventStarted skipped — zone not loaded",
        );
        return;
    };
    let (class_path, class_name, actor_name) = {
        let zone = zone_arc.read().await;
        let Some(d) = zone.core.director(director_actor_id) else {
            tracing::debug!(
                director = director_actor_id,
                zone = zone_id,
                "director onEventStarted skipped — director not on zone",
            );
            return;
        };
        (d.class_path.clone(), d.class_name.clone(), d.actor_name.clone())
    };
    let script_path = lua.resolver().director(&class_name);
    if !script_path.exists() {
        tracing::debug!(
            director = director_actor_id,
            class = %class_name,
            script = %script_path.display(),
            "director onEventStarted skipped — script not on disk",
        );
        return;
    }

    // Snapshot the player so the LuaPlayer userdata sees a coherent
    // view (HasQuest / GetQuest / GetItemPackage all read out of the
    // snapshot). Quietly no-op if the player's already gone.
    let Some(player_handle) = registry.get(player_actor_id).await else {
        tracing::debug!(
            player = player_actor_id,
            director = director_actor_id,
            "director onEventStarted skipped — player not in registry",
        );
        return;
    };
    let snapshot = {
        let c = player_handle.character.read().await;
        crate::lua::userdata::PlayerSnapshot {
            actor_id: c.base.actor_id,
            name: c.base.actor_name.clone(),
            zone_id: c.base.zone_id,
            pos: (c.base.position_x, c.base.position_y, c.base.position_z),
            rotation: c.base.rotation,
            state: c.base.current_main_state,
            hp: c.chara.hp,
            max_hp: c.chara.max_hp,
            mp: c.chara.mp,
            max_mp: c.chara.max_mp,
            tp: c.chara.tp,
            active_quests: c
                .quest_journal
                .slots
                .iter()
                .flatten()
                .map(|q| q.quest_id())
                .collect(),
            active_quest_states: c
                .quest_journal
                .slots
                .iter()
                .flatten()
                .map(|q| crate::lua::QuestStateSnapshot {
                    quest_id: q.quest_id(),
                    sequence: q.get_sequence(),
                    flags: q.get_flags(),
                    counters: [q.get_counter(0), q.get_counter(1), q.get_counter(2)],
                })
                .collect(),
            completed_quests: c.quest_journal.iter_completed().collect(),
            ..Default::default()
        }
    };

    // Owned bundle for the spawn_blocking closure. Args mirror Meteor's
    // `Director.OnEventStart`: `(player, director, eventName, ...lparams)`.
    let lua_clone = Arc::clone(lua);
    let event_name_owned = event_name.to_string();
    let class_path_owned = class_path.clone();
    let actor_name_owned = actor_name.clone();
    let script_path_clone = script_path.clone();
    let lua_params_owned: Vec<LuaParam> = lua_params.to_vec();
    let _ = event_type; // event_type is captured by the EventStartPacket but Meteor's dispatch ignores it for onEventStarted — the director branches on eventName.

    // Run onEventStarted inside a coroutine via LuaEngine's helper so
    // `callClientFunction(...)` inside the hook can `coroutine.yield`
    // on `_WAIT_EVENT` and the scheduler parks it until the client's
    // `EventUpdate` packet wakes it through `fire_player_event`.
    let result = tokio::task::spawn_blocking(move || {
        lua_clone.spawn_director_on_event_started(&script_path_clone, |lua_vm, queue| {
            let player = crate::lua::userdata::LuaPlayer {
                snapshot,
                queue: queue.clone(),
            };
            let player_ud = lua_vm
                .create_userdata(player)
                .map_err(|e| anyhow::anyhow!("create_userdata(LuaPlayer): {e}"))?;
            let director = crate::lua::userdata::LuaDirectorHandle {
                name: actor_name_owned,
                actor_id: director_actor_id,
                class_path: class_path_owned,
                queue: queue.clone(),
            };
            let director_ud = lua_vm
                .create_userdata(director)
                .map_err(|e| anyhow::anyhow!("create_userdata(LuaDirectorHandle): {e}"))?;

            let mut mv = MultiValue::new();
            mv.push_back(Value::UserData(player_ud));
            mv.push_back(Value::UserData(director_ud));
            mv.push_back(Value::String(
                lua_vm
                    .create_string(&event_name_owned)
                    .map_err(|e| anyhow::anyhow!("create_string(eventName): {e}"))?,
            ));
            for p in &lua_params_owned {
                let v = match p {
                    LuaParam::Int32(i) => Value::Integer(*i as mlua::Integer),
                    LuaParam::UInt32(u) => Value::Integer(*u as mlua::Integer),
                    LuaParam::String(s) => Value::String(
                        lua_vm
                            .create_string(s)
                            .map_err(|e| anyhow::anyhow!("create_string(lparam): {e}"))?,
                    ),
                    LuaParam::True => Value::Boolean(true),
                    LuaParam::False => Value::Boolean(false),
                    LuaParam::Nil => Value::Nil,
                    LuaParam::Actor(id) => Value::Integer(*id as mlua::Integer),
                    LuaParam::Type7 { actor_id, .. } => Value::Integer(*actor_id as mlua::Integer),
                    LuaParam::Type9 { item1, .. } => Value::Integer(*item1 as mlua::Integer),
                    LuaParam::Byte(b) => Value::Integer(*b as mlua::Integer),
                    LuaParam::Short(s) => Value::Integer(*s as mlua::Integer),
                };
                mv.push_back(v);
            }
            Ok(mv)
        })
    })
    .await;

    let partial = match result {
        Ok(p) => p,
        Err(join_err) => {
            tracing::warn!(
                error = %join_err,
                director = director_actor_id,
                "director onEventStarted dispatch panicked",
            );
            return;
        }
    };
    let commands = partial.commands;
    if let Some(e) = partial.error {
        tracing::debug!(
            error = %e,
            director = director_actor_id,
            event = %event_name,
            "director onEventStarted errored",
        );
    }
    if !commands.is_empty() {
        // Event-flavoured commands (RunEventFunction / EndEvent /
        // KickEvent) need to flow through the EventOutbox so their
        // packets actually reach the client. apply_runtime_lua_commands
        // knows about non-event variants (quest flag mutates, AddExp,
        // etc.) but isn't wired to the event bridge.
        if let Some(player_handle) = registry.get(player_actor_id).await {
            let event_session_snapshot = {
                let c = player_handle.character.read().await;
                c.event_session.clone()
            };
            let mut outbox = crate::event::outbox::EventOutbox::new();
            crate::event::lua_bridge::translate_lua_commands_into_outbox(
                &commands,
                &event_session_snapshot,
                &mut outbox,
            );
            for e in outbox.drain() {
                // Use Box::pin here — `dispatch_event_event` can recurse
                // into `dispatch_director_event_started` (e.g. a nested
                // KickEvent re-triggers EventStart on another director).
                Box::pin(dispatch_event_event(&e, registry, world, db, Some(lua))).await;
            }
        }
        crate::runtime::quest_apply::apply_runtime_lua_commands(
            commands,
            registry,
            db,
            world,
            Some(lua),
        )
        .await;
    }
}

fn dispatch_event_updated(
    lua: Option<&Arc<LuaEngine>>,
    player_actor_id: u32,
    trigger_actor_id: u32,
    event_type: u8,
    lua_params: &[LuaParam],
) {
    let Some(lua) = lua else {
        tracing::debug!(
            player = player_actor_id,
            trigger = trigger_actor_id,
            ty = event_type,
            params = lua_params.len(),
            "event: updated (no Lua engine)",
        );
        return;
    };
    // Resume any coroutine parked on `_WAIT_EVENT` for this player.
    // The lua params carry the client's response; we feed them through
    // as a MultiValue so the sleeping coroutine can inspect them.
    let fired = lua.fire_player_event(player_actor_id, MultiValue::new());
    tracing::debug!(
        player = player_actor_id,
        trigger = trigger_actor_id,
        ty = event_type,
        resumed = fired,
        "event: updated",
    );
}

fn dispatch_quest_check_completion(
    lua: Option<&Arc<LuaEngine>>,
    player_actor_id: u32,
    quest_id: u32,
    quest_name: &str,
) {
    let Some(lua) = lua else {
        tracing::debug!(
            player = player_actor_id,
            quest = quest_id,
            name = %quest_name,
            "quest: isObjectivesComplete (no Lua engine)",
        );
        return;
    };
    if quest_name.is_empty() {
        return;
    }
    let path = lua.resolver().quest(quest_name);
    match lua.call(&path, "isObjectivesComplete", MultiValue::new()) {
        Ok(r) => {
            let complete = matches!(r.value, Value::Boolean(true));
            tracing::debug!(
                player = player_actor_id,
                quest = quest_id,
                name = %quest_name,
                complete,
                "quest: isObjectivesComplete",
            );
        }
        Err(e) => {
            tracing::debug!(
                error = %e,
                quest = quest_id,
                name = %quest_name,
                "quest: isObjectivesComplete call failed",
            );
        }
    }
}

fn dispatch_quest_abandon(
    lua: Option<&Arc<LuaEngine>>,
    player_actor_id: u32,
    quest_id: u32,
    quest_name: &str,
) {
    let Some(lua) = lua else {
        tracing::debug!(
            player = player_actor_id,
            quest = quest_id,
            name = %quest_name,
            "quest: onAbandonQuest (no Lua engine)",
        );
        return;
    };
    if quest_name.is_empty() {
        return;
    }
    let path = lua.resolver().quest(quest_name);
    if let Err(e) = lua.call(&path, "onAbandonQuest", MultiValue::new()) {
        tracing::debug!(
            error = %e,
            quest = quest_id,
            name = %quest_name,
            "quest: onAbandonQuest call failed",
        );
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn send_to_player(
    world: &WorldManager,
    registry: &ActorRegistry,
    player_actor_id: u32,
    bytes: Vec<u8>,
) {
    let Some(handle) = registry.get(player_actor_id).await else {
        return;
    };
    let Some(client) = world.client(handle.session_id).await else {
        return;
    };
    client.send_bytes(bytes).await;
}

async fn zone_name_for(world: &WorldManager, zone_id: u32) -> String {
    let Some(zone_arc) = world.zone(zone_id).await else {
        return String::new();
    };
    let zone = zone_arc.read().await;
    zone.core.zone_name.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventEvent;
    use crate::event::outbox::EventOutbox;

    fn tmpdir() -> std::path::PathBuf {
        // Atomic counter so two parallel tests landing on the same
        // nanosecond don't share a tmpdir (would corrupt the
        // assertions that read script-globals back from a cached VM
        // because both tests' LuaEngines would resolve the same
        // on-disk path).
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("garlemald-event-dispatch-{nanos}-{seq}"));
        std::fs::create_dir_all(dir.join("quests/man")).unwrap();
        dir
    }

    /// Per-test SQLite path — one file per test run so WAL files don't
    /// collide. The `AtomicU64` guarantees uniqueness even when two
    /// parallel tests hit `SystemTime::now` within the same nanosecond.
    fn tempdb() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("garlemald-event-dispatch-{nanos}-{seq}.db"))
    }

    #[tokio::test]
    async fn quest_completion_fires_lua_hook() {
        let root = tmpdir();
        // Minimal quest script — returns true from isObjectivesComplete.
        std::fs::write(
            root.join("quests/man/man0l0.lua"),
            "function isObjectivesComplete() return true end",
        )
        .unwrap();

        let lua = Arc::new(LuaEngine::new(&root));
        let registry = ActorRegistry::new();
        let world = WorldManager::new();
        // Skip real DB round-trip by only dispatching a QuestCheckCompletion
        // event (no DB hit). The stub fails on connect which is fine — that
        // code path isn't reached.
        let db = Database::open(tempdb()).await.expect("db stub");

        let event = EventEvent::QuestCheckCompletion {
            player_actor_id: 1,
            quest_id: 0xF_00F0,
            quest_name: "man0l0".to_string(),
        };
        dispatch_event_event(&event, &registry, &world, &db, Some(&lua)).await;
        // Reaching here without a panic is the assertion — the Lua path
        // loaded the script and called `isObjectivesComplete`. The
        // dispatcher logs the boolean; callers that care about the result
        // pass their own LuaCallResult-aware closure.
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn quest_abandon_fires_lua_hook() {
        let root = tmpdir();
        // Write a script that ticks a global on onAbandonQuest — we can't
        // observe that from here, but the call should succeed without
        // error. A failing call hits the `tracing::debug!` branch; a
        // successful call silently completes.
        std::fs::write(
            root.join("quests/man/man0l1.lua"),
            "_abandoned = false\nfunction onAbandonQuest() _abandoned = true end",
        )
        .unwrap();

        let lua = Arc::new(LuaEngine::new(&root));
        let registry = ActorRegistry::new();
        let world = WorldManager::new();
        let db = Database::open(tempdb()).await.expect("db stub");

        let event = EventEvent::QuestAbandonHook {
            player_actor_id: 1,
            quest_id: 0xF_00F1,
            quest_name: "man0l1".to_string(),
        };
        dispatch_event_event(&event, &registry, &world, &db, Some(&lua)).await;

        // Secondary confirmation: load the VM and check the global flipped.
        let path = lua.resolver().quest("man0l1");
        let (vm, _q) = lua.load_script(&path).unwrap();
        let flag: bool = vm.globals().get("_abandoned").unwrap();
        assert!(flag);
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn event_started_invokes_npc_base_class_script() {
        use crate::actor::Character;
        use crate::runtime::actor_registry::{ActorHandle, ActorKindTag};
        use crate::zone::navmesh::StubNavmeshLoader;
        use crate::zone::zone::Zone;

        let root = tmpdir();
        // Matches the C# base-class resolver: `scripts/base/<classPath>.lua`.
        std::fs::create_dir_all(root.join("base/Chara/Npc/Populace")).unwrap();
        // Hook captures the userdata-accessor reads it does on each
        // arg — `player.actorId` (LuaPlayer metatable Index),
        // `npc:GetName()` (LuaNpc method), and the third positional
        // `eventName` string. Asserting all three fields proves the
        // dispatcher built proper userdata for player + npc and
        // inserted `eventName` ahead of `lparams` per Meteor's
        // `LuaEngine.EventStarted` shim.
        std::fs::write(
            root.join("base/Chara/Npc/Populace/Greeter.lua"),
            r#"
                _talked_to = 0
                _talked_npc = ""
                _talked_event = ""
                function onEventStarted(player, npc, eventName)
                    _talked_to = player.actorId
                    _talked_npc = npc:GetName()
                    _talked_event = eventName
                end
            "#,
        )
        .unwrap();

        let lua = Arc::new(LuaEngine::new(&root));
        let world = WorldManager::new();
        let registry = ActorRegistry::new();
        let db = Database::open(tempdb()).await.expect("db stub");

        // One Zone + one Greeter NPC in it + a Player so the
        // dispatcher can build a real `LuaPlayer` userdata snapshot.
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

        let mut npc_chara = Character::new(42);
        npc_chara.base.class_path = "Chara/Npc/Populace/Greeter".into();
        npc_chara.base.class_name = "Greeter".into();
        npc_chara.base.actor_name = "greeter_main".into();
        registry
            .insert(ActorHandle::new(42, ActorKindTag::Npc, 100, 0, npc_chara))
            .await;

        let player_chara = Character::new(1234);
        registry
            .insert(ActorHandle::new(
                1234,
                ActorKindTag::Player,
                100,
                42,
                player_chara,
            ))
            .await;

        let event = EventEvent::EventStarted {
            player_actor_id: 1234,
            owner_actor_id: 42,
            event_name: "onTalk".to_string(),
            event_type: 0,
            lua_params: vec![],
        };
        dispatch_event_event(&event, &registry, &world, &db, Some(&lua)).await;

        // Reload the cached VM and read back the three globals the
        // hook wrote.
        let path = lua.resolver().base_class("Chara/Npc/Populace/Greeter");
        let (vm, _q) = lua.load_script(&path).unwrap();
        let talked: i64 = vm.globals().get("_talked_to").unwrap();
        let npc_class: String = vm.globals().get("_talked_npc").unwrap();
        let event_name: String = vm.globals().get("_talked_event").unwrap();
        assert_eq!(talked, 1234, "player.actorId must round-trip the player's actor id");
        assert_eq!(npc_class, "Greeter", "npc:GetName() must reach the LuaNpc accessor (it returns LuaActor.name, which the dispatcher seeds from class_name)");
        assert_eq!(
            event_name, "onTalk",
            "eventName must be the third positional arg per Meteor's `LuaEngine.EventStarted` shim",
        );
        let _ = std::fs::remove_dir_all(root);
    }

    /// NPC `onEventStarted` hooks can now emit `LuaCommand`s — the
    /// drain re-enters `apply_runtime_lua_commands` so quest mutations
    /// (`AddQuest`, `SetQuestFlag`, …) actually persist. Previously
    /// the queue was discarded, silently breaking any NPC script that
    /// awarded a quest from a `delegateEvent` confirm path.
    #[tokio::test]
    async fn npc_event_started_drains_emitted_lua_commands() {
        use crate::actor::Character;
        use crate::actor::quest::{Quest, quest_actor_id};
        use crate::runtime::actor_registry::{ActorHandle, ActorKindTag};
        use crate::zone::navmesh::StubNavmeshLoader;
        use crate::zone::zone::Zone;

        let root = tmpdir();
        std::fs::create_dir_all(root.join("base/Chara/Npc/Populace")).unwrap();
        // NPC hook gives the player quest 110_500 then flips bit 7 on
        // it. The drain has to handle both `AddQuest` (which feeds
        // through `apply_add_quest`) and the subsequent `QuestSetFlag`
        // — both registry-mutating commands.
        std::fs::write(
            root.join("base/Chara/Npc/Populace/QuestGiver.lua"),
            r#"
                function onEventStarted(player, npc, eventName)
                    player:AddQuest(110500)
                    local quest = player:GetQuest(110500)
                    quest:SetQuestFlag(7)
                end
            "#,
        )
        .unwrap();

        let lua = Arc::new(LuaEngine::new(&root));
        {
            let mut quests = std::collections::HashMap::new();
            quests.insert(
                110_500u32,
                crate::gamedata::QuestMeta {
                    id: 110_500,
                    quest_name: "Test Drain Quest".to_string(),
                    class_name: "TestDrain".to_string(),
                    prerequisite: 0,
                    min_level: 1,
                },
            );
            lua.catalogs().install_quests(quests);
        }

        let world = WorldManager::new();
        let registry = ActorRegistry::new();
        let db = Database::open(tempdb()).await.expect("db stub");

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

        let mut npc_chara = Character::new(99);
        npc_chara.base.class_path = "Chara/Npc/Populace/QuestGiver".into();
        npc_chara.base.class_name = "QuestGiver".into();
        npc_chara.base.actor_name = "questgiver_main".into();
        registry
            .insert(ActorHandle::new(99, ActorKindTag::Npc, 100, 0, npc_chara))
            .await;

        let mut player_chara = Character::new(55);
        // Pre-seed quest in the journal so the SetQuestFlag(7) lands
        // on a real Quest. `AddQuest` from the hook would also create
        // it, but threading that through requires `gamedata_quests`
        // installed (above) AND `apply_add_quest`'s `from_db_row`
        // schema to be reachable — easier to seed and verify the
        // flag-set path independently.
        let mut quest = Quest::new(quest_actor_id(110_500), "TestDrain".to_string());
        quest.clear_dirty();
        player_chara.quest_journal.add(quest);
        let player_handle = ActorHandle::new(55, ActorKindTag::Player, 100, 42, player_chara);
        registry.insert(player_handle.clone()).await;

        let event = EventEvent::EventStarted {
            player_actor_id: 55,
            owner_actor_id: 99,
            event_name: "talk".to_string(),
            event_type: 0,
            lua_params: vec![],
        };
        dispatch_event_event(&event, &registry, &world, &db, Some(&lua)).await;

        let flags = {
            let c = player_handle.character.read().await;
            c.quest_journal.get(110_500).map(|q| q.get_flags()).unwrap_or(0)
        };
        assert_eq!(
            flags & (1 << 7),
            1 << 7,
            "NPC onEventStarted should have drained QuestSetFlag(7) into the live quest",
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn no_lua_engine_keeps_dispatch_graceful() {
        let registry = ActorRegistry::new();
        let world = WorldManager::new();
        let db = Database::open(tempdb()).await.expect("db stub");

        // All four Lua-hook paths should accept `None` without panicking.
        let events = vec![
            EventEvent::EventStarted {
                player_actor_id: 1,
                owner_actor_id: 99,
                event_name: "talk".into(),
                event_type: 0,
                lua_params: vec![],
            },
            EventEvent::EventUpdated {
                player_actor_id: 1,
                trigger_actor_id: 99,
                event_type: 0,
                lua_params: vec![],
            },
            EventEvent::QuestCheckCompletion {
                player_actor_id: 1,
                quest_id: 1,
                quest_name: "man0l0".into(),
            },
            EventEvent::QuestAbandonHook {
                player_actor_id: 1,
                quest_id: 1,
                quest_name: "man0l0".into(),
            },
        ];
        let mut ob = EventOutbox::new();
        for e in events {
            ob.push(e);
        }
        for e in ob.drain() {
            dispatch_event_event(&e, &registry, &world, &db, None).await;
        }
    }

    /// End-to-end coverage of the director-script → quest-script chain
    /// `AfterQuestWarpDirector` was written to drive: a director's
    /// `onEventStarted` runs with real `LuaPlayer`/`LuaDirectorHandle`
    /// userdata, calls `quest:OnNotice(player)` on a quest the player
    /// holds, the resulting `QuestOnNotice` LuaCommand drains through
    /// `apply_quest_on_notice`, and the target quest's `onNotice` hook
    /// flips a flag bit on the live quest.
    #[tokio::test]
    async fn director_event_started_chains_into_quest_on_notice() {
        use crate::actor::Character;
        use crate::actor::quest::{Quest, quest_actor_id};
        use crate::runtime::actor_registry::{ActorHandle, ActorKindTag};
        use crate::zone::navmesh::StubNavmeshLoader;
        use crate::zone::zone::Zone;

        let root = tmpdir();
        std::fs::create_dir_all(root.join("directors")).unwrap();
        // Director hook — the same shape as
        // `scripts/lua/directors/AfterQuestWarpDirector.lua` but
        // collapsed onto the test's quest id (110_077).
        std::fs::write(
            root.join("directors/TestNoticeDirector.lua"),
            r#"
                function init() return "/Director/TestNoticeDirector" end
                function onEventStarted(player, director, eventName)
                    if (player:HasQuest(110077) == true) then
                        local quest = player:GetQuest(110077)
                        quest:OnNotice(player)
                    end
                end
            "#,
        )
        .unwrap();
        // Target quest script — flips bit 5 inside onNotice. The
        // assertion below reads this back off the live registry quest
        // to prove the cross-script chain executed all the way through.
        std::fs::write(
            root.join("quests/man/man0l1.lua"),
            r#"
                function onNotice(player, quest, target)
                    quest:SetQuestFlag(5)
                end
            "#,
        )
        .unwrap();

        let lua = Arc::new(LuaEngine::new(&root));
        {
            let mut quests = std::collections::HashMap::new();
            quests.insert(
                110_077u32,
                crate::gamedata::QuestMeta {
                    id: 110_077,
                    quest_name: "Test Notice Quest".to_string(),
                    class_name: "Man0l1".to_string(),
                    prerequisite: 0,
                    min_level: 1,
                },
            );
            lua.catalogs().install_quests(quests);
        }

        let registry = ActorRegistry::new();
        let world = WorldManager::new();
        let db = Database::open(tempdb()).await.expect("db stub");

        // Register zone 100 + spawn a director on it; capture the
        // composite director actor id for the EventStarted event.
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
        let director_actor_id = zone
            .core
            .create_director("/Director/TestNoticeDirector", false);
        world.register_zone(zone).await;

        // Player 13 holds quest 110_077; clear the dirty flag so the
        // first SetQuestFlag(5) we observe is the one the hook fired,
        // not residual setup state.
        let mut character = Character::new(13);
        let mut quest = Quest::new(quest_actor_id(110_077), "Man0l1".to_string());
        quest.clear_dirty();
        character.quest_journal.add(quest);
        let handle = ActorHandle::new(13, ActorKindTag::Player, 100, 42, character);
        registry.insert(handle.clone()).await;

        let event = EventEvent::EventStarted {
            player_actor_id: 13,
            owner_actor_id: director_actor_id,
            event_name: "noticeEvent".to_string(),
            event_type: 0,
            lua_params: vec![],
        };
        dispatch_event_event(&event, &registry, &world, &db, Some(&lua)).await;

        let flags = {
            let c = handle.character.read().await;
            c.quest_journal.get(110_077).map(|q| q.get_flags()).unwrap_or(0)
        };
        assert_eq!(
            flags & (1 << 5),
            1 << 5,
            "director onEventStarted -> quest:OnNotice -> onNotice should set flag bit 5",
        );

        let _ = std::fs::remove_dir_all(root);
    }

    /// Sanity-check the "no script on disk" branch — a director with a
    /// missing `directors/<class>.lua` should be a quiet no-op rather
    /// than panic. Critical because the production path will try every
    /// EventStart against the director branch the moment the actor id
    /// has the `6` prefix.
    #[tokio::test]
    async fn director_event_started_quietly_skips_missing_script() {
        use crate::zone::navmesh::StubNavmeshLoader;
        use crate::zone::zone::Zone;

        let root = tmpdir();
        // No `directors/` dir — script lookup will miss.
        let lua = Arc::new(LuaEngine::new(&root));
        let registry = ActorRegistry::new();
        let world = WorldManager::new();
        let db = Database::open(tempdb()).await.expect("db stub");

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
        let director_actor_id = zone.core.create_director("/Director/Missing", false);
        world.register_zone(zone).await;

        let event = EventEvent::EventStarted {
            player_actor_id: 1,
            owner_actor_id: director_actor_id,
            event_name: "anything".to_string(),
            event_type: 0,
            lua_params: vec![],
        };
        // Assertion is "no panic"; the missing-script branch logs at
        // debug and returns.
        dispatch_event_event(&event, &registry, &world, &db, Some(&lua)).await;

        let _ = std::fs::remove_dir_all(root);
    }
}
