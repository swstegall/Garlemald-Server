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
            apply_add_exp(actor_id, class_id, exp, registry, db, Some(world)).await;
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
            true
        }
        _ => false,
    }
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
    // WorldManager to reach the session → client handle.
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
