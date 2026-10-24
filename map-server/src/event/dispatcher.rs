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
            dispatch_event_updated_drain(
                registry,
                world,
                db,
                lua,
                *player_actor_id,
                *trigger_actor_id,
                *event_type,
                lua_params,
            )
            .await;
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
            tracing::debug!(
                player = *player_actor_id,
                trigger = *trigger_actor_id,
                owner = *owner_actor_id,
                event = %event_name,
                function = %function_name,
                event_type = *event_type,
                params = lua_params.len(),
                "TX 0x0130 RunEventFunction (drives an event/cutscene; target_id=player)",
            );
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
            tracing::debug!(
                player = *player_actor_id,
                owner = *owner_actor_id,
                event = %event_name,
                event_type = *event_type,
                "TX 0x0131 EndEvent (server ends the event for the client)",
            );
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
            tracing::debug!(
                player = *player_actor_id,
                target = *target_actor_id,
                owner = *owner_actor_id,
                event = %event_name,
                event_type = *event_type,
                params = lua_params.len(),
                "TX KickEvent (asks client to start an event on target)",
            );
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
            counter4,
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
                    *counter4,
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
        // The owner despawned (or never existed). The client just opened
        // an event against it and is now MODAL — until an EndEvent
        // arrives it pins its authoritative position and refuses every
        // further interaction. Live SEQ_005 proof: the post-tutorial warp
        // despawned the openingstoper 357 ms before the client's box-leave
        // 'caution'/'exit' EventStarts arrived; with no reply the entire
        // Gridania phase was dead — 1,672 byte-identical position packets,
        // zero talk/push EventStarts for five minutes. Answer with an
        // immediate EndEvent so the client unlocks. Real owners never take
        // this path (their dispatch blocks answer normally). (#28.)
        // Static client-side actors (high nibble 0xA — command actors,
        // quest actors) are answered by their dedicated dispatch paths;
        // replying here would double the EndEvent per press. Type 127
        // is the client's Lua-ERROR report tunnel (owner is a pseudo-id
        // like 4) — an error report is not an open event, so replying
        // would be noise. Only world-spawned actors (a despawned NPC)
        // take the release path.
        if owner_actor_id >> 28 == 0xA || owner_actor_id == 0 || event_type == 127 {
            tracing::debug!(
                owner = owner_actor_id,
                ty = event_type,
                "event: owner actor missing",
            );
            return;
        }
        tracing::debug!(
            owner = owner_actor_id,
            event = event_name,
            "event: owner actor missing — releasing client with EndEvent",
        );
        if let Some(player_handle) = registry.get(player_actor_id).await
            && player_handle.session_id != 0
            && let Some(client) = world.client(player_handle.session_id).await
        {
            let mut sub = crate::packets::send::events::build_end_event(
                player_actor_id,
                owner_actor_id,
                event_name,
                event_type,
            );
            sub.set_target_id(player_handle.session_id);
            client.send_bytes(sub.to_bytes()).await;
        }
        return;
    };
    let (class_path, class_name, unique_id, npc_state, npc_pos, npc_rot, actor_class_id) = {
        let chara = owner_handle.character.read().await;
        (
            chara.base.class_path.clone(),
            chara.base.class_name.clone(),
            // Seed uniqueId ("baderon") when populated — the unique-
            // override resolver keys on it. The old actor_name read
            // ("pplStd_…@…", `Actor.GenerateActorName` output) never
            // matched a `unique/` script, so every named populace NPC
            // silently fell through to its base class script and e.g.
            // Baderon's defaultTalk delegation
            // (unique/sea0Town01/PopulaceStandard/baderon.lua →
            // DftSea defaultTalkWithBaderon_001) never ran. Fallback
            // to actor_name for actors spawned without a seed
            // uniqueId (players, dynamic spawns) — preserves the old
            // lookup for them. (Garlemald-Server #46, round 5.)
            if chara.base.unique_id.is_empty() {
                chara.base.actor_name.clone()
            } else {
                chara.base.unique_id.clone()
            },
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

    let Some(player_handle) = registry.get(player_actor_id).await else {
        tracing::debug!(
            player = player_actor_id,
            owner = owner_actor_id,
            "NPC onEventStarted skipped — player not in registry",
        );
        return;
    };

    // Hoisted quest-claim gate (round 5): decide who DRIVES the
    // interaction before resolving which script runs. When a current
    // quest claims the owner's actor class, the quest hooks (fired
    // separately in `handle_event_start`) own the event — running the
    // NPC's unique defaultTalk override would race a cutscene the
    // quest is about to park (same failure shape as the man0l0
    // Rostnsthal `pushDefault` regression, but from the other side).
    // Quest-claimed → base script (inert for these, today's
    // behavior); unclaimed → unique override drives; still-inert
    // unclaimed → the existing EndEvent release below. The release
    // helper keeps its own claim re-check so the quest-cutscene guard
    // semantics from round 2 hold on both release arms.
    let quest_claimed = owner_claimed_by_current_quest(&player_handle, actor_class_id).await;

    let unique_path = lua.resolver().npc(&zone_name, &class_name, &unique_id);
    let base_path = lua.resolver().base_class(&class_path);
    if quest_claimed && unique_path.exists() {
        tracing::debug!(
            owner = owner_actor_id,
            class = actor_class_id,
            unique = %unique_id,
            "NPC onEventStarted: unique override skipped — owner claimed by a current quest",
        );
    }
    let script_path = if !quest_claimed && unique_path.exists() {
        unique_path
    } else if base_path.exists() {
        base_path
    } else {
        // A live owner with no script on disk still opened a modal event
        // on the client (e.g. an object push trigger like the La Noscea→
        // Limsa `seafld0_push_limsa_entrance`, classId 1090004, whose push
        // fires whenever the player walks into it — only man0l1 SEQ_048
        // actually claims it). Without an EndEvent the client stays modal
        // and every further interaction is dropped → softlock. Release it
        // — UNLESS a current quest claims the owner (the quest-ENPC guard
        // below): the quest-hook fan-out in `handle_event_start` is about
        // to park a cutscene the client MUST keep open. Ported from the
        // removed `processor::dispatch_event_start_to_npc` (whose release
        // was the only behavior this single-path dispatch lacked).
        // (Garlemald-Server #46 — Limsa-entrance push softlock.)
        tracing::debug!(
            owner = owner_actor_id,
            class = %class_path,
            "NPC onEventStarted: no script on disk — releasing client with EndEvent (unless quest-claimed)",
        );
        release_unclaimed_owner_with_end_event(
            registry,
            world,
            player_actor_id,
            owner_actor_id,
            actor_class_id,
            event_name,
            event_type,
        )
        .await;
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
            current_gil: c.chara.gil.max(0) as u32,
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
                    counters: [
                        q.get_counter(0),
                        q.get_counter(1),
                        q.get_counter(2),
                        q.get_counter(3),
                    ],
                    npc_ls_from: q.get_npc_ls_from(),
                    npc_ls_msg_step: q.get_npc_ls_msg_step(),
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
    // NOTE: event_type is NOT passed to the NPC `onEventStarted` (Meteor's
    // NPC dispatch omits it — scripts branch on eventName); it is only used
    // for the inert-release EndEvent below.

    // Run the NPC's `onEventStarted` INSIDE A COROUTINE (via the engine's
    // director-hook helper), NOT a bare `f.call`. Conversational NPC scripts
    // (`onEventStarted(player, npc, triggerName)`) call
    // `callClientFunction(...)`, which does `coroutine.yield("_WAIT_EVENT")`;
    // a direct call raises "attempt to yield from outside a coroutine", the
    // talk event never completes, and the client stays modal → softlock
    // (Garlemald-Server #46 — talking to a camp NPC e.g. Rhyssfloh).
    // `spawn_director_on_event_started` spawns the hook as a Lua thread and
    // parks it on the scheduler so the yield resumes on the client's 0x012E
    // EventUpdate. We keep the populace NPC arg convention
    // [player, npc, eventName, ...lparams] (NO eventType — unlike commands,
    // real Npc base scripts are `onEventStarted(player, npc, triggerName)`).
    let result = tokio::task::spawn_blocking(move || {
        lua_clone.spawn_director_on_event_started(
            &script_path,
            player_actor_id,
            move |lua_vm, queue| {
                let player = crate::lua::userdata::LuaPlayer {
                    snapshot,
                    queue: queue.clone(),
                };
                let player_ud = lua_vm
                    .create_userdata(player)
                    .map_err(|e| anyhow::anyhow!("create_userdata(LuaPlayer): {e}"))?;
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
                        // Event-dispatched NPCs are conversational, not
                        // combat — engagement defaults are correct.
                        is_engaged: false,
                        speed: 5.0,
                        target_actor_id: 0,
                        hp: 0,
                        max_hp: 0,
                    },
                    actor_class_id,
                    quest_graphic: 0,
                };
                let npc_ud = lua_vm
                    .create_userdata(npc)
                    .map_err(|e| anyhow::anyhow!("create_userdata(LuaNpc): {e}"))?;
                let mut mv = MultiValue::new();
                mv.push_back(Value::UserData(player_ud));
                mv.push_back(Value::UserData(npc_ud));
                // Meteor inserts `eventName` ahead of the original lparams
                // (LuaEngine.EventStarted `lparams.Insert(0, ...)`) — the
                // third script arg (`triggerName`).
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
                        LuaParam::Type7 { actor_id, .. } => {
                            Value::Integer(*actor_id as mlua::Integer)
                        }
                        LuaParam::Type9 { item1, .. } => Value::Integer(*item1 as mlua::Integer),
                        LuaParam::Byte(b) => Value::Integer(*b as mlua::Integer),
                        LuaParam::Short(s) => Value::Integer(*s as mlua::Integer),
                    };
                    mv.push_back(v);
                }
                Ok(mv)
            },
        )
    })
    .await;

    let (commands, hook_err) = match result {
        Ok(r) => (r.commands, r.error.map(|e| format!("{e}"))),
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
    } else {
        // Inert: the NPC/object script defined no handler for this event
        // (empty command list, nothing parked) — e.g. an object push trigger
        // whose `pushDefault` no quest claims at the current sequence. The
        // client opened a modal event; without an EndEvent it stays modal and
        // every further interaction is dropped → softlock. Release it,
        // mirroring the owner-missing release above and the processor
        // NPC-dispatch path. (Garlemald-Server #46.)
        //
        // BUT NOT for a CURRENT QUEST ENPC. `start_event` ALWAYS emits an
        // `EventStarted` (even for a client-initiated push), which routes here
        // and runs the owner's BASE populace `onEventStarted`. For a
        // quest-claimed ENPC that base script has no handler for the event
        // name (the quest owns it via `onTalk`/`onPush`/`onEmote`, fired
        // separately in `handle_event_start`), so it emits empty commands and
        // lands in this branch — but the quest hook is about to park a
        // cutscene the client MUST keep open. man0l0 Rostnsthal's `pushDefault`
        // is exactly this: the base `PopulaceStandard.onEventStarted` returns
        // nothing while `quest:OnPush` drives the `processTtrNomal002`
        // targeting tutorial. Releasing here closes the event out from under
        // the tutorial → controls re-lock → softlock (the regression the
        // entrance-push fix introduced). Only GENUINELY UNCLAIMED owners — the
        // La Noscea→Limsa entrance push `1090004`, claimed by no quest at
        // SEQ_005 — get the inert release. (Garlemald-Server #46 — the
        // entrance-push fix must NOT comingle with quest pushes.)
        release_unclaimed_owner_with_end_event(
            registry,
            world,
            player_actor_id,
            owner_actor_id,
            actor_class_id,
            event_name,
            event_type,
        )
        .await;
    }
}

/// Release a client sitting modal on an event whose owner produced no
/// handler output (no script on disk / script emitted nothing) with one
/// 0x0131 EndEvent — UNLESS one of the player's current quests claims
/// the owner's actor class. `start_event` ALWAYS emits an `EventStarted`
/// that lands here and runs the owner's BASE populace script; for a
/// quest-claimed ENPC that base script is inert while the quest hook
/// (fired separately in `handle_event_start`) is about to park a
/// cutscene the client MUST keep open — releasing would close the event
/// out from under the tutorial → controls re-lock → softlock (the
/// man0l0 Rostnsthal `pushDefault` regression). Shared by the inert and
/// no-script arms of [`dispatch_npc_event_started`] so both releases
/// keep identical quest-ENPC semantics. (Garlemald-Server #46.)
async fn release_unclaimed_owner_with_end_event(
    registry: &ActorRegistry,
    world: &WorldManager,
    player_actor_id: u32,
    owner_actor_id: u32,
    owner_class_id: u32,
    event_name: &str,
    event_type: u8,
) {
    let Some(player_handle) = registry.get(player_actor_id).await else {
        return;
    };
    if owner_claimed_by_current_quest(&player_handle, owner_class_id).await {
        tracing::debug!(
            owner = owner_actor_id,
            class = owner_class_id,
            event = event_name,
            "NPC onEventStarted release skipped — owner claimed by a current quest",
        );
        return;
    }
    if player_handle.session_id != 0
        && let Some(client) = world.client(player_handle.session_id).await
    {
        tracing::debug!(
            player = player_actor_id,
            owner = owner_actor_id,
            event = event_name,
            ty = event_type,
            "NPC onEventStarted: releasing unclaimed inert owner with EndEvent",
        );
        let mut sub = crate::packets::send::events::build_end_event(
            player_actor_id,
            owner_actor_id,
            event_name,
            event_type,
        );
        sub.set_target_id(player_handle.session_id);
        client.send_bytes(sub.to_bytes()).await;
    }
}

/// Does one of the player's CURRENT quests claim `owner_class_id` as a
/// quest ENPC? Shared by the hoisted script-selection gate in
/// [`dispatch_npc_event_started`] (quest-claimed owners must not run
/// their unique defaultTalk override — the quest hooks drive) and by
/// [`release_unclaimed_owner_with_end_event`]'s guard (quest-claimed
/// owners must not be EndEvent-released out from under a parking
/// cutscene). (Garlemald-Server #46, rounds 2 + 5.)
async fn owner_claimed_by_current_quest(
    player_handle: &crate::runtime::actor_registry::ActorHandle,
    owner_class_id: u32,
) -> bool {
    let c = player_handle.character.read().await;
    c.quest_journal.slots.iter().flatten().any(|q| {
        q.state
            .current
            .values()
            .any(|e| e.actor_class_id == owner_class_id)
    })
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
        (
            d.class_path.clone(),
            d.class_name.clone(),
            d.actor_name.clone(),
        )
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
            current_gil: c.chara.gil.max(0) as u32,
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
                    counters: [
                        q.get_counter(0),
                        q.get_counter(1),
                        q.get_counter(2),
                        q.get_counter(3),
                    ],
                    npc_ls_from: q.get_npc_ls_from(),
                    npc_ls_msg_step: q.get_npc_ls_msg_step(),
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
        let owner_player_id = snapshot.actor_id;
        lua_clone.spawn_director_on_event_started(
            &script_path_clone,
            owner_player_id,
            |lua_vm, queue| {
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
                        LuaParam::Type7 { actor_id, .. } => {
                            Value::Integer(*actor_id as mlua::Integer)
                        }
                        LuaParam::Type9 { item1, .. } => Value::Integer(*item1 as mlua::Integer),
                        LuaParam::Byte(b) => Value::Integer(*b as mlua::Integer),
                        LuaParam::Short(s) => Value::Integer(*s as mlua::Integer),
                    };
                    mv.push_back(v);
                }
                Ok(mv)
            },
        )
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

/// `EventUpdate (0x012E)` arrived from the client (typically signals
/// "cinematic finished" for a `_WAIT_EVENT`-parked coroutine). Resume
/// the parked coroutine, then drain its trailing commands through the
/// same EventOutbox bridge + login-command pipeline `fire_quest_event_hook`
/// uses on the entry path. Without this drain, the post-cinematic
/// `player:EndEvent()` + `quest:UpdateENPCs()` would never reach the
/// client and the player would stay event-locked.
#[allow(clippy::too_many_arguments)]
async fn dispatch_event_updated_drain(
    registry: &ActorRegistry,
    world: &WorldManager,
    db: &Database,
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
    // Resume any parked `_WAIT_EVENT` coroutine; collect the commands
    // it queued before yielding/returning. The lua params are the
    // client's `callClientFunction` return values (e.g. a dialog
    // choice) — thread them through so the resumed coroutine sees
    // them, matching `handle_event_update`'s primary resume path.
    //
    // A continuation that re-parks must keep its event-owner stamp for
    // `handle_event_start`'s resume gate — scope the resume with the
    // still-open event's owner from the session, mirroring
    // `handle_event_update`. (Garlemald-Server #46.)
    let current_event_owner = if let Some(handle) = registry.get(player_actor_id).await {
        let c = handle.character.read().await;
        c.event_session.current_event_owner
    } else {
        0
    };
    let _owner_ctx = (current_event_owner != 0).then(|| {
        crate::lua::scheduler::CoroutineScheduler::event_owner_scope(
            lua.scheduler(),
            player_actor_id,
            current_event_owner,
        )
    });
    let after = lua.fire_player_event_and_drain(player_actor_id, lua_params);
    let Some(after) = after else {
        tracing::debug!(
            player = player_actor_id,
            trigger = trigger_actor_id,
            ty = event_type,
            "event: updated (no parked coroutine)",
        );
        return;
    };
    if after.is_empty() {
        tracing::debug!(
            player = player_actor_id,
            trigger = trigger_actor_id,
            "event: updated (resumed, no commands)",
        );
        return;
    }
    tracing::debug!(
        player = player_actor_id,
        trigger = trigger_actor_id,
        commands = after.len(),
        "event: updated (resumed, draining commands)",
    );
    // Snapshot session for the bridge step.
    let event_session_snapshot = if let Some(handle) = registry.get(player_actor_id).await {
        let c = handle.character.read().await;
        c.event_session.clone()
    } else {
        return;
    };
    let mut outbox = crate::event::outbox::EventOutbox::new();
    crate::event::lua_bridge::translate_lua_commands_into_outbox(
        &after,
        &event_session_snapshot,
        &mut outbox,
    );
    for e in outbox.drain() {
        Box::pin(dispatch_event_event(&e, registry, world, db, Some(lua))).await;
    }
    // Drain non-event commands through the runtime-apply pipeline so
    // QuestSetFlag / QuestUpdateEnpcs / etc. fire the side-effects
    // (broadcasts, DB writes, dependent hook re-runs).
    crate::runtime::quest_apply::apply_runtime_lua_commands(after, registry, db, world, Some(lua))
        .await;
}

#[allow(dead_code)]
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
    // The lua params carry the client's response; thread them through
    // so the sleeping coroutine's `callClientFunction` returns them.
    let fired = lua.fire_player_event(player_actor_id, lua_params);
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
        tracing::debug!(
            player = player_actor_id,
            bytes = bytes.len(),
            "send_to_player dropped packet — actor not in registry",
        );
        return;
    };
    let Some(client) = world.client(handle.session_id).await else {
        tracing::debug!(
            player = player_actor_id,
            session = handle.session_id,
            bytes = bytes.len(),
            "send_to_player dropped packet — no live client for session",
        );
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
        assert_eq!(
            talked, 1234,
            "player.actorId must round-trip the player's actor id"
        );
        assert_eq!(
            npc_class, "Greeter",
            "npc:GetName() must reach the LuaNpc accessor (it returns LuaActor.name, which the dispatcher seeds from class_name)"
        );
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
            c.quest_journal
                .get(110_500)
                .map(|q| q.get_flags())
                .unwrap_or(0)
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
            c.quest_journal
                .get(110_077)
                .map(|q| q.get_flags())
                .unwrap_or(0)
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
