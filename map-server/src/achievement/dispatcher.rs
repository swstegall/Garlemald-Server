//! Turn `AchievementEvent`s into real packets on the player's queue.

#![allow(dead_code)]

use crate::packets::send as tx;
use crate::runtime::actor_registry::ActorRegistry;
use crate::world_manager::WorldManager;

use super::outbox::AchievementEvent;

pub async fn dispatch_achievement_event(
    event: &AchievementEvent,
    registry: &ActorRegistry,
    world: &WorldManager,
) {
    let (player_actor_id, bytes) = match event {
        AchievementEvent::Earned {
            player_actor_id,
            achievement_id,
        } => {
            let sub = tx::build_achievement_earned(*player_actor_id, *achievement_id);
            (*player_actor_id, sub.to_bytes())
        }
        AchievementEvent::SetPoints {
            player_actor_id,
            points,
        } => {
            let sub = tx::build_set_achievement_points(*player_actor_id, *points);
            (*player_actor_id, sub.to_bytes())
        }
        AchievementEvent::SetLatest {
            player_actor_id,
            latest_ids,
        } => {
            let sub = tx::build_set_latest_achievements(*player_actor_id, latest_ids);
            (*player_actor_id, sub.to_bytes())
        }
        AchievementEvent::SetCompleted {
            player_actor_id,
            bits,
        } => {
            let sub = tx::build_set_completed_achievements(*player_actor_id, bits);
            (*player_actor_id, sub.to_bytes())
        }
        AchievementEvent::SendRate {
            player_actor_id,
            achievement_id,
            progress_count,
            progress_flags,
        } => {
            let sub = tx::build_send_achievement_rate(
                *player_actor_id,
                *achievement_id,
                *progress_count,
                *progress_flags,
            );
            (*player_actor_id, sub.to_bytes())
        }
        AchievementEvent::SetPlayerTitle {
            player_actor_id,
            title_id,
        } => {
            let sub = tx::build_set_player_title(*player_actor_id, *title_id);
            (*player_actor_id, sub.to_bytes())
        }
    };

    let Some(handle) = registry.get(player_actor_id).await else {
        return;
    };
    let Some(client) = world.client(handle.session_id).await else {
        return;
    };
    client.send_bytes(bytes).await;
}
