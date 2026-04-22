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
            apply_recalc_stats(registry, *owner_actor_id).await;
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
    let base = battle_utils::attack_calculate_base_damage(&mut rng);

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
                Ok(()) => apply_recalc_stats(registry, *owner_actor_id).await,
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
                Ok(()) => apply_recalc_stats(registry, *owner_actor_id).await,
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

/// Recompute HP/MP pools from the modifier map, then — for Players — apply
/// Meteor's primary→secondary derivation (STR→Attack, VIT→Defense, etc.).
/// Dispatched from both `StatusEvent::RecalcStats` and the equip/unequip
/// arms of `dispatch_inventory_event`.
///
/// We do not broadcast HP/MP packets here yet. The modifier map is only
/// populated from the `characters_inventory_equipment` gear table in
/// follow-up work, so recomputed pools rarely differ from the pre-call
/// values. Add the delta-broadcast once equipment paramBonus summing
/// lands.
async fn apply_recalc_stats(registry: &ActorRegistry, actor_id: u32) {
    let Some(handle) = registry.get(actor_id).await else {
        return;
    };
    let is_player = handle.is_player();
    let mut c = handle.character.write().await;
    c.recalculate_stats();
    if is_player {
        c.apply_player_stat_derivation();
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
async fn die_if_defender_fell(
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
    let bnpc_class_id = if is_bnpc {
        let c = handle.character.read().await;
        c.chara.actor_class_id
    } else {
        0
    };

    apply_die(defender_actor_id, registry, world, zone).await;

    if !is_bnpc {
        return;
    }
    let Some(attacker_id) = attacker_actor_id else {
        return;
    };
    let Some(lua) = lua else {
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
