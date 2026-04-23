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

//! World server packet dispatch. Matches the C# PacketProcessor shape: frame
//! the incoming `BasePacket`, fan sub-packets out by type, and route the
//! result back to the same connection (or downstream zone server).

use std::sync::Arc;

use anyhow::Result;
use common::BasePacket;
use common::subpacket::{SUBPACKET_TYPE_GAMEMESSAGE, SubPacket};

use crate::data::{ClientHandle, Session, SessionChannel};
use crate::database::Database;
use crate::group::{LINKSHELL_TYPE, PARTY_TYPE};
use crate::packets::receive as rx;
use crate::packets::send as tx;
use crate::server::SessionRegistry;
use crate::world_master::WorldMaster;

pub struct PacketProcessor {
    pub db: Arc<Database>,
    pub world: Arc<WorldMaster>,
    pub sessions: Arc<SessionRegistry>,
}

impl PacketProcessor {
    pub async fn process_packet(
        &self,
        client: &ClientHandle,
        mut packet: BasePacket,
    ) -> Result<()> {
        if packet.header.is_compressed == 0x01 {
            packet.decompress()?;
        }

        for sub in packet.get_subpackets()? {
            let ty = sub.header.r#type;
            let handler = match ty {
                0x01 => "hello",
                0x07 => "ping",
                0x08 => "zoning_stub",
                SUBPACKET_TYPE_GAMEMESSAGE => "game_message",
                t if t >= 0x1000 => "world_packet",
                _ => "(unhandled)",
            };
            tracing::debug!(
                client_id = client.id,
                r#type = format!("0x{ty:X}"),
                opcode = format!("0x{:X}", sub.game_message.opcode),
                handler,
                "subpacket dispatch"
            );
            match ty {
                // Initial hello → create session
                0x01 => self.handle_hello(client, &packet, &sub).await?,
                // Ping
                0x07 => self.handle_ping(client).await?,
                // Zoning-related; just log (the C# DebugPrintPacket path)
                0x08 => {
                    tracing::debug!(session = client.id, "zoning packet stub");
                }
                // Game messages (route to owning zone server)
                SUBPACKET_TYPE_GAMEMESSAGE => self.handle_game_message(&sub).await?,
                t if t >= 0x1000 => self.handle_world_packet(client, &sub).await?,
                _ => {
                    tracing::warn!(r#type = format!("0x{ty:X}"), "unhandled subpacket");
                }
            }
        }
        Ok(())
    }

    async fn handle_hello(
        &self,
        client: &ClientHandle,
        packet: &BasePacket,
        _sub: &SubPacket,
    ) -> Result<()> {
        let hello = rx::HelloPacket::parse(&packet.data)?;
        let channel = match packet.header.connection_type {
            common::PACKET_TYPE_ZONE => SessionChannel::Zone,
            common::PACKET_TYPE_CHAT => SessionChannel::Chat,
            _ => SessionChannel::Zone,
        };
        tracing::info!(
            session_id = hello.session_id,
            channel = ?channel,
            "hello received; session starting"
        );
        let session = Arc::new(Session::new(hello.session_id, channel, client.clone()));

        // Load character data so we have zone/linkshell info ready.
        if let Ok(Some(snap)) = self.db.load_zone_session_info(hello.session_id).await {
            let mut state = session.state.lock().await;
            state.character_name = snap.character_name;
            state.current_zone_id = snap.current_zone_id;
            state.active_linkshell_name = snap.active_linkshell;
        }

        if channel == SessionChannel::Zone {
            let zone_id = session.state.lock().await.current_zone_id;
            if let Some(handle) = self.world.zone_server_for(zone_id).await {
                session.state.lock().await.routing1 = Some(handle.clone());
                let begin = tx::build_session_begin(hello.session_id, true);
                handle.send_bytes(begin.to_bytes()).await;
            }
        }
        self.sessions.add(channel, hello.session_id, session).await;

        // Complete handshake (0x07 + 0x02)
        let ack7 = tx::build_0x7_packet(0x0E01_6EE5);
        let ack2 = tx::build_0x2_packet(hello.session_id);
        client.send_bytes(ack7.to_bytes()).await;
        client.send_bytes(ack2.to_bytes()).await;
        Ok(())
    }

    async fn handle_ping(&self, client: &ClientHandle) -> Result<()> {
        let reply = tx::build_0x8_ping_packet(client.id);
        client.send_bytes(reply.to_bytes()).await;
        Ok(())
    }

    async fn handle_game_message(&self, sub: &SubPacket) -> Result<()> {
        let target = sub.header.target_id;
        let Some(session) = self.sessions.get(SessionChannel::Zone, target).await else {
            return Ok(());
        };

        // Party-chat intercept so other zone servers can fan out.
        if sub.game_message.opcode == 0x00C9
            && let Ok(chat) = rx::PartyChatMessagePacket::parse(&sub.data)
            && let Some(party) = self.world.party_manager.get_party(target).await
        {
            for member in &party.members {
                if *member == target {
                    continue;
                }
                if let Some(peer) = self.sessions.get(SessionChannel::Zone, *member).await {
                    let sender_name = session.state.lock().await.character_name.clone();
                    let packet = tx::build_send_message(
                        target,
                        peer.session_id,
                        tx::MESSAGE_TYPE_PARTY,
                        &sender_name,
                        &chat.message,
                    );
                    peer.client.send_bytes(packet.to_bytes()).await;
                }
            }
            return Ok(());
        }

        // LanguageCode (0x0006) — C# `World Server/PacketProcessor.cs`
        // intercepts this before forwarding and runs
        // `WorldMaster.DoLogin(session)`, which fans out the MotD banner,
        // party/retainer/linkshell group packets, and the "active
        // linkshell" packet. The client expects this completion signal
        // from the world server half of the handshake — without it the
        // login finalisation on the client side stalls even though the
        // map-server zone-in bundle lands cleanly.
        //
        // The full DoLogin also synchs retainer state and every
        // linkshell the player belongs to; for a fresh character those
        // collections are empty, so the MotD burst alone is enough to
        // satisfy the world-login side of the handshake. We still
        // forward the subpacket below so the map server's 0x0006
        // handler (which triggers `do_zone_change` + zone-in bundle)
        // runs on its own.
        if sub.game_message.opcode == 0x0006 {
            let motd_lines = [
                "-------- Login Message --------",
                "Welcome to Garlemald!",
                "Welcome to Eorzea!",
                "Here is a test Message of the Day from the World Server!",
            ];
            for line in motd_lines {
                let packet = tx::build_send_message(
                    target,
                    target,
                    tx::MESSAGE_TYPE_GENERAL_INFO,
                    "",
                    line,
                );
                session.client.send_bytes(packet.to_bytes()).await;
            }
            // Final step of C# `WorldMaster.DoLogin`: tell the client
            // which linkshell is active. For a fresh character with no
            // linkshells the group id is 0 and the "has active" flag
            // collapses to 0 (see build_set_active_linkshell). This is
            // the world-side "handshake complete" packet the client
            // waits on before finalising its chat/UI state.
            let active_ls = tx::build_set_active_linkshell(target, 0);
            session.client.send_bytes(active_ls.to_bytes()).await;
            tracing::info!(session = target, "DoLogin MotD + active-LS dispatched");
        }

        // Group creation notification — currently just logged; the full
        // SynchGroupWorkValues fanout ships with Phase 4 alongside the Map
        // Server's group-work encoder.
        if sub.game_message.opcode == 0x0133 {
            tracing::debug!(session = target, "group created notification");
        }

        // Default: forward the gamemessage subpacket to the session's owning
        // zone server.
        let (r1, r2) = {
            let s = session.state.lock().await;
            (s.routing1.clone(), s.routing2.clone())
        };
        if let Some(zone) = r1 {
            zone.send_bytes(sub.to_bytes()).await;
        }
        if let Some(zone) = r2 {
            zone.send_bytes(sub.to_bytes()).await;
        }
        Ok(())
    }

    async fn handle_world_packet(&self, _client: &ClientHandle, sub: &SubPacket) -> Result<()> {
        let target = sub.header.target_id;
        let session = self.sessions.get(SessionChannel::Zone, target).await;

        match sub.header.r#type {
            0x1000 => {
                if let Ok(p) = rx::SessionBeginConfirmPacket::parse(&sub.data)
                    && (p.error_code == 0)
                {
                    tracing::error!(session = p.session_id, "error beginning session");
                }
            }
            0x1001 => {
                if let Ok(p) = rx::SessionEndConfirmPacket::parse(&sub.data) {
                    if p.error_code != 0 {
                        tracing::error!(session = p.session_id, "error ending session");
                    } else if p.destination_zone != 0
                        && let Some(ref session) = session
                    {
                        if let Some(handle) = self.world.zone_server_for(p.destination_zone).await {
                            let begin = tx::build_session_begin(p.session_id, false);
                            handle.send_bytes(begin.to_bytes()).await;
                            session.state.lock().await.routing1 = Some(handle);
                        }
                    } else {
                        self.sessions.remove(SessionChannel::Zone, p.session_id).await;
                        self.sessions.remove(SessionChannel::Chat, p.session_id).await;
                    }
                }
            }
            0x1002 => {
                if let Ok(p) = rx::WorldRequestZoneChangePacket::parse(&sub.data)
                    && let Some(ref session) = session
                {
                    let (r1, _r2) = {
                        let state = session.state.lock().await;
                        (state.routing1.clone(), state.routing2.clone())
                    };
                    if let Some(handle) = r1 {
                        let end = tx::build_session_end_with_zone(
                            p.session_id,
                            p.destination_zone_id,
                            p.destination_spawn_type,
                            p.destination_x,
                            p.destination_y,
                            p.destination_z,
                            p.destination_rot,
                        );
                        handle.send_bytes(end.to_bytes()).await;
                    }
                }
            }
            0x1020 => self.handle_party_modify(sub).await?,
            0x1021 => self.handle_party_leave(sub).await?,
            0x1022 => self.handle_party_invite(sub).await?,
            0x1023 => self.handle_group_invite_result(sub).await?,
            0x1025 => self.handle_create_linkshell(sub).await?,
            0x1026 => self.handle_modify_linkshell(sub).await?,
            0x1027 => self.handle_delete_linkshell(sub).await?,
            0x1028 => self.handle_linkshell_change(sub).await?,
            0x1029 => self.handle_linkshell_invite(sub).await?,
            0x1030 => self.handle_linkshell_invite_cancel(sub).await?,
            0x1031 => self.handle_linkshell_leave(sub).await?,
            0x1032 => self.handle_linkshell_rank_change(sub).await?,
            other => {
                tracing::debug!(opcode = format!("0x{other:X}"), "unhandled world opcode");
            }
        }
        Ok(())
    }

    async fn handle_party_modify(&self, sub: &SubPacket) -> Result<()> {
        let p = rx::PartyModifyPacket::parse(&sub.data)?;
        let target = sub.header.target_id;
        if let Some(mut party) = self.world.party_manager.get_party(target).await
            && party.member_count() > 1
        {
            match p.command {
                c if c == rx::PartyModifyPacket::MODIFY_LEADER
                    || c == rx::PartyModifyPacket::MODIFY_LEADER + 2 =>
                {
                    party.leader = p.actor_id;
                }
                c if c == rx::PartyModifyPacket::MODIFY_KICKPLAYER
                    || c == rx::PartyModifyPacket::MODIFY_KICKPLAYER + 2 =>
                {
                    party.remove_member(p.actor_id);
                }
                _ => {}
            }
            // Write-through: caller might reuse `party`, but the mutations
            // were on a clone; push them back.
            self.world.party_manager.disband(target).await;
            let mut map_guard = self.world.party_manager.get_party(target).await;
            let _ = map_guard.take();
        }
        Ok(())
    }

    async fn handle_party_leave(&self, sub: &SubPacket) -> Result<()> {
        let p = rx::PartyLeavePacket::parse(&sub.data)?;
        let source = sub.header.source_id;
        if p.is_disband {
            self.world.party_manager.disband(source).await;
        } else {
            self.world.party_manager.remove_member(source, source).await;
        }
        Ok(())
    }

    async fn handle_party_invite(&self, sub: &SubPacket) -> Result<()> {
        let p = rx::PartyInvitePacket::parse(&sub.data)?;
        let source = sub.header.source_id;
        let _ = self.world.party_manager.ensure_party(source).await;
        if p.command == 1 && p.actor_id != 0 {
            self.world.party_manager.add_member(source, p.actor_id).await;
        }
        Ok(())
    }

    async fn handle_group_invite_result(&self, sub: &SubPacket) -> Result<()> {
        let p = rx::GroupInviteResultPacket::parse(&sub.data)?;
        match p.group_type as u16 {
            PARTY_TYPE => tracing::debug!(result = p.result, "party invite result"),
            LINKSHELL_TYPE => tracing::debug!(result = p.result, "linkshell invite result"),
            _ => {}
        }
        Ok(())
    }

    async fn handle_create_linkshell(&self, sub: &SubPacket) -> Result<()> {
        let p = rx::CreateLinkshellPacket::parse(&sub.data)?;
        let source = sub.header.source_id;

        let err = self.world.linkshell_manager.can_create_linkshell(&self.db, &p.name).await;
        let mut final_err = err;
        if err == 0
            && let Err(e) = self
                .world
                .linkshell_manager
                .create_linkshell(&self.db, &p.name, p.crest_id, p.master)
                .await
        {
            tracing::error!(error = %e, "linkshell create failed");
            final_err = 3;
        }
        if let Some(session) = self.sessions.get(SessionChannel::Zone, source).await {
            let reply = tx::build_linkshell_result(source, final_err);
            session.client.send_bytes(reply.to_bytes()).await;
        }
        Ok(())
    }

    async fn handle_modify_linkshell(&self, sub: &SubPacket) -> Result<()> {
        let p = rx::ModifyLinkshellPacket::parse(&sub.data)?;
        match p.arg_code {
            rx::ModifyLinkshellPacket::CODE_CRESTCHANGE => {
                self.world
                    .linkshell_manager
                    .change_linkshell_crest(&self.db, &p.current_name, p.crest_id)
                    .await?;
            }
            rx::ModifyLinkshellPacket::CODE_MASTERCHANGE => {
                self.world
                    .linkshell_manager
                    .change_linkshell_master(&self.db, &p.current_name, p.master)
                    .await?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_delete_linkshell(&self, sub: &SubPacket) -> Result<()> {
        let p = rx::DeleteLinkshellPacket::parse(&sub.data)?;
        self.world.linkshell_manager.delete_linkshell(&p.name).await?;
        Ok(())
    }

    async fn handle_linkshell_change(&self, sub: &SubPacket) -> Result<()> {
        let p = rx::LinkshellChangePacket::parse(&sub.data)?;
        if let Some(session) = self.sessions.get(SessionChannel::Zone, sub.header.source_id).await {
            self.db.set_active_ls(session.session_id, &p.ls_name).await?;
            session.state.lock().await.active_linkshell_name = p.ls_name;
        }
        Ok(())
    }

    async fn handle_linkshell_invite(&self, sub: &SubPacket) -> Result<()> {
        let p = rx::LinkshellInvitePacket::parse(&sub.data)?;
        let Some(ls) = self
            .world
            .linkshell_manager
            .get_or_load_linkshell(&self.db, &p.ls_name)
            .await?
        else {
            tracing::warn!(ls = %p.ls_name, "linkshell invite: unknown ls");
            return Ok(());
        };
        self.world
            .linkshell_manager
            .add_member(
                &self.db,
                ls.db_id,
                p.actor_id,
                crate::managers::LinkshellManager::RANK_MEMBER,
            )
            .await?;
        // Refresh from cache after the add so the member list we
        // notify against includes the joiner. `OnPlayerJoin` text
        // ids 25157 (to joiner: "You join %s") + 25284 (to others:
        // "%s has joined %s"). Mirrors `Linkshell.OnPlayerJoin` in
        // `World Server/DataObjects/Group/Linkshell.cs`.
        self.notify_linkshell_join(ls.db_id, p.actor_id, &p.ls_name).await;
        let _ = sub;
        Ok(())
    }

    async fn handle_linkshell_invite_cancel(&self, _sub: &SubPacket) -> Result<()> {
        tracing::debug!("linkshell invite cancel");
        Ok(())
    }

    async fn handle_linkshell_leave(&self, sub: &SubPacket) -> Result<()> {
        let p = rx::LinkshellLeavePacket::parse(&sub.data)?;
        let Some(ls) = self
            .world
            .linkshell_manager
            .get_or_load_linkshell(&self.db, &p.ls_name)
            .await?
        else {
            return Ok(());
        };
        let target = if p.is_kicked {
            match self.db.character_id_by_name(&p.kicked_name).await? {
                Some(id) => id,
                None => {
                    tracing::warn!(name = %p.kicked_name, "linkshell kick: unknown name");
                    return Ok(());
                }
            }
        } else {
            sub.header.source_id
        };
        // Snapshot the member list BEFORE the remove so we can fan
        // the kick/leave message to everyone (including the
        // departing member). For self-leave, only the leaver hears
        // 25162 ("You leave %s"); for kick, kicked hears 25184
        // ("You have been exiled from %s") and the rest hear 25280
        // ("%s has been exiled from %s").
        if p.is_kicked {
            self.notify_linkshell_kick(ls.db_id, target, &p.kicked_name, &p.ls_name)
                .await;
        } else {
            self.notify_linkshell_self_leave(target, &p.ls_name).await;
        }
        self.world
            .linkshell_manager
            .remove_member(&self.db, ls.db_id, target)
            .await?;
        Ok(())
    }

    async fn handle_linkshell_rank_change(&self, sub: &SubPacket) -> Result<()> {
        let p = rx::LinkshellRankChangePacket::parse(&sub.data)?;
        let Some(ls) = self
            .world
            .linkshell_manager
            .get_or_load_linkshell(&self.db, &p.ls_name)
            .await?
        else {
            return Ok(());
        };
        let Some(target_id) = self.db.character_id_by_name(&p.name).await? else {
            tracing::warn!(name = %p.name, "linkshell rank change: unknown name");
            return Ok(());
        };
        self.world
            .linkshell_manager
            .change_rank(&self.db, ls.db_id, target_id, p.rank)
            .await?;
        // 25277 ("…has been promoted to rank…") — Meteor encodes the
        // rank in the textId arg as `100000 + rank`.
        self.notify_linkshell_rank_change(target_id, p.rank, &p.ls_name)
            .await;
        let _ = sub;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Linkshell notification helpers (Tier 1 #5 follow-up — Meteor
    // `WorldServer/DataObjects/Group/Linkshell.cs::OnPlayerJoin/etc.`).
    // -----------------------------------------------------------------------

    /// Best-effort send of one game-message subpacket to a single
    /// session, identified by character id (which == session id for
    /// players in this server). No-op if the session isn't online.
    async fn send_game_message_to(
        &self,
        recipient_chara_id: u32,
        text_id: u16,
        lua_params: Vec<common::luaparam::LuaParam>,
    ) {
        let Some(session) = self
            .sessions
            .get(crate::data::SessionChannel::Zone, recipient_chara_id)
            .await
        else {
            return;
        };
        let pkt = tx::build_game_message(
            recipient_chara_id,
            tx::GameMessageOptions {
                sender_actor_id: 0,
                receiver_actor_id: recipient_chara_id,
                text_id,
                log: 0x20,
                display_id: None,
                custom_sender: None,
                lua_params,
            },
        );
        session.client.send_bytes(pkt.to_bytes()).await;
    }

    async fn notify_linkshell_join(&self, ls_id: u64, joiner_id: u32, ls_name: &str) {
        let Some(ls) = self.world.linkshell_manager.get_cached_by_id(ls_id).await else {
            return;
        };
        let joiner_name = self
            .session_character_name(joiner_id)
            .await
            .unwrap_or_default();
        for member in &ls.members {
            if member.character_id == joiner_id {
                self.send_game_message_to(
                    joiner_id,
                    25157,
                    vec![
                        common::luaparam::LuaParam::Int32(0),
                        common::luaparam::LuaParam::Actor(joiner_id),
                        common::luaparam::LuaParam::String(ls_name.to_string()),
                    ],
                )
                .await;
            } else {
                self.send_game_message_to(
                    member.character_id,
                    25284,
                    vec![
                        common::luaparam::LuaParam::Int32(0),
                        common::luaparam::LuaParam::String(joiner_name.clone()),
                        common::luaparam::LuaParam::String(ls_name.to_string()),
                    ],
                )
                .await;
            }
        }
    }

    async fn notify_linkshell_self_leave(&self, leaver_id: u32, ls_name: &str) {
        // Only the leaver hears 25162; remaining members aren't
        // notified per Meteor's `LeaveRequest` flow (members
        // discover via the next group sync).
        self.send_game_message_to(
            leaver_id,
            25162,
            vec![
                common::luaparam::LuaParam::Int32(1),
                common::luaparam::LuaParam::String(ls_name.to_string()),
            ],
        )
        .await;
    }

    async fn notify_linkshell_kick(
        &self,
        ls_id: u64,
        kicked_id: u32,
        kicked_name: &str,
        ls_name: &str,
    ) {
        let Some(ls) = self.world.linkshell_manager.get_cached_by_id(ls_id).await else {
            return;
        };
        for member in &ls.members {
            if member.character_id == kicked_id {
                self.send_game_message_to(
                    kicked_id,
                    25184,
                    vec![
                        common::luaparam::LuaParam::Int32(1),
                        common::luaparam::LuaParam::String(ls_name.to_string()),
                    ],
                )
                .await;
            } else {
                self.send_game_message_to(
                    member.character_id,
                    25280,
                    vec![
                        common::luaparam::LuaParam::Int32(1),
                        common::luaparam::LuaParam::String(kicked_name.to_string()),
                        common::luaparam::LuaParam::String(ls_name.to_string()),
                    ],
                )
                .await;
            }
        }
    }

    async fn notify_linkshell_rank_change(&self, target_id: u32, rank: u8, ls_name: &str) {
        // Meteor passes `(100000 + rank)` as the int-arg slot to
        // index into the textId table for the new rank label.
        self.send_game_message_to(
            target_id,
            25277,
            vec![
                common::luaparam::LuaParam::Int32(100_000 + rank as i32),
                common::luaparam::LuaParam::String(ls_name.to_string()),
            ],
        )
        .await;
    }

    /// Look up a character's display name through their live session.
    /// Returns `None` for offline characters; the caller falls back
    /// to whatever string the client supplied (kicked-target name).
    async fn session_character_name(&self, chara_id: u32) -> Option<String> {
        let session = self
            .sessions
            .get(crate::data::SessionChannel::Zone, chara_id)
            .await?;
        Some(session.state.lock().await.character_name.clone())
    }
}
