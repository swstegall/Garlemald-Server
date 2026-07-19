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
            // Persist the owner's active effects. `owner_actor_id` == the
            // character id in this server's lobby flow (handle_session_begin
            // sets `actor_id = session_id`), so it feeds
            // `save_player_status_effects` directly. Mirrors C#
            // `Database.SavePlayerStatusEffects`.
            if let Some(handle) = registry.get(*owner_actor_id).await {
                let entries = {
                    let c = handle.character.read().await;
                    c.status_effects.to_db_entries()
                };
                if let Err(e) = db
                    .save_player_status_effects(*owner_actor_id, &entries)
                    .await
                {
                    tracing::warn!(
                        owner = owner_actor_id,
                        err = %e,
                        "status: save_player_status_effects failed",
                    );
                }
            }
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
            // Buff/debuff countdown. Retail rides `statusShownTime[i]` on the
            // ActorPropertyPacketUtil bundle keyed by the target object path
            // `charaWork/status`, with the property named
            // `charaWork.statusShownTime[i]` (pmeteor
            // `StatusEffectContainer.SetTimeAtIndex`,
            // StatusEffectContainer.cs:389-392). We emit the equivalent as a
            // single 0x0137 SetActorProperty u32 write — the property id is the
            // Murmur2 hash of the indexed dotted name, matching the hashing the
            // shared `ActorPropertyPacketBuilder::add_int` uses. `expires_at`
            // is already stance-corrected (u32::MAX sentinel) by
            // `StatusEffect::wire_end_time`.
            let prop_name = format!("charaWork.statusShownTime[{slot_index}]");
            let prop_id = common::utils::murmur_hash2(&prop_name, 0);
            let sub = tx::build_set_actor_property_u32(
                *owner_actor_id,
                "charaWork/status",
                prop_id,
                *expires_at,
            );
            let bytes = sub.to_bytes();
            // Owner sees their own countdown; neighbours get the nameplate
            // buff-row time (companion to the PacketSetStatus icon arm).
            send_to_self_if_player(registry, world, *owner_actor_id, bytes.clone()).await;
            broadcast_to_neighbours(world, registry, *owner_actor_id, bytes).await;
        }
        StatusEvent::HpTick {
            owner_actor_id,
            delta,
        } => {
            if apply_hp_delta(registry, *owner_actor_id, *delta).await {
                sync_pools_after_tick(registry, world, *owner_actor_id).await;
            }
        }
        StatusEvent::MpTick {
            owner_actor_id,
            delta,
        } => {
            if apply_mp_delta(registry, *owner_actor_id, *delta).await {
                sync_pools_after_tick(registry, world, *owner_actor_id).await;
            }
        }
        StatusEvent::TpTick {
            owner_actor_id,
            delta,
        } => {
            if apply_tp_delta(registry, *owner_actor_id, *delta).await {
                sync_pools_after_tick(registry, world, *owner_actor_id).await;
            }
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
            play_gain_animation,
        } => {
            // "You gain/lose the effect of X." pmeteor packs this into a
            // CommandResult (owner.Id, textId, effectId | HitEffect flag) on
            // the battle-action container; we render the same text sheet via
            // the game-message builder with the effect id as a LuaParam. The
            // header source must be the always-present WorldMaster static
            // actor (the client dispatches game messages by header source —
            // routing through the player is the #28 crash path), and the text
            // owner is the WorldMaster system sheet.
            let params = [common::luaparam::LuaParam::UInt32(*status_effect_id)];
            let sub = tx::build_game_message_actor1_with_params(
                tx::WORLD_MASTER_ACTOR_ID,
                *owner_actor_id,
                tx::WORLD_MASTER_ACTOR_ID,
                *text_id,
                tx::MESSAGE_TYPE_SYSTEM,
                &params,
            );
            let bytes = sub.to_bytes();
            // The owner always sees the log line; neighbours only receive the
            // packet when the gain/lose animation should splash on nearby
            // clients (pmeteor's `playEffect` / `HitEffect.StatusEffectType`).
            send_to_self_if_player(registry, world, *owner_actor_id, bytes.clone()).await;
            if *play_gain_animation {
                broadcast_to_neighbours(world, registry, *owner_actor_id, bytes).await;
            }
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
            target_actor_id,
        } => {
            tracing::debug!(owner = owner_actor_id, kind = ?event_tag(event), "battle event");
            // 0x0195 SetEnmityIndicator — paint the mob's hate gem red
            // and lock it onto the player who pulled aggro. Decoded
            // from `ffxiv_traces/combat_skills.pcapng`; see
            // `captures/retail_pcap_gap_analysis.md`. Project Meteor
            // never emits this opcode so combat-side targeting in the
            // C# fork comes through entirely via 0x00DB SetActorTarget
            // (which drives only the in-world reticle, not the hate
            // gem on the nameplate).
            let sub = tx::actor_battle::build_set_enmity_indicator(
                *owner_actor_id,
                *target_actor_id,
                100,
            );
            broadcast_around_actor(world, registry, zone, *owner_actor_id, sub.to_bytes()).await;
            // Seed the hate container so `most_hated()` resolves to the target
            // we just engaged. Without this, the very next combat tick sees
            // `most_hated == None` and `should_deaggro` immediately disengages
            // the mob (controller.rs::do_combat_tick) — the wolf would swing
            // once and then drop out of combat. The auto-attack swing clock is
            // driven by the AttackState pushed in `internal_engage`; this entry
            // is what keeps the controller in its combat branch tick-to-tick.
            if let Some(handle) = registry.get(*owner_actor_id).await {
                let mut chara = handle.character.write().await;
                chara.hate.update_hate(*target_actor_id, 1);
            }
            // pmeteor re-broadcasts the State trio per NPC at engage time
            // (reference capture 15:54:31.414, one per combatant) — the
            // mode-change action that arms the client's combat
            // presentation for this actor. See `emit_change_state_trio`.
            emit_change_state_trio(
                *owner_actor_id,
                crate::actor::MAIN_STATE_ACTIVE,
                registry,
                world,
                zone,
            )
            .await;
            // Hostile engage flip: `npcWork.hateType = 2` (HATE_TYPE_ENGAGED)
            // — the engaged ORANGE nameplate tint, party-independent.
            // NEVER 3: the round-2 decomp of `DepictionJudge:
            // judgeNameplate()` showed 3 renders RED only when the mob's
            // party is the player party's occupancy group (which needs
            // the 0x0187 Set Occupancy Group claim wiring garlemald
            // doesn't emit); without the claim the client falls through
            // to the PURPLE "claimed by another party" tint (2026-07-01
            // retest). The judge RE-RUNS on every WorkSync — no latch —
            // so this live flip re-colors the plate; hateType drives
            // COLOR only (no HP gauge — `_setNameplateGauge` is a RET
            // 0x8 stub in 1.23b). Full table on
            // `build_npc_hate_type_packet`.
            // Allies/players skip — friendly nameplates stay passive.
            let is_hostile_bnpc = matches!(
                registry.get(*owner_actor_id).await.map(|h| h.kind),
                Some(crate::runtime::actor_registry::ActorKindTag::BattleNpc)
            );
            if is_hostile_bnpc {
                let sub = tx::actor::build_npc_hate_type_packet(
                    *owner_actor_id,
                    crate::npc::HATE_TYPE_ENGAGED,
                );
                send_to_self_if_player(registry, world, *owner_actor_id, sub.to_bytes()).await;
                broadcast_around_actor(world, registry, zone, *owner_actor_id, sub.to_bytes())
                    .await;
            }
        }
        BattleEvent::Disengage { owner_actor_id } => {
            tracing::debug!(owner = owner_actor_id, kind = ?event_tag(event), "battle event");
            // Clear the hate gem — (no target, hate=0).
            let sub = tx::actor_battle::build_set_enmity_indicator(
                *owner_actor_id,
                tx::actor_battle::NO_ENMITY_TARGET,
                0,
            );
            broadcast_around_actor(world, registry, zone, *owner_actor_id, sub.to_bytes()).await;
            // State trio back to passive. The helper's corpse guard
            // keeps a dead actor dead (the death path owns the DEAD
            // trio; overwriting it would stand the corpse back up).
            emit_change_state_trio(
                *owner_actor_id,
                crate::actor::MAIN_STATE_PASSIVE,
                registry,
                world,
                zone,
            )
            .await;
            // Claim release — hateType back to passive 1 (gauge off).
            if matches!(
                registry.get(*owner_actor_id).await.map(|h| h.kind),
                Some(crate::runtime::actor_registry::ActorKindTag::BattleNpc)
            ) {
                let sub = tx::actor::build_npc_hate_type_packet(*owner_actor_id, 1);
                broadcast_around_actor(world, registry, zone, *owner_actor_id, sub.to_bytes())
                    .await;
            }
            // 0x00DE ResetHead — stop the mob's head from continuing
            // to track the player's last position. Captured retail
            // pairs this with the gem clear at disengage. Project
            // Meteor leaves the head locked indefinitely; emitting
            // ResetHead returns the mob to its neutral pose.
            let head = tx::actor::build_reset_head(*owner_actor_id);
            broadcast_around_actor(world, registry, zone, *owner_actor_id, head.to_bytes()).await;
            // Drop all hate so `most_hated()` is None and the controller stays
            // in its non-combat branch — otherwise residual hate would re-arm
            // `do_combat_tick` on the next tick and the mob would re-engage.
            if let Some(handle) = registry.get(*owner_actor_id).await {
                let mut chara = handle.character.write().await;
                chara.hate.clear_hate(None);
            }
        }
        BattleEvent::Spawn { owner_actor_id }
        | BattleEvent::Despawn { owner_actor_id }
        | BattleEvent::RecalcStats { owner_actor_id } => {
            tracing::debug!(owner = owner_actor_id, kind = ?event_tag(event), "battle event");
        }
        BattleEvent::Die { owner_actor_id } => {
            // `BattleEvent::Die` carries no attacker (scripted / GM
            // `!die` paths don't need one). `die_if_defender_fell` is
            // where real combat deaths route through with an attacker
            // id for `onKillBNpc` dispatch.
            apply_die(*owner_actor_id, registry, world, zone, lua, db).await;
        }
        BattleEvent::Revive { owner_actor_id } => {
            apply_revive(*owner_actor_id, registry, world, zone).await;
        }
        BattleEvent::TargetChange {
            owner_actor_id,
            new_target_actor_id,
        } => {
            tracing::debug!(owner = owner_actor_id, "battle: target change");
            // Re-lock the hate gem onto the new target.
            let target = new_target_actor_id.unwrap_or(tx::actor_battle::NO_ENMITY_TARGET);
            let hate = if new_target_actor_id.is_some() {
                100
            } else {
                0
            };
            let sub = tx::actor_battle::build_set_enmity_indicator(*owner_actor_id, target, hate);
            broadcast_around_actor(world, registry, zone, *owner_actor_id, sub.to_bytes()).await;
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
                // X10 carries 10 rows max post-S0.6 (pmeteor capacity).
                let sub = if wire.len() - offset <= 10 {
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
        BattleEvent::CastStart {
            owner_actor_id,
            target_actor_id,
            command_id,
            cast_time_ms: _,
            cast_type,
        } => {
            // #28 S2.4 — pmeteor `MagicState.OnStart` (MagicState.cs:61-105):
            // chant glow on (0x0144 chantId 0xF0) + the "You begin
            // casting" line as a 0x0139 whose animation is
            // `0x6F000000 | castType` (6F000002 = BLM/THM, 6F000003 =
            // WHM, 6F000008 = BRD), text 30128, param 1. The anim value
            // is self-consistent with the seed's castType but visually
            // unverified — flagged for the live pass.
            let chant = tx::actor::build_set_actor_sub_state(*owner_actor_id, 0, 0xF0, 0, 0, 0, 0);
            broadcast_around_actor(world, registry, zone, *owner_actor_id, chant.to_bytes()).await;
            let row = tx::actor_battle::CommandResult {
                target_id: *target_actor_id,
                worldmaster_text_id: 30128,
                param: 1,
                hit_num: 1,
                ..Default::default()
            };
            let begin = tx::actor_battle::build_command_result_x01(
                *owner_actor_id,
                0x6F00_0000 | *cast_type as u32,
                *command_id,
                &row,
            );
            broadcast_around_actor(world, registry, zone, *owner_actor_id, begin.to_bytes()).await;
        }
        BattleEvent::CastComplete { owner_actor_id, .. }
        | BattleEvent::CastInterrupted { owner_actor_id, .. } => {
            // Chant glow off (pmeteor `MagicState.Cleanup`: chantId = 0).
            // Completion damage rides the separate ResolveAction event.
            let sub = tx::actor::build_set_actor_sub_state(*owner_actor_id, 0, 0, 0, 0, 0, 0);
            broadcast_around_actor(world, registry, zone, *owner_actor_id, sub.to_bytes()).await;
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
        BattleEvent::AutoAttackOutOfRange { owner_actor_id } => {
            // pmeteor AttackState.cs:222-229: a range-blocked player
            // auto-attack answers with worldMaster text 32537 "The
            // target is too far away." — player-only (the C# gates on
            // `owner is Player`; `send_to_self_if_player` is our
            // equivalent), once per swing-delay window (the AIContainer
            // re-arms before emitting). (#199 round 2.)
            let sub = tx::build_game_message_actor1(
                tx::WORLD_MASTER_ACTOR_ID,
                *owner_actor_id,
                tx::WORLD_MASTER_ACTOR_ID,
                32537,
                tx::MESSAGE_TYPE_SYSTEM,
            );
            send_to_self_if_player(registry, world, *owner_actor_id, sub.to_bytes()).await;
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
            // #28 S3.3 — defender TP on damage taken (pmeteor
            // `Character.OnDamageTaken`, Character.cs:905-908):
            // ceil(5 · e^(−0.0667 · level) · damage). The dead bank
            // nothing (pmeteor `AddTP` IsAlive gate).
            if d.is_alive() {
                let gain = (5.0 * (-0.0667 * f64::from(def_level)).exp() * f64::from(dr.amount))
                    .ceil() as i32;
                d.add_tp(gain);
            }
        }
    }

    // #28 S3.3 — attacker TP per landed swing (pmeteor
    // `Character.OnAttack`, Character.cs:886-895): delay-seconds × 100.
    // StoreTp omitted — no lvl-1 sources. Misses bank nothing.
    let landed = dmg_request.map_or(0, |dr| dr.amount);
    let attacker_tp_after = if landed > 0 {
        let mut a = attacker_handle.character.write().await;
        let delay_ms = a.get_attack_delay_ms();
        a.add_tp((f64::from(delay_ms) / 1000.0 * 100.0) as i32);
        a.chara.tp
    } else {
        0
    };
    // Tutorial milestone signals (pmeteor parity): "playerAttack" fires
    // at the end of every player swing (hit or miss — Player.OnAttack);
    // "tpOver1000" whenever an accrual leaves the player at/above 1000
    // (Character.AddTP). Both no-op without an active content script.
    if attacker_handle.is_player() {
        fire_content_signal(attacker_actor_id, "playerAttack", registry, world, lua, db).await;
        if attacker_tp_after >= 1000 {
            fire_content_signal(attacker_actor_id, "tpOver1000", registry, world, lua, db).await;
        }
    }
    // Defender-side accrual can also cross the threshold (the player
    // taking wolf hits banks TP too — pmeteor OnDamageTaken → AddTP).
    if defender_handle.is_player() {
        let tp = defender_handle.character.read().await.chara.tp;
        if tp >= 1000 {
            fire_content_signal(defender_actor_id, "tpOver1000", registry, world, lua, db).await;
        }
    }
    // Combat-resolution log — before this line existed, the entire
    // damage pipeline was wire-only (zero log vocabulary for swings /
    // damage / TP), which is why the live SEQ_005 failures took a
    // packet-capture diff to localise. One line per resolved swing.
    {
        let (def_hp, def_max) = {
            let d = defender_handle.character.read().await;
            (d.chara.hp, d.chara.max_hp)
        };
        tracing::debug!(
            attacker = format!("0x{attacker_actor_id:08X}"),
            defender = format!("0x{defender_actor_id:08X}"),
            damage = landed,
            defender_hp = format!("{def_hp}/{def_max}"),
            attacker_tp = attacker_tp_after,
            "battle: auto-attack resolved",
        );
    }

    // Swing animation per attacker kind — pmeteor `AttackState.cs:141-146`:
    // `(25 << 24 | 1 << 12)` for players, `(17 << 24 | 1 << 12)` for NPCs.
    // Retail confirms the player value (combat_skills.pcapng record #3,
    // anim 0x19001000); the NPC value is pmeteor-source INFERENCE flagged
    // for the live A/B (#28 S0.6).
    let battle_animation = if attacker_handle.is_player() {
        0x1900_1000
    } else {
        0x1100_1000
    };
    broadcast_results(
        attacker_actor_id,
        battle_animation,
        AUTO_ATTACK_COMMAND_ID,
        &container.main_results,
        registry,
        world,
        zone,
    )
    .await;

    // #28 S3.3 — HP/TP wire sync: the defender's nameplate/roster HP bar
    // and the attacker's TP gauge only move when a fresh
    // `charaWork/stateAtQuicklyForAll` lands; without it the init
    // bundle's values go stale on the first hit.
    if landed > 0 {
        broadcast_state_quickly(registry, world, zone, defender_actor_id).await;
        broadcast_state_quickly(registry, world, zone, attacker_actor_id).await;
    }

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

#[allow(clippy::too_many_arguments)]
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
    // The old flat-100 placeholder one-shot the SEQ_005 tutorial wolves
    // (100 HP fallback) on every Papalymo Thunder — each wolf died to a
    // single cast ~8 s apart and the whole fight was over in 18 s. Scale
    // the placeholder by attacker level so a lvl-1 tutorial caster lands
    // ~20 while higher-level content still produces a visible number.
    result.amount = if command.base_potency > 0 {
        command.base_potency
    } else {
        (15 + 5 * i32::from(atk_level.max(1))).min(9999) as u16
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
            d_write
                .hate
                .update_hate(attacker_actor_id, dr.enmity as i32);
            // #28 S3.3 — defender TP on damage taken, same formula as
            // the auto-attack arm (pmeteor `OnDamageTaken` runs for any
            // landed damage, Character.cs:905-908).
            if d_write.is_alive() {
                let gain = (5.0 * (-0.0667 * f64::from(def_level)).exp() * f64::from(dr.amount))
                    .ceil() as i32;
                d_write.add_tp(gain);
            }
        }
    }

    // #28 S3.3 — player completion bookkeeping: costs come off the
    // pools, the used hotbar slot starts its recast (in-memory +
    // persisted via the equip-ability upsert), and the
    // `charaWork/commandDetailForSelf` pair reaches the owning client.
    // pmeteor splits this across `DoBattleCommand` (DelMP/DelTP,
    // Character.cs:1159-1160) and `Player.OnWeaponSkill/OnAbility/
    // OnCast` (`UpdateHotbarTimer`, Player.cs:2901-2928).
    let landed = dmg_request.map_or(0, |dr| dr.amount);
    if attacker_handle.is_player() {
        let now_unix = common::utils::unix_timestamp() as u32;
        let recast_end = now_unix.saturating_add(command.max_recast_time_seconds);
        let (class_id, slot0) = {
            let mut a = attacker_handle.character.write().await;
            if command.mp_cost != 0 {
                a.add_mp(-i32::from(command.mp_cost));
            }
            if command.tp_cost != 0 {
                a.add_tp(-i32::from(command.tp_cost));
            }
            // Abilities accrue TP like swings (pmeteor `OnAttack` gate:
            // AutoAttack | Ability, non-miss); WS/spells gain none.
            if command.command_type == CommandType::ABILITY && landed > 0 {
                let delay_ms = a.get_attack_delay_ms();
                a.add_tp((f64::from(delay_ms) / 1000.0 * 100.0) as i32);
            }
            let class_id = a.chara.class.max(0) as u8;
            let slot0 = a
                .chara
                .hotbar
                .iter_mut()
                .find(|e| (e.command_id & 0xFFFF) as u16 == command.id)
                .map(|e| {
                    e.recast_time = recast_end;
                    e.hotbar_slot
                });
            (class_id, slot0)
        };
        if let Some(slot0) = slot0 {
            if let Some(db) = db
                && let Err(e) = db
                    .equip_ability(
                        attacker_actor_id,
                        class_id,
                        slot0,
                        u32::from(command.id),
                        recast_end,
                    )
                    .await
            {
                tracing::warn!(
                    player = attacker_actor_id,
                    command = command.id,
                    err = %e,
                    "resolve_action: recast persist failed",
                );
            }
            for sub in tx::actor::build_hotbar_recast_update(
                attacker_actor_id,
                slot0,
                command.max_recast_time_seconds as u16,
                recast_end,
            ) {
                // Self-only; the helper stamps the session id (proxy rule).
                send_to_self_if_player(registry, world, attacker_actor_id, sub.to_bytes()).await;
            }
        }
    }

    // Combat-resolution log (companion to the auto-attack line).
    {
        let (def_hp, def_max) = {
            let d = defender_handle.character.read().await;
            (d.chara.hp, d.chara.max_hp)
        };
        tracing::debug!(
            attacker = format!("0x{attacker_actor_id:08X}"),
            defender = format!("0x{defender_actor_id:08X}"),
            command = command.id,
            amount = landed,
            heal = is_heal,
            defender_hp = format!("{def_hp}/{def_max}"),
            "battle: action resolved",
        );
    }

    // Tutorial milestone signal: "weaponskillUsed" fires when a player's
    // weaponskill resolves (pmeteor `WeaponSkillState.OnComplete`).
    if attacker_handle.is_player() && command.command_type == CommandType::WEAPON_SKILL {
        fire_content_signal(
            attacker_actor_id,
            "weaponskillUsed",
            registry,
            world,
            lua,
            db,
        )
        .await;
    }

    broadcast_results(
        attacker_actor_id,
        command.battle_animation,
        command.id, // raw catalog id — retail stamps it on every row
        &container.main_results,
        registry,
        world,
        zone,
    )
    .await;

    // #28 S3.3 — HP/TP wire sync (see the auto-attack arm). The attacker
    // re-sync is unconditional for players: costs drain even on a miss.
    if landed > 0 || is_heal {
        broadcast_state_quickly(registry, world, zone, defender_actor_id).await;
    }
    if attacker_handle.is_player() {
        broadcast_state_quickly(registry, world, zone, attacker_actor_id).await;
    }

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

/// pmeteor `CommandResultX01PacketCommand.Attack` — the synthetic
/// command id retail stamps on every auto-attack result row
/// (`/Command/Game/AttackCommand`; byte-confirmed in
/// `combat_skills.pcapng` record #3, cmd 0x5658).
pub(crate) const AUTO_ATTACK_COMMAND_ID: u16 = 22104;

async fn broadcast_results(
    source_actor_id: u32,
    battle_animation: u32,
    command_id: u16,
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
        // pmeteor container choice: X01 for the single-row case (what
        // retail ships per skill press / swing), X10 up to 10 rows,
        // X18 beyond.
        let remaining = wire.len() - offset;
        let sub = if remaining == 1 {
            let row = &wire[offset];
            offset += 1;
            tx::actor_battle::build_command_result_x01(
                source_actor_id,
                battle_animation,
                command_id,
                row,
            )
        } else if remaining <= 10 {
            tx::actor_battle::build_command_result_x10(
                source_actor_id,
                battle_animation,
                command_id,
                &wire,
                &mut offset,
            )
        } else {
            tx::actor_battle::build_command_result_x18(
                source_actor_id,
                battle_animation,
                command_id,
                &wire,
                &mut offset,
            )
        };
        let bytes = sub.to_bytes();
        // The acting player must SEE their own result —
        // `broadcast_around_actor` excludes the source (same bug class
        // as the draw trio, quest_apply.rs). The helper stamps the
        // session id into the target_id (proxy drop rule). (#28 S0.6.)
        send_to_self_if_player(registry, world, source_actor_id, bytes.clone()).await;
        broadcast_around_actor(world, registry, zone, source_actor_id, bytes).await;
    }
}

/// One `charaWork/stateAtQuicklyForAll` bundle for the actor's current
/// pools — self-send (the player's own HP/MP/TP gauges) + in-zone
/// broadcast (nameplate + content-roster HP bars). pmeteor drives the
/// same wire from `Character.PostUpdate`'s `HpTpMp` update flag; the
/// resolve fns call this after any pool change. (#28 S3.3.)
async fn broadcast_state_quickly(
    registry: &ActorRegistry,
    world: &WorldManager,
    zone: &Arc<RwLock<Zone>>,
    actor_id: u32,
) {
    let Some(handle) = registry.get(actor_id).await else {
        return;
    };
    let (hp, hp_max, mp, mp_max, tp) = {
        let c = handle.character.read().await;
        (
            c.chara.hp.max(0) as u16,
            c.chara.max_hp.max(0) as u16,
            c.chara.mp.max(0) as u16,
            c.chara.max_mp.max(0) as u16,
            c.chara.tp,
        )
    };
    for sub in tx::actor::build_chara_state_at_quickly_for_all(actor_id, hp, hp_max, mp, mp_max, tp)
    {
        let bytes = sub.to_bytes();
        send_to_self_if_player(registry, world, actor_id, bytes.clone()).await;
        broadcast_around_actor(world, registry, zone, actor_id, bytes).await;
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
            for sub in
                tx::actor_inventory::build_mass_set_item_modifier_frame(*owner_actor_id, items)
            {
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
            // Per-recipient target stamp — untargeted subpackets are
            // dropped by the world-server proxy fan-out.
            let payload = tx::build_set_weather(*area_id, *weather_id, *transition_time).to_bytes();
            if let (Some(aid), false) = (target_actor_id, zone_wide) {
                if let Some(handle) = registry.get(*aid).await
                    && let Some(client) = world.client(handle.session_id).await
                {
                    let mut bytes = payload;
                    common::subpacket::SubPacket::stamp_target_id_if_zero(
                        &mut bytes,
                        handle.session_id,
                    );
                    client.send_bytes(bytes).await;
                }
            } else {
                // Zone-wide — queue to every player in this zone.
                let players = registry.actors_in_zone(*area_id).await;
                for p in players {
                    if !p.is_player() {
                        continue;
                    }
                    if let Some(client) = world.client(p.session_id).await {
                        let mut bytes = payload.clone();
                        common::subpacket::SubPacket::stamp_target_id_if_zero(
                            &mut bytes,
                            p.session_id,
                        );
                        client.send_bytes(bytes).await;
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
pub(crate) async fn spawn_bundle_fanout(
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

pub(crate) async fn broadcast_to_neighbours(
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

// The pool appliers report whether the value actually moved — clamping
// at full HP / empty TP returns `false` so idle-at-rest actors produce
// zero wire traffic from the regen tick.

async fn apply_hp_delta(registry: &ActorRegistry, actor_id: u32, delta: i32) -> bool {
    let Some(handle) = registry.get(actor_id).await else {
        return false;
    };
    let mut c = handle.character.write().await;
    let before = c.chara.hp;
    c.add_hp(delta);
    c.chara.hp != before
}

async fn apply_mp_delta(registry: &ActorRegistry, actor_id: u32, delta: i32) -> bool {
    let Some(handle) = registry.get(actor_id).await else {
        return false;
    };
    let mut c = handle.character.write().await;
    let before = c.chara.mp;
    c.add_mp(delta);
    c.chara.mp != before
}

async fn apply_tp_delta(registry: &ActorRegistry, actor_id: u32, delta: i32) -> bool {
    let Some(handle) = registry.get(actor_id).await else {
        return false;
    };
    let mut c = handle.character.write().await;
    let before = c.chara.tp;
    c.add_tp(delta);
    c.chara.tp != before
}

/// Pool-tick → client sync. The Hp/Mp/TpTick arms previously mutated
/// memory only, leaving the client's gauges frozen until an unrelated
/// pool broadcast fired. Resolve the owner's zone and ride the same
/// `charaWork/stateAtQuicklyForAll` bundle the battle resolve fns use
/// (pmeteor `Character.PostUpdate`'s HpTpMp update flag).
async fn sync_pools_after_tick(registry: &ActorRegistry, world: &WorldManager, actor_id: u32) {
    let Some(handle) = registry.get(actor_id).await else {
        return;
    };
    let Some(zone) = world.zone(handle.zone_id).await else {
        return;
    };
    broadcast_state_quickly(registry, world, &zone, actor_id).await;
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
/// Set an actor's main state and broadcast pmeteor's State-flag flush
/// TRIO: `0x0134 SetActorState` + `0x013C CommandResultX00` (anim
/// 0x72000062) + `0x0139 CommandResultX01` (anim 0x7C000062, command
/// 21001 Activate, one self-hit). The 1.x client keys its per-actor
/// combat presentation — draw/sheathe animation AND whether subsequent
/// battle actions/HP/death states are *applied visually* — off this
/// mode-change action, not the bare 0x0134. The live SEQ_005 run proved
/// it: every damage/HP/death packet reached the client (ws2_32 trace)
/// yet none rendered, and the one delta vs pmeteor's reference capture
/// is that pmeteor re-broadcasts this trio per NPC at engage while
/// garlemald shipped bare 0x0134s. Mirrors `quest_apply::
/// apply_change_state` (the player F-press path, which fixed the
/// identical invisible-draw bug in round 6). (Garlemald-Server #28.)
pub(crate) async fn emit_change_state_trio(
    actor_id: u32,
    main_state: u16,
    registry: &ActorRegistry,
    world: &WorldManager,
    zone: &Arc<RwLock<Zone>>,
) {
    let Some(handle) = registry.get(actor_id).await else {
        return;
    };
    {
        let mut c = handle.character.write().await;
        // A corpse must stay a corpse: a stale Engage/Disengage event
        // racing a death would otherwise stand the actor back up
        // client-side. Revival transitions out of DEAD via
        // `apply_revive`, which doesn't route through this helper.
        if c.base.current_main_state == crate::actor::MAIN_STATE_DEAD
            && main_state != crate::actor::MAIN_STATE_DEAD
        {
            return;
        }
        c.base.current_main_state = main_state;
        c.chara.new_main_state = main_state;
    }
    // Pre-warp content NPCs: keep the stored-state mutation, suppress
    // the wire broadcast — the client hasn't been sent those actors yet
    // (their spawn fan-out is deferred to the zone-in bundle, which
    // replays the stored state). Engages can't normally fire pre-warp
    // (the content script's battleStarted latch needs an engaged
    // player), so on those paths this is belt-and-braces parity with
    // the Lua ChangeState path that has always enforced it.
    for snap in world.all_sessions().await {
        if let Some(active) = snap.active_content_script.as_ref()
            && !active.warp_complete
            && snap
                .transient_director_members
                .get(&active.director_actor_id)
                .is_some_and(|roster| roster.contains(&actor_id))
        {
            tracing::debug!(
                actor = format!("0x{actor_id:08X}"),
                main_state,
                "state trio stored; broadcast suppressed (pre-warp content roster)",
            );
            return;
        }
    }
    let state_sub = tx::build_set_actor_state(actor_id, (main_state & 0xFF) as u8, 0);
    let x00 = tx::actor_battle::build_command_result_x00(actor_id, 0x7200_0062, 0);
    let x01 = tx::actor_battle::build_command_result_x01(
        actor_id,
        0x7C00_0062,
        21001,
        &tx::actor_battle::CommandResult {
            target_id: actor_id,
            hit_num: 1,
            hit_effect: 1,
            ..Default::default()
        },
    );
    let mut bytes = state_sub.to_bytes();
    bytes.extend(x00.to_bytes());
    bytes.extend(x01.to_bytes());
    send_to_self_if_player(registry, world, actor_id, bytes.clone()).await;
    let recipients = broadcast_around_actor(world, registry, zone, actor_id, bytes).await;
    tracing::debug!(
        actor = format!("0x{actor_id:08X}"),
        main_state,
        recipients,
        "state trio applied (0x134 + X00/X01)",
    );
}

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

    apply_die(defender_actor_id, registry, world, zone, lua, db).await;

    if !is_bnpc {
        return;
    }
    // #28 S4.1 — the content-area kill gate fires from `apply_die`'s tail
    // (which this function just called with the same lua/db pair), so it
    // already covered this death — including ally killing blows, whose
    // attacker the gates below reject. A second call here was a proven
    // duplicate dispatch (today's live log fired the gate twice for the
    // third wolf, the second a resumed=0 no-op) and would double-resume
    // the director if the script ever re-parks on "battleComplete".
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

    // Kill EXP — pmeteor pays it in the same death batch
    // (`BattleNpc.cs:376` → `BattleUtils.AddBattleBonusEXP`), and it
    // likewise fires before the quest-hook chain so `onKillBNpc` sees
    // the post-award EXP state. Tutorial-content kills are gated off
    // inside (their Lua already self-pays a fixed grant).
    award_battle_exp(
        &attacker_handle,
        defender_actor_id,
        mob_level,
        registry,
        world,
        zone,
        db_arc,
        lua,
    )
    .await;

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

/// #28 S4.1 — the all-hostiles-dead gate for scripted content (the
/// SEQ_005 combat tutorial). Scans sessions for the one whose active
/// content roster (`transient_director_members[director]`) contains the
/// dead actor, counts roster members that are still-live hostiles
/// (`kind == BattleNpc` — Yda/Papalymo register as `Ally`, so only wolf
/// deaths move the count; `apply_die` flips the dying wolf to DEAD
/// before this runs, so it counts itself out), and on zero live fires
/// `SendSignal("battleComplete")` through
/// `quest_apply::apply_event_script_commands` with the owning player's
/// handle — byte-for-byte the same routing as the working F-press
/// `playerActive` resume, so the director's continuation (success
/// widgets → processEvent020_1 → StartSequence(10) → ContentFinished →
/// DoZoneChange) runs in the death-path call stack with full wire
/// access. Firing the signal with nothing parked is a logged no-op, and
/// the caller-side death-transition guards keep this single-fire per
/// death; signal scoping per player is deferred (single-player server —
/// `"battleComplete:" .. actorId` is the trivial multi-session upgrade).
/// Fire a named scheduler signal through the event bridge for a player
/// who owns an active content script — the tutorial-milestone twin of
/// `check_content_battle_complete`'s battleComplete dispatch. pmeteor's
/// emitters: `Player.OnAttack` → "playerAttack" (every swing),
/// `Character.AddTP` → "tpOver1000" (every accrual past 1000),
/// `WeaponSkillState.OnComplete` → "weaponskillUsed". Repeat fires with
/// nothing parked are logged no-ops (waitForSignal de-registers), and
/// the active-content gate keeps open-world combat off the scheduler.
/// (Garlemald-Server #28, tutorial tooltip chain.)
pub(crate) async fn fire_content_signal(
    player_actor_id: u32,
    signal: &str,
    registry: &ActorRegistry,
    world: &WorldManager,
    lua: Option<&Arc<crate::lua::LuaEngine>>,
    db: Option<&Arc<crate::database::Database>>,
) {
    let (Some(lua), Some(db)) = (lua, db) else {
        return;
    };
    let Some(handle) = registry.get(player_actor_id).await else {
        return;
    };
    if !handle.is_player() || handle.session_id == 0 {
        return;
    }
    let Some(session) = world.session(handle.session_id).await else {
        return;
    };
    if session.active_content_script.is_none() {
        return;
    }
    crate::runtime::quest_apply::apply_event_script_commands(
        &handle,
        vec![crate::lua::command::LuaCommand::SendSignal {
            name: signal.into(),
        }],
        registry,
        db,
        world,
        Some(lua),
    )
    .await;
}

pub(crate) async fn check_content_battle_complete(
    dead_actor_id: u32,
    registry: &ActorRegistry,
    world: &WorldManager,
    db: &Arc<crate::database::Database>,
    lua: &Arc<crate::lua::LuaEngine>,
) {
    for session in world.all_sessions().await {
        let Some(active) = session.active_content_script.as_ref() else {
            continue;
        };
        let Some(roster) = session
            .transient_director_members
            .get(&active.director_actor_id)
        else {
            continue;
        };
        if !roster.contains(&dead_actor_id) {
            continue;
        }
        let mut live = 0u32;
        for &member_id in roster {
            let Some(member) = registry.get(member_id).await else {
                continue;
            };
            if !matches!(
                member.kind,
                crate::runtime::actor_registry::ActorKindTag::BattleNpc,
            ) {
                continue;
            }
            let c = member.character.read().await;
            if c.base.current_main_state != crate::actor::MAIN_STATE_DEAD {
                live += 1;
            }
        }
        if live > 0 {
            return;
        }
        let Some(player) = registry.by_session(session.id).await else {
            return;
        };
        tracing::info!(
            session = session.id,
            dead = format!("0x{dead_actor_id:08X}"),
            director = format!("0x{:08X}", active.director_actor_id),
            "content battle complete — firing battleComplete through the event bridge",
        );
        crate::runtime::quest_apply::apply_event_script_commands(
            &player,
            vec![crate::lua::command::LuaCommand::SendSignal {
                name: "battleComplete".into(),
            }],
            registry,
            db,
            world,
            Some(lua),
        )
        .await;
        return;
    }
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

/// Tutorial content scripts that pay fixed kill EXP from their own Lua
/// `onUpdate` (`owner:AddExp(1000/3000, …)` — SimpleContent30010.lua:132,
/// SimpleContent30002.lua:123, SimpleContent30079.lua:112). The generic
/// death-path award skips kills made under these to avoid a double-pay;
/// every other content script — notably the man0l1 escort
/// (`SimpleContentMan0l101`), whose retail footage shows 601/625 EXP per
/// ankle biter — defers to the generic path.
const SELF_PAYING_CONTENT_SCRIPTS: [&str; 3] = [
    "SimpleContent30010", // Gridania opener wolves
    "SimpleContent30002", // Limsa opener aurelias
    "SimpleContent30079", // Ul'dah opener goobbue
];

/// Kill-EXP award — port of pmeteor's death-path payout
/// (`BattleNpc.cs:363-385` → `BattleUtils.cs:AddBattleBonusEXP`):
///
///   1. the 30108 "<attacker> defeats <target>." row (BattleNpc.cs:364),
///   2. on a running EXP chain, the 33919 "EXP chain N!" row with the
///      next window in seconds (BattleUtils.cs:1001),
///   3. base EXP from [`crate::battle::exp::base_kill_exp`] with the
///      chain bonus folded in like the Lua `AddExp` binding
///      (`exp += ceil(exp*bonus/100)`), riding the shared
///      `apply_add_exp` → `emit_exp_property_updates` pipeline which
///      renders "You earn N experience points." / "You attain level N."
///      and persists everything.
///
/// Chain state is the hidden EXPChain status (id 300004, seed
/// 032_server_statuseffects.sql:209) whose `tier` counts qualifying
/// kills; the refresh stores `tier + 1` with the *previous* tier's
/// window as duration (BattleUtils.cs:995-1005). EXP goes to the
/// killer's CURRENT class — pmeteor `attacker.GetClass()`. Link bonus
/// stays 0 until LinkCount mob-mod plumbing lands; party distribution
/// (0.667 modifier paid to every member) is deferred with real parties
/// — the killer is paid solo.
#[allow(clippy::too_many_arguments)]
async fn award_battle_exp(
    attacker_handle: &crate::runtime::actor_registry::ActorHandle,
    defender_actor_id: u32,
    mob_level: i16,
    registry: &ActorRegistry,
    world: &WorldManager,
    zone: &Arc<RwLock<Zone>>,
    db: &Arc<crate::database::Database>,
    lua: Option<&Arc<crate::lua::LuaEngine>>,
) {
    if attacker_handle.session_id != 0
        && let Some(session) = world.session(attacker_handle.session_id).await
        && let Some(active) = session.active_content_script.as_ref()
        && SELF_PAYING_CONTENT_SCRIPTS.contains(&active.content_script.as_str())
    {
        tracing::debug!(
            attacker = attacker_handle.actor_id,
            script = %active.content_script,
            "kill EXP skipped — tutorial content script self-pays",
        );
        return;
    }
    let (player_level, class_id) = {
        let c = attacker_handle.character.read().await;
        (c.chara.level, c.chara.class.clamp(0, u8::MAX as i16) as u8)
    };
    let base = crate::battle::exp::base_kill_exp(player_level, mob_level, 1) as i32;
    if base == 0 {
        // dlvl <= -20: no EXP and no message (BattleUtils.cs:876).
        return;
    }

    // EXP chain — an existing (unexpired) EXPChain effect sets the
    // chain number; the refresh below stores tier+1 with the previous
    // tier's time window as the new duration.
    let now_ms = (common::utils::unix_timestamp() as u64).saturating_mul(1000);
    let now_s = (now_ms / 1000) as u32;
    let mut status_outbox = crate::status::StatusOutbox::new();
    let (chain_tier, chain_window_s) = {
        let mut c = attacker_handle.character.write().await;
        let tier = c
            .status_effects
            .get(crate::status::ids::STATUS_EXP_CHAIN)
            // The ticker sweeps expired effects lazily — a stale entry
            // is "no chain".
            .filter(|e| e.end_time > now_s)
            .map(|e| e.tier)
            .unwrap_or(0);
        let window_s = crate::battle::exp::chain_time_limit_s(tier) as u32;
        let mut chain = crate::status::StatusEffect::new(
            attacker_handle.actor_id,
            crate::status::ids::STATUS_EXP_CHAIN,
            0.0,
            0,
            window_s,
            tier.saturating_add(1),
            now_ms,
        );
        // Mirror the seed template row (032_server_statuseffects.sql:209
        // — flags 273, overwrite Always, hidden, silent both ways): the
        // chain marker is server-side bookkeeping, nothing rides the
        // wire for it.
        chain.name = "expchain".into();
        chain.flags = crate::status::StatusEffectFlags::from(273);
        chain.overwrite = crate::status::StatusEffectOverwrite::Always;
        chain.hidden = true;
        chain.silent_on_gain = true;
        chain.silent_on_loss = true;
        c.status_effects.add_status_effect(
            chain,
            attacker_handle.actor_id,
            now_ms,
            crate::status::DEFAULT_GAIN_TEXT_ID,
            &mut status_outbox,
        );
        (tier, window_s)
    };
    let catalogs = lua
        .map(|e| e.catalogs().clone())
        .unwrap_or_else(|| std::sync::Arc::new(crate::lua::Catalogs::new()));
    for event in status_outbox.drain() {
        dispatch_status_event(&event, registry, world, db, &catalogs).await;
    }

    // pmeteor packs these rows into the killing blow's actionContainer
    // (`CommandResultContainer.CombineLists`); garlemald resolves the
    // death after the attack batch has shipped, so they ride their own
    // X01/X10 container right behind it.
    let mut rows = vec![tx::actor_battle::CommandResult {
        target_id: defender_actor_id,
        worldmaster_text_id: 30108, // "<attacker> defeats <target>."
        hit_num: 1,
        ..Default::default()
    }];
    if chain_tier > 0 {
        rows.push(tx::actor_battle::CommandResult {
            target_id: attacker_handle.actor_id,
            worldmaster_text_id: 33919, // "EXP chain N!" — amount = tier
            amount: chain_tier as u32,
            param: chain_window_s, // window (s) for the next link
            hit_num: 1,
            ..Default::default()
        });
    }
    let mut offset = 0usize;
    while offset < rows.len() {
        let remaining = rows.len() - offset;
        let sub = if remaining == 1 {
            let row = &rows[offset];
            offset += 1;
            tx::actor_battle::build_command_result_x01(attacker_handle.actor_id, 0, 0, row)
        } else {
            tx::actor_battle::build_command_result_x10(
                attacker_handle.actor_id,
                0,
                0,
                &rows,
                &mut offset,
            )
        };
        let bytes = sub.to_bytes();
        send_to_self_if_player(registry, world, attacker_handle.actor_id, bytes.clone()).await;
        broadcast_around_actor(world, registry, zone, attacker_handle.actor_id, bytes).await;
    }

    let total_bonus = (crate::battle::exp::link_bonus(0) as u32)
        .saturating_add(crate::battle::exp::chain_bonus(chain_tier) as u32)
        .min(u8::MAX as u32) as u8;
    let total_exp = crate::battle::exp::with_bonus(base, total_bonus);
    crate::runtime::quest_apply::apply_add_exp(
        attacker_handle.actor_id,
        class_id,
        total_exp,
        registry,
        db,
        Some(world),
        lua,
    )
    .await;
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
    let current = db.get_seals(member_handle.actor_id, gc).await.unwrap_or(0);
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
/// owner, then purge every status effect tagged `LoseOnDeath` and drain
/// the resulting wire/save/recalc events so the client-side icon strip
/// clears in lock-step with the death animation. The `lua` / `db` args
/// are optional so synthetic test paths can still call apply_die
/// without standing up the full engine; when either is absent the
/// status removal still mutates the in-memory container but the wire
/// emission and DbSave events are dropped (matching the existing test
/// posture for the rest of the dispatcher).
pub(crate) async fn apply_die(
    owner_actor_id: u32,
    registry: &ActorRegistry,
    world: &WorldManager,
    zone: &Arc<RwLock<Zone>>,
    lua: Option<&Arc<crate::lua::LuaEngine>>,
    db: Option<&Arc<crate::database::Database>>,
) {
    let Some(handle) = registry.get(owner_actor_id).await else {
        return;
    };
    let mut status_outbox = crate::status::StatusOutbox::new();
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
        // Purge status effects flagged LoseOnDeath. Mirrors Meteor's
        // `Character.cs` death tail (the `RemoveStatusEffectsByFlags`
        // call that lives in `DeathState.OnStart`). The outbox is drained
        // after the lock is released so dispatch_status_event can
        // re-acquire the registry handle for packet emission.
        c.status_effects.remove_by_flag(
            crate::status::StatusEffectFlags::LOSE_ON_DEATH,
            &mut status_outbox,
        );
        // Drop the dead actor's own hate so a respawned NPC starts fresh.
        c.hate.clear_hate(None);
    }

    // Disengage every OTHER actor that was targeting (or hated) the now-dead
    // actor. Without this, an attacker's `AttackState` keeps `Continue`-ing on
    // the corpse — `ai_container.is_engaged()` stays true and `most_hated()`
    // keeps pointing at the dead id, so `should_deaggro` never fires and the
    // attacker is permanently combat-locked. For SEQ_005 this is load-bearing:
    // the content `onUpdate` re-engages an ally onto the next wolf only
    // `if not ally:IsEngaged()`, so a stale AttackState on the first dead wolf
    // would stop the ally ever helping with wolves 2 and 3. Mobs locked onto a
    // dead player would likewise never return to neutral. (Garlemald #28.)
    let zone_id = handle.zone_id;
    for other in registry.actors_in_zone(zone_id).await {
        if other.actor_id == owner_actor_id {
            continue;
        }
        let mut oc = other.character.write().await;
        let targets_dead = oc
            .ai_container
            .current_state()
            .map(|s| s.target_actor_id == owner_actor_id)
            .unwrap_or(false);
        // `clear_hate(Some(id))` is idempotent — a no-op when no entry exists.
        oc.hate.clear_hate(Some(owner_actor_id));
        if targets_dead {
            // Clear the swing state so `is_engaged()` flips false; the next
            // controller tick re-engages the next live `most_hated` (if any)
            // via the retaliation branch, or goes idle.
            oc.ai_container.clear_states();
        }
    }

    // DEAD-state trio — pmeteor's `DeathState.OnStart` routes through
    // `ChangeState(MAIN_STATE_DEAD)`, whose PostUpdate flush is the
    // 0x0134 + X00/X01 mode-change trio. The bare 0x0134 this used to
    // send flips the logical state but the client never *plays* the
    // collapse — the live SEQ_005 run delivered all three wolf deaths
    // to the client socket and the user still saw standing wolves.
    // (`emit_change_state_trio` re-stamps current_main_state = DEAD;
    // the write above already set it, so it's an idempotent set.)
    emit_change_state_trio(
        owner_actor_id,
        crate::actor::MAIN_STATE_DEAD,
        registry,
        world,
        zone,
    )
    .await;
    // Retail rides npcWork.hateType=4 with the death state (pcap kill
    // sequences pack it with the hp=0 stateAtQuicklyForAll) — the
    // corpse plate style. BattleNpcs only; players carry no npcWork.
    // (Round-3 nameplate RCA, 2026-07-02.)
    if matches!(
        handle.kind,
        crate::runtime::actor_registry::ActorKindTag::BattleNpc
    ) {
        let sub = crate::packets::send::actor::build_npc_hate_type_packet(
            owner_actor_id,
            crate::npc::HATE_TYPE_DEAD,
        );
        broadcast_around_actor(world, registry, zone, owner_actor_id, sub.to_bytes()).await;
    }
    if let (Some(lua_ref), Some(db_ref)) = (lua, db) {
        for evt in status_outbox.drain() {
            dispatch_status_event(&evt, registry, world, db_ref, lua_ref.catalogs()).await;
        }
    }

    // #28 S4.1 — second gate call site: scripted deaths (`BattleEvent::
    // Die`, GM `!die`) reach `apply_die` without transiting
    // `die_if_defender_fell`, so the content kill gate fires here too —
    // placement-independent. The already-DEAD guard above keeps this on
    // the death transition only.
    if matches!(
        handle.kind,
        crate::runtime::actor_registry::ActorKindTag::BattleNpc,
    ) && let (Some(lua_ref), Some(db_ref)) = (lua, db)
    {
        check_content_battle_complete(owner_actor_id, registry, world, db_ref, lua_ref).await;
    }
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
    let sub = tx::build_set_actor_state(owner_actor_id, crate::actor::MAIN_STATE_PASSIVE as u8, 0);
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

/// Ceiling for the `!speed` GM command's multiplier.
const MOVEMENT_SPEED_MAX_MULTIPLIER: f32 = 10.0;

/// Outcome of [`apply_movement_speed`] - surfaced so the GM command can
/// echo what actually happened after the reset/clamp policy applied.
#[derive(Debug, Clone, Copy)]
pub(crate) enum MovementSpeedOutcome {
    /// Bands scaled to `multiplier`x (already clamped to the ceiling).
    Applied { multiplier: f32 },
    /// Requested value was <= 0 (or NaN) - bands back to the defaults.
    Reset,
    /// Player not in the registry - nothing happened.
    UnknownPlayer,
}

/// Scale a player's movement-speed bands to `requested`x the defaults
/// (the `!speed` GM command). `requested <= 0` or NaN resets to 1x;
/// values above [`MOVEMENT_SPEED_MAX_MULTIPLIER`] clamp to it. Also
/// pins `Modifier::MovementSpeed` to the scaled run speed so
/// `get_speed()` (Lua snapshots, the zone-in band derivation in
/// `world_manager`) agrees with what the client renders. Fan-out
/// mirrors C# `Actor.PostUpdate` for `ActorUpdateFlags.Speed`: the
/// packet goes to the player and everyone around them.
pub(crate) async fn apply_movement_speed(
    player_id: u32,
    requested: f32,
    registry: &ActorRegistry,
    world: &WorldManager,
    zone: &Arc<RwLock<Zone>>,
) -> MovementSpeedOutcome {
    let Some(handle) = registry.get(player_id).await else {
        return MovementSpeedOutcome::UnknownPlayer;
    };
    let reset = requested.is_nan() || requested <= 0.0;
    let multiplier = if reset {
        1.0
    } else {
        requested.min(MOVEMENT_SPEED_MAX_MULTIPLIER)
    };
    {
        let mut c = handle.character.write().await;
        c.chara.mods.set(
            crate::actor::modifier::Modifier::MovementSpeed,
            (tx::actor::SPEED_DEFAULT_RUN * multiplier) as f64,
        );
    }
    let bytes = tx::actor::build_set_actor_speed_scaled(player_id, multiplier).to_bytes();
    send_to_self_if_player(registry, world, player_id, bytes.clone()).await;
    broadcast_around_actor(world, registry, zone, player_id, bytes).await;
    if reset {
        MovementSpeedOutcome::Reset
    } else {
        MovementSpeedOutcome::Applied { multiplier }
    }
}

/// The zone broadcaster excludes the source actor, so Players who die or
/// revive wouldn't otherwise see their own state-change packet. Send it
/// directly to their session.
pub(crate) async fn send_to_self_if_player(
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
        // Same proxy rule as `broadcast_around_actor`: untargeted
        // subpackets are dropped by the world-server fan-out, so stamp
        // the recipient's session id into any header still carrying 0.
        let mut bytes = bytes;
        common::subpacket::SubPacket::stamp_target_id_if_zero(&mut bytes, handle.session_id);
        client.send_bytes(bytes).await;
    }
}

#[cfg(test)]
mod movement_speed_tests {
    use super::*;
    use crate::actor::Character;
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

    async fn fixture() -> (
        Arc<WorldManager>,
        Arc<ActorRegistry>,
        Arc<RwLock<Zone>>,
        ActorHandle,
    ) {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        let zone_id = 100u32;
        world.register_zone(make_zone(zone_id)).await;
        let character = Character::new(7);
        let handle = ActorHandle::new(7, ActorKindTag::Player, zone_id, 42, character);
        registry.insert(handle.clone()).await;
        let zone = world.zone(zone_id).await.expect("zone registered");
        (world, registry, zone, handle)
    }

    #[tokio::test]
    async fn multiplier_scales_the_movement_speed_mod() {
        let (world, registry, zone, handle) = fixture().await;
        let outcome = apply_movement_speed(7, 5.0, &registry, &world, &zone).await;
        match outcome {
            MovementSpeedOutcome::Applied { multiplier } => assert_eq!(multiplier, 5.0),
            other => panic!("expected Applied, got {other:?}"),
        }
        let c = handle.character.read().await;
        assert_eq!(c.get_speed(), 25.0, "run speed = 5.0 * multiplier");
    }

    #[tokio::test]
    async fn multiplier_caps_at_ten() {
        let (world, registry, zone, handle) = fixture().await;
        let outcome = apply_movement_speed(7, 15.0, &registry, &world, &zone).await;
        match outcome {
            MovementSpeedOutcome::Applied { multiplier } => assert_eq!(multiplier, 10.0),
            other => panic!("expected Applied, got {other:?}"),
        }
        let c = handle.character.read().await;
        assert_eq!(c.get_speed(), 50.0);
    }

    #[tokio::test]
    async fn zero_and_negative_reset_to_default() {
        let (world, registry, zone, handle) = fixture().await;
        apply_movement_speed(7, 5.0, &registry, &world, &zone).await;
        let outcome = apply_movement_speed(7, 0.0, &registry, &world, &zone).await;
        assert!(matches!(outcome, MovementSpeedOutcome::Reset));
        assert_eq!(handle.character.read().await.get_speed(), 5.0);

        apply_movement_speed(7, 5.0, &registry, &world, &zone).await;
        let outcome = apply_movement_speed(7, -3.0, &registry, &world, &zone).await;
        assert!(matches!(outcome, MovementSpeedOutcome::Reset));
        assert_eq!(handle.character.read().await.get_speed(), 5.0);
    }

    /// NaN parses as a valid f32 (`"NaN".parse::<f32>()` succeeds) and
    /// would sail through `min()` as 10x - it must reset instead.
    #[tokio::test]
    async fn nan_resets_to_default() {
        let (world, registry, zone, handle) = fixture().await;
        apply_movement_speed(7, 5.0, &registry, &world, &zone).await;
        let outcome = apply_movement_speed(7, f32::NAN, &registry, &world, &zone).await;
        assert!(matches!(outcome, MovementSpeedOutcome::Reset));
        assert_eq!(handle.character.read().await.get_speed(), 5.0);
    }

    #[tokio::test]
    async fn unknown_player_is_reported() {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        world.register_zone(make_zone(100)).await;
        let zone = world.zone(100).await.expect("zone registered");
        let outcome = apply_movement_speed(999, 2.0, &registry, &world, &zone).await;
        assert!(matches!(outcome, MovementSpeedOutcome::UnknownPlayer));
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
        let session = Session {
            id: 42,
            current_zone_id: source_zone,
            ..Default::default()
        };
        world.upsert_session(session).await;

        let zone_arc = world.zone(source_zone).await.expect("zone registered");
        let outcome = apply_home_point_revive(7, &registry, &world, &zone_arc).await;

        match outcome {
            HomePointReviveOutcome::Warped {
                homepoint, zone_id, ..
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

    /// Corpse-lock regression (#28): when an actor dies, every other actor
    /// whose `AttackState` targets it must disengage and drop its hate toward
    /// the dead id — otherwise an ally that kills the first wolf stays
    /// `is_engaged()` on the corpse and the content `onUpdate` never re-engages
    /// it onto the next wolf.
    #[tokio::test]
    async fn apply_die_disengages_actors_targeting_the_dead() {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        let zone_id = 100u32;
        world.register_zone(make_zone(zone_id)).await;
        let zone_arc = world.zone(zone_id).await.expect("zone registered");

        // The wolf that is about to die.
        let mut wolf = Character::new(2);
        wolf.chara.max_hp = 100;
        wolf.chara.hp = 1;
        wolf.base.zone_id = zone_id;
        registry
            .insert(ActorHandle::new(
                2,
                ActorKindTag::BattleNpc,
                zone_id,
                0,
                wolf,
            ))
            .await;

        // An ally attacking the wolf: AttackState on id 2 + hate toward 2.
        let mut ally = Character::new(3);
        ally.base.zone_id = zone_id;
        ally.ai_container.internal_engage(2, 0, 2000);
        ally.hate.update_hate(2, 50);
        let ally_handle = ActorHandle::new(3, ActorKindTag::Ally, zone_id, 0, ally);
        registry.insert(ally_handle.clone()).await;

        assert!(
            ally_handle.character.read().await.ai_container.is_engaged(),
            "pre-condition: ally engaged on the wolf",
        );

        apply_die(2, &registry, &world, &zone_arc, None, None).await;

        let c = ally_handle.character.read().await;
        assert!(
            !c.ai_container.is_engaged(),
            "ally must disengage from the dead wolf",
        );
        assert_eq!(
            c.hate.most_hated(),
            None,
            "ally hate toward the dead wolf must be cleared",
        );
    }

    /// NPC death must ship pmeteor's State trio (0x0134 DEAD + X00 +
    /// X01 cmd-21001 mode-change action), not a bare 0x0134 — the live
    /// SEQ_005 run delivered bare death states to the client and the
    /// wolves never visibly fell (Garlemald #28).
    #[tokio::test]
    async fn apply_die_broadcasts_dead_state_trio() {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        let zone_id = 100u32;
        world.register_zone(make_zone(zone_id)).await;
        let zone_arc = world.zone(zone_id).await.expect("zone registered");

        // Grid: dying wolf at origin + a player 5 yalms away.
        {
            let mut z = zone_arc.write().await;
            let mut ob = crate::zone::outbox::AreaOutbox::new();
            for (id, kind, x) in [
                (2u32, crate::zone::area::ActorKind::BattleNpc, 0.0f32),
                (9u32, crate::zone::area::ActorKind::Player, 5.0),
            ] {
                z.core.add_actor(
                    crate::zone::area::StoredActor {
                        actor_id: id,
                        kind,
                        position: common::Vector3::new(x, 0.0, 0.0),
                        grid: (0, 0),
                        is_alive: true,
                    },
                    &mut ob,
                );
            }
        }

        let mut wolf = Character::new(2);
        wolf.chara.max_hp = 100;
        wolf.chara.hp = 0;
        wolf.base.zone_id = zone_id;
        registry
            .insert(ActorHandle::new(
                2,
                ActorKindTag::BattleNpc,
                zone_id,
                0,
                wolf,
            ))
            .await;
        registry
            .insert(ActorHandle::new(
                9,
                ActorKindTag::Player,
                zone_id,
                /* session */ 77,
                Character::new(9),
            ))
            .await;
        let (tx_ch, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(8);
        world
            .register_client(77, crate::data::ClientHandle::new(77, tx_ch))
            .await;

        apply_die(2, &registry, &world, &zone_arc, None, None).await;

        let bytes = rx.try_recv().expect("death trio broadcast to the player");
        // Walk the concatenated subpackets: opcode lives at +0x12 of
        // each subpacket (u16 size header first).
        let mut opcodes = Vec::new();
        let mut off = 0usize;
        while off + 0x14 <= bytes.len() {
            let size = u16::from_le_bytes([bytes[off], bytes[off + 1]]) as usize;
            let opcode = u16::from_le_bytes([bytes[off + 0x12], bytes[off + 0x13]]);
            opcodes.push(opcode);
            off += size.max(0x14);
        }
        assert_eq!(
            opcodes,
            vec![
                crate::packets::opcodes::OP_SET_ACTOR_STATE,
                crate::packets::opcodes::OP_COMMAND_RESULT_X00,
                crate::packets::opcodes::OP_COMMAND_RESULT_X01,
            ],
            "death must ship the 0x0134 + 0x013C + 0x0139 trio in order",
        );
        // The 0x0134 body's first byte is the main_state — DEAD = 1.
        assert_eq!(bytes[0x20], crate::actor::MAIN_STATE_DEAD as u8);
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

#[cfg(test)]
mod battle_exp_award_tests {
    use super::*;
    use crate::actor::Character;
    use crate::data::{ActiveContentScript, ClientHandle, Session};
    use crate::runtime::actor_registry::{ActorHandle, ActorKindTag};
    use crate::zone::navmesh::StubNavmeshLoader;

    const PLAYER_ID: u32 = 9;
    const SESSION_ID: u32 = 77;
    const ZONE_ID: u32 = 100;
    const GLADIATOR: i16 = 3;

    fn tempdb() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("garlemald-exp-award-{nanos}-{seq}.db"))
    }

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

    /// Zone 100 with a level-6 Gladiator (session 77, client rx attached)
    /// and `mob_count` dead level-6 BattleNpcs (ids 2, 3, …) ready for
    /// `die_if_defender_fell`.
    async fn setup(
        mob_count: u32,
    ) -> (
        Arc<WorldManager>,
        Arc<ActorRegistry>,
        Arc<RwLock<Zone>>,
        Arc<crate::database::Database>,
        ActorHandle,
        tokio::sync::mpsc::Receiver<Vec<u8>>,
    ) {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        world.register_zone(make_zone(ZONE_ID)).await;
        let zone_arc = world.zone(ZONE_ID).await.expect("zone registered");
        {
            let mut z = zone_arc.write().await;
            let mut ob = crate::zone::outbox::AreaOutbox::new();
            z.core.add_actor(
                crate::zone::area::StoredActor {
                    actor_id: PLAYER_ID,
                    kind: crate::zone::area::ActorKind::Player,
                    position: common::Vector3::new(5.0, 0.0, 0.0),
                    grid: (0, 0),
                    is_alive: true,
                },
                &mut ob,
            );
            for mob_id in 2..2 + mob_count {
                z.core.add_actor(
                    crate::zone::area::StoredActor {
                        actor_id: mob_id,
                        kind: crate::zone::area::ActorKind::BattleNpc,
                        position: common::Vector3::ZERO,
                        grid: (0, 0),
                        is_alive: true,
                    },
                    &mut ob,
                );
            }
        }

        let mut player = Character::new(PLAYER_ID);
        player.chara.max_hp = 300;
        player.chara.hp = 300;
        player.chara.class = GLADIATOR;
        player.chara.level = 6;
        player.battle_save.skill_level[GLADIATOR as usize] = 6;
        player.base.zone_id = ZONE_ID;
        let player_handle =
            ActorHandle::new(PLAYER_ID, ActorKindTag::Player, ZONE_ID, SESSION_ID, player);
        registry.insert(player_handle.clone()).await;

        for mob_id in 2..2 + mob_count {
            let mut mob = Character::new(mob_id);
            mob.chara.max_hp = 60;
            mob.chara.hp = 0; // killing blow already settled
            mob.chara.level = 6;
            mob.base.zone_id = ZONE_ID;
            registry
                .insert(ActorHandle::new(
                    mob_id,
                    ActorKindTag::BattleNpc,
                    ZONE_ID,
                    0,
                    mob,
                ))
                .await;
        }

        let (tx_ch, rx) = tokio::sync::mpsc::channel::<Vec<u8>>(64);
        world
            .register_client(SESSION_ID, ClientHandle::new(SESSION_ID, tx_ch))
            .await;
        let db = Arc::new(
            crate::database::Database::open(tempdb())
                .await
                .expect("test db"),
        );
        (world, registry, zone_arc, db, player_handle, rx)
    }

    /// Drain every frame queued so far and collect the worldmaster text
    /// ids from X01 (0x0139) and X10 (0x013A) CommandResult subpackets.
    fn drain_text_ids(rx: &mut tokio::sync::mpsc::Receiver<Vec<u8>>) -> Vec<u16> {
        let mut ids = Vec::new();
        while let Ok(frame) = rx.try_recv() {
            let mut off = 0usize;
            while off + 0x20 <= frame.len() {
                let size = u16::from_le_bytes([frame[off], frame[off + 1]]) as usize;
                if size < 0x20 || off + size > frame.len() {
                    break;
                }
                let opcode = u16::from_le_bytes([frame[off + 0x12], frame[off + 0x13]]);
                let body = &frame[off + 0x20..off + size];
                match opcode {
                    // X01 row: worldMasterTextId at +0x2E of the body.
                    0x0139 if body.len() >= 0x30 => {
                        ids.push(u16::from_le_bytes([body[0x2E], body[0x2F]]));
                    }
                    // X10 rows: numActions at +0x20, text-id column at
                    // +0x64 (CommandResultX10Packet.cs:54-81).
                    0x013A if body.len() >= 0x78 => {
                        let n = u32::from_le_bytes([body[0x20], body[0x21], body[0x22], body[0x23]])
                            as usize;
                        for i in 0..n.min(10) {
                            let p = 0x64 + i * 2;
                            ids.push(u16::from_le_bytes([body[p], body[p + 1]]));
                        }
                    }
                    _ => {}
                }
                off += size;
            }
        }
        ids
    }

    /// A player-killed BattleNpc pays the dlvl-0 base row (601 at level
    /// 6 — the retail ankle-biter calibration point), emits the 30108
    /// "defeats" line plus the 33935 Gladiator "You earn N experience
    /// points." row, and arms the EXP-chain status at tier 1.
    #[tokio::test]
    async fn player_kill_awards_exp_and_emits_defeat_line() {
        let (world, registry, zone_arc, db, player_handle, mut rx) = setup(1).await;

        die_if_defender_fell(
            2,
            Some(PLAYER_ID),
            &registry,
            &world,
            &zone_arc,
            None,
            Some(&db),
        )
        .await;

        {
            let c = player_handle.character.read().await;
            assert_eq!(
                c.battle_save.skill_point[GLADIATOR as usize], 601,
                "level-6 kill at dlvl 0 pays the calibrated 601 base",
            );
            assert_eq!(c.chara.level, 6, "601 SP must not cross the 6→7 threshold");
            let chain = c
                .status_effects
                .get(crate::status::ids::STATUS_EXP_CHAIN)
                .expect("EXP-chain status armed by the kill");
            assert_eq!(chain.tier, 1, "first kill arms the chain at tier 1");
        }

        let ids = drain_text_ids(&mut rx);
        assert!(ids.contains(&30108), "\"defeats\" row shipped: {ids:?}");
        assert!(
            ids.contains(&33935),
            "Gladiator \"You earn N experience points.\" row shipped: {ids:?}",
        );
        assert!(
            !ids.contains(&33919),
            "no chain line on the chain-opening kill: {ids:?}",
        );
    }

    /// A second kill inside the chain window pays the tier-1 bonus
    /// (601 + ceil(20%) = 722) and ships the 33919 chain line.
    #[tokio::test]
    async fn chained_kill_pays_chain_bonus_and_emits_chain_line() {
        let (world, registry, zone_arc, db, player_handle, mut rx) = setup(2).await;

        die_if_defender_fell(
            2,
            Some(PLAYER_ID),
            &registry,
            &world,
            &zone_arc,
            None,
            Some(&db),
        )
        .await;
        die_if_defender_fell(
            3,
            Some(PLAYER_ID),
            &registry,
            &world,
            &zone_arc,
            None,
            Some(&db),
        )
        .await;

        {
            let c = player_handle.character.read().await;
            assert_eq!(
                c.battle_save.skill_point[GLADIATOR as usize],
                601 + 722,
                "second kill pays 601 + ceil(601*20/100) chain bonus",
            );
            let chain = c
                .status_effects
                .get(crate::status::ids::STATUS_EXP_CHAIN)
                .expect("EXP-chain status still armed");
            assert_eq!(chain.tier, 2, "chain advanced to tier 2");
        }

        let ids = drain_text_ids(&mut rx);
        assert!(
            ids.contains(&33919),
            "\"EXP chain\" row shipped on the chained kill: {ids:?}",
        );
    }

    fn content_session(script: &str) -> Session {
        let mut session = Session::new(SESSION_ID);
        session.current_zone_id = ZONE_ID;
        session.active_content_script = Some(ActiveContentScript {
            parent_zone_id: ZONE_ID,
            area_name: "test".into(),
            area_class_path: "/Area/PrivateArea/Test".into(),
            director_name: "TestDirector".into(),
            director_actor_id: 0x5FF8_0001,
            content_area_actor_id: 0x5FF8_0002,
            content_script: script.into(),
            warp_complete: true,
            spawned_actor_ids: Vec::new(),
        });
        session
    }

    /// Kills made under the three tutorial content scripts are skipped —
    /// their Lua `onUpdate` already self-pays a fixed AddExp, so the
    /// generic award double-paying was the hazard.
    #[tokio::test]
    async fn tutorial_content_kill_skips_generic_award() {
        let (world, registry, zone_arc, db, player_handle, _rx) = setup(1).await;
        world
            .upsert_session(content_session("SimpleContent30010"))
            .await;

        die_if_defender_fell(
            2,
            Some(PLAYER_ID),
            &registry,
            &world,
            &zone_arc,
            None,
            Some(&db),
        )
        .await;

        let c = player_handle.character.read().await;
        assert_eq!(
            c.battle_save.skill_point[GLADIATOR as usize], 0,
            "tutorial content self-pays via Lua — the generic award must skip",
        );
    }

    /// The man0l1 escort runs under a content script too, but it does
    /// NOT self-pay — retail screenshots show 601/625 EXP per ankle
    /// biter during the duty, so the generic award must fire for it.
    #[tokio::test]
    async fn escort_content_kill_still_awards() {
        let (world, registry, zone_arc, db, player_handle, _rx) = setup(1).await;
        world
            .upsert_session(content_session("SimpleContentMan0l101"))
            .await;

        die_if_defender_fell(
            2,
            Some(PLAYER_ID),
            &registry,
            &world,
            &zone_arc,
            None,
            Some(&db),
        )
        .await;

        let c = player_handle.character.read().await;
        assert_eq!(
            c.battle_save.skill_point[GLADIATOR as usize], 601,
            "non-tutorial content (escort) defers to the generic award",
        );
    }
}

#[cfg(test)]
mod pool_tick_sync_tests {
    use super::*;
    use crate::actor::Character;
    use crate::data::ClientHandle;
    use crate::runtime::actor_registry::{ActorHandle, ActorKindTag};
    use crate::status::outbox::StatusEvent;
    use crate::zone::navmesh::StubNavmeshLoader;

    const PLAYER_ID: u32 = 5;
    const SESSION_ID: u32 = 88;
    const ZONE_ID: u32 = 100;

    fn tempdb() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("garlemald-pool-tick-{nanos}-{seq}.db"))
    }

    /// Zone 100 with one Player (session + client channel attached),
    /// pools at `hp`/`tp`, ready for a dispatched pool-tick event.
    async fn setup(
        hp: i16,
        tp: u16,
    ) -> (
        Arc<WorldManager>,
        Arc<ActorRegistry>,
        Arc<crate::database::Database>,
        ActorHandle,
        tokio::sync::mpsc::Receiver<Vec<u8>>,
    ) {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        let mut zone = Zone::new(
            ZONE_ID,
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
        let mut ob = crate::zone::outbox::AreaOutbox::new();
        zone.core.add_actor(
            crate::zone::area::StoredActor {
                actor_id: PLAYER_ID,
                kind: crate::zone::area::ActorKind::Player,
                position: common::Vector3::ZERO,
                grid: (0, 0),
                is_alive: true,
            },
            &mut ob,
        );
        world.register_zone(zone).await;

        let mut player = Character::new(PLAYER_ID);
        player.chara.max_hp = 1000;
        player.chara.hp = hp;
        player.chara.max_mp = 500;
        player.chara.mp = 250;
        player.chara.tp = tp;
        player.base.zone_id = ZONE_ID;
        let handle = ActorHandle::new(PLAYER_ID, ActorKindTag::Player, ZONE_ID, SESSION_ID, player);
        registry.insert(handle.clone()).await;

        let (tx_ch, rx) = tokio::sync::mpsc::channel::<Vec<u8>>(64);
        world
            .register_client(SESSION_ID, ClientHandle::new(SESSION_ID, tx_ch))
            .await;
        let db = Arc::new(
            crate::database::Database::open(tempdb())
                .await
                .expect("test db"),
        );
        (world, registry, db, handle, rx)
    }

    /// True if any queued frame carries the `charaWork/stateAtQuicklyForAll`
    /// target path (the pool-gauge bundle).
    fn received_state_bundle(rx: &mut tokio::sync::mpsc::Receiver<Vec<u8>>) -> bool {
        let needle = b"charaWork/stateAtQuicklyForAll";
        let mut found = false;
        while let Ok(frame) = rx.try_recv() {
            if frame.windows(needle.len()).any(|w| w == needle) {
                found = true;
            }
        }
        found
    }

    /// A regen HpTick that moves the pool must ship the
    /// `charaWork/stateAtQuicklyForAll` bundle — pre-change the tick arms
    /// mutated memory only and the client gauge stayed frozen.
    #[tokio::test]
    async fn hp_tick_that_changes_pool_syncs_client() {
        let (world, registry, db, handle, mut rx) = setup(500, 0).await;
        let catalogs = std::sync::Arc::new(crate::lua::Catalogs::default());

        dispatch_status_event(
            &StatusEvent::HpTick {
                owner_actor_id: PLAYER_ID,
                delta: 20,
            },
            &registry,
            &world,
            &db,
            &catalogs,
        )
        .await;

        assert_eq!(handle.character.read().await.chara.hp, 520);
        assert!(
            received_state_bundle(&mut rx),
            "HP tick must ride the stateAtQuicklyForAll sync",
        );
    }

    /// A tick that clamps (full HP) produces zero wire traffic — the
    /// idle-at-full case must stay silent.
    #[tokio::test]
    async fn hp_tick_clamped_at_full_emits_nothing() {
        let (world, registry, db, handle, mut rx) = setup(1000, 0).await;
        let catalogs = std::sync::Arc::new(crate::lua::Catalogs::default());

        dispatch_status_event(
            &StatusEvent::HpTick {
                owner_actor_id: PLAYER_ID,
                delta: 20,
            },
            &registry,
            &world,
            &db,
            &catalogs,
        )
        .await;

        assert_eq!(handle.character.read().await.chara.hp, 1000);
        assert!(
            !received_state_bundle(&mut rx),
            "clamped tick must not emit a state bundle",
        );
    }

    /// TP decay ticks sync the client too (the TP gauge empties visibly),
    /// and a decay against an already-empty pool stays silent.
    #[tokio::test]
    async fn tp_tick_syncs_client_and_empty_pool_stays_silent() {
        let (world, registry, db, handle, mut rx) = setup(1000, 200).await;
        let catalogs = std::sync::Arc::new(crate::lua::Catalogs::default());

        dispatch_status_event(
            &StatusEvent::TpTick {
                owner_actor_id: PLAYER_ID,
                delta: -90,
            },
            &registry,
            &world,
            &db,
            &catalogs,
        )
        .await;
        assert_eq!(handle.character.read().await.chara.tp, 110);
        assert!(
            received_state_bundle(&mut rx),
            "TP decay must ride the stateAtQuicklyForAll sync",
        );

        {
            let mut c = handle.character.write().await;
            c.chara.tp = 0;
        }
        dispatch_status_event(
            &StatusEvent::TpTick {
                owner_actor_id: PLAYER_ID,
                delta: -90,
            },
            &registry,
            &world,
            &db,
            &catalogs,
        )
        .await;
        assert_eq!(handle.character.read().await.chara.tp, 0);
        assert!(
            !received_state_bundle(&mut rx),
            "empty-TP decay must not emit a state bundle",
        );
    }
}
