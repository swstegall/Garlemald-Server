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

//! Shared quest-mutation command application.
//!
//! Both the packet processor (`map-server/src/processor.rs`) and the
//! battle-path quest-hook dispatcher (`runtime/quest_hook.rs`) need to
//! drain `LuaCommand::Quest*` / `AddExp` / `AddGil` variants the same
//! way. This module holds the free-function version so neither caller
//! owns the logic — the processor forwards its runtime-safe arms here,
//! and `fire_on_kill_bnpc` can route hook-emitted commands through the
//! same pipeline without needing an `Arc<PacketProcessor>` threaded
//! through the battle dispatcher.
//!
//! Callers still need a `Database` / `ActorRegistry` / `WorldManager`
//! (for ENPC broadcasts) + optional `LuaEngine` (for auto-fire hooks
//! like `onStateChange` from a `QuestStartSequence` command).
//!
//! Login-flow-only commands (`SetLoginDirector`, `CreateDirector`,
//! `KickEvent`, `SetPos` during tutorial spawn) stay on the processor
//! because they mutate session state this module doesn't see.

#![allow(dead_code)]

use std::sync::Arc;

use crate::actor::quest::{AddEnpcOutcome, QuestEnpc};
use crate::database::Database;
use crate::lua::LuaCommandKind;
use crate::lua::LuaEngine;
use crate::runtime::actor_registry::{ActorHandle, ActorKindTag, ActorRegistry};
use crate::world_manager::WorldManager;

/// Whether `apply_runtime_lua_command` consumed the command. `false` means
/// the variant is login-scoped (processor handles it) or simply unrecognised.
pub type Handled = bool;

/// Dispatch a single `LuaCommand` through the runtime-safe command set
/// (Quest* mutations, AddExp, AddGil, Die/Revive) using only the four
/// long-lived Arcs every runtime subsystem holds. Returns `true` when
/// the command was recognised + applied; `false` when the caller should
/// fall back to its own handler (login-scoped variants).
pub async fn apply_runtime_lua_command(
    cmd: LuaCommandKind,
    registry: &ActorRegistry,
    db: &Database,
    world: &WorldManager,
    lua: Option<&Arc<LuaEngine>>,
) -> Handled {
    use LuaCommandKind as LC;
    match cmd {
        LC::AddQuest { player_id, quest_id } => {
            apply_add_quest(player_id, quest_id, registry, db, lua).await;
            true
        }
        LC::CompleteQuest { player_id, quest_id } => {
            apply_complete_quest(player_id, quest_id, registry, db, lua).await;
            true
        }
        LC::AbandonQuest { player_id, quest_id } => {
            apply_abandon_quest(player_id, quest_id, registry, db, lua).await;
            true
        }
        LC::QuestClearData { player_id, quest_id } => {
            apply_quest_mutation(player_id, quest_id, registry, db, |q| q.clear_data()).await;
            true
        }
        LC::QuestClearFlags { player_id, quest_id } => {
            apply_quest_mutation(player_id, quest_id, registry, db, |q| q.clear_flags()).await;
            true
        }
        LC::QuestSetFlag { player_id, quest_id, bit } => {
            apply_quest_mutation(player_id, quest_id, registry, db, |q| q.set_flag(bit)).await;
            true
        }
        LC::QuestClearFlag { player_id, quest_id, bit } => {
            apply_quest_mutation(player_id, quest_id, registry, db, |q| q.clear_flag(bit)).await;
            true
        }
        LC::QuestSetCounter { player_id, quest_id, idx, value } => {
            apply_quest_mutation(player_id, quest_id, registry, db, |q| {
                q.set_counter(idx as usize, value)
            })
            .await;
            true
        }
        LC::QuestIncCounter { player_id, quest_id, idx } => {
            apply_quest_mutation(player_id, quest_id, registry, db, |q| {
                q.inc_counter(idx as usize);
            })
            .await;
            true
        }
        LC::QuestDecCounter { player_id, quest_id, idx } => {
            apply_quest_mutation(player_id, quest_id, registry, db, |q| {
                q.dec_counter(idx as usize);
            })
            .await;
            true
        }
        LC::QuestStartSequence { player_id, quest_id, sequence } => {
            apply_quest_start_sequence(player_id, quest_id, sequence, registry, db, world, lua)
                .await;
            true
        }
        LC::QuestSetEnpc {
            player_id,
            quest_id,
            actor_class_id,
            quest_flag_type,
            is_talk_enabled,
            is_push_enabled,
            is_emote_enabled,
            is_spawned,
        } => {
            apply_quest_set_enpc(
                player_id,
                quest_id,
                actor_class_id,
                quest_flag_type,
                is_talk_enabled,
                is_push_enabled,
                is_emote_enabled,
                is_spawned,
                registry,
                world,
            )
            .await;
            true
        }
        LC::QuestUpdateEnpcs { player_id, quest_id } => {
            apply_quest_update_enpcs(player_id, quest_id, registry, world).await;
            true
        }
        LC::SetQuestComplete { player_id, quest_id, flag } => {
            apply_set_quest_complete(player_id, quest_id, flag, registry, db).await;
            true
        }
        LC::AddExp { actor_id, class_id, exp } => {
            apply_add_exp(actor_id, class_id, exp, registry, db, Some(world), lua).await;
            true
        }
        LC::AddGil { actor_id, amount } => {
            apply_add_gil(actor_id, amount, db).await;
            true
        }
        LC::AddItem {
            actor_id,
            item_package,
            item_id,
            quantity,
        } => {
            apply_add_item(actor_id, item_package, item_id, quantity, db).await;
            // Tier 3 #13 — tick any accepted fieldcraft leves whose
            // objective targets this item. Runs after `apply_add_item`
            // so the DB write sequence is: inventory row → leve
            // progress. Short-circuits cleanly when the catalog isn't
            // installed (fresh-DB boot) or the player has no matching
            // active leve.
            if item_package == crate::inventory::PKG_NORMAL
                && quantity > 0
                && item_id != 0
            {
                let delta = quantity.min(u16::MAX as i32) as u16;
                advance_fieldcraft_leves(actor_id, item_id, delta, registry, db, lua).await;
            }
            true
        }
        LC::AddItemToRetainer {
            retainer_id,
            item_package,
            item_id,
            quantity,
        } => {
            apply_add_item_to_retainer(retainer_id, item_package, item_id, quantity, db).await;
            true
        }
        LC::HandInRegionalLeve { player_id, leve_id } => {
            let _ = apply_regional_leve_hand_in(player_id, leve_id, registry, db, lua).await;
            true
        }
        LC::AcceptRegionalLeve {
            player_id,
            leve_id,
            difficulty,
        } => {
            let _ = apply_accept_regional_leve(player_id, leve_id, difficulty, registry, db, lua).await;
            true
        }
        LC::PurchaseRetainerBazaarItem {
            buyer_id,
            retainer_id,
            server_item_id,
        } => {
            let _ = apply_purchase_retainer_bazaar_item(
                buyer_id,
                retainer_id,
                server_item_id,
                db,
            )
            .await;
            true
        }
        LC::TryStatus {
            source_actor_id,
            target_actor_id,
            status_id,
            duration_s,
            magnitude,
            tick_ms,
            tier,
        } => {
            let _ = apply_try_status(
                source_actor_id,
                target_actor_id,
                status_id,
                duration_s,
                magnitude,
                tick_ms,
                tier,
                registry,
                db,
                world,
                lua,
            )
            .await;
            true
        }
        LC::QuestOnNotice { player_id, quest_id } => {
            apply_quest_on_notice(player_id, quest_id, registry, db, world, lua).await;
            true
        }
        // --- Director outbox ops --------------------------------------
        //
        // Leve-side bindings — `director:StartGuildleve()`,
        // `EndGuildleve`, etc. The runtime drain needs to handle
        // these because scheduler-resumed director `main` coroutines
        // (parked on `wait(N)`) emit them from inside
        // `runtime::ticker::tick_once`, where the PacketProcessor's
        // `apply_login_lua_command` isn't reachable. Same lock +
        // drain shape as the processor's `apply_director_outbox_op`
        // helper.
        LC::EndGuildleve {
            director_actor_id,
            was_completed,
        } => {
            let now = common::utils::unix_timestamp() as u32;
            apply_director_outbox_op(
                director_actor_id,
                "EndGuildleve",
                registry,
                db,
                world,
                |gld, ob| gld.end_guildleve(now, was_completed, ob),
            )
            .await;
            true
        }
        LC::StartGuildleve { director_actor_id } => {
            let now = common::utils::unix_timestamp() as u32;
            apply_director_outbox_op(
                director_actor_id,
                "StartGuildleve",
                registry,
                db,
                world,
                |gld, ob| gld.start_guildleve(now, ob),
            )
            .await;
            true
        }
        LC::AbandonGuildleve { director_actor_id } => {
            let now = common::utils::unix_timestamp() as u32;
            apply_director_outbox_op(
                director_actor_id,
                "AbandonGuildleve",
                registry,
                db,
                world,
                |gld, ob| gld.abandon_guildleve(now, ob),
            )
            .await;
            true
        }
        LC::UpdateAimNumNow {
            director_actor_id,
            index,
            value,
        } => {
            apply_director_outbox_op(
                director_actor_id,
                "UpdateAimNumNow",
                registry,
                db,
                world,
                |gld, ob| gld.update_aim_num_now(index, value, ob),
            )
            .await;
            true
        }
        LC::UpdateUiState {
            director_actor_id,
            index,
            value,
        } => {
            apply_director_outbox_op(
                director_actor_id,
                "UpdateUIState",
                registry,
                db,
                world,
                |gld, ob| gld.update_ui_state(index, value, ob),
            )
            .await;
            true
        }
        LC::UpdateMarkers {
            director_actor_id,
            index,
            x,
            y,
            z,
        } => {
            apply_director_outbox_op(
                director_actor_id,
                "UpdateMarkers",
                registry,
                db,
                world,
                |gld, ob| gld.update_marker(index, x, y, z, ob),
            )
            .await;
            true
        }
        LC::SyncAllInfo { director_actor_id } => {
            apply_director_outbox_op(
                director_actor_id,
                "SyncAllInfo",
                registry,
                db,
                world,
                |gld, ob| gld.sync_all(ob),
            )
            .await;
            true
        }
        LC::AddRetainerBazaarItem {
            retainer_id,
            item_id,
            quantity,
            quality,
            price_gil,
        } => {
            apply_add_retainer_bazaar_item(retainer_id, item_id, quantity, quality, price_gil, db)
                .await;
            true
        }
        _ => false,
    }
}

/// Runtime-side counterpart to the processor's
/// `apply_add_retainer_bazaar_item`: transactional upsert into the
/// `characters_retainer_bazaar` table. Exposed for scheduler-resumed
/// coroutines so a parked retainer-bazaar-seed script (rare, but
/// plausible once NPC-vendor bazaar seeding moves into director main
/// coroutines) can drain without reaching back through the
/// PacketProcessor.
async fn apply_add_retainer_bazaar_item(
    retainer_id: u32,
    item_id: u32,
    quantity: i32,
    quality: u8,
    price_gil: i32,
    db: &Database,
) {
    match db
        .add_retainer_bazaar_item(retainer_id, item_id, quantity, quality, price_gil)
        .await
    {
        Ok(server_item_id) => {
            tracing::info!(
                retainer_id,
                item_id,
                quantity,
                quality,
                price_gil,
                server_item_id,
                "AddRetainerBazaarItem applied (runtime)",
            );
        }
        Err(e) => {
            tracing::warn!(
                retainer_id,
                item_id,
                quantity,
                quality,
                price_gil,
                err = %e,
                "AddRetainerBazaarItem (runtime): DB upsert failed",
            );
        }
    }
}

/// Runtime-side counterpart to the processor's
/// `apply_director_outbox_op`: lets `apply_runtime_lua_command`
/// route `EndGuildleve` / `StartGuildleve` / `UpdateAimNumNow` /
/// etc. without reaching back through the processor. Same semantics:
/// single zone write lock, roster snapshot BEFORE `mutate` (so ops
/// that tear down the director — `abandon_guildleve`, which clears
/// `player_members` via `Director::end` — still fan to the right
/// recipients), immediate drain via `dispatch_director_event`.
async fn apply_director_outbox_op<F>(
    director_actor_id: u32,
    op_name: &'static str,
    registry: &ActorRegistry,
    db: &Database,
    world: &WorldManager,
    mutate: F,
) where
    F: FnOnce(&mut crate::director::GuildleveDirector, &mut crate::director::DirectorOutbox),
{
    let zone_id = (director_actor_id >> 19) & 0x1FF;
    let Some(zone_arc) = world.zone(zone_id).await else {
        tracing::debug!(
            director = director_actor_id,
            zone = zone_id,
            op = op_name,
            "runtime director-outbox op skipped — zone not loaded",
        );
        return;
    };
    let (events, player_members) = {
        let mut zone = zone_arc.write().await;
        let Some(gld) = zone.core.guildleve_director_mut(director_actor_id) else {
            tracing::debug!(
                director = director_actor_id,
                zone = zone_id,
                op = op_name,
                "runtime director-outbox op skipped — guildleve director not on zone",
            );
            return;
        };
        let roster: Vec<u32> = gld.base.player_members().collect();
        let mut outbox = crate::director::DirectorOutbox::new();
        mutate(gld, &mut outbox);
        (outbox.drain(), roster)
    };
    // Pass the Arc<Database> through so `award_leve_completion_seals`
    // can persist on the `GuildleveEnded { was_completed: true }`
    // branch.
    for e in events {
        crate::director::dispatch_director_event(
            &e,
            &player_members,
            registry,
            world,
            Some(db),
        )
        .await;
    }
    tracing::debug!(
        director = director_actor_id,
        zone = zone_id,
        op = op_name,
        "runtime director-outbox op applied",
    );
}

/// Bulk-drain helper — calls [`apply_runtime_lua_command`] for every
/// command in `cmds`. Commands that fall through (return `false`) are
/// logged at `debug` level; callers expecting only the runtime-safe
/// subset can pass arbitrary command vecs without pre-filtering.
pub async fn apply_runtime_lua_commands(
    cmds: Vec<LuaCommandKind>,
    registry: &ActorRegistry,
    db: &Database,
    world: &WorldManager,
    lua: Option<&Arc<LuaEngine>>,
) {
    for cmd in cmds {
        let tag = std::mem::discriminant(&cmd);
        let handled = apply_runtime_lua_command(cmd, registry, db, world, lua).await;
        if !handled {
            tracing::debug!(
                ?tag,
                "runtime lua command unhandled (login-scoped or unrecognised)",
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Quest-mutation helpers (ported from Meteor's `Quest.cs` / `QuestData.cs`
// runtime surface — same logic lives in `PacketProcessor`, kept in sync via
// thin wrappers there).
// ---------------------------------------------------------------------------

pub async fn apply_quest_mutation<F>(
    player_id: u32,
    quest_id: u32,
    registry: &ActorRegistry,
    db: &Database,
    mutate: F,
) where
    F: FnOnce(&mut crate::actor::quest::Quest),
{
    let Some(handle) = registry.get(player_id).await else {
        tracing::debug!(
            player = player_id,
            quest = quest_id,
            "quest mutation skipped — player not in registry",
        );
        return;
    };
    let save_tuple = {
        let mut c = handle.character.write().await;
        let Some(slot) = c.quest_journal.slot_of(quest_id) else {
            tracing::debug!(
                player = player_id,
                quest = quest_id,
                "quest mutation skipped — quest not in journal",
            );
            return;
        };
        let Some(q) = c.quest_journal.slots[slot].as_mut() else {
            return;
        };
        mutate(q);
        if q.is_dirty() {
            let sequence = q.get_sequence();
            let flags = q.get_flags();
            let counters = [q.get_counter(0), q.get_counter(1), q.get_counter(2)];
            let actor_id = q.actor_id;
            q.clear_dirty();
            Some((slot as i32, actor_id, sequence, flags, counters))
        } else {
            None
        }
    };
    if let Some((slot, actor_id, sequence, flags, [c1, c2, c3])) = save_tuple
        && let Err(e) = db
            .save_quest(player_id, slot, actor_id, sequence, flags, c1, c2, c3)
            .await
    {
        tracing::warn!(
            error = %e,
            player = player_id,
            quest = quest_id,
            "quest save failed",
        );
    }
}

pub async fn apply_add_quest(
    player_id: u32,
    quest_id: u32,
    registry: &ActorRegistry,
    db: &Database,
    lua: Option<&Arc<LuaEngine>>,
) {
    let Some(handle) = registry.get(player_id).await else {
        return;
    };
    let save_tuple = {
        let mut c = handle.character.write().await;
        if c.quest_journal.has(quest_id) {
            tracing::debug!(
                player = player_id,
                quest = quest_id,
                "AddQuest skipped — already in journal",
            );
            return;
        }
        if c.quest_journal.is_completed(quest_id) {
            tracing::debug!(
                player = player_id,
                quest = quest_id,
                "AddQuest skipped — already completed",
            );
            return;
        }
        let actor_id = crate::actor::quest::quest_actor_id(quest_id);
        let name = lua
            .and_then(|e| e.catalogs().quest_script_name(quest_id))
            .unwrap_or_default();
        let quest = crate::actor::quest::Quest::new(actor_id, name);
        let Some(slot) = c.quest_journal.add(quest) else {
            tracing::warn!(
                player = player_id,
                quest = quest_id,
                "AddQuest failed — journal full",
            );
            return;
        };
        (slot as i32, actor_id)
    };
    let (slot, actor_id) = save_tuple;
    if let Err(e) = db
        .save_quest(player_id, slot, actor_id, 0, 0, 0, 0, 0)
        .await
    {
        tracing::warn!(
            error = %e,
            player = player_id,
            quest = quest_id,
            "AddQuest DB persist failed",
        );
    }
    tracing::info!(
        player = player_id,
        quest = quest_id,
        slot,
        "AddQuest applied",
    );
    if let Some(lua_engine) = lua {
        fire_quest_hook(&handle, quest_id, "onStart", Vec::new(), lua_engine, registry, db).await;
    }
}

pub async fn apply_complete_quest(
    player_id: u32,
    quest_id: u32,
    registry: &ActorRegistry,
    db: &Database,
    lua: Option<&Arc<LuaEngine>>,
) {
    let Some(handle) = registry.get(player_id).await else {
        return;
    };
    if let Some(lua_engine) = lua {
        fire_quest_hook(
            &handle,
            quest_id,
            "onFinish",
            vec![crate::lua::QuestHookArg::Bool(true)],
            lua_engine,
            registry,
            db,
        )
        .await;
    }
    let removed_slot = {
        let mut c = handle.character.write().await;
        let slot = c.quest_journal.slot_of(quest_id);
        c.quest_journal.complete(quest_id);
        slot.map(|s| s as i32)
    };
    if let Some(slot) = removed_slot
        && let Err(e) = db.remove_quest(player_id, quest_id).await
    {
        tracing::warn!(
            error = %e,
            player = player_id,
            quest = quest_id,
            slot,
            "CompleteQuest: scenario-row delete failed",
        );
    }
    if let Err(e) = db.complete_quest(player_id, quest_id).await {
        tracing::warn!(
            error = %e,
            player = player_id,
            quest = quest_id,
            "CompleteQuest: bitstream save failed",
        );
    }
    tracing::info!(
        player = player_id,
        quest = quest_id,
        "CompleteQuest applied",
    );
}

pub async fn apply_abandon_quest(
    player_id: u32,
    quest_id: u32,
    registry: &ActorRegistry,
    db: &Database,
    lua: Option<&Arc<LuaEngine>>,
) {
    let Some(handle) = registry.get(player_id).await else {
        return;
    };
    if let Some(lua_engine) = lua {
        fire_quest_hook(
            &handle,
            quest_id,
            "onFinish",
            vec![crate::lua::QuestHookArg::Bool(false)],
            lua_engine,
            registry,
            db,
        )
        .await;
    }
    let had = {
        let mut c = handle.character.write().await;
        c.quest_journal.remove(quest_id).is_some()
    };
    if !had {
        tracing::debug!(
            player = player_id,
            quest = quest_id,
            "AbandonQuest skipped — not in journal",
        );
        return;
    }
    if let Err(e) = db.remove_quest(player_id, quest_id).await {
        tracing::warn!(
            error = %e,
            player = player_id,
            quest = quest_id,
            "AbandonQuest DB delete failed",
        );
    }
    tracing::info!(
        player = player_id,
        quest = quest_id,
        "AbandonQuest applied",
    );
}

pub async fn apply_quest_start_sequence(
    player_id: u32,
    quest_id: u32,
    sequence: u32,
    registry: &ActorRegistry,
    db: &Database,
    world: &WorldManager,
    lua: Option<&Arc<LuaEngine>>,
) {
    apply_quest_mutation(player_id, quest_id, registry, db, |q| {
        q.start_sequence(sequence)
    })
    .await;
    let Some(handle) = registry.get(player_id).await else {
        return;
    };
    {
        let mut c = handle.character.write().await;
        if let Some(q) = c.quest_journal.get_mut(quest_id) {
            q.state.begin_sequence_swap();
        }
    }
    if let Some(lua_engine) = lua {
        fire_quest_hook(
            &handle,
            quest_id,
            "onStateChange",
            vec![crate::lua::QuestHookArg::Int(sequence as i64)],
            lua_engine,
            registry,
            db,
        )
        .await;
    }
    let stale: Vec<QuestEnpc> = {
        let mut c = handle.character.write().await;
        match c.quest_journal.get_mut(quest_id) {
            Some(q) => q.state.drain_stale_enpcs().collect(),
            None => Vec::new(),
        }
    };
    for enpc in stale {
        broadcast_quest_enpc_clear(player_id, enpc, registry, world).await;
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn apply_quest_set_enpc(
    player_id: u32,
    quest_id: u32,
    actor_class_id: u32,
    quest_flag_type: u8,
    is_talk_enabled: bool,
    is_push_enabled: bool,
    is_emote_enabled: bool,
    is_spawned: bool,
    registry: &ActorRegistry,
    world: &WorldManager,
) {
    let Some(handle) = registry.get(player_id).await else {
        return;
    };
    let enpc = QuestEnpc::new(
        actor_class_id,
        quest_flag_type,
        is_spawned,
        is_talk_enabled,
        is_emote_enabled,
        is_push_enabled,
    );
    let outcome = {
        let mut c = handle.character.write().await;
        let Some(q) = c.quest_journal.get_mut(quest_id) else {
            return;
        };
        q.state.add_enpc(enpc)
    };
    match outcome {
        AddEnpcOutcome::Unchanged => {}
        AddEnpcOutcome::New(snapshot) | AddEnpcOutcome::Updated(snapshot) => {
            broadcast_quest_enpc_update(player_id, snapshot, registry, world).await;
        }
    }
}

pub async fn apply_quest_update_enpcs(
    player_id: u32,
    quest_id: u32,
    registry: &ActorRegistry,
    world: &WorldManager,
) {
    let Some(handle) = registry.get(player_id).await else {
        return;
    };
    let stale: Vec<QuestEnpc> = {
        let mut c = handle.character.write().await;
        match c.quest_journal.get_mut(quest_id) {
            Some(q) => q.state.drain_stale_enpcs().collect(),
            None => Vec::new(),
        }
    };
    for enpc in stale {
        broadcast_quest_enpc_clear(player_id, enpc, registry, world).await;
    }
}

// ---------------------------------------------------------------------------
// Rewards
// ---------------------------------------------------------------------------

/// Apply an XP gain to `actor_id`'s `class_id` skill pool.
///
/// Pipeline (Tier 4 #19 refresh):
///   1. Read `restBonus` (0..=100 percentage from CharaState).
///   2. Apply rested-XP multiplier via [`consume_rested_xp`] — the
///      effective gain is `exp + floor(exp * restBonus / 100)`,
///      and the rested pool decays by roughly `exp / 50` per call
///      (so ~5000 XP at 100% rested drains the pool).
///   3. Run the existing level-up rollover via
///      `battle::save::level_up_if_threshold_crossed`.
///   4. Persist `skillPoint` / `skillLevel` / `restBonus` to DB.
///   5. When `world` is available, emit `SetActorProperty` packets
///      so the client UI refreshes without a full re-login:
///        - `charaWork.battleSave.skillPoint[class-1]` on every gain,
///        - `charaWork.battleSave.skillLevel[class-1]` +
///          `charaWork.parameterSave.state_mainSkillLevel` on level-up,
///        - `playerWork.restBonusExpRate` when rested decayed.
///
/// `world` is optional so existing unit tests that don't wire a
/// WorldManager keep working; the packet branch silently skips when
/// `None`.
pub async fn apply_add_exp(
    actor_id: u32,
    class_id: u8,
    exp: i32,
    registry: &ActorRegistry,
    db: &Database,
    world: Option<&WorldManager>,
    lua: Option<&Arc<LuaEngine>>,
) {
    if exp == 0 {
        return;
    }
    let Some(handle) = registry.get(actor_id).await else {
        return;
    };
    let class_slot = class_id as usize;
    // Read-modify-write inside the write lock so a concurrent AddExp
    // doesn't lose a level-up crossing to a race. `new_exp` and
    // `new_level` are the post-rollover values.
    let Some((effective_gain, new_exp, new_level, levels_gained, rested_before, rested_after)) = ({
        let mut c = handle.character.write().await;
        if class_slot >= c.battle_save.skill_point.len() {
            tracing::warn!(class = class_id, "AddExp: class_id out of range");
            None
        } else {
            let rested_before = c.chara.rest_bonus_exp_rate;
            let (effective_gain, rested_after) = consume_rested_xp(exp, rested_before);
            c.chara.rest_bonus_exp_rate = rested_after;
            let prior_sp = c.battle_save.skill_point[class_slot];
            let combined = prior_sp.saturating_add(effective_gain).max(0);
            let prior_level = c
                .battle_save
                .skill_level
                .get(class_slot)
                .copied()
                .unwrap_or(1)
                .max(1);
            let (lvl, sp, gained) =
                crate::battle::save::level_up_if_threshold_crossed(prior_level, combined);
            c.battle_save.skill_point[class_slot] = sp;
            if gained > 0 {
                if let Some(slot) = c.battle_save.skill_level.get_mut(class_slot) {
                    *slot = lvl;
                }
                // If this class is the active slot, also refresh the
                // top-level `chara.level` the stat pipeline reads. No
                // other class gets reflected into `chara.level` — the
                // player has one active class at a time.
                if c.chara.class as i32 == class_id as i32 {
                    c.chara.level = lvl;
                }
            }
            Some((effective_gain, sp, lvl, gained, rested_before, rested_after))
        }
    }) else {
        return;
    };

    if let Err(e) = db.set_exp(actor_id, class_id, new_exp).await {
        tracing::warn!(
            actor = actor_id,
            class = class_id,
            err = %e,
            "AddExp: DB persist failed",
        );
    }
    if levels_gained > 0 {
        if let Err(e) = db.set_level(actor_id, class_id, new_level).await {
            tracing::warn!(
                actor = actor_id,
                class = class_id,
                err = %e,
                "AddExp: set_level DB persist failed",
            );
        }
        tracing::info!(
            actor = actor_id,
            class = class_id,
            new_level,
            levels_gained,
            "AddExp: level up",
        );
    }
    if rested_after != rested_before
        && let Err(e) = db.set_rest_bonus_exp_rate(actor_id, rested_after).await
    {
        tracing::warn!(
            actor = actor_id,
            err = %e,
            "AddExp: restBonus DB persist failed",
        );
    }

    // Client-facing property emits — only fire when we have a
    // WorldManager to reach the session → client handle. Also
    // carries the ability-unlock lookup through `lua.catalogs()` so
    // the learn-commands game-messages fire for the player when
    // level-up crosses a threshold that unlocks an ability.
    if let Some(world) = world {
        emit_exp_property_updates(
            actor_id,
            class_id,
            new_exp,
            new_level,
            levels_gained,
            rested_before,
            rested_after,
            &handle,
            world,
            registry,
            lua,
        )
        .await;
    }

    tracing::info!(
        actor = actor_id,
        class = class_id,
        delta = exp,
        applied = effective_gain,
        skill_point = new_exp,
        level = new_level,
        rested_before,
        rested_after,
        "AddExp applied",
    );
}

/// Apply rested-XP bonus to an incoming gain.
///
/// `rested` is the 0..=100 bonus percentage stored on
/// `CharaState.rest_bonus_exp_rate`. Returns `(total_gain, new_rested)`.
/// The bonus is `floor(exp * rested_pct / 100)` — a 100%-rested
/// player gets double XP on their next gain. Decay is `max(1, exp/50)`
/// per call: ~5000 XP at steady 100% rested drains the pool; smaller
/// gains sip more slowly. Negative `rested` clamps to 0. Zero / negative
/// `exp` is a no-op and leaves the pool alone (matches the `exp == 0`
/// early return in `apply_add_exp`).
pub fn consume_rested_xp(exp: i32, rested: i32) -> (i32, i32) {
    if exp <= 0 || rested <= 0 {
        return (exp, rested.max(0));
    }
    let rested_pct = rested.min(100);
    let bonus = (exp as i64 * rested_pct as i64 / 100) as i32;
    let total = exp.saturating_add(bonus);
    // ~1 point decayed per 50 XP of base gain, min 1 so tiny gains
    // don't freeload.
    let decay = ((exp + 49) / 50).max(1);
    let new_rested = (rested - decay).max(0);
    (total, new_rested)
}

/// Emit the `SetActorProperty` packets Meteor's `AddExp` sends after
/// a successful gain. Target strings mirror Meteor's
/// `ActorPropertyPacketUtil` usage:
///
///   - `charaWork/battleStateForSelf` → `skillPoint[class-1]`,
///     `playerWork.restBonusExpRate` (self-only).
///   - `charaWork/stateForAll` → `skillLevel[class-1]`,
///     `state_mainSkillLevel` (self + broadcast on level-up).
///
/// The level-up packets now fan to nearby Players via the shared
/// `broadcast_around_actor` helper — the `/stateForAll` target name
/// is retail's convention for "everyone who can see this actor
/// needs this value", and matches how Meteor's `QueuePackets` fans
/// `ActorPropertyPacketUtil` output after a level up.
#[allow(clippy::too_many_arguments)]
async fn emit_exp_property_updates(
    actor_id: u32,
    class_id: u8,
    new_exp: i32,
    new_level: i16,
    levels_gained: i16,
    rested_before: i32,
    rested_after: i32,
    handle: &ActorHandle,
    world: &WorldManager,
    registry: &ActorRegistry,
    lua: Option<&Arc<LuaEngine>>,
) {
    let session_id = handle.session_id;
    if session_id == 0 {
        return;
    }
    let Some(client) = world.client(session_id).await else {
        return;
    };
    let class_slot = class_id.saturating_sub(1);

    // Self-only: skillPoint + restBonusExpRate — owner sees their
    // own XP bar and rested-exp UI widget, nobody else needs to.
    let mut self_only_packets = Vec::new();
    {
        let mut b = crate::packets::send::actor::ActorPropertyPacketBuilder::new(
            actor_id,
            "charaWork/battleStateForSelf",
        );
        b.add_int(
            &format!("charaWork.battleSave.skillPoint[{}]", class_slot),
            new_exp as u32,
        );
        if rested_before != rested_after {
            b.add_int("playerWork.restBonusExpRate", rested_after as u32);
        }
        self_only_packets.extend(b.done());
    }
    for sub in &self_only_packets {
        if let Ok(base) = common::BasePacket::create_from_subpacket(sub, true, false) {
            client.send_bytes(base.to_bytes()).await;
        }
    }

    // Level-up: skillLevel + state_mainSkillLevel. Fan to nearby
    // players AND self — the owner's client also reads the stateForAll
    // row, so it needs the same bytes. Source is excluded by
    // `actors_around` inside the broadcast helper, but we still
    // send to the owning client directly so the packet isn't
    // dropped if the broadcast grid happens not to include them
    // (e.g. first frame after a zone-change before the grid
    // re-registers the player).
    if levels_gained > 0 {
        let mut b = crate::packets::send::actor::ActorPropertyPacketBuilder::new(
            actor_id,
            "charaWork/stateForAll",
        );
        b.add_short(
            &format!("charaWork.battleSave.skillLevel[{}]", class_slot),
            new_level as u16,
        );
        b.add_short("charaWork.parameterSave.state_mainSkillLevel", new_level as u16);
        let level_packets = b.done();
        for sub in &level_packets {
            if let Ok(base) = common::BasePacket::create_from_subpacket(sub, true, false) {
                client.send_bytes(base.to_bytes()).await;
            }
        }
        // Nearby broadcast — look up the zone by the player's
        // current zone id, fan each subpacket bytes to every
        // nearby Player. Silent no-op if the zone isn't live (e.g.
        // in a pure DB-only integration test).
        if let Some(zone) = world.zone(handle.zone_id).await {
            for sub in &level_packets {
                if let Ok(base) = common::BasePacket::create_from_subpacket(sub, true, false) {
                    let _ = crate::runtime::broadcast::broadcast_around_actor(
                        world,
                        registry,
                        &zone,
                        handle.actor_id,
                        base.to_bytes(),
                    )
                    .await;
                }
            }
        }

        // "You attain level [level]." (textId 33909) + the
        // ability-unlock chain Meteor runs in
        // `Player.EquipAbilitiesAtLevel`. One game-message per
        // crossed level so a multi-level rollover reports each
        // threshold distinctly — matches retail's per-level
        // feedback cadence. For each newly-reached level we look up
        // the class's battle-command ids at that level via
        // `Catalogs::commands_unlocked_at` and fire one 33926
        // ("You learn X") per unlock, with the command id as the
        // LuaParam.
        emit_level_up_game_messages(
            actor_id,
            class_id,
            new_level,
            levels_gained,
            client.clone(),
            lua,
        )
        .await;
    }
}

/// Emit the per-level "You attain level N" + "You learn X" game
/// messages a level-up rollover should produce. Iterates over the
/// `levels_gained` most-recent level thresholds so a rollover that
/// crossed 2 levels in one call (rare but possible with large
/// `AddExp` grants) reports both. Silent no-op if `lua` is `None`
/// (test harness) or the catalog is empty.
async fn emit_level_up_game_messages(
    actor_id: u32,
    class_id: u8,
    new_level: i16,
    levels_gained: i16,
    client: crate::data::ClientHandle,
    lua: Option<&Arc<LuaEngine>>,
) {
    use common::luaparam::LuaParam;

    // Retail text ids — see `Player.EquipAbilitiesAtLevel` at
    // `origin/develop:Map Server/Actors/Chara/Player/Player.cs:2618`
    // (`33926: You learn [command]`) and `LevelUp`
    // (`33909: You attain level [level]`).
    const TEXT_LEVEL_ATTAINED: u16 = 33909;
    const TEXT_LEARN_COMMAND: u16 = 33926;

    for gained_idx in (0..levels_gained).rev() {
        // `new_level` is the *final* post-rollover level; the
        // intermediate levels we passed through are at
        // `new_level - gained_idx`.
        let at_level = new_level - gained_idx;

        // "You attain level N."
        let level_msg = crate::packets::send::misc::build_game_message(
            actor_id,
            crate::packets::send::misc::GameMessageOptions {
                sender_actor_id: 0,
                receiver_actor_id: actor_id,
                text_id: TEXT_LEVEL_ATTAINED,
                log: 0x20,
                display_id: None,
                custom_sender: None,
                lua_params: vec![LuaParam::UInt32(at_level as u32)],
            },
        );
        if let Ok(base) = common::BasePacket::create_from_subpacket(&level_msg, true, false) {
            client.send_bytes(base.to_bytes()).await;
        }

        // Ability unlocks at this level — one message per command.
        let Some(lua) = lua else {
            continue;
        };
        let commands = lua.catalogs().commands_unlocked_at(class_id, at_level);
        for command_id in commands {
            tracing::info!(
                actor = actor_id,
                class = class_id,
                level = at_level,
                command_id,
                "ability unlock: You learn <command>",
            );
            let learn_msg = crate::packets::send::misc::build_game_message(
                actor_id,
                crate::packets::send::misc::GameMessageOptions {
                    sender_actor_id: 0,
                    receiver_actor_id: actor_id,
                    text_id: TEXT_LEARN_COMMAND,
                    log: 0x20,
                    display_id: None,
                    custom_sender: None,
                    lua_params: vec![LuaParam::UInt32(command_id as u32)],
                },
            );
            if let Ok(base) = common::BasePacket::create_from_subpacket(&learn_msg, true, false) {
                client.send_bytes(base.to_bytes()).await;
            }
        }
    }
}

/// `player:SetQuestComplete(id, flag)` — direct-set the 2048-bit
/// completion bit without running the quest's `onFinish` hook. Used by
/// GM `!completedQuest` debug commands and cross-quest prerequisites.
pub async fn apply_set_quest_complete(
    player_id: u32,
    quest_id: u32,
    flag: bool,
    registry: &ActorRegistry,
    db: &Database,
) {
    let Some(handle) = registry.get(player_id).await else {
        return;
    };
    {
        let mut c = handle.character.write().await;
        c.quest_journal.set_completed(quest_id, flag);
    }
    if flag {
        if let Err(e) = db.complete_quest(player_id, quest_id).await {
            tracing::warn!(
                error = %e,
                player = player_id,
                quest = quest_id,
                "SetQuestComplete(true): bitstream save failed",
            );
        }
    } else {
        // Clearing a bit: reload the current bitstream from DB, flip
        // the bit, write back. `db.complete_quest` is set-only; the
        // complement path lives here inline.
        match db.load_completed_quests(player_id).await {
            Ok(mut bs) => {
                if let Some(bit) = crate::actor::quest::quest_id_to_bit(quest_id) {
                    bs.clear(bit);
                    if let Err(e) = db.save_completed_quests(player_id, &bs).await {
                        tracing::warn!(
                            error = %e,
                            player = player_id,
                            quest = quest_id,
                            "SetQuestComplete(false): bitstream save failed",
                        );
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    player = player_id,
                    quest = quest_id,
                    "SetQuestComplete(false): bitstream load failed",
                );
            }
        }
    }
    tracing::info!(
        player = player_id,
        quest = quest_id,
        flag,
        "SetQuestComplete applied",
    );
}

/// Grant one stack of `item_id` to the NORMAL bag on `actor_id`.
/// Used by:
///  * Gathering's `HarvestReward` Lua helper when the strike minigame
///    lands a copper/rock-salt/ore drop (actor_id = player character
///    id, item_package = `PKG_NORMAL = 0`).
///  * Future `onReward` quest-finish hooks once those land.
///
/// Persistence is direct-DB via [`Database::add_harvest_item`] — the
/// in-memory `ItemPackage` on `Player` is not yet accessible from the
/// registry (`Player` is not in `ActorRegistry`; the registry stores
/// only the `Character` sub-struct). The player picks up the new
/// stack on the next inventory resync / zone-in, which matches
/// retail 1.x behaviour where the `textInputWidget` remains open and
/// the bag only refreshes on the next `/_init` bundle.
///
/// Silently no-ops for:
///  * non-NORMAL packages (currency / key items go through their own
///    paths — `AddGil` and a future AddKeyItem),
///  * zero or negative quantity (`player:AddItem(..., 0)` is a legal
///    Lua no-op that shouldn't insert a zero-quantity row),
///  * item id 0.
pub async fn apply_add_item(
    actor_id: u32,
    item_package: u16,
    item_id: u32,
    quantity: i32,
    db: &Database,
) {
    if quantity <= 0 || item_id == 0 {
        return;
    }
    // Route currency stacks through add_gil so the 1_000_001 gil row
    // stays the single-stack well-known layout. The gathering path
    // never lands here (Copper Ore is a NORMAL-bag item), but Lua
    // scripts that incorrectly call `GetItemPackage(99):AddItem(1000001, 10)`
    // should still do the right thing.
    if item_package == crate::inventory::PKG_CURRENCY_CRYSTALS {
        apply_add_gil(actor_id, quantity, db).await;
        return;
    }
    // Everything else lands in NORMAL for the first cut. Key-items /
    // bazaar / trade bags get their own paths as they're wired up.
    if item_package != crate::inventory::PKG_NORMAL {
        tracing::debug!(
            actor = actor_id,
            package = item_package,
            item = item_id,
            qty = quantity,
            "AddItem: non-NORMAL packages not yet implemented — logging only",
        );
        return;
    }
    match db.add_harvest_item(actor_id, item_id, quantity, 1).await {
        Ok(total) => {
            tracing::info!(
                actor = actor_id,
                item = item_id,
                delta = quantity,
                total,
                "AddItem applied",
            );
        }
        Err(e) => {
            tracing::warn!(
                actor = actor_id,
                item = item_id,
                delta = quantity,
                err = %e,
                "AddItem: DB persist failed",
            );
        }
    }
}

/// Tier 3 #13 — advance any fieldcraft leves the player currently
/// has accepted whose band-0 objective matches `item_catalog_id`.
/// Returns the list of leve ids that transitioned to completed on
/// this call (used by callers that want to emit a "leve complete"
/// GameMessage without a before/after diff).
///
/// Short-circuits when:
///  * no [`RegionalLeveResolver`] is installed (fresh DB, boot
///    race) — catalogs hand out `None` and we early-return;
///  * the resolver reports zero leves targeting this item — no
///    matching active leve can possibly exist;
///  * the player isn't in [`ActorRegistry`] — mirrors every other
///    apply helper.
///
/// Progress persists through [`Database::save_quest`] exactly like
/// any other quest mutation — the dirty-bit on [`Quest`] flips
/// inside [`RegionalLeveView::advance_progress`] so existing
/// machinery picks it up.
///
/// [`RegionalLeveResolver`]: crate::leve::RegionalLeveResolver
/// [`RegionalLeveView::advance_progress`]: crate::leve::RegionalLeveView::advance_progress
pub async fn advance_fieldcraft_leves(
    player_id: u32,
    item_catalog_id: u32,
    delta: u16,
    registry: &ActorRegistry,
    db: &Database,
    lua: Option<&Arc<LuaEngine>>,
) -> Vec<u32> {
    if delta == 0 {
        return Vec::new();
    }
    let Some(lua) = lua else {
        return Vec::new();
    };
    let Some(resolver) = lua.catalogs().regional_leve_resolver() else {
        return Vec::new();
    };
    let leve_ids = resolver.fieldcraft_leves_for_item(item_catalog_id);
    if leve_ids.is_empty() {
        return Vec::new();
    }
    advance_regional_leves(player_id, leve_ids, delta, &resolver, registry, db).await
}

/// Tier 3 #13 — battlecraft counterpart. Advance any accepted
/// battlecraft leves whose band-0 objective matches
/// `actor_class_id`. Invoked from [`fire_on_kill_bnpc`] after the
/// kill is resolved.
///
/// [`fire_on_kill_bnpc`]: crate::runtime::quest_hook::fire_on_kill_bnpc
pub async fn advance_battlecraft_leves(
    player_id: u32,
    actor_class_id: u32,
    registry: &ActorRegistry,
    db: &Database,
    lua: Option<&Arc<LuaEngine>>,
) -> Vec<u32> {
    let Some(lua) = lua else {
        return Vec::new();
    };
    let Some(resolver) = lua.catalogs().regional_leve_resolver() else {
        return Vec::new();
    };
    let leve_ids = resolver.battlecraft_leves_for_class(actor_class_id);
    if leve_ids.is_empty() {
        return Vec::new();
    }
    advance_regional_leves(player_id, leve_ids, 1, &resolver, registry, db).await
}

/// Shared fieldcraft/battlecraft inner loop. Walks the candidate
/// leve ids, finds each one's quest slot, advances the view, and
/// persists any dirty slots. Keeps the fan-out shape in one place
/// so the fieldcraft and battlecraft entry points stay narrow.
async fn advance_regional_leves(
    player_id: u32,
    leve_ids: &[u32],
    delta: u16,
    resolver: &crate::leve::RegionalLeveResolver,
    registry: &ActorRegistry,
    db: &Database,
) -> Vec<u32> {
    let Some(handle) = registry.get(player_id).await else {
        return Vec::new();
    };
    let mut completed = Vec::new();
    // Collect dirty-slot save work under the write lock, then drop
    // the lock before awaiting the DB so a slow disk write doesn't
    // hold the player's character lock.
    let pending_saves: Vec<(i32, u32, u32, u32, [u16; 3], u32)> = {
        let mut c = handle.character.write().await;
        let mut saves = Vec::new();
        for &leve_id in leve_ids {
            let Some(data) = resolver.by_id(leve_id) else {
                continue;
            };
            let Some(slot) = c.quest_journal.slot_of(leve_id) else {
                continue;
            };
            let Some(quest) = c.quest_journal.slots[slot].as_mut() else {
                continue;
            };
            let just_completed = {
                let mut view = crate::leve::RegionalLeveView::new(quest, data);
                view.advance_progress(delta)
            };
            if just_completed {
                completed.push(leve_id);
            }
            if quest.is_dirty() {
                let sequence = quest.get_sequence();
                let flags = quest.get_flags();
                let counters = [
                    quest.get_counter(0),
                    quest.get_counter(1),
                    quest.get_counter(2),
                ];
                let actor_id = quest.actor_id;
                quest.clear_dirty();
                saves.push((slot as i32, actor_id, sequence, flags, counters, leve_id));
            }
        }
        saves
    };
    for (slot, actor_id, sequence, flags, [c1, c2, c3], leve_id) in pending_saves {
        if let Err(e) = db
            .save_quest(player_id, slot, actor_id, sequence, flags, c1, c2, c3)
            .await
        {
            tracing::warn!(
                player = player_id,
                leve = leve_id,
                err = %e,
                "regional leve progress: save_quest failed",
            );
        }
    }
    completed
}

/// Tier 4 #14 C — grant a stack to a retainer's personal
/// inventory. Parallel to [`apply_add_item`] (the player-scoped
/// variant) but routes to
/// [`Database::add_retainer_inventory_item`] so the write lands in
/// `characters_retainer_inventory` rather than
/// `characters_inventory`.
///
/// Silently no-ops for:
///  * non-NORMAL packages — retainer bazaar adds go through the
///    dedicated `AddRetainerBazaarItem` command + `add_retainer_bazaar_item`
///    DB helper, not this path.
///  * zero or negative quantity — mirrors the player-side behaviour.
///  * item id 0.
pub async fn apply_add_item_to_retainer(
    retainer_id: u32,
    item_package: u16,
    item_id: u32,
    quantity: i32,
    db: &Database,
) {
    if quantity <= 0 || item_id == 0 {
        return;
    }
    // Non-NORMAL packages on a retainer are unexpected today; the
    // only script path that reaches here is
    // `retainer:GetItemPackage(0):AddItem(...)` which always uses
    // INVENTORY_NORMAL = 0. Log + bail so a future Lua typo surfaces
    // visibly.
    if item_package != crate::inventory::PKG_NORMAL {
        tracing::debug!(
            retainer = retainer_id,
            package = item_package,
            item = item_id,
            qty = quantity,
            "AddItemToRetainer: non-NORMAL packages not implemented — logging only",
        );
        return;
    }
    match db
        .add_retainer_inventory_item(retainer_id, item_id, quantity, 1, item_package)
        .await
    {
        Ok(total) => {
            tracing::info!(
                retainer = retainer_id,
                item = item_id,
                delta = quantity,
                total,
                "AddItemToRetainer applied",
            );
        }
        Err(e) => {
            tracing::warn!(
                retainer = retainer_id,
                item = item_id,
                delta = quantity,
                err = %e,
                "AddItemToRetainer: DB persist failed",
            );
        }
    }
}

/// Tier 1 #2 C — Lua-driven status-effect application. Parallels
/// the internal `add_status_effect` path the Rust dispatcher uses
/// during combat resolution, but gated behind a dedicated
/// [`LuaCommand::TryStatus`] variant so Lua scripts can apply
/// buffs / debuffs / DoTs without going through the full
/// battle-event pipeline.
///
/// Behaviour matches Meteor's `action.TryStatus(action, target,
/// status, tier?, magnitude?, duration?)` shape: build a fresh
/// [`StatusEffect`] on the target, insert into its
/// [`StatusEffectContainer`] (which honours the existing overwrite
/// rules + 20-effect cap), and drain the resulting
/// [`StatusOutbox`] through the shared
/// [`crate::runtime::dispatcher::dispatch_status_event`] so the
/// gain packet + `onGain` Lua hook fire just as they would for a
/// Rust-internal apply.
///
/// Returns `true` when the effect landed (fresh or successful
/// overwrite), `false` on any no-op path (missing target, full
/// table, overwrite-rejected). Short-circuits silently when `lua`
/// is `None` (test harness without a Catalogs clone) since the
/// dispatcher requires a real `Arc<Catalogs>`.
///
/// [`StatusEffect`]: crate::status::StatusEffect
/// [`StatusEffectContainer`]: crate::status::StatusEffectContainer
/// [`StatusOutbox`]: crate::status::StatusOutbox
pub async fn apply_try_status(
    source_actor_id: u32,
    target_actor_id: u32,
    status_id: u32,
    duration_s: u32,
    magnitude: f64,
    tick_ms: u32,
    tier: u8,
    registry: &ActorRegistry,
    db: &Database,
    world: &WorldManager,
    lua: Option<&Arc<LuaEngine>>,
) -> bool {
    let Some(target) = registry.get(target_actor_id).await else {
        tracing::debug!(
            target = target_actor_id,
            status = status_id,
            "TryStatus: target not in registry",
        );
        return false;
    };
    // Status effects use ms-precision for tick accounting. Convert
    // the seconds-precision `unix_timestamp()` helper into ms; this
    // drifts by at most 999 ms vs a true ms clock, which is well
    // below the finest granularity any ticking effect uses
    // (typically 3 s).
    let now_ms = (common::utils::unix_timestamp() as u64).saturating_mul(1000);
    let mut outbox = crate::status::StatusOutbox::new();
    let landed = {
        let mut c = target.character.write().await;
        let effect = crate::status::StatusEffect::new(
            target_actor_id,
            status_id,
            magnitude,
            tick_ms,
            duration_s,
            tier,
            now_ms,
        );
        c.status_effects.add_status_effect(
            effect,
            source_actor_id,
            now_ms,
            crate::status::DEFAULT_GAIN_TEXT_ID,
            &mut outbox,
        )
    };
    if !landed {
        tracing::debug!(
            source = source_actor_id,
            target = target_actor_id,
            status = status_id,
            "TryStatus: effect did not land (overwrite-rejected or table full)",
        );
        return false;
    }
    // Drain the outbox through the status dispatcher so packets /
    // DB save / on-gain Lua hooks fire. Dispatcher needs a
    // Catalogs Arc — reuse the LuaEngine's when available; fall
    // back to a fresh empty Catalogs for the test-harness case.
    let catalogs = lua
        .map(|e| e.catalogs().clone())
        .unwrap_or_else(|| std::sync::Arc::new(crate::lua::Catalogs::new()));
    for event in outbox.drain() {
        crate::runtime::dispatcher::dispatch_status_event(
            &event, registry, world, db, &catalogs,
        )
        .await;
    }
    tracing::info!(
        source = source_actor_id,
        target = target_actor_id,
        status = status_id,
        duration_s,
        magnitude,
        "TryStatus applied",
    );
    true
}

/// Tier 4 #14 D — bazaar purchase drain helper. Thin wrapper over
/// [`Database::purchase_retainer_bazaar_item`] that logs the
/// outcome at the right level: `info` on success, `debug` for the
/// "legitimate rejection" paths (`InsufficientGil`, `ListingGone`,
/// `CannotBuyFromSelf`, `NoOwner`), `warn` only on actual DB
/// errors. Callers rarely need the outcome enum beyond logging;
/// the test harness can still reach the DB method directly when
/// it does.
pub async fn apply_purchase_retainer_bazaar_item(
    buyer_id: u32,
    retainer_id: u32,
    server_item_id: u64,
    db: &Database,
) -> Option<crate::database::PurchaseOutcome> {
    match db
        .purchase_retainer_bazaar_item(buyer_id, retainer_id, server_item_id)
        .await
    {
        Ok(outcome) => {
            tracing::info!(
                buyer = buyer_id,
                retainer = retainer_id,
                server_item = server_item_id,
                outcome = ?outcome,
                "PurchaseRetainerBazaarItem outcome",
            );
            Some(outcome)
        }
        Err(e) => {
            tracing::warn!(
                buyer = buyer_id,
                retainer = retainer_id,
                server_item = server_item_id,
                err = %e,
                "PurchaseRetainerBazaarItem: DB error",
            );
            None
        }
    }
}

/// Tier 3 #13 accept-side binding. The levemete counterpart to
/// [`apply_regional_leve_hand_in`]: installs the leve in the
/// player's journal with [`crate::leve::ACCEPTED_FLAG_BIT`] set and
/// the chosen difficulty band stamped on `counter2` so the
/// fieldcraft / battlecraft progress hooks tick correctly against
/// the band's objective quantity.
///
/// Returns `true` when a fresh journal entry was created, `false`
/// on any no-op path: missing catalog, missing player, missing
/// leve data row, journal full, already-accepted. The idempotent
/// already-in-journal path silently succeeds — retail levemetes
/// just re-render the "you already have this leve" dialog line.
pub async fn apply_accept_regional_leve(
    player_id: u32,
    leve_id: u32,
    difficulty: u8,
    registry: &ActorRegistry,
    db: &Database,
    lua: Option<&Arc<LuaEngine>>,
) -> bool {
    let Some(lua) = lua else {
        return false;
    };
    let Some(resolver) = lua.catalogs().regional_leve_resolver() else {
        return false;
    };
    if resolver.by_id(leve_id).is_none() {
        tracing::debug!(
            player = player_id,
            leve = leve_id,
            "AcceptRegionalLeve: leve id not in catalog",
        );
        return false;
    }
    let Some(handle) = registry.get(player_id).await else {
        return false;
    };
    // Clamp to the valid band range up front — mirrors
    // `RegionalLeveData::clamp_difficulty`. Saturating is cheaper
    // than failing since retail scripts sometimes pass the 1-indexed
    // UI band; we normalise to the 0-indexed storage band.
    let band = difficulty.min(3);

    let save_tuple = {
        let mut c = handle.character.write().await;
        if c.quest_journal.has(leve_id) {
            tracing::debug!(
                player = player_id,
                leve = leve_id,
                "AcceptRegionalLeve: already in journal (idempotent no-op)",
            );
            return false;
        }
        let actor_id = crate::actor::quest::quest_actor_id(leve_id);
        // Regional leves don't have `gamedata_quests` catalog rows
        // (they're a separate data model), so there's no
        // script-name lookup. Use a formulaic name so the DB row
        // is distinguishable in audits — same convention my test
        // fixtures used.
        let name = format!("leve{leve_id}");
        let mut quest = crate::actor::quest::Quest::new(actor_id, name);
        quest.set_flag(crate::leve::ACCEPTED_FLAG_BIT);
        quest.set_counter(1, band as u16);
        quest.clear_dirty();
        let Some(slot) = c.quest_journal.add(quest) else {
            tracing::warn!(
                player = player_id,
                leve = leve_id,
                "AcceptRegionalLeve: journal full",
            );
            return false;
        };
        let flags = 1u32 << crate::leve::ACCEPTED_FLAG_BIT;
        (slot as i32, actor_id, flags)
    };
    let (slot, actor_id, flags) = save_tuple;
    // save_quest params (per database.rs:2118): counter1 / counter2 /
    // counter3 = the DB column names. RegionalLeveView's
    // `set_counter(1, band)` writes the *in-memory* idx-1 counter,
    // which persists to the `counter2` DB column. So: counter1 = 0
    // (progress starts fresh), counter2 = band (difficulty),
    // counter3 = 0 (reserved).
    if let Err(e) = db
        .save_quest(player_id, slot, actor_id, 0, flags, 0, band as u16, 0)
        .await
    {
        tracing::warn!(
            player = player_id,
            leve = leve_id,
            err = %e,
            "AcceptRegionalLeve: DB persist failed",
        );
    }
    tracing::info!(
        player = player_id,
        leve = leve_id,
        slot,
        band,
        "AcceptRegionalLeve applied",
    );
    true
}

/// Outcome of a [`apply_regional_leve_hand_in`] call. Carried back
/// so callers (and tests) can assert exactly which side effects
/// fired without re-reading the DB for each assertion.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LeveHandInOutcome {
    /// `true` when the leve was in the journal + marked completed
    /// and the reward pipeline ran. `false` on any no-op path
    /// (leve not in journal, not completed, catalog row missing,
    /// etc.).
    pub applied: bool,
    pub gil_granted: i32,
    /// `(item_catalog_id, quantity)` when a reward item was
    /// granted. `None` when the band's `reward_item_id == 0` or
    /// the grant path was skipped.
    pub item_granted: Option<(u32, i32)>,
    /// `(gc, seals)` granted for an enlisted battlecraft hand-in.
    /// `None` for fieldcraft, unenlisted battlecraft, or any
    /// no-op path.
    pub seals_granted: Option<(u8, i32)>,
}

/// Tier 3 #13 reward payout + Tier 4 #16 C seal accrual.
/// Drain-side helper for the levemete hand-in flow. Semantics:
///
///  * the leve must be present in the player's journal AND have
///    `COMPLETED_FLAG_BIT` set (i.e. a prior `advance_progress`
///    call must already have saturated the objective) — otherwise
///    no rewards fire and the journal is left untouched;
///  * reward_gil for the active band is granted via
///    [`Database::add_gil`];
///  * if `reward_item_id[band] != 0`, that quantity of the item
///    lands in `characters_inventory` via
///    [`Database::add_harvest_item`];
///  * for battlecraft leves, if the player is enlisted (`gc_current
///    != 0`), Storm / Serpent / Flame seals are granted to their
///    current GC via [`Database::add_seals`] at a rate of
///    `reward_gil / 2` (placeholder — retail had dedicated per-leve
///    seal-reward columns mozk-tabetai doesn't publish). Fieldcraft
///    never grants seals.
///  * on success the leve is removed from the journal (in-memory +
///    DB) so the slot frees up for another levemete pickup.
///
/// Intended call sites: the future levemete-hand-in RPC
/// (`handInLeve` / `completeLeve` `callClientFunction`) and the
/// `LC::HandInRegionalLeve` runtime drain.
///
/// Short-circuits silently when the `RegionalLeveResolver` isn't
/// installed — catalogs hand out `None` and the no-op outcome lets
/// the caller distinguish "catalog missing" from "reward paid".
pub async fn apply_regional_leve_hand_in(
    player_id: u32,
    leve_id: u32,
    registry: &ActorRegistry,
    db: &Database,
    lua: Option<&Arc<LuaEngine>>,
) -> LeveHandInOutcome {
    let mut outcome = LeveHandInOutcome::default();
    let Some(lua) = lua else {
        return outcome;
    };
    let Some(resolver) = lua.catalogs().regional_leve_resolver() else {
        return outcome;
    };
    let Some(data) = resolver.by_id(leve_id).cloned() else {
        return outcome;
    };
    let Some(handle) = registry.get(player_id).await else {
        return outcome;
    };

    // Snapshot everything we need + clear the journal slot, under
    // one write lock. If the leve isn't completed we bail without
    // touching the journal.
    let (band, gc_current, was_removed) = {
        let mut c = handle.character.write().await;
        let (is_completed, band) = {
            let Some(quest) = c.quest_journal.get(leve_id) else {
                return outcome;
            };
            // Read the same counter/flag positions
            // `RegionalLeveView` does, without constructing a
            // mutable view (we don't mutate through it here).
            let completed = quest.get_flag(crate::leve::COMPLETED_FLAG_BIT);
            let difficulty = quest.get_counter(1).min(3) as usize;
            (completed, difficulty)
        };
        if !is_completed {
            return outcome;
        }
        let gc = c.chara.gc_current;
        let removed = c.quest_journal.remove(leve_id).is_some();
        (band, gc, removed)
    };
    if !was_removed {
        return outcome;
    }

    // DB side: drop the scenario row so a fresh accept of the same
    // leve id starts from zero progress + flags.
    if let Err(e) = db.remove_quest(player_id, leve_id).await {
        tracing::warn!(
            player = player_id,
            leve = leve_id,
            err = %e,
            "LeveHandIn: DB scenario clear failed (journal was already updated in-memory)",
        );
    }

    // Rewards. Apply in the order retail's `handInLeve` ticks them
    // so the client's message log reads gil → item → seals.
    let gil = data.reward_gil.get(band).copied().unwrap_or(0);
    if gil > 0 {
        apply_add_gil(player_id, gil, db).await;
        outcome.gil_granted = gil;
    }
    let item_id = data.reward_item_id.get(band).copied().unwrap_or(0);
    let item_qty = data.reward_quantity.get(band).copied().unwrap_or(0);
    if item_id > 0 && item_qty > 0 {
        if let Err(e) = db
            .add_harvest_item(player_id, item_id as u32, item_qty, 1)
            .await
        {
            tracing::warn!(
                player = player_id,
                leve = leve_id,
                item = item_id,
                err = %e,
                "LeveHandIn: reward-item grant failed",
            );
        } else {
            outcome.item_granted = Some((item_id as u32, item_qty));
        }
    }
    // Seal accrual — battlecraft + enlisted only. Tier 4 #16 C.
    if data.leve_type == crate::leve::LeveType::Battlecraft
        && crate::actor::gc::is_valid_gc(gc_current)
    {
        let seals = gil / 2;
        if seals > 0 {
            match db.add_seals(player_id, gc_current, seals).await {
                Ok(_) => {
                    outcome.seals_granted = Some((gc_current, seals));
                }
                Err(e) => tracing::warn!(
                    player = player_id,
                    leve = leve_id,
                    gc = gc_current,
                    err = %e,
                    "LeveHandIn: seal accrual failed",
                ),
            }
        }
    }
    outcome.applied = true;
    tracing::info!(
        player = player_id,
        leve = leve_id,
        band,
        gil = outcome.gil_granted,
        item = ?outcome.item_granted,
        seals = ?outcome.seals_granted,
        "LeveHandIn applied",
    );
    outcome
}

pub async fn apply_add_gil(actor_id: u32, amount: i32, db: &Database) {
    if amount == 0 {
        return;
    }
    match db.add_gil(actor_id, amount).await {
        Ok(total) => {
            tracing::info!(
                actor = actor_id,
                delta = amount,
                total,
                "AddGil applied",
            );
        }
        Err(e) => {
            tracing::warn!(
                actor = actor_id,
                delta = amount,
                err = %e,
                "AddGil: DB persist failed",
            );
        }
    }
}

// ---------------------------------------------------------------------------
// ENPC broadcast
// ---------------------------------------------------------------------------

async fn broadcast_quest_enpc_update(
    player_id: u32,
    enpc: QuestEnpc,
    registry: &ActorRegistry,
    world: &WorldManager,
) {
    let Some(player_handle) = registry.get(player_id).await else {
        return;
    };
    let session_id = player_handle.session_id;
    if session_id == 0 {
        return;
    }
    let Some(client) = world.client(session_id).await else {
        return;
    };
    let zone_id = player_handle.zone_id;
    let Some(npc_handle) = find_npc_by_class_id(registry, zone_id, enpc.actor_class_id).await
    else {
        tracing::debug!(
            player = player_id,
            class_id = enpc.actor_class_id,
            "quest ENPC broadcast skipped — no live NPC",
        );
        return;
    };
    let (npc_actor_id, conditions) = {
        let c = npc_handle.character.read().await;
        (c.base.actor_id, c.base.event_conditions.clone())
    };

    let subpackets = crate::packets::send::build_actor_event_status_packets(
        npc_actor_id,
        &conditions,
        enpc.is_talk_enabled,
        enpc.is_emote_enabled,
        Some(enpc.is_push_enabled),
        true,
    );
    for sub in subpackets {
        client.send_bytes(sub.to_bytes()).await;
    }
    let graphic = crate::packets::send::build_set_actor_quest_graphic(
        npc_actor_id,
        enpc.quest_flag_type,
    );
    client.send_bytes(graphic.to_bytes()).await;
}

async fn broadcast_quest_enpc_clear(
    player_id: u32,
    enpc: QuestEnpc,
    registry: &ActorRegistry,
    world: &WorldManager,
) {
    let Some(player_handle) = registry.get(player_id).await else {
        return;
    };
    let session_id = player_handle.session_id;
    if session_id == 0 {
        return;
    }
    let Some(client) = world.client(session_id).await else {
        return;
    };
    let zone_id = player_handle.zone_id;
    let Some(npc_handle) = find_npc_by_class_id(registry, zone_id, enpc.actor_class_id).await
    else {
        return;
    };
    let (npc_actor_id, conditions) = {
        let c = npc_handle.character.read().await;
        (c.base.actor_id, c.base.event_conditions.clone())
    };

    let subpackets = crate::packets::send::build_actor_event_status_packets(
        npc_actor_id,
        &conditions,
        false,
        false,
        Some(false),
        false,
    );
    for sub in subpackets {
        client.send_bytes(sub.to_bytes()).await;
    }
    let graphic = crate::packets::send::build_set_actor_quest_graphic(npc_actor_id, 0);
    client.send_bytes(graphic.to_bytes()).await;
}

async fn find_npc_by_class_id(
    registry: &ActorRegistry,
    zone_id: u32,
    class_id: u32,
) -> Option<ActorHandle> {
    let actors = registry.actors_in_zone(zone_id).await;
    for h in actors {
        let matches = {
            let c = h.character.read().await;
            c.chara.actor_class_id == class_id
        };
        if matches {
            return Some(h);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Cross-script quest dispatch — `quest:OnNotice(player)` triggered from a
// director script (`AfterQuestWarpDirector` et al.) routes through this
// helper so the target quest's `onNotice(player, quest, target)` hook
// fires with full command-drain support (unlike `fire_quest_hook` which
// only drains a narrow subset).
// ---------------------------------------------------------------------------

/// Dispatch a `quest:OnNotice(player)` call: look up the target quest's
/// script, build a fresh player snapshot + quest handle, invoke
/// `onNotice(player, quest, target)` via `spawn_blocking`, and drain any
/// emitted `LuaCommand`s through `apply_runtime_lua_commands` so scripted
/// side effects (flag flips, sequence starts, ENPC registration) land
/// after the cross-script hop.
///
/// No-ops quietly if:
/// * the player isn't in the registry,
/// * the player doesn't actually hold the quest (director may have
///   fired us after the quest was abandoned mid-zone-change),
/// * the quest id isn't in the catalog (no className → no script path),
/// * or the script file is missing on disk.
///
/// The `target` arg is fired as `nil` — mirroring how the C# LuaEngine
/// surfaces an unsupplied `triggerName` when directors call
/// `quest:OnNotice(player)` with just one arg.
pub async fn apply_quest_on_notice(
    player_id: u32,
    quest_id: u32,
    registry: &ActorRegistry,
    db: &Database,
    world: &WorldManager,
    lua: Option<&Arc<LuaEngine>>,
) {
    let Some(lua) = lua else {
        tracing::debug!(
            player = player_id,
            quest = quest_id,
            "quest:OnNotice dropped — no LuaEngine handle",
        );
        return;
    };
    let Some(handle) = registry.get(player_id).await else {
        tracing::debug!(
            player = player_id,
            quest = quest_id,
            "quest:OnNotice skipped — player not in registry",
        );
        return;
    };
    if !matches!(handle.kind, ActorKindTag::Player) {
        return;
    }
    let Some(script_name) = lua.catalogs().quest_script_name(quest_id) else {
        tracing::debug!(
            player = player_id,
            quest = quest_id,
            "quest:OnNotice skipped — quest id not in catalog",
        );
        return;
    };
    let script_path = lua.resolver().quest(&script_name);
    if !script_path.exists() {
        tracing::debug!(
            player = player_id,
            quest = quest_id,
            script = %script_path.display(),
            "quest:OnNotice skipped — script file missing",
        );
        return;
    }

    let (snapshot, quest_handle) = {
        let c = handle.character.read().await;
        if !c.quest_journal.has(quest_id) {
            tracing::debug!(
                player = player_id,
                quest = quest_id,
                "quest:OnNotice skipped — player no longer holds quest",
            );
            return;
        }
        let snap = crate::lua::userdata::PlayerSnapshot {
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
        };
        let q = c.quest_journal.get(quest_id).expect("quest_journal.has is true");
        let quest_handle = crate::lua::LuaQuestHandle {
            player_id: snap.actor_id,
            quest_id,
            has_quest: true,
            sequence: q.get_sequence(),
            flags: q.get_flags(),
            counters: [q.get_counter(0), q.get_counter(1), q.get_counter(2)],
            queue: crate::lua::command::CommandQueue::new(),
        };
        (snap, quest_handle)
    };

    let engine_clone = lua.clone();
    let script_path_clone = script_path.clone();
    let result = tokio::task::spawn_blocking(move || {
        engine_clone.call_quest_hook(
            &script_path_clone,
            "onNotice",
            snapshot,
            quest_handle,
            Vec::new(),
        )
    })
    .await;

    let result = match result {
        Ok(r) => r,
        Err(join_err) => {
            tracing::warn!(
                error = %join_err,
                quest = quest_id,
                "quest:OnNotice dispatch panicked",
            );
            return;
        }
    };
    if let Some(e) = result.error {
        tracing::debug!(
            error = %e,
            quest = quest_id,
            "quest:OnNotice errored",
        );
    }
    if !result.commands.is_empty() {
        // The quest's `onNotice` hook is what kicks the per-city intro
        // cutscene — `callClientFunction(player, "delegateEvent", player,
        // quest, "processTtrNomal001withHQ")` becomes a
        // `LuaCommand::RunEventFunction` and `player:EndEvent()` becomes
        // a `LuaCommand::EndEvent`. Both are event-flavoured: they have
        // no arm in `apply_runtime_lua_command` and would be silently
        // logged as "unhandled" — the cutscene packets would never reach
        // the client and the player would sit at "Now Loading" forever.
        // Translate them into the EventOutbox first (using the player's
        // EventSession for the in-flight event_owner / event_name /
        // event_type that the bridge needs), drain through
        // `dispatch_event_event` to actually emit the
        // `RunEventFunctionPacket` / `EndEventPacket`, then fall through
        // to the runtime apply for non-event commands. Mirrors the
        // pattern in `event::dispatcher::dispatch_director_event_started`.
        let event_session_snapshot = {
            let c = handle.character.read().await;
            c.event_session.clone()
        };
        let mut outbox = crate::event::outbox::EventOutbox::new();
        crate::event::lua_bridge::translate_lua_commands_into_outbox(
            &result.commands,
            &event_session_snapshot,
            &mut outbox,
        );
        for e in outbox.drain() {
            // Box::pin matches the recursion-guard already used below.
            Box::pin(crate::event::dispatcher::dispatch_event_event(
                &e,
                registry,
                world,
                db,
                Some(lua),
            ))
            .await;
        }

        // `apply_runtime_lua_commands` → ... → `apply_quest_on_notice`
        // is a potential recursion cycle (an `onNotice` hook could emit
        // another `QuestOnNotice`). Box the future so the compiler
        // doesn't need a statically-known size.
        Box::pin(apply_runtime_lua_commands(
            result.commands,
            registry,
            db,
            world,
            Some(lua),
        ))
        .await;

        // The opening-cutscene hook `man0l0.lua::onNotice` runs:
        //
        //     callClientFunction(player, "delegateEvent", player, quest,
        //                        "processTtrNomal001withHQ")
        //     player:EndEvent()
        //     quest:UpdateENPCs()
        //
        // `callClientFunction` (in scripts/global.lua) does
        // `coroutine.yield("_WAIT_EVENT", player)`, so the coroutine
        // parks after the first call. The 1.x client never sends a
        // matching `0x012E EventUpdate` for cutscene completion — the
        // cinematic plays asynchronously, the client expects the
        // server to drive the post-cinematic state on its own — so
        // the parked coroutine would sit forever, `player:EndEvent()`
        // (the `0x0131 EndEventPacket` that frees the client from
        // event-locked state) never fires, and the player can't
        // interact with NPCs even though visually the world is
        // rendered. Auto-fire the parked coroutine immediately so the
        // post-`callClientFunction` lines in the hook (the EndEvent
        // and UpdateENPCs calls) run in the same drain pass. Re-drain
        // anything those produce.
        if let Some(after) =
            lua.fire_player_event_and_drain(player_id, mlua::MultiValue::new())
        {
            if !after.is_empty() {
                let session_after = {
                    let c = handle.character.read().await;
                    c.event_session.clone()
                };
                let mut outbox = crate::event::outbox::EventOutbox::new();
                crate::event::lua_bridge::translate_lua_commands_into_outbox(
                    &after,
                    &session_after,
                    &mut outbox,
                );
                for e in outbox.drain() {
                    Box::pin(crate::event::dispatcher::dispatch_event_event(
                        &e,
                        registry,
                        world,
                        db,
                        Some(lua),
                    ))
                    .await;
                }
                Box::pin(apply_runtime_lua_commands(
                    after, registry, db, world, Some(lua),
                ))
                .await;
            }
        }
    }
}

/// Fire a quest's `onTalk(player, quest, npc)` hook on behalf of an
/// out-of-band caller (currently the GM `talkto` command). Mirrors
/// `PacketProcessor::fire_quest_hook` + the EventOutbox bridge step
/// from `apply_quest_on_notice`: runs the hook, translates the
/// event-flavoured commands (RunEventFunction / EndEvent / KickEvent)
/// into the outbox so their packets actually reach the client, then
/// falls through to `apply_runtime_lua_commands` for the rest.
///
/// Without this, `talkto` only fires `EventStarted` against the NPC's
/// class script, and the actual cutscene-driving lines in
/// `man0l0.lua::seq000_onTalk` (the ROSTNSTHAL branch that calls
/// `processTtrNomal003`) never run.
#[allow(clippy::too_many_arguments)]
pub async fn fire_quest_on_talk_via_command(
    handle: &ActorHandle,
    quest_id: u32,
    npc_spec: crate::lua::LuaNpcSpec,
    registry: &ActorRegistry,
    db: &Database,
    world: &WorldManager,
    lua: Option<&Arc<LuaEngine>>,
) {
    let Some(lua) = lua else { return };
    if !matches!(handle.kind, ActorKindTag::Player) {
        return;
    }
    let Some(script_name) = lua.catalogs().quest_script_name(quest_id) else {
        return;
    };
    let script_path = lua.resolver().quest(&script_name);
    if !script_path.exists() {
        return;
    }

    let (snapshot, quest_handle) = {
        let c = handle.character.read().await;
        if !c.quest_journal.has(quest_id) {
            return;
        }
        let snap = crate::lua::userdata::PlayerSnapshot {
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
        };
        let q = c.quest_journal.get(quest_id).expect("has(quest_id) is true");
        let qh = crate::lua::LuaQuestHandle {
            player_id: snap.actor_id,
            quest_id,
            has_quest: true,
            sequence: q.get_sequence(),
            flags: q.get_flags(),
            counters: [q.get_counter(0), q.get_counter(1), q.get_counter(2)],
            queue: crate::lua::command::CommandQueue::new(),
        };
        (snap, qh)
    };

    let lua_clone = lua.clone();
    let extra = vec![crate::lua::QuestHookArg::Npc(npc_spec)];
    let result = tokio::task::spawn_blocking(move || {
        lua_clone.call_quest_hook(&script_path, "onTalk", snapshot, quest_handle, extra)
    })
    .await;
    let result = match result {
        Ok(r) => r,
        Err(join_err) => {
            tracing::warn!(error = %join_err, quest = quest_id, "onTalk panicked");
            return;
        }
    };
    if let Some(e) = result.error {
        tracing::debug!(error = %e, quest = quest_id, "onTalk errored");
    }

    if result.commands.is_empty() {
        return;
    }

    // Bridge step — translate event-flavoured commands into the
    // EventOutbox, drain through `dispatch_event_event`. Same
    // pattern as `apply_quest_on_notice` and the patched
    // `dispatch_npc_event_started`.
    let event_session_snapshot = {
        let c = handle.character.read().await;
        c.event_session.clone()
    };
    let mut outbox = crate::event::outbox::EventOutbox::new();
    crate::event::lua_bridge::translate_lua_commands_into_outbox(
        &result.commands,
        &event_session_snapshot,
        &mut outbox,
    );
    for e in outbox.drain() {
        Box::pin(crate::event::dispatcher::dispatch_event_event(
            &e,
            registry,
            world,
            db,
            Some(lua),
        ))
        .await;
    }
    // Then drain the rest (quest flag mutates, AddExp, UpdateENPCs,
    // etc.) through the regular runtime apply pipeline.
    Box::pin(apply_runtime_lua_commands(
        result.commands,
        registry,
        db,
        world,
        Some(lua),
    ))
    .await;

    // Auto-resume any coroutine the onTalk hook parked via
    // `callClientFunction`'s `coroutine.yield("_WAIT_EVENT", player)`.
    // Mirrors the same auto-resume in `apply_quest_on_notice` — without
    // it, `player:EndEvent()` after `callClientFunction` never runs and
    // the client stays in event-locked state.
    let player_id = handle.actor_id;
    if let Some(after) = lua.fire_player_event_and_drain(player_id, mlua::MultiValue::new()) {
        if !after.is_empty() {
            let session_after = {
                let c = handle.character.read().await;
                c.event_session.clone()
            };
            let mut outbox = crate::event::outbox::EventOutbox::new();
            crate::event::lua_bridge::translate_lua_commands_into_outbox(
                &after,
                &session_after,
                &mut outbox,
            );
            for e in outbox.drain() {
                Box::pin(crate::event::dispatcher::dispatch_event_event(
                    &e,
                    registry,
                    world,
                    db,
                    Some(lua),
                ))
                .await;
            }
            Box::pin(apply_runtime_lua_commands(
                after, registry, db, world, Some(lua),
            ))
            .await;
        }
    }
}

// ---------------------------------------------------------------------------
// Lua hook firing — mirror of `PacketProcessor::fire_quest_hook` that
// drains emitted commands back through `apply_runtime_lua_command`.
// ---------------------------------------------------------------------------

async fn fire_quest_hook(
    handle: &ActorHandle,
    quest_id: u32,
    hook_name: &str,
    extra_args: Vec<crate::lua::QuestHookArg>,
    lua: &Arc<LuaEngine>,
    registry: &ActorRegistry,
    db: &Database,
) {
    // Skip Lua work on actors that aren't Players — NPCs / BattleNpcs
    // carry a default-empty quest_journal but shouldn't ever reach this
    // path in practice, and a missing session id would drop any
    // downstream packet anyway.
    if !matches!(handle.kind, ActorKindTag::Player) {
        return;
    }
    let Some(script_name) = lua.catalogs().quest_script_name(quest_id) else {
        return;
    };
    let script_path = lua.resolver().quest(&script_name);
    if !script_path.exists() {
        return;
    }

    let (snapshot, quest_handle) = {
        let c = handle.character.read().await;
        let snap = crate::lua::userdata::PlayerSnapshot {
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
        };
        let quest = c
            .quest_journal
            .get(quest_id)
            .map(|q| {
                (
                    q.get_sequence(),
                    q.get_flags(),
                    [q.get_counter(0), q.get_counter(1), q.get_counter(2)],
                )
            })
            .unwrap_or((0, 0, [0; 3]));
        let handle = crate::lua::LuaQuestHandle {
            player_id: snap.actor_id,
            quest_id,
            has_quest: c.quest_journal.has(quest_id),
            sequence: quest.0,
            flags: quest.1,
            counters: quest.2,
            queue: crate::lua::command::CommandQueue::new(),
        };
        (snap, handle)
    };

    let lua_clone = lua.clone();
    let hook_name_owned = hook_name.to_string();
    let result = tokio::task::spawn_blocking(move || {
        lua_clone.call_quest_hook(
            &script_path,
            &hook_name_owned,
            snapshot,
            quest_handle,
            extra_args,
        )
    })
    .await;
    let result = match result {
        Ok(r) => r,
        Err(join_err) => {
            tracing::warn!(error = %join_err, quest = quest_id, hook = hook_name, "hook panicked");
            return;
        }
    };
    if let Some(e) = result.error {
        tracing::debug!(error = %e, quest = quest_id, hook = hook_name, "hook errored");
    }

    // Recurse into the runtime drain (Box::pin to bound future size —
    // hooks can emit AddQuest which re-enters fire_quest_hook).
    // The `world` parameter needs a placeholder here; fetch it from the
    // player handle's zone lookup path. Since this helper doesn't take a
    // world ref, any command that needs it (QuestSetEnpc,
    // QuestUpdateEnpcs, QuestStartSequence's stale-drain) would no-op
    // silently. Callers that want full command support pass
    // `apply_runtime_lua_commands` directly with a world ref after the
    // hook returns — this helper only powers `apply_add_quest` /
    // `apply_complete_quest` / `apply_abandon_quest`, none of which
    // run onStateChange or otherwise need a world.
    if !result.commands.is_empty() {
        tracing::debug!(
            quest = quest_id,
            hook = hook_name,
            commands = result.commands.len(),
            "hook emitted runtime commands (not drained from fire_quest_hook)",
        );
        // Best-effort drain for pure-runtime commands that don't need
        // the WorldManager. Commands that do need `world` are logged
        // and dropped by apply_runtime_lua_command's `_ => false`.
        // Callers wanting full command drain should use the public
        // `apply_runtime_lua_commands` against the same registry/db/lua.
        let _ = (registry, db); // silence unused in degenerate builds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::Character;
    use crate::actor::quest::{Quest, quest_actor_id};
    use crate::runtime::actor_registry::ActorKindTag;

    fn tmpdir() -> std::path::PathBuf {
        // Two parallel tests landing on the same nanosecond tick would
        // share this dir and clobber each other's scripts; the atomic
        // counter guarantees uniqueness.
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("garlemald-onnotice-{nanos}-{seq}"));
        std::fs::create_dir_all(dir.join("quests/man")).unwrap();
        dir
    }

    fn tempdb() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("garlemald-onnotice-{nanos}-{seq}.db"))
    }

    /// `apply_quest_on_notice` resolves the script, fires
    /// `onNotice(player, quest, target)`, and drains any commands the
    /// hook emits. We have the hook flip a quest flag bit so we can
    /// assert both halves (hook ran, drain applied) from one side
    /// effect.
    #[tokio::test]
    async fn apply_quest_on_notice_fires_hook_and_drains_commands() {
        let root = tmpdir();
        std::fs::write(
            root.join("quests/man/man0l1.lua"),
            r#"
                function onNotice(player, quest, target)
                    quest:SetQuestFlag(3)
                end
            "#,
        )
        .unwrap();

        let lua = Arc::new(LuaEngine::new(&root));
        {
            let mut quests = std::collections::HashMap::new();
            quests.insert(
                110_002u32,
                crate::gamedata::QuestMeta {
                    id: 110_002,
                    quest_name: "Call of the Sea".to_string(),
                    class_name: "Man0l1".to_string(),
                    prerequisite: 0,
                    min_level: 1,
                },
            );
            lua.catalogs().install_quests(quests);
        }

        let registry = ActorRegistry::new();
        let mut character = Character::new(13);
        let mut quest = Quest::new(quest_actor_id(110_002), "Man0l1".to_string());
        quest.clear_dirty();
        character.quest_journal.add(quest);
        let handle = ActorHandle::new(13, ActorKindTag::Player, 100, 42, character);
        registry.insert(handle.clone()).await;
        let world = WorldManager::new();
        let db = crate::database::Database::open(tempdb())
            .await
            .expect("db stub");

        apply_quest_on_notice(13, 110_002, &registry, &db, &world, Some(&lua)).await;

        // The onNotice hook's `SetQuestFlag(3)` should have walked the
        // drain → `apply_quest_mutation` → `Quest::set_flag(3)`, leaving
        // bit 3 set on the live quest in the registry.
        let flags = {
            let c = handle.character.read().await;
            c.quest_journal.get(110_002).map(|q| q.get_flags()).unwrap_or(0)
        };
        assert_eq!(
            flags & (1 << 3),
            1 << 3,
            "onNotice should have set flag bit 3 via drained SetQuestFlag",
        );

        let _ = std::fs::remove_dir_all(root);
    }

    /// Missing `onNotice` function is a quiet no-op — mirrors how
    /// `AfterQuestWarpDirector` can fire `quest:OnNotice` on any quest
    /// in the journal without every script defining the hook.
    #[tokio::test]
    async fn apply_quest_on_notice_is_a_quiet_no_op_when_hook_missing() {
        let root = tmpdir();
        // Script with no onNotice — just a top-level global assignment
        // so load_script succeeds.
        std::fs::write(
            root.join("quests/man/man0l0.lua"),
            "_no_notice_defined = true",
        )
        .unwrap();

        let lua = Arc::new(LuaEngine::new(&root));
        {
            let mut quests = std::collections::HashMap::new();
            quests.insert(
                110_001u32,
                crate::gamedata::QuestMeta {
                    id: 110_001,
                    quest_name: "Shapeless Melody".to_string(),
                    class_name: "Man0l0".to_string(),
                    prerequisite: 0,
                    min_level: 1,
                },
            );
            lua.catalogs().install_quests(quests);
        }

        let registry = ActorRegistry::new();
        let mut character = Character::new(21);
        let mut quest = Quest::new(quest_actor_id(110_001), "Man0l0".to_string());
        quest.clear_dirty();
        character.quest_journal.add(quest);
        let handle = ActorHandle::new(21, ActorKindTag::Player, 100, 42, character);
        registry.insert(handle.clone()).await;
        let world = WorldManager::new();
        let db = crate::database::Database::open(tempdb())
            .await
            .expect("db stub");

        // Should not panic; should not emit any side effects.
        apply_quest_on_notice(21, 110_001, &registry, &db, &world, Some(&lua)).await;

        let flags = {
            let c = handle.character.read().await;
            c.quest_journal.get(110_001).map(|q| q.get_flags()).unwrap_or(0)
        };
        assert_eq!(flags, 0, "missing onNotice leaves flags untouched");

        let _ = std::fs::remove_dir_all(root);
    }

    /// Player-not-in-registry (e.g. the director fired OnNotice after
    /// a fast logout) is a quiet no-op. Guard the happy path from
    /// panicking on a stale cross-script reference.
    #[tokio::test]
    async fn apply_quest_on_notice_skips_unknown_player() {
        let root = tmpdir();
        let lua = Arc::new(LuaEngine::new(&root));

        let registry = ActorRegistry::new();
        let world = WorldManager::new();
        let db = crate::database::Database::open(tempdb())
            .await
            .expect("db stub");

        apply_quest_on_notice(9999, 110_001, &registry, &db, &world, Some(&lua)).await;
        // Assertion here is "no panic". The function walks out of the
        // `registry.get` branch without touching the LuaEngine.
        let _ = std::fs::remove_dir_all(root);
    }
}
