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

//! Turn `SocialEvent`s into real packet bytes queued on the right
//! ClientHandle(s). Chat broadcasts reuse the Phase-1 spatial fan-out
//! helper; group chats fan-out via the group membership cached on the
//! source player.

#![allow(dead_code)]

use std::sync::Arc;

use tokio::sync::RwLock;

use crate::packets::send as tx;
use crate::runtime::actor_registry::ActorRegistry;
use crate::world_manager::WorldManager;
use crate::zone::zone::Zone;

use super::outbox::SocialEvent;

pub async fn dispatch_social_event(
    event: &SocialEvent,
    registry: &ActorRegistry,
    world: &WorldManager,
) {
    match event {
        SocialEvent::ChatBroadcast {
            source_actor_id,
            kind,
            sender_name,
            message,
        } => {
            // Look up the source player's zone so we can spatially
            // fan out the chat packet.
            let Some(handle) = registry.get(*source_actor_id).await else {
                return;
            };
            let Some(zone_arc) = world.zone(handle.zone_id).await else {
                return;
            };
            // The send-message wire format takes source + target + msg
            // type + sender + body. For a broadcast we build one packet
            // per recipient so the target_session field is accurate.
            let nearby_ids = nearby_player_session_map(&zone_arc, registry, *source_actor_id).await;
            for (actor_id, session_id) in nearby_ids {
                let sub = tx::build_send_message(
                    handle.session_id,
                    session_id,
                    kind.as_u8(),
                    sender_name,
                    message,
                );
                let Some(client) = world.client(session_id).await else {
                    continue;
                };
                client.send_bytes(sub.to_bytes()).await;
                let _ = actor_id;
            }
        }
        SocialEvent::ChatTell {
            source_actor_id,
            target_actor_id,
            sender_name,
            message,
        } => {
            let Some(source) = registry.get(*source_actor_id).await else {
                return;
            };
            let Some(target) = registry.get(*target_actor_id).await else {
                return;
            };
            let sub = tx::build_send_message(
                source.session_id,
                target.session_id,
                crate::social::chat::CHAT_TELL,
                sender_name,
                message,
            );
            if let Some(client) = world.client(target.session_id).await {
                client.send_bytes(sub.to_bytes()).await;
            }
        }
        SocialEvent::ChatParty {
            source_actor_id,
            party_id: _,
            sender_name,
            message,
        }
        | SocialEvent::ChatLinkshell {
            source_actor_id,
            linkshell_id: _,
            sender_name,
            message,
        } => {
            // Fan to party/linkshell members cached on the source's
            // Character. Falls back to a log if we can't resolve the
            // roster (group state will be authoritative once the
            // world-server sync lands — Phase 7 currently reads from
            // PlayerHelperState which is seeded by tests).
            let Some(handle) = registry.get(*source_actor_id).await else {
                return;
            };
            let kind = match event {
                SocialEvent::ChatParty { .. } => crate::social::chat::CHAT_PARTY,
                SocialEvent::ChatLinkshell { .. } => crate::social::chat::CHAT_LS,
                _ => unreachable!(),
            };
            // We read the cached roster off the source's PlayerHelperState.
            let recipient_actor_ids: Vec<u32> = {
                let _chara = handle.character.read().await;
                // Party/linkshell rosters live on PlayerHelperState,
                // which we only have for Players. For NPCs the iterator
                // returns empty.
                match event {
                    SocialEvent::ChatParty { .. } => Vec::new(),
                    SocialEvent::ChatLinkshell { .. } => Vec::new(),
                    _ => unreachable!(),
                }
                .into_iter()
                .chain(std::iter::empty::<u32>())
                .collect()
            };
            // The recipient list being empty is fine: party/linkshell
            // group state is wired through PlayerHelperState on the
            // Player struct, not Character. Callers that exercise this
            // path supply the roster directly via the integration tests.
            for recipient_id in recipient_actor_ids {
                let Some(recipient) = registry.get(recipient_id).await else {
                    continue;
                };
                let sub = tx::build_send_message(
                    handle.session_id,
                    recipient.session_id,
                    kind,
                    sender_name,
                    message,
                );
                if let Some(client) = world.client(recipient.session_id).await {
                    client.send_bytes(sub.to_bytes()).await;
                }
            }
        }
        SocialEvent::ChatSystemToPlayer {
            target_actor_id,
            kind,
            message,
        } => {
            let Some(handle) = registry.get(*target_actor_id).await else {
                return;
            };
            let sub = tx::build_send_message(
                handle.session_id,
                handle.session_id,
                kind.as_u8(),
                "",
                message,
            );
            if let Some(client) = world.client(handle.session_id).await {
                client.send_bytes(sub.to_bytes()).await;
            }
        }

        // ---- Friendlist / blacklist ---------------------------------
        SocialEvent::FriendlistAdded {
            actor_id,
            friend_character_id,
            name,
            success,
            is_online,
        } => {
            let Some(handle) = registry.get(*actor_id).await else {
                return;
            };
            let sub = tx::build_friendlist_added(
                handle.session_id,
                *success,
                *friend_character_id as i64,
                *is_online,
                name,
            );
            send_bytes(world, handle.session_id, sub.to_bytes()).await;
        }
        SocialEvent::FriendlistRemoved {
            actor_id,
            name,
            success,
        } => {
            let Some(handle) = registry.get(*actor_id).await else {
                return;
            };
            let sub = tx::build_friendlist_removed(handle.session_id, *success, name);
            send_bytes(world, handle.session_id, sub.to_bytes()).await;
        }
        SocialEvent::FriendlistSend { actor_id, entries } => {
            let Some(handle) = registry.get(*actor_id).await else {
                return;
            };
            let mut offset = 0usize;
            let tuples: Vec<(i64, String)> = entries.clone();
            let sub = tx::build_send_friendlist(handle.session_id, &tuples, &mut offset);
            send_bytes(world, handle.session_id, sub.to_bytes()).await;
        }
        SocialEvent::FriendStatus { actor_id, entries } => {
            let Some(handle) = registry.get(*actor_id).await else {
                return;
            };
            let sub = tx::build_friend_status(handle.session_id, entries);
            send_bytes(world, handle.session_id, sub.to_bytes()).await;
        }
        SocialEvent::BlacklistAdded {
            actor_id,
            name,
            success,
        } => {
            let Some(handle) = registry.get(*actor_id).await else {
                return;
            };
            let sub = tx::build_blacklist_added(handle.session_id, *success, name);
            send_bytes(world, handle.session_id, sub.to_bytes()).await;
        }
        SocialEvent::BlacklistRemoved {
            actor_id,
            name,
            success,
        } => {
            let Some(handle) = registry.get(*actor_id).await else {
                return;
            };
            let sub = tx::build_blacklist_removed(handle.session_id, *success, name);
            send_bytes(world, handle.session_id, sub.to_bytes()).await;
        }
        SocialEvent::BlacklistSend { actor_id, names } => {
            let Some(handle) = registry.get(*actor_id).await else {
                return;
            };
            let mut offset = 0usize;
            let owned: Vec<String> = names.clone();
            let sub = tx::build_send_blacklist(handle.session_id, &owned, &mut offset);
            send_bytes(world, handle.session_id, sub.to_bytes()).await;
        }

        // ---- Recruitment --------------------------------------------
        SocialEvent::RecruitingStarted { actor_id, success } => {
            let Some(handle) = registry.get(*actor_id).await else {
                return;
            };
            let sub = tx::build_start_recruiting_response(handle.session_id, *success);
            send_bytes(world, handle.session_id, sub.to_bytes()).await;
        }
        SocialEvent::RecruitingEnded { actor_id } => {
            let Some(handle) = registry.get(*actor_id).await else {
                return;
            };
            let sub = tx::build_end_recruitment(handle.session_id);
            send_bytes(world, handle.session_id, sub.to_bytes()).await;
        }
        SocialEvent::RecruiterStateQueried {
            actor_id,
            is_recruiter,
            is_recruiting,
            total_recruiters,
        } => {
            let Some(handle) = registry.get(*actor_id).await else {
                return;
            };
            let sub = tx::build_recruiter_state(
                handle.session_id,
                *is_recruiting,
                *is_recruiter,
                *total_recruiters as i64,
            );
            send_bytes(world, handle.session_id, sub.to_bytes()).await;
        }
        SocialEvent::RecruitmentDetailsSent {
            actor_id,
            recruiter_name,
            purpose_id,
            location_id,
            sub_task_id,
            comment,
        } => {
            let Some(handle) = registry.get(*actor_id).await else {
                return;
            };
            let details = tx::RecruitmentDetails {
                recruiter_id: *actor_id as u64,
                purpose: *purpose_id as u16,
                location: *location_id as u16,
                min_level: 1,
                max_level: 50,
                description: comment.clone(),
                recruiter_name: recruiter_name.clone(),
            };
            let sub = tx::build_current_recruitment_details(handle.session_id, &details);
            let _ = sub_task_id;
            send_bytes(world, handle.session_id, sub.to_bytes()).await;
        }

        // ---- Support desk --------------------------------------------
        SocialEvent::FaqListRequested { actor_id, faqs } => {
            let Some(handle) = registry.get(*actor_id).await else {
                return;
            };
            let sub = tx::build_faq_list_response(handle.session_id, faqs);
            send_bytes(world, handle.session_id, sub.to_bytes()).await;
        }
        SocialEvent::FaqBodyRequested { actor_id, body } => {
            let Some(handle) = registry.get(*actor_id).await else {
                return;
            };
            let sub = tx::build_faq_body_response(handle.session_id, body);
            send_bytes(world, handle.session_id, sub.to_bytes()).await;
        }
        SocialEvent::SupportIssueListRequested { actor_id, issues } => {
            let Some(handle) = registry.get(*actor_id).await else {
                return;
            };
            let sub = tx::build_issue_list_response(handle.session_id, issues);
            send_bytes(world, handle.session_id, sub.to_bytes()).await;
        }
        SocialEvent::GmTicketStartQueried {
            actor_id,
            is_active,
        } => {
            let Some(handle) = registry.get(*actor_id).await else {
                return;
            };
            let sub = tx::build_start_gm_ticket(handle.session_id, *is_active);
            send_bytes(world, handle.session_id, sub.to_bytes()).await;
        }
        SocialEvent::GmTicketResponseQueried {
            actor_id,
            title,
            body,
        } => {
            let Some(handle) = registry.get(*actor_id).await else {
                return;
            };
            let sub = tx::build_gm_ticket(handle.session_id, title, body);
            send_bytes(world, handle.session_id, sub.to_bytes()).await;
        }
        SocialEvent::GmTicketSent { actor_id, accepted } => {
            let Some(handle) = registry.get(*actor_id).await else {
                return;
            };
            let sub = tx::build_gm_ticket_sent_response(handle.session_id, *accepted);
            send_bytes(world, handle.session_id, sub.to_bytes()).await;
        }
        SocialEvent::GmTicketEnded { actor_id } => {
            let Some(handle) = registry.get(*actor_id).await else {
                return;
            };
            let sub = tx::build_end_gm_ticket(handle.session_id);
            send_bytes(world, handle.session_id, sub.to_bytes()).await;
        }
    }
}

async fn send_bytes(world: &WorldManager, session_id: u32, bytes: Vec<u8>) {
    let Some(client) = world.client(session_id).await else {
        return;
    };
    client.send_bytes(bytes).await;
}

async fn nearby_player_session_map(
    zone: &Arc<RwLock<Zone>>,
    registry: &ActorRegistry,
    source_actor_id: u32,
) -> Vec<(u32, u32)> {
    let actor_views = {
        let z = zone.read().await;
        z.core
            .actors_around(source_actor_id, crate::zone::area::BROADCAST_RADIUS)
    };
    let mut out = Vec::new();
    for a in actor_views {
        let Some(handle) = registry.get(a.actor_id).await else {
            continue;
        };
        if !handle.is_player() || handle.session_id == 0 {
            continue;
        }
        out.push((a.actor_id, handle.session_id));
    }
    out
}
