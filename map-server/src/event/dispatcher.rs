//! Dispatcher for `EventEvent`s. Turns outbox rows into:
//!
//! * Outbound packets (`RunEventFunctionPacket`, `EndEventPacket`,
//!   `KickEventPacket`, `SendGameMessage`).
//! * Lua dispatch (EventStarted / EventUpdated / QuestCheckCompletion /
//!   QuestAbandonHook). Phase 4 logs these; the Lua bridge side in
//!   Phase 4d hooks them into real callbacks.
//! * DB writes (`QuestSaveToDb`).

#![allow(dead_code)]

use crate::database::Database;
use crate::packets::send as tx;
use crate::runtime::actor_registry::ActorRegistry;
use crate::world_manager::WorldManager;

use super::outbox::EventEvent;

pub async fn dispatch_event_event(
    event: &EventEvent,
    registry: &ActorRegistry,
    world: &WorldManager,
    db: &Database,
) {
    match event {
        EventEvent::EventStarted {
            player_actor_id,
            owner_actor_id,
            event_name,
            event_type,
            lua_params,
        } => {
            tracing::debug!(
                player = player_actor_id,
                owner = owner_actor_id,
                name = %event_name,
                ty = event_type,
                params = lua_params.len(),
                "event: started (Lua hook pending)",
            );
        }
        EventEvent::EventUpdated {
            player_actor_id,
            trigger_actor_id,
            event_type,
            lua_params,
        } => {
            tracing::debug!(
                player = player_actor_id,
                trigger = trigger_actor_id,
                ty = event_type,
                params = lua_params.len(),
                "event: updated (Lua hook pending)",
            );
        }
        EventEvent::RunEventFunction {
            player_actor_id,
            trigger_actor_id,
            owner_actor_id,
            event_name,
            event_type,
            function_name,
            lua_params,
        } => {
            let sub = tx::build_run_event_function(
                *trigger_actor_id,
                *owner_actor_id,
                event_name,
                *event_type,
                function_name,
                lua_params,
            );
            send_to_player(world, registry, *player_actor_id, sub.to_bytes()).await;
        }
        EventEvent::EndEvent {
            player_actor_id,
            owner_actor_id,
            event_name,
            event_type,
        } => {
            let sub = tx::build_end_event(
                *player_actor_id,
                *owner_actor_id,
                event_name,
                *event_type,
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
            let sub = tx::build_kick_event(
                *target_actor_id,
                *owner_actor_id,
                event_name,
                *event_type,
                lua_params,
            );
            send_to_player(world, registry, *player_actor_id, sub.to_bytes()).await;
        }
        EventEvent::QuestCheckCompletion { player_actor_id, quest_id } => {
            tracing::debug!(
                player = player_actor_id,
                quest = quest_id,
                "quest: isObjectivesComplete (Lua hook pending)",
            );
        }
        EventEvent::QuestAbandonHook { player_actor_id, quest_id } => {
            tracing::debug!(
                player = player_actor_id,
                quest = quest_id,
                "quest: onAbandonQuest (Lua hook pending)",
            );
        }
        EventEvent::QuestSaveToDb {
            player_actor_id,
            quest_id,
            phase,
            flags,
            data,
        } => {
            // Slot is Phase 4-opaque — the quest registry on the player
            // owns the slot index; we hand `0` here and let the slot-aware
            // QuestJournal port layer in a later phase plug it in.
            if let Err(e) = db
                .save_quest(
                    *player_actor_id,
                    /* slot */ 0,
                    *quest_id,
                    *phase,
                    data,
                    *flags,
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
            // A GameMessage-style text broadcast with the quest id packed
            // in as the first param. The real builder will land with the
            // richer SendGameMessage port; for now log the intent so the
            // quest loop stays observable.
            tracing::debug!(
                player = player_actor_id,
                text = text_id,
                quest = quest_id,
                "quest: game message (send-builder pending)",
            );
        }
    }
}

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
