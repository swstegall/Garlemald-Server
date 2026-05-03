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

//! Outbox → side-effect dispatchers.
//!
//! Four entry points — one per typed outbox — turn events into real
//! packets (routed to the right client via the SessionRegistry held on
//! WorldManager), DB writes (via `Database`), and script hooks (via
//! `LuaEngine`). Broadcast events fan out through `broadcast.rs`.
//!
//! Each dispatch function is `async` because the side effects are:
//! channel sends (socket queues), SQL round-trips, and Lua dispatch. The
//! functions are intentionally small and independently callable so the
//! game ticker can sequence them explicitly per tick.

#![allow(dead_code)]

use std::sync::Arc;

use tokio::sync::RwLock;

use crate::battle::command::{BattleCommand, CommandResult, CommandResultContainer, CommandType};
use crate::battle::effects::{ActionProperty, ActionType};
use crate::battle::outbox::BattleEvent;
use crate::battle::utils as battle_utils;
use crate::database::Database;
use crate::inventory::outbox::InventoryEvent;
use crate::packets::send as tx;
use crate::status::outbox::StatusEvent;
use crate::world_manager::WorldManager;
use crate::zone::outbox::AreaEvent;
use crate::zone::zone::Zone;

use super::actor_registry::ActorRegistry;
use super::broadcast::broadcast_around_actor;

// Stateless RNG adapter for the battle-utils `Rng` trait. Each call pulls
// a fresh `f64` from the thread-local generator. Implemented this way
// (instead of wrapping `rand::rngs::ThreadRng`) because the dispatcher
// future must be `Send`, and `ThreadRng` holds an `Rc` internally.
struct ThreadRng;

impl battle_utils::Rng for ThreadRng {
    fn next_f64(&mut self) -> f64 {
        rand::random::<f64>()
    }
}

// ---------------------------------------------------------------------------
// Status events
// ---------------------------------------------------------------------------

pub async fn dispatch_status_event(
    event: &StatusEvent,
    registry: &ActorRegistry,
    world: &WorldManager,
    db: &Database,
    catalogs: &std::sync::Arc<crate::lua::Catalogs>,
) {
    match event {
        StatusEvent::DbSave { owner_actor_id } => {
            // Phase 1 logs — the actual `save_player_status_effects` wire
            // lands alongside the status-specific SQL in a later phase.
            tracing::debug!(owner = owner_actor_id, "status: DbSave (TODO)");
            let _ = db;
        }
        StatusEvent::PacketSetStatus {
            owner_actor_id,
            slot_index,
            status_id,
        } => {
            // Emit a SetActorStatusPacket to every client around the owner.
            let sub = tx::build_set_actor_status(*owner_actor_id, *slot_index, *status_id);
            broadcast_to_neighbours(world, registry, *owner_actor_id, sub.to_bytes()).await;
        }
        StatusEvent::PacketSetStatusTime {
            owner_actor_id,
            slot_index,
            expires_at,
        } => {
            // `statusShownTime[i]` updates ride on the ActorPropertyPacketUtil
            // bundle in retail — a small batching helper that hasn't landed
            // in our packet builders yet. Log the intent; real emission
            // follows in Phase 2 with the rest of the property-delta flow.
            tracing::debug!(
                owner = owner_actor_id,
                slot = slot_index,
                expires_at = expires_at,
                "status: set-status-time (pending property-util)"
            );
        }
        StatusEvent::HpTick {
            owner_actor_id,
            delta,
        } => {
            apply_hp_delta(registry, *owner_actor_id, *delta).await;
        }
        StatusEvent::MpTick {
            owner_actor_id,
            delta,
        } => {
            apply_mp_delta(registry, *owner_actor_id, *delta).await;
        }
        StatusEvent::TpTick {
            owner_actor_id,
            delta,
        } => {
            apply_tp_delta(registry, *owner_actor_id, *delta).await;
        }
        StatusEvent::RecalcStats { owner_actor_id } => {
            apply_recalc_stats(registry, world, catalogs, db, *owner_actor_id).await;
        }
        StatusEvent::LuaCall {
            owner_actor_id,
            status_effect_id,
            function_name,
        } => {
            tracing::debug!(
                owner = owner_actor_id,
                effect = status_effect_id,
                fn_name = function_name,
                "status: LuaCall (TODO)"
            );
        }
        StatusEvent::WorldMasterText {
            owner_actor_id,
            text_id,
            status_effect_id,
            play_gain_animation: _,
        } => {
            tracing::debug!(
                owner = owner_actor_id,
                text = text_id,
                effect = status_effect_id,
                "status: WorldMasterText (TODO)"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Battle events
// ---------------------------------------------------------------------------

pub async fn dispatch_battle_event(
    event: &BattleEvent,
    registry: &ActorRegistry,
    world: &WorldManager,
    zone: &Arc<RwLock<Zone>>,
    lua: Option<&Arc<crate::lua::LuaEngine>>,
    db: Option<&Arc<crate::database::Database>>,
) {
    match event {
        BattleEvent::Engage {
            owner_actor_id,
            target_actor_id: _,
        }
        | BattleEvent::Disengage { owner_actor_id }
        | BattleEvent::Spawn { owner_actor_id }
        | BattleEvent::Despawn { owner_actor_id }
        | BattleEvent::RecalcStats { owner_actor_id } => {
            tracing::debug!(owner = owner_actor_id, kind = ?event_tag(event), "battle event");
        }
        BattleEvent::Die { owner_actor_id } => {
            // `BattleEvent::Die` carries no attacker (scripted / GM
            // `!die` paths don't need one). `die_if_defender_fell` is
            // where real combat deaths route through with an attacker
            // id for `onKillBNpc` dispatch.
            apply_die(*owner_actor_id, registry, world, zone).await;
        }
        BattleEvent::Revive { owner_actor_id } => {
            apply_revive(*owner_actor_id, registry, world, zone).await;
        }
        BattleEvent::TargetChange {
            owner_actor_id,
            new_target_actor_id: _,
        } => {
            tracing::debug!(owner = owner_actor_id, "battle: target change");
        }
        BattleEvent::DoBattleAction {
            owner_actor_id,
            skill_handler: _,
            battle_animation,
            results,
        } => {
            // Render each CommandResult into a wire-level entry and pack
            // into the appropriate container.
            let wire: Vec<tx::actor_battle::CommandResult> =
                results.iter().map(battle_result_to_wire).collect();

            let mut offset = 0;
            while offset < wire.len() {
                let sub = if wire.len() - offset <= 16 {
                    tx::actor_battle::build_command_result_x10(
                        *owner_actor_id,
                        *battle_animation,
                        /* command_id */ 0,
                        &wire,
                        &mut offset,
                    )
                } else {
                    tx::actor_battle::build_command_result_x18(
                        *owner_actor_id,
                        *battle_animation,
                        0,
                        &wire,
                        &mut offset,
                    )
                };
                broadcast_around_actor(world, registry, zone, *owner_actor_id, sub.to_bytes())
                    .await;
            }
        }
        BattleEvent::PlayAnimation {
            owner_actor_id,
            animation,
        } => {
            let sub = tx::actor_battle::build_command_result_x00(*owner_actor_id, *animation, 0);
            broadcast_around_actor(world, registry, zone, *owner_actor_id, sub.to_bytes()).await;
        }
        BattleEvent::CastStart { owner_actor_id, .. }
        | BattleEvent::CastComplete { owner_actor_id, .. }
        | BattleEvent::CastInterrupted { owner_actor_id, .. } => {
            tracing::debug!(owner = owner_actor_id, "battle: cast-bar (TODO)");
        }
        BattleEvent::HateAdd {
            owner_actor_id,
            target_actor_id,
            amount,
        } => {
            // Apply to the character's hate container so the next controller
            // tick reflects it.
            if let Some(handle) = registry.get(*owner_actor_id).await {
                let mut chara = handle.character.write().await;
                chara.hate.update_hate(*target_actor_id, *amount);
            }
        }
        BattleEvent::HateClear {
            owner_actor_id,
            target_actor_id,
        } => {
            if let Some(handle) = registry.get(*owner_actor_id).await {
                let mut chara = handle.character.write().await;
                chara.hate.clear_hate(*target_actor_id);
            }
        }
        BattleEvent::QueryTargets { .. } => {
            // Target queries are run inline by the battle engine; this
            // event is here for observability only.
        }
        BattleEvent::LuaCall { function_name, .. } => {
            tracing::debug!(fn_name = function_name, "battle: LuaCall (TODO)");
        }
        BattleEvent::WorldMasterText {
            owner_actor_id,
            text_id,
        } => {
            tracing::debug!(
                owner = owner_actor_id,
                text = text_id,
                "battle: WorldMasterText"
            );
        }
        BattleEvent::ResolveAutoAttack {
            attacker_actor_id,
            defender_actor_id,
        } => {
            resolve_auto_attack(
                *attacker_actor_id,
                *defender_actor_id,
                registry,
                world,
                zone,
                lua,
                db,
            )
            .await;
        }
        BattleEvent::ResolveAction {
            attacker_actor_id,
            defender_actor_id,
            command,
        } => {
            resolve_action(
                *attacker_actor_id,
                *defender_actor_id,
                command,
                registry,
                world,
                zone,
                lua,
                db,
            )
            .await;
        }
    }
}

// ---------------------------------------------------------------------------
// Auto-attack + cast-complete resolution.
//
// Called from the battle-event dispatcher when the AIContainer emits a
// `ResolveAutoAttack` (Attack state ready) or `ResolveAction` (cast state
// complete). Responsibilities:
//
//   1. Look up both actors. If either is missing or dead, bail.
//   2. Snapshot attacker stats (immutable) while holding a short read lock.
//   3. Take the defender's write lock. Snapshot its mods for the immutable
//      `CombatView`, then hand the live map to `finish_action_*` so
//      stoneskin can consume its pool in place.
//   4. Apply HP delta + hate update on the defender.
//   5. Broadcast each `CommandResult` row via the same packet builders the
//      existing `DoBattleAction` event uses.
// ---------------------------------------------------------------------------

async fn resolve_auto_attack(
    attacker_actor_id: u32,
    defender_actor_id: u32,
    registry: &ActorRegistry,
    world: &WorldManager,
    zone: &Arc<RwLock<Zone>>,
    lua: Option<&Arc<crate::lua::LuaEngine>>,
    db: Option<&Arc<crate::database::Database>>,
) {
    if attacker_actor_id == defender_actor_id {
        return;
    }
    let Some(attacker_handle) = registry.get(attacker_actor_id).await else {
        return;
    };
    let Some(defender_handle) = registry.get(defender_actor_id).await else {
        return;
    };

    // Attacker snapshot — released before we lock the defender.
    let (atk_level, atk_max_hp, atk_mods) = {
        let a = attacker_handle.character.read().await;
        if !a.is_alive() {
            return;
        }
        (a.chara.level, a.chara.max_hp, a.chara.mods.clone())
    };

    let mut rng = ThreadRng;
    // Compute base damage off an attacker-only CombatView — the defender
    // view is only needed for post-base mitigation, which runs further
    // down inside the defender lock scope via calculate_physical_damage_taken.
    let atk_view_for_base = battle_utils::CombatView {
        actor_id: attacker_actor_id,
        level: atk_level,
        max_hp: atk_max_hp,
        mods: &atk_mods,
        has_aegis_boon: false,
        has_protect: false,
        has_shell: false,
        has_stoneskin: false,
    };
    let base = battle_utils::attack_calculate_base_damage(&atk_view_for_base, &mut rng);

    let mut result = CommandResult::for_target(defender_actor_id, 0, 0);
    result.amount = base.max(0) as u16;
    result.command_type = CommandType::AUTO_ATTACK;
    result.action_type = ActionType::Physical;
    result.action_property = ActionProperty::Slashing;
    result.enmity = result.amount;

    let mut container = CommandResultContainer::new();
    let dmg_request;
    {
        let mut d = defender_handle.character.write().await;
        if !d.is_alive() {
            return;
        }
        let def_level = d.chara.level;
        let def_max_hp = d.chara.max_hp;
        let def_mods_snapshot = d.chara.mods.clone();

        let atk_view = battle_utils::CombatView {
            actor_id: attacker_actor_id,
            level: atk_level,
            max_hp: atk_max_hp,
            mods: &atk_mods,
            has_aegis_boon: false,
            has_protect: false,
            has_shell: false,
            has_stoneskin: false,
        };
        let def_view = battle_utils::CombatView {
            actor_id: defender_actor_id,
            level: def_level,
            max_hp: def_max_hp,
            mods: &def_mods_snapshot,
            has_aegis_boon: false,
            has_protect: false,
            has_shell: false,
            has_stoneskin: false,
        };

        battle_utils::calc_rates(&atk_view, &def_view, None, &mut result);
        dmg_request = battle_utils::finish_action_physical(
            &atk_view,
            &def_view,
            &mut d.chara.mods,
            None,
            &mut result,
            &mut container,
            &mut rng,
        );

        if let Some(dr) = dmg_request
            && dr.amount > 0
        {
            d.add_hp(-(dr.amount as i32));
            d.hate.update_hate(attacker_actor_id, dr.enmity as i32);
        }
    }

    broadcast_results(
        attacker_actor_id,
        0, // battle_animation — auto-attacks have no distinct animation id
        &container.main_results,
        registry,
        world,
        zone,
    )
    .await;

    die_if_defender_fell(
        defender_actor_id,
        Some(attacker_actor_id),
        registry,
        world,
        zone,
        lua,
        db,
    )
    .await;
}

async fn resolve_action(
    attacker_actor_id: u32,
    defender_actor_id: u32,
    command: &BattleCommand,
    registry: &ActorRegistry,
    world: &WorldManager,
    zone: &Arc<RwLock<Zone>>,
    lua: Option<&Arc<crate::lua::LuaEngine>>,
    db: Option<&Arc<crate::database::Database>>,
) {
    let Some(attacker_handle) = registry.get(attacker_actor_id).await else {
        return;
    };
    let Some(defender_handle) = registry.get(defender_actor_id).await else {
        return;
    };

    let (atk_level, atk_max_hp, atk_mods) = {
        let a = attacker_handle.character.read().await;
        if !a.is_alive() {
            return;
        }
        (a.chara.level, a.chara.max_hp, a.chara.mods.clone())
    };

    let mut rng = ThreadRng;
    let mut result = CommandResult::from_command(defender_actor_id, command, 1);
    // Lua scripts normally drop the base potency into `action.amount`
    // before calling `DoAction`. Until those bindings land, use the
    // command's `base_potency` directly; zero falls back to a conservative
    // placeholder so casts still produce a visible hit number.
    result.amount = if command.base_potency > 0 {
        command.base_potency
    } else {
        100
    };
    result.enmity = result.amount;

    let mut container = CommandResultContainer::new();
    let dmg_request;
    let is_heal = command.action_type == ActionType::Heal;

    {
        let mut d_write = defender_handle.character.write().await;
        if !d_write.is_alive() && !is_heal {
            return;
        }
        let def_level = d_write.chara.level;
        let def_max_hp = d_write.chara.max_hp;
        let def_mods_snapshot = d_write.chara.mods.clone();

        let atk_view = battle_utils::CombatView {
            actor_id: attacker_actor_id,
            level: atk_level,
            max_hp: atk_max_hp,
            mods: &atk_mods,
            has_aegis_boon: false,
            has_protect: false,
            has_shell: false,
            has_stoneskin: false,
        };
        let def_view = battle_utils::CombatView {
            actor_id: defender_actor_id,
            level: def_level,
            max_hp: def_max_hp,
            mods: &def_mods_snapshot,
            has_aegis_boon: false,
            has_protect: false,
            has_shell: false,
            has_stoneskin: false,
        };

        let mut skill = command.clone();
        battle_utils::calc_rates(&atk_view, &def_view, Some(&skill), &mut result);
        dmg_request = match command.action_type {
            ActionType::Physical => battle_utils::finish_action_physical(
                &atk_view,
                &def_view,
                &mut d_write.chara.mods,
                Some(&mut skill),
                &mut result,
                &mut container,
                &mut rng,
            ),
            ActionType::Magic => battle_utils::finish_action_spell(
                &atk_view,
                &def_view,
                &mut d_write.chara.mods,
                Some(&mut skill),
                &mut result,
                &mut container,
                &mut rng,
            ),
            ActionType::Heal => {
                let heal = battle_utils::finish_action_heal(
                    &atk_view,
                    &def_view,
                    &mut result,
                    &mut container,
                );
                d_write.add_hp(heal.amount as i32);
                None
            }
            ActionType::Status | ActionType::None => {
                battle_utils::finish_action_status(&skill, &mut result, &mut container);
                None
            }
        };

        if let Some(dr) = dmg_request
            && dr.amount > 0
        {
            d_write.add_hp(-(dr.amount as i32));
            d_write.hate.update_hate(attacker_actor_id, dr.enmity as i32);
        }
    }

    broadcast_results(
        attacker_actor_id,
        command.battle_animation,
        &container.main_results,
        registry,
        world,
        zone,
    )
    .await;

    die_if_defender_fell(
        defender_actor_id,
        Some(attacker_actor_id),
        registry,
        world,
        zone,
        lua,
        db,
    )
    .await;
}

async fn broadcast_results(
    source_actor_id: u32,
    battle_animation: u32,
    results: &[CommandResult],
    registry: &ActorRegistry,
    world: &WorldManager,
    zone: &Arc<RwLock<Zone>>,
) {
    if results.is_empty() {
        return;
    }
    let wire: Vec<tx::actor_battle::CommandResult> =
        results.iter().map(battle_result_to_wire).collect();
    let mut offset = 0;
    while offset < wire.len() {
        let sub = if wire.len() - offset <= 16 {
            tx::actor_battle::build_command_result_x10(
                source_actor_id,
                battle_animation,
                0,
                &wire,
                &mut offset,
            )
        } else {
            tx::actor_battle::build_command_result_x18(
                source_actor_id,
                battle_animation,
                0,
                &wire,
                &mut offset,
            )
        };
        broadcast_around_actor(world, registry, zone, source_actor_id, sub.to_bytes()).await;
    }
}

fn event_tag(event: &BattleEvent) -> &'static str {
    match event {
        BattleEvent::Engage { .. } => "engage",
        BattleEvent::Disengage { .. } => "disengage",
        BattleEvent::Spawn { .. } => "spawn",
        BattleEvent::Die { .. } => "die",
        BattleEvent::Despawn { .. } => "despawn",
        BattleEvent::RecalcStats { .. } => "recalc",
        _ => "other",
    }
}

fn battle_result_to_wire(
    r: &crate::battle::command::CommandResult,
) -> tx::actor_battle::CommandResult {
    tx::actor_battle::CommandResult {
        target_id: r.target_actor_id,
        hit_num: r.hit_num,
        sub_command: 0,
        hit_effect: r.effect_id,
        mitigated_amount: r.amount_mitigated as u32,
        amount: r.amount as u32,
        command_type: r.command_type.bits() as u8,
        animation_id: r.animation,
        worldmaster_text_id: r.world_master_text_id,
        param: r.param as u32,
        action_property: r.action_property as u8,
    }
}

// ---------------------------------------------------------------------------
// Inventory events
// ---------------------------------------------------------------------------

pub async fn dispatch_inventory_event(
    event: &InventoryEvent,
    registry: &ActorRegistry,
    world: &WorldManager,
    db: &Database,
    catalogs: &std::sync::Arc<crate::lua::Catalogs>,
) {
    match event {
        // ---- DB mutations --------------------------------------------------
        InventoryEvent::DbAdd {
            owner_actor_id,
            item,
            item_package,
            slot,
        } => {
            if let Err(e) = db
                .add_item(*owner_actor_id, item.unique_id, *item_package, *slot)
                .await
            {
                tracing::warn!(owner = owner_actor_id, err = %e, "inventory: add_item failed");
            }
        }
        InventoryEvent::DbRemove {
            owner_actor_id,
            server_item_id,
        } => {
            if let Err(e) = db.remove_item(*owner_actor_id, *server_item_id).await {
                tracing::warn!(
                    owner = owner_actor_id,
                    iid = server_item_id,
                    err = %e,
                    "inventory: remove_item failed",
                );
            }
        }
        InventoryEvent::DbQuantity {
            server_item_id,
            quantity,
        } => {
            if let Err(e) = db.set_quantity(*server_item_id, *quantity).await {
                tracing::warn!(iid = server_item_id, err = %e, "inventory: set_quantity failed");
            }
        }
        InventoryEvent::DbPositions { updates } => {
            if let Err(e) = db.update_item_positions(updates).await {
                tracing::warn!(err = %e, "inventory: update_item_positions failed");
            }
        }
        InventoryEvent::DbEquip {
            owner_actor_id,
            equip_slot,
            unique_item_id,
        } => {
            let class_id = resolve_current_class_id(registry, *owner_actor_id).await;
            let is_undergarment = *equip_slot == crate::actor::player::SLOT_UNDERSHIRT
                || *equip_slot == crate::actor::player::SLOT_UNDERGARMENT;
            match db
                .equip_item(
                    *owner_actor_id,
                    class_id,
                    *equip_slot,
                    *unique_item_id,
                    is_undergarment,
                )
                .await
            {
                Ok(()) => apply_recalc_stats(registry, world, catalogs, db, *owner_actor_id).await,
                Err(e) => tracing::warn!(
                    owner = owner_actor_id,
                    slot = equip_slot,
                    err = %e,
                    "inventory: equip_item failed",
                ),
            }
        }
        InventoryEvent::DbUnequip {
            owner_actor_id,
            equip_slot,
        } => {
            let class_id = resolve_current_class_id(registry, *owner_actor_id).await;
            match db
                .unequip_item(*owner_actor_id, class_id, *equip_slot)
                .await
            {
                Ok(()) => apply_recalc_stats(registry, world, catalogs, db, *owner_actor_id).await,
                Err(e) => tracing::warn!(
                    owner = owner_actor_id,
                    slot = equip_slot,
                    err = %e,
                    "inventory: unequip_item failed",
                ),
            }
        }

        // ---- Packet emissions ---------------------------------------------
        InventoryEvent::PacketBeginChange { owner_actor_id } => {
            send_inventory_packet(
                registry,
                world,
                *owner_actor_id,
                tx::actor_inventory::build_inventory_begin_change(*owner_actor_id, false),
            )
            .await;
        }
        InventoryEvent::PacketEndChange { owner_actor_id } => {
            send_inventory_packet(
                registry,
                world,
                *owner_actor_id,
                tx::actor_inventory::build_inventory_end_change(*owner_actor_id),
            )
            .await;
        }
        InventoryEvent::PacketSetBegin {
            owner_actor_id,
            capacity,
            code,
        } => {
            send_inventory_packet(
                registry,
                world,
                *owner_actor_id,
                tx::actor_inventory::build_inventory_set_begin(*owner_actor_id, *capacity, *code),
            )
            .await;
        }
        InventoryEvent::PacketSetEnd { owner_actor_id } => {
            send_inventory_packet(
                registry,
                world,
                *owner_actor_id,
                tx::actor_inventory::build_inventory_set_end(*owner_actor_id),
            )
            .await;
        }
        InventoryEvent::PacketItems {
            owner_actor_id,
            items,
        } => {
            let Some(client) = resolve_client(registry, world, *owner_actor_id).await else {
                return;
            };
            let mut offset = 0usize;
            while offset < items.len() {
                let remaining = items.len() - offset;
                let sub = if remaining >= 64 {
                    tx::actor_inventory::build_inventory_list_x64(
                        *owner_actor_id,
                        items,
                        &mut offset,
                    )
                } else if remaining >= 32 {
                    tx::actor_inventory::build_inventory_list_x32(
                        *owner_actor_id,
                        items,
                        &mut offset,
                    )
                } else if remaining >= 16 {
                    tx::actor_inventory::build_inventory_list_x16(
                        *owner_actor_id,
                        items,
                        &mut offset,
                    )
                } else if remaining >= 8 {
                    tx::actor_inventory::build_inventory_list_x08_n(
                        *owner_actor_id,
                        items,
                        &mut offset,
                    )
                } else {
                    let sub = tx::actor_inventory::build_inventory_list_x01(
                        *owner_actor_id,
                        &items[offset],
                    );
                    offset += 1;
                    sub
                };
                client.send_bytes(sub.to_bytes()).await;
            }
        }
        InventoryEvent::PacketModifierFrame {
            owner_actor_id,
            items,
        } => {
            let Some(client) = resolve_client(registry, world, *owner_actor_id).await else {
                return;
            };
            for sub in tx::actor_inventory::build_mass_set_item_modifier_frame(
                *owner_actor_id,
                items,
            ) {
                client.send_bytes(sub.to_bytes()).await;
            }
        }
        InventoryEvent::PacketRemoveSlots {
            owner_actor_id,
            slots,
        } => {
            let Some(client) = resolve_client(registry, world, *owner_actor_id).await else {
                return;
            };
            let mut offset = 0usize;
            while offset < slots.len() {
                let remaining = slots.len() - offset;
                let sub = if remaining >= 64 {
                    tx::actor_inventory::build_inventory_remove_x64(
                        *owner_actor_id,
                        slots,
                        &mut offset,
                    )
                } else if remaining >= 32 {
                    tx::actor_inventory::build_inventory_remove_x32(
                        *owner_actor_id,
                        slots,
                        &mut offset,
                    )
                } else if remaining >= 16 {
                    tx::actor_inventory::build_inventory_remove_x16(
                        *owner_actor_id,
                        slots,
                        &mut offset,
                    )
                } else if remaining >= 8 {
                    tx::actor_inventory::build_inventory_remove_x08(
                        *owner_actor_id,
                        slots,
                        &mut offset,
                    )
                } else {
                    let sub = tx::actor_inventory::build_inventory_remove_x01(
                        *owner_actor_id,
                        slots[offset],
                    );
                    offset += 1;
                    sub
                };
                client.send_bytes(sub.to_bytes()).await;
            }
        }
        InventoryEvent::PacketLinkedSingle {
            owner_actor_id,
            position,
            item,
        } => {
            send_inventory_packet(
                registry,
                world,
                *owner_actor_id,
                tx::actor_inventory::build_linked_item_list_x01(
                    *owner_actor_id,
                    *position,
                    item.as_ref(),
                ),
            )
            .await;
        }
        InventoryEvent::PacketLinkedMany {
            owner_actor_id,
            items,
        } => {
            let Some(client) = resolve_client(registry, world, *owner_actor_id).await else {
                return;
            };
            let mut offset = 0usize;
            while offset < items.len() {
                let remaining = items.len() - offset;
                let sub = if remaining >= 64 {
                    tx::actor_inventory::build_linked_item_list_x64(
                        *owner_actor_id,
                        items,
                        &mut offset,
                    )
                } else if remaining >= 32 {
                    tx::actor_inventory::build_linked_item_list_x32(
                        *owner_actor_id,
                        items,
                        &mut offset,
                    )
                } else if remaining >= 16 {
                    tx::actor_inventory::build_linked_item_list_x16(
                        *owner_actor_id,
                        items,
                        &mut offset,
                    )
                } else if remaining >= 8 {
                    tx::actor_inventory::build_linked_item_list_x08(
                        *owner_actor_id,
                        items,
                        &mut offset,
                    )
                } else {
                    let (position, item) = &items[offset];
                    let sub = tx::actor_inventory::build_linked_item_list_x01(
                        *owner_actor_id,
                        *position,
                        Some(item),
                    );
                    offset += 1;
                    sub
                };
                client.send_bytes(sub.to_bytes()).await;
            }
        }
    }
}

async fn resolve_client(
    registry: &ActorRegistry,
    world: &WorldManager,
    owner_actor_id: u32,
) -> Option<crate::data::ClientHandle> {
    let handle = registry.get(owner_actor_id).await?;
    world.client(handle.session_id).await
}

async fn send_inventory_packet(
    registry: &ActorRegistry,
    world: &WorldManager,
    owner_actor_id: u32,
    sub: common::subpacket::SubPacket,
) {
    if let Some(client) = resolve_client(registry, world, owner_actor_id).await {
        client.send_bytes(sub.to_bytes()).await;
    }
}

/// Resolve the `class_id` Player currently has equipped (or 0 for NPCs).
/// Mirrors the C# `player.charaWork.parameterSave.state_mainSkill[0]` lookup
/// that `Database.EquipItem/UnequipItem` feeds as the `classId` column.
async fn resolve_current_class_id(registry: &ActorRegistry, owner_actor_id: u32) -> u8 {
    let Some(handle) = registry.get(owner_actor_id).await else {
        return 0;
    };
    let chara = handle.character.read().await;
    if chara.chara.current_job != 0 {
        chara.chara.current_job as u8
    } else {
        chara.chara.class as u8
    }
}

// ---------------------------------------------------------------------------
// Area events
// ---------------------------------------------------------------------------

pub async fn dispatch_area_event(
    event: &AreaEvent,
    registry: &ActorRegistry,
    world: &WorldManager,
    zone: &Arc<RwLock<Zone>>,
) {
    match event {
        AreaEvent::BroadcastAroundActor {
            source_actor_id,
            payload,
            ..
        } => {
            broadcast_around_actor(world, registry, zone, *source_actor_id, payload.clone()).await;
        }
        AreaEvent::WeatherChange {
            area_id,
            weather_id,
            transition_time,
            target_actor_id,
            zone_wide,
        } => {
            let payload = tx::build_set_weather(*area_id, *weather_id, *transition_time).to_bytes();
            if let (Some(aid), false) = (target_actor_id, zone_wide) {
                if let Some(handle) = registry.get(*aid).await
                    && let Some(client) = world.client(handle.session_id).await
                {
                    client.send_bytes(payload).await;
                }
            } else {
                // Zone-wide — queue to every player in this zone.
                let players = registry.actors_in_zone(*area_id).await;
                for p in players {
                    if !p.is_player() {
                        continue;
                    }
                    if let Some(client) = world.client(p.session_id).await {
                        client.send_bytes(payload.clone()).await;
                    }
                }
            }
        }
        AreaEvent::ActorAdded { area_id, actor_id } => {
            // Fan the Npc::GetSpawnPackets bundle to every Player
            // within 50 yalms. Uses the zone's spatial grid — no
            // broadcast when the actor being added *is* the player
            // themselves (they'll get a self-spawn via a separate
            // zone-in flow).
            spawn_bundle_fanout(world, registry, zone, *area_id, *actor_id).await;
        }
        AreaEvent::ActorRemoved {
            area_id: _,
            actor_id,
        } => {
            // Broadcast RemoveActor to nearby players. The grid has
            // already dropped the projection; we reach around it via
            // broadcast_around_actor's 50-yalm neighbour set using the
            // actor's last-known cell. The broadcast helper is happy
            // with a missing actor (falls back to no-op).
            let packet = tx::actor::build_remove_actor(*actor_id);
            broadcast_around_actor(world, registry, zone, *actor_id, packet.to_bytes()).await;
        }
        AreaEvent::ActorMoved { .. }
        | AreaEvent::DirectorCreated { .. }
        | AreaEvent::DirectorDeleted { .. }
        | AreaEvent::ContentAreaCreated { .. }
        | AreaEvent::ContentAreaDeleted { .. }
        | AreaEvent::SpawnActor { .. } => {
            tracing::debug!(?event, "area event (observability-only)");
        }
    }
}

/// Pump the full actor-spawn bundle to every Player within
/// BROADCAST_RADIUS of `actor_id`. Mirrors C# `Npc::GetSpawnPackets`:
/// AddActor + Speed + SpawnPosition + **Appearance** + Name + State +
/// **SubState** + **StatusAll** + **Icon** + IsZoning. Without the
/// starred packets the client's `DepictionJudge:judgeNameplate` Lua
/// tries to read appearance/status fields that are still nil on the
/// neighbouring actor and crashes ~10s after zone-in. ScriptBind
/// (0x00CC ActorInstantiate) is still deferred — it needs the full
/// LuaParam bind list and is a follow-up once Lua wire-up for
/// broadcast spawns lands.
async fn spawn_bundle_fanout(
    world: &WorldManager,
    registry: &ActorRegistry,
    zone: &Arc<RwLock<Zone>>,
    _area_id: u32,
    actor_id: u32,
) {
    let Some(handle) = registry.get(actor_id).await else {
        return;
    };
    // Snapshot the character's base + appearance state for the spawn
    // bundle. Holding the read lock across the whole snapshot keeps
    // the fields consistent (position + appearance + status together).
    let (name, state, display_name_id, position, rotation, model_id, appearance_ids) = {
        let c = handle.character.read().await;
        (
            c.base.display_name().to_string(),
            c.base.current_main_state as u8,
            c.base.display_name_id,
            c.base.position(),
            c.base.rotation,
            c.chara.model_id,
            c.chara.appearance_ids,
        )
    };
    let packets = [
        tx::actor::build_add_actor(actor_id, 0).to_bytes(),
        tx::actor::build_set_actor_speed_default(actor_id).to_bytes(),
        tx::actor::build_set_actor_position(
            actor_id, -1, position.x, position.y, position.z, rotation, 1, false,
        )
        .to_bytes(),
        tx::actor::build_set_actor_appearance(actor_id, model_id, &appearance_ids).to_bytes(),
        tx::actor::build_set_actor_name(actor_id, display_name_id, &name).to_bytes(),
        tx::actor::build_set_actor_state(actor_id, state, 0).to_bytes(),
        tx::actor::build_set_actor_sub_state(actor_id, 0, 0, 0, 0, 0, 0).to_bytes(),
        tx::actor::build_set_actor_status_all(actor_id, &[0u16; 20]).to_bytes(),
        tx::actor::build_set_actor_icon(actor_id, 0).to_bytes(),
        tx::actor::build_set_actor_is_zoning(actor_id, false).to_bytes(),
    ];
    for bytes in packets {
        broadcast_around_actor(world, registry, zone, actor_id, bytes).await;
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn broadcast_to_neighbours(
    world: &WorldManager,
    registry: &ActorRegistry,
    source_actor_id: u32,
    packet_bytes: Vec<u8>,
) {
    let Some(handle) = registry.get(source_actor_id).await else {
        return;
    };
    let Some(zone) = world.zone(handle.zone_id).await else {
        return;
    };
    broadcast_around_actor(world, registry, &zone, source_actor_id, packet_bytes).await;
}

async fn apply_hp_delta(registry: &ActorRegistry, actor_id: u32, delta: i32) {
    if let Some(handle) = registry.get(actor_id).await {
        let mut c = handle.character.write().await;
        c.add_hp(delta);
    }
}

async fn apply_mp_delta(registry: &ActorRegistry, actor_id: u32, delta: i32) {
    if let Some(handle) = registry.get(actor_id).await {
        let mut c = handle.character.write().await;
        c.add_mp(delta);
    }
}

async fn apply_tp_delta(registry: &ActorRegistry, actor_id: u32, delta: i32) {
    if let Some(handle) = registry.get(actor_id).await {
        let mut c = handle.character.write().await;
        c.add_tp(delta);
    }
}

/// Recompute HP/MP pools from the modifier map, then — for Players — run
/// the four-stage stat pipeline (reset → baseline → gear sum → derivation).
/// Dispatched from both `StatusEvent::RecalcStats` and the equip/unequip
/// arms of `dispatch_inventory_event`.
///
/// **Player pipeline ordering is load-bearing:**
///   1. [`Character::reset_player_bonus_stats`] — zeroes primaries +
///      Hp/Mp/Tp + derived secondaries so the pipeline is idempotent.
///      Without this, unequipping gear would never remove its stat
///      contributions (gear_sum uses `add`).
///   2. [`Character::apply_player_stat_baseline`] — class+level seeded
///      placeholder (real per-level curves not reversed). Seeds the
///      primaries freshly since step 1 zeroed them.
///   3. [`Character::apply_player_gear_stats`] — sums paramBonus from
///      currently-equipped items, mapping `paramBonusType -
///      15001 → Modifier` per the decoder in `actor/modifier.rs`.
///      Equipment comes from a DB roundtrip; the catalog from the
///      `Catalogs::items` reader installed at boot.
///   4. [`Character::apply_player_stat_derivation`] — STR→Attack,
///      VIT→Defense, etc.
///
/// For Players, a post-pipeline diff of the four pool values drives the
/// `charaWork/stateAtQuicklyForAll` broadcast — if any of hp/hpMax/mp/mpMax
/// changed, both the owner and neighbor clients receive the bundle so
/// nameplate HP bars and the self-HUD stay in sync with the new gear
/// pool. Unchanged recalcs (status-effect tick routed through
/// `StatusEvent::RecalcStats`, for example) produce no wire traffic.
async fn apply_recalc_stats(
    registry: &ActorRegistry,
    world: &WorldManager,
    catalogs: &std::sync::Arc<crate::lua::Catalogs>,
    db: &crate::database::Database,
    actor_id: u32,
) {
    let Some(handle) = registry.get(actor_id).await else {
        return;
    };
    let is_player = handle.is_player();

    // Resolve the active class *before* taking the write lock — the
    // lookup itself reads the character, so holding a write lock for
    // the roundtrip would deadlock on the registry's own shared state.
    let class_id = if is_player {
        resolve_current_class_id(registry, actor_id).await
    } else {
        0
    };

    // Load equipment catalog ids from DB outside the write lock. `.ok()`
    // swallows the unlikely I/O error — an empty equipment set just
    // means gear_sum skips, baseline + derivation still produce usable
    // numbers.
    let equipped: std::collections::HashMap<u16, u32> = if is_player {
        db.load_equipped_catalog_ids(actor_id, class_id)
            .await
            .unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };

    let (pre_pools, post_pools, class_slot, main_skill_level) = {
        let mut c = handle.character.write().await;
        let pre = if is_player {
            Some((c.chara.hp, c.chara.max_hp, c.chara.mp, c.chara.max_mp))
        } else {
            None
        };
        if is_player {
            // Full Player pipeline — ordering load-bearing:
            //   1. reset — zeroes the pipeline's targets so the rest
            //      of the passes are idempotent.
            //   2. baseline — seeds primaries + Hp/Mp/Tp from class+
            //      level (seed-if-zero, so DB-persisted pool values
            //      survive the reset→baseline round-trip at login).
            //   3. gear_sum — additive paramBonus deltas from all slots.
            //   4. weapon_stats — Delay/AttackType/HitCount from main-
            //      hand (overrides any paramBonus HitCount); flat
            //      Attack/Parry from both hands stack.
            //   5. weapon_damage_power — main-hand `damagePower`
            //      stashed on Modifier::WeaponDamagePower for the
            //      auto-attack damage formula to read.
            //   6. derivation — STR → Attack, VIT → Defense, etc.
            //   7. recalculate_stats — project Hp/Mp mods onto the
            //      character's HP/MP pools and plant the HitCount=1
            //      h2h fallback if the weapon stage didn't set one.
            c.reset_player_bonus_stats();
            c.apply_player_stat_baseline();
            if let Ok(items) = catalogs.items.read() {
                c.apply_player_gear_stats(&items, &equipped);
                c.apply_player_weapon_stats(&items, &equipped);
                c.apply_player_weapon_damage_power(&items, &equipped);
            }
            c.apply_player_stat_derivation();
        }
        c.recalculate_stats();
        let post = if is_player {
            Some((c.chara.hp, c.chara.max_hp, c.chara.mp, c.chara.max_mp))
        } else {
            None
        };
        // `chara.class` is the active class slot — same value
        // `state_main_skill[0]` stores on the DB side. `chara.level`
        // mirrors `skill_level[class]` and is what the baseline +
        // nameplate expect.
        let main_skill = c.chara.class.max(0) as u8;
        let main_skill_level = c.chara.level.max(1) as u16;
        (pre, post, main_skill, main_skill_level)
    };

    // Broadcast HP/MP if any of the four pool values changed. Gating
    // keeps no-op recalcs (e.g. status-effect tick reroute) quiet — a
    // Player standing still with no equip events fires ~0 bundles/sec.
    if let (Some(pre), Some(post)) = (pre_pools, post_pools)
        && pre != post
    {
        let hp = post.0.max(0) as u16;
        let hp_max = post.1.max(0) as u16;
        let mp = post.2.max(0) as u16;
        let mp_max = post.3.max(0) as u16;
        // `tp` is read separately because it's on a different tick
        // lifecycle than HP/MP — if TP didn't change we still need a
        // value to fill the packet. Use the current snapshot.
        let tp = {
            let c = handle.character.read().await;
            c.chara.tp
        };
        let mut subs = crate::packets::send::actor::build_chara_state_at_quickly_for_all(
            actor_id, hp, hp_max, mp, mp_max, tp,
        );
        subs.extend(
            crate::packets::send::actor::build_player_state_at_quickly_for_all(
                actor_id,
                hp,
                hp_max,
                class_slot,
                main_skill_level,
            ),
        );
        for sub in subs {
            let bytes = sub.to_bytes();
            send_to_self_if_player(registry, world, actor_id, bytes.clone()).await;
            broadcast_to_neighbours(world, registry, actor_id, bytes).await;
        }
    }
}

// ---------------------------------------------------------------------------
// Death / revive
// ---------------------------------------------------------------------------

/// Inline-post-damage death check. Called from `resolve_auto_attack` /
/// `resolve_action` once the defender's write lock is released. If the
/// defender's HP crossed to 0 on this frame and they're not already in
/// the DEAD state, flip them into it and broadcast the wire change.
/// Idempotent — a double-invoke for the same actor is a cheap no-op.
///
/// `attacker_actor_id` and `lua` are threaded through for the
/// `onKillBNpc(player, quest, bnpc_class_id)` hook: when the defender
/// is a BattleNpc and the attacker is a Player, fire the hook once per
/// quest in the attacker's journal (Meteor's convention — scripts
/// filter by `bnpc_class_id` themselves).
pub(crate) async fn die_if_defender_fell(
    defender_actor_id: u32,
    attacker_actor_id: Option<u32>,
    registry: &ActorRegistry,
    world: &WorldManager,
    zone: &Arc<RwLock<Zone>>,
    lua: Option<&Arc<crate::lua::LuaEngine>>,
    db: Option<&Arc<crate::database::Database>>,
) {
    let Some(handle) = registry.get(defender_actor_id).await else {
        return;
    };
    let (is_dead_now, already_marked) = {
        let c = handle.character.read().await;
        (
            c.is_dead(),
            c.base.current_main_state == crate::actor::MAIN_STATE_DEAD,
        )
    };
    if !is_dead_now || already_marked {
        return;
    }
    let is_bnpc = matches!(
        handle.kind,
        crate::runtime::actor_registry::ActorKindTag::BattleNpc,
    );
    let (bnpc_class_id, mob_level) = if is_bnpc {
        let c = handle.character.read().await;
        (c.chara.actor_class_id, c.chara.level.max(1))
    } else {
        (0, 1)
    };

    apply_die(defender_actor_id, registry, world, zone).await;

    if !is_bnpc {
        return;
    }
    let Some(attacker_id) = attacker_actor_id else {
        return;
    };
    let Some(attacker_handle) = registry.get(attacker_id).await else {
        return;
    };
    if !matches!(
        attacker_handle.kind,
        crate::runtime::actor_registry::ActorKindTag::Player,
    ) {
        return;
    }
    let Some(db_arc) = db else {
        // No DB in scope (test harness) — the runtime-command drain for
        // AddExp/QuestSetFlag/etc. needs it to persist state. Skip the
        // hook rather than emitting packets without persistence.
        return;
    };

    // Grand Company seal reward — port of Meteor's "kill grants
    // seals" payout. Hook fires before the quest-hook chain so a
    // quest that consumes the kill (e.g. "kill 10 hectaeyes for
    // the Maelstrom") sees the post-reward seal balance if it
    // queries it. Approximation of retail's per-rank/per-dlvl
    // curve: `seals = mob_level` (clamped to [1, 100]); promotion
    // through the rank-cap table eventually reduces returns once
    // the player exceeds the mob level. The richer curve from
    // `BattleUtils.cs:GetEXPReward` is the follow-up.
    award_grand_company_seals(&attacker_handle, mob_level, db_arc).await;

    let Some(lua) = lua else {
        return;
    };
    crate::runtime::quest_hook::fire_on_kill_bnpc(
        &attacker_handle,
        lua,
        bnpc_class_id,
        registry,
        db_arc,
        world,
    )
    .await;
}

/// Award GC seals to `attacker_handle` for killing a mob of the given
/// level. No-ops when the attacker isn't enlisted. Cap is the per-rank
/// seal cap (matches `gcseals.lua::AddGCSeals` — refusing the deposit
/// over-cap), implemented at the DB layer in `add_seals`'s
/// transactional upsert. Failure to persist is logged but not fatal.
async fn award_grand_company_seals(
    attacker_handle: &crate::runtime::actor_registry::ActorHandle,
    mob_level: i16,
    db: &Arc<crate::database::Database>,
) {
    let (gc, gc_rank) = {
        let c = attacker_handle.character.read().await;
        let gc = c.chara.gc_current;
        if gc == 0 {
            return;
        }
        let rank = match gc {
            crate::actor::gc::GC_MAELSTROM => c.chara.gc_rank_limsa,
            crate::actor::gc::GC_TWIN_ADDER => c.chara.gc_rank_gridania,
            crate::actor::gc::GC_IMMORTAL_FLAMES => c.chara.gc_rank_uldah,
            _ => return,
        };
        (gc, rank)
    };
    let base_reward = (mob_level as i32).clamp(1, 100);
    let cap = crate::actor::gc::rank_seal_cap(gc_rank);
    if cap == 0 {
        // Rank-0 (no enlistment), or post-cap rank like Chief Admiral
        // (111) — the Lua helper returns INV_ERROR_FULL there too.
        return;
    }
    let current = db
        .get_seals(attacker_handle.actor_id, gc)
        .await
        .unwrap_or(0);
    if current >= cap {
        return;
    }
    let grant = base_reward.min(cap - current);
    if grant <= 0 {
        return;
    }
    if let Err(e) = db.add_seals(attacker_handle.actor_id, gc, grant).await {
        tracing::warn!(
            attacker = attacker_handle.actor_id,
            gc,
            err = %e,
            "GC seal reward DB persist failed",
        );
    } else {
        tracing::info!(
            attacker = attacker_handle.actor_id,
            gc,
            mob_level,
            grant,
            "GC seal reward applied",
        );
    }
}

/// Per-difficulty base seal payout for a completed guildleve. Retail's
/// canonical formula isn't preserved in any of the local archive
/// dumps, so this uses a sensible escalation pinned at the dialogue-
/// anchored Recruit→Pvt3 cost (100 seals, the cheapest hop in
/// `gc_promotion_cost`). A 1-star leve grants 150 seals — enough to
/// cover the first promotion plus a small buffer; a 5-star leve grants
/// 550. Calibrated so a player completing two 3-star leves can afford
/// the Recruit→Private Third Class hop with seals to spare, and a
/// streak of higher-difficulty leves keeps pace with the escalating
/// promotion-cost curve. Returns 0 for unknown difficulty values so
/// the caller cleanly skips the deposit.
pub fn leve_completion_seal_reward(difficulty: u8) -> i32 {
    match difficulty {
        1 => 150,
        2 => 250,
        3 => 350,
        4 => 450,
        5 => 550,
        _ => 0,
    }
}

/// Award GC seals to `member_handle` for completing a guildleve at
/// the given star difficulty. Mirrors `award_grand_company_seals` but
/// keyed on leve difficulty rather than mob level, called from
/// `director::dispatcher` on `GuildleveEnded { was_completed: true }`.
/// No-op when the player isn't enlisted, when the difficulty isn't in
/// the reward table (`leve_completion_seal_reward` returns 0), or when
/// the player's seal balance is already at their rank's cap.
pub async fn award_leve_completion_seals(
    member_handle: &crate::runtime::actor_registry::ActorHandle,
    difficulty: u8,
    db: &crate::database::Database,
) {
    let (gc, gc_rank) = {
        let c = member_handle.character.read().await;
        let gc = c.chara.gc_current;
        if gc == 0 {
            return;
        }
        let rank = match gc {
            crate::actor::gc::GC_MAELSTROM => c.chara.gc_rank_limsa,
            crate::actor::gc::GC_TWIN_ADDER => c.chara.gc_rank_gridania,
            crate::actor::gc::GC_IMMORTAL_FLAMES => c.chara.gc_rank_uldah,
            _ => return,
        };
        (gc, rank)
    };
    let base_reward = leve_completion_seal_reward(difficulty);
    if base_reward <= 0 {
        return;
    }
    let cap = crate::actor::gc::rank_seal_cap(gc_rank);
    if cap == 0 {
        return;
    }
    let current = db
        .get_seals(member_handle.actor_id, gc)
        .await
        .unwrap_or(0);
    if current >= cap {
        return;
    }
    let grant = base_reward.min(cap - current);
    if grant <= 0 {
        return;
    }
    if let Err(e) = db.add_seals(member_handle.actor_id, gc, grant).await {
        tracing::warn!(
            member = member_handle.actor_id,
            gc,
            difficulty,
            err = %e,
            "leve-completion seal reward DB persist failed",
        );
    } else {
        tracing::info!(
            member = member_handle.actor_id,
            gc,
            difficulty,
            grant,
            "leve-completion seal reward applied",
        );
    }
}

/// Port of Meteor's `DeathState.OnStart` tail: disengage the AI, flip
/// `current_main_state` to DEAD, broadcast `SetActorState` around the
/// owner. Status-effect cleanup (`LoseOnDeath` flag) is deferred — the
/// status table lacks the flag surfacing today.
pub(crate) async fn apply_die(
    owner_actor_id: u32,
    registry: &ActorRegistry,
    world: &WorldManager,
    zone: &Arc<RwLock<Zone>>,
) {
    let Some(handle) = registry.get(owner_actor_id).await else {
        return;
    };
    {
        let mut c = handle.character.write().await;
        if c.base.current_main_state == crate::actor::MAIN_STATE_DEAD {
            return;
        }
        // Force HP to zero so `is_dead` stays consistent if some caller
        // invoked `apply_die` without a prior damage settle (e.g. a
        // scripted `BattleNpc::die`).
        if c.chara.hp > 0 {
            c.chara.hp = 0;
        }
        c.base.current_main_state = crate::actor::MAIN_STATE_DEAD;
        c.chara.new_main_state = crate::actor::MAIN_STATE_DEAD;
        // Stamp the death timestamp so the ticker's NPC respawn pass
        // (and the Modifier::Raise auto-revive pass) can measure
        // elapsed time. Wall-clock seconds, mirroring how
        // `last_rest_accrual_utc` carries the inn-tick anchor.
        c.chara.time_of_death_utc = common::utils::unix_timestamp() as u32;
        // Skip `internal_disengage` here — it would push a follow-up
        // Disengage event. The state-change packet we emit below is
        // load-bearing; the per-actor AI also picks the DEAD state on its
        // next tick, which short-circuits Attack/Cast states without us
        // needing an explicit Disengage broadcast.
        c.ai_container.clear_states();
    }
    let sub = tx::build_set_actor_state(owner_actor_id, crate::actor::MAIN_STATE_DEAD as u8, 0);
    let bytes = sub.to_bytes();
    send_to_self_if_player(registry, world, owner_actor_id, bytes.clone()).await;
    broadcast_around_actor(world, registry, zone, owner_actor_id, bytes).await;
}

/// Bring an actor back from the DEAD state. For Players this is the
/// home-point revive button; for NPCs the spawner re-spawns them through
/// the same entry point. Mirrors Meteor's `Spawn` tail used when
/// `Modifier.Raise > 0` catches a player before the despawn timer fires.
pub(crate) async fn apply_revive(
    owner_actor_id: u32,
    registry: &ActorRegistry,
    world: &WorldManager,
    zone: &Arc<RwLock<Zone>>,
) {
    let Some(handle) = registry.get(owner_actor_id).await else {
        return;
    };
    {
        let mut c = handle.character.write().await;
        if c.base.current_main_state != crate::actor::MAIN_STATE_DEAD {
            return;
        }
        let max_hp = c.chara.max_hp as i32;
        let max_mp = c.chara.max_mp as i32;
        c.set_hp(max_hp);
        c.set_mp(max_mp);
        c.base.current_main_state = crate::actor::MAIN_STATE_PASSIVE;
        c.chara.new_main_state = crate::actor::MAIN_STATE_PASSIVE;
        // Clear the death-timestamp so the ticker stops tracking the
        // raise / respawn window for this actor.
        c.chara.time_of_death_utc = 0;
    }
    let sub = tx::build_set_actor_state(
        owner_actor_id,
        crate::actor::MAIN_STATE_PASSIVE as u8,
        0,
    );
    let bytes = sub.to_bytes();
    send_to_self_if_player(registry, world, owner_actor_id, bytes.clone()).await;
    broadcast_around_actor(world, registry, zone, owner_actor_id, bytes).await;
}

/// Outcome of [`apply_home_point_revive`] — surfaced for callers that
/// need to render a different message depending on whether the warp
/// actually happened.
#[derive(Debug, Clone, Copy)]
pub enum HomePointReviveOutcome {
    /// HP/state restored AND the player was warped to their home-point
    /// aetheryte. Carries the destination zone id + xyz so the caller
    /// can echo / verify.
    Warped {
        homepoint: u32,
        zone_id: u32,
        x: f32,
        y: f32,
        z: f32,
    },
    /// HP/state restored but the player wasn't warped — either they
    /// have no homepoint set (`homepoint == 0`) or the homepoint id
    /// isn't in the [`crate::actor::aetheryte::AETHERYTE_SPAWNS`]
    /// table. Mirrors the safe fallback `apply_revive` already
    /// provides for the in-place case.
    InPlace,
    /// Player not in the registry — nothing happened.
    UnknownPlayer,
}

/// Home-point revive flow: restore HP/MP/state via [`apply_revive`],
/// then warp the player to the coords for their stored
/// `chara.homepoint` (if any). Mutates `BaseActor` position +
/// `Session.destination_*` so the next zone-in bundle (or any in-
/// flight position broadcast) lands at the new spot. The packet-side
/// `DoZoneChange` emission is left for the caller — `runtime` doesn't
/// own the session/client glue and the GM-command path is satisfied
/// by the server-state mutation alone.
///
/// Mirrors the "bandaid fix for returning while dead" branch in
/// `scripts/lua/commands/TeleportCommand.lua`: `SetHP(maxHP) +
/// ChangeState(0) + DoZoneChange`. Wrapping it server-side means GM
/// `home <name>` and any future `player:HomePointRevive()` Lua
/// binding use the same path.
pub async fn apply_home_point_revive(
    player_id: u32,
    registry: &ActorRegistry,
    world: &WorldManager,
    zone: &Arc<RwLock<Zone>>,
) -> HomePointReviveOutcome {
    let Some(handle) = registry.get(player_id).await else {
        return HomePointReviveOutcome::UnknownPlayer;
    };
    apply_revive(player_id, registry, world, zone).await;
    let homepoint = {
        let c = handle.character.read().await;
        c.chara.homepoint
    };
    if homepoint == 0 {
        return HomePointReviveOutcome::InPlace;
    }
    let Some(spawn) = crate::actor::aetheryte::lookup(homepoint) else {
        tracing::debug!(
            player = player_id,
            homepoint,
            "home-point revive: aetheryte id not in coords table; in-place revive only",
        );
        return HomePointReviveOutcome::InPlace;
    };
    {
        let mut c = handle.character.write().await;
        c.base.zone_id = spawn.zone_id;
        c.base.position_x = spawn.x;
        c.base.position_y = spawn.y;
        c.base.position_z = spawn.z;
    }
    if let Some(mut session) = world.session(handle.session_id).await {
        session.destination_zone_id = spawn.zone_id;
        session.destination_x = spawn.x;
        session.destination_y = spawn.y;
        session.destination_z = spawn.z;
        // Spawn type 2 = retail "warp by gm" / aetheryte-teleport code,
        // matching the value `TeleportCommand.lua` uses in its
        // `DoZoneChange(player, zone, nil, 0, 2, ...)` call.
        session.destination_spawn_type = 2;
        world.upsert_session(session).await;
    }
    HomePointReviveOutcome::Warped {
        homepoint,
        zone_id: spawn.zone_id,
        x: spawn.x,
        y: spawn.y,
        z: spawn.z,
    }
}

/// The zone broadcaster excludes the source actor, so Players who die or
/// revive wouldn't otherwise see their own state-change packet. Send it
/// directly to their session.
async fn send_to_self_if_player(
    registry: &ActorRegistry,
    world: &WorldManager,
    owner_actor_id: u32,
    bytes: Vec<u8>,
) {
    let Some(handle) = registry.get(owner_actor_id).await else {
        return;
    };
    if !handle.is_player() {
        return;
    }
    if let Some(client) = world.client(handle.session_id).await {
        client.send_bytes(bytes).await;
    }
}

#[cfg(test)]
mod home_point_revive_tests {
    use super::*;
    use crate::actor::Character;
    use crate::data::Session;
    use crate::runtime::actor_registry::{ActorHandle, ActorKindTag};
    use crate::zone::navmesh::StubNavmeshLoader;

    fn make_zone(zone_id: u32) -> Zone {
        Zone::new(
            zone_id,
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
        )
    }

    /// Happy path: dead player with a Limsa-CAP homepoint set →
    /// `Warped` outcome, HP restored, base position + session
    /// destination updated to the aetheryte coords.
    #[tokio::test]
    async fn warps_dead_player_to_homepoint_aetheryte() {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());

        let source_zone = 100u32;
        let zone = make_zone(source_zone);
        world.register_zone(zone).await;

        let mut character = Character::new(7);
        character.chara.max_hp = 1000;
        character.chara.max_mp = 500;
        character.chara.hp = 0;
        character.chara.mp = 0;
        character.base.current_main_state = crate::actor::MAIN_STATE_DEAD;
        character.base.zone_id = source_zone;
        character.base.position_x = 1.0;
        character.base.position_y = 2.0;
        character.base.position_z = 3.0;
        character.chara.homepoint = 1_280_001; // Limsa Lominsa CAP → zone 230.
        let handle = ActorHandle::new(7, ActorKindTag::Player, source_zone, 42, character);
        registry.insert(handle.clone()).await;

        // Pre-seat a session so destination_* can be updated.
        let mut session = Session::default();
        session.id = 42;
        session.current_zone_id = source_zone;
        world.upsert_session(session).await;

        let zone_arc = world.zone(source_zone).await.expect("zone registered");
        let outcome = apply_home_point_revive(7, &registry, &world, &zone_arc).await;

        match outcome {
            HomePointReviveOutcome::Warped {
                homepoint,
                zone_id,
                ..
            } => {
                assert_eq!(homepoint, 1_280_001);
                assert_eq!(zone_id, 230);
            }
            other => panic!("expected Warped, got {other:?}"),
        }

        let c = handle.character.read().await;
        assert_eq!(c.chara.hp, c.chara.max_hp, "HP restored");
        assert_eq!(c.chara.mp, c.chara.max_mp, "MP restored");
        assert_eq!(
            c.base.current_main_state,
            crate::actor::MAIN_STATE_PASSIVE,
            "no longer dead",
        );
        assert_eq!(c.base.zone_id, 230, "warped to Limsa CAP zone");
        assert_eq!(c.base.position_x, -407.0);
        assert!((c.base.position_y - 42.5).abs() < 1e-3);
        assert_eq!(c.base.position_z, 337.0);

        let session = world.session(42).await.expect("session present");
        assert_eq!(session.destination_zone_id, 230);
        assert_eq!(session.destination_x, -407.0);
        assert_eq!(session.destination_spawn_type, 2);
    }

    /// Player with `homepoint == 0` (never attuned to an aetheryte)
    /// falls back to in-place revive. HP/state restored, position
    /// unchanged.
    #[tokio::test]
    async fn revives_in_place_when_homepoint_unset() {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        let source_zone = 100u32;
        let zone = make_zone(source_zone);
        world.register_zone(zone).await;

        let mut character = Character::new(8);
        character.chara.max_hp = 500;
        character.chara.hp = 0;
        character.base.current_main_state = crate::actor::MAIN_STATE_DEAD;
        character.base.zone_id = source_zone;
        character.base.position_x = 50.0;
        character.base.position_y = 0.0;
        character.base.position_z = 50.0;
        character.chara.homepoint = 0;
        let handle = ActorHandle::new(8, ActorKindTag::Player, source_zone, 0, character);
        registry.insert(handle.clone()).await;

        let zone_arc = world.zone(source_zone).await.expect("zone registered");
        let outcome = apply_home_point_revive(8, &registry, &world, &zone_arc).await;

        assert!(
            matches!(outcome, HomePointReviveOutcome::InPlace),
            "expected InPlace, got {outcome:?}",
        );

        let c = handle.character.read().await;
        assert_eq!(c.chara.hp, c.chara.max_hp);
        assert_eq!(c.base.zone_id, source_zone, "still in source zone");
        assert_eq!(c.base.position_x, 50.0, "position unchanged");
    }

    /// Unknown aetheryte id (something outside the
    /// `1_280_001..=1_280_125` range, or a missing-from-table id)
    /// also falls back to in-place revive.
    #[tokio::test]
    async fn unknown_aetheryte_id_falls_back_to_in_place() {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        let source_zone = 100u32;
        let zone = make_zone(source_zone);
        world.register_zone(zone).await;

        let mut character = Character::new(9);
        character.chara.max_hp = 200;
        character.chara.hp = 0;
        character.base.current_main_state = crate::actor::MAIN_STATE_DEAD;
        character.base.zone_id = source_zone;
        character.chara.homepoint = 9_999_999; // Definitely not in the table.
        let handle = ActorHandle::new(9, ActorKindTag::Player, source_zone, 0, character);
        registry.insert(handle.clone()).await;

        let zone_arc = world.zone(source_zone).await.expect("zone registered");
        let outcome = apply_home_point_revive(9, &registry, &world, &zone_arc).await;

        assert!(
            matches!(outcome, HomePointReviveOutcome::InPlace),
            "expected InPlace, got {outcome:?}",
        );
        let c = handle.character.read().await;
        assert_eq!(c.base.zone_id, source_zone);
    }

    /// Player not in the registry → `UnknownPlayer` outcome, no
    /// panic, no state mutated.
    #[tokio::test]
    async fn missing_player_returns_unknown_outcome() {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        let source_zone = 100u32;
        let zone = make_zone(source_zone);
        world.register_zone(zone).await;

        let zone_arc = world.zone(source_zone).await.expect("zone registered");
        let outcome = apply_home_point_revive(404, &registry, &world, &zone_arc).await;
        assert!(matches!(outcome, HomePointReviveOutcome::UnknownPlayer));
    }

    /// Alive player calling home-point revive still warps — the
    /// underlying `apply_revive` no-ops (state isn't DEAD), but the
    /// warp half of the operation still runs. Mirrors how `/return`
    /// behaves at retail when a live player invokes it.
    #[tokio::test]
    async fn alive_player_still_warps_to_homepoint() {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        let source_zone = 100u32;
        let zone = make_zone(source_zone);
        world.register_zone(zone).await;

        let mut character = Character::new(11);
        character.chara.max_hp = 300;
        character.chara.hp = 250; // alive
        character.base.current_main_state = crate::actor::MAIN_STATE_PASSIVE;
        character.base.zone_id = source_zone;
        character.chara.homepoint = 1_280_031; // Ul'dah CAP → zone 175.
        let handle = ActorHandle::new(11, ActorKindTag::Player, source_zone, 0, character);
        registry.insert(handle.clone()).await;

        let zone_arc = world.zone(source_zone).await.expect("zone registered");
        let outcome = apply_home_point_revive(11, &registry, &world, &zone_arc).await;

        match outcome {
            HomePointReviveOutcome::Warped { zone_id, .. } => assert_eq!(zone_id, 175),
            other => panic!("expected Warped, got {other:?}"),
        }
        let c = handle.character.read().await;
        // HP unchanged because apply_revive's MAIN_STATE_DEAD guard
        // skipped the restore branch — the warp still happened, but
        // the HP path is short-circuited (matches retail: a live
        // `/return` doesn't burst-heal).
        assert_eq!(c.chara.hp, 250);
        assert_eq!(c.base.zone_id, 175, "warped despite being alive");
    }
}
