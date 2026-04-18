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

use crate::battle::outbox::BattleEvent;
use crate::database::Database;
use crate::inventory::outbox::InventoryEvent;
use crate::packets::send as tx;
use crate::status::outbox::StatusEvent;
use crate::world_manager::WorldManager;
use crate::zone::outbox::AreaEvent;
use crate::zone::zone::Zone;

use super::actor_registry::ActorRegistry;
use super::broadcast::broadcast_around_actor;

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
            tracing::debug!(owner = owner_actor_id, "status: RecalcStats (TODO)");
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
) {
    match event {
        BattleEvent::Engage {
            owner_actor_id,
            target_actor_id: _,
        }
        | BattleEvent::Disengage { owner_actor_id }
        | BattleEvent::Spawn { owner_actor_id }
        | BattleEvent::Die { owner_actor_id }
        | BattleEvent::Despawn { owner_actor_id }
        | BattleEvent::RecalcStats { owner_actor_id } => {
            tracing::debug!(owner = owner_actor_id, kind = ?event_tag(event), "battle event");
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
    _registry: &ActorRegistry,
    _world: &WorldManager,
    _db: &Database,
) {
    // Phase 1 logs — real packet/DB emission wires in Phase 2 alongside
    // the zone-in handshake that first sends the inventory.
    tracing::debug!(kind = ?std::any::type_name_of_val(event), "inventory event (TODO)");
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

/// Pump the seven-packet actor-spawn bundle to every Player within
/// BROADCAST_RADIUS of `actor_id`. Matches the C# `Npc::GetSpawnPackets`
/// sequence: AddActor + Speed + SpawnPosition + Name + State +
/// IsZoning + (ScriptBind later once Lua wire-up lands).
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
    // Snapshot the character's base state for the spawn bundle.
    let (name, state, display_name_id, position, rotation) = {
        let c = handle.character.read().await;
        (
            c.base.display_name().to_string(),
            c.base.current_main_state as u8,
            c.base.display_name_id,
            c.base.position(),
            c.base.rotation,
        )
    };
    let packets = [
        tx::actor::build_add_actor(actor_id, 0).to_bytes(),
        tx::actor::build_set_actor_speed_default(actor_id).to_bytes(),
        tx::actor::build_set_actor_position(
            actor_id, -1, position.x, position.y, position.z, rotation, 1, false,
        )
        .to_bytes(),
        tx::actor::build_set_actor_name(actor_id, display_name_id, &name).to_bytes(),
        tx::actor::build_set_actor_state(actor_id, state, 0).to_bytes(),
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
