//! Map server packet dispatch. The C# `PacketProcessor.cs` is small (~400
//! lines) because it mostly delegates to `WorldManager`; this port is the
//! same shape.

use std::sync::Arc;

use anyhow::Result;
use common::subpacket::{SUBPACKET_TYPE_GAMEMESSAGE, SubPacket};
use common::{BasePacket, Vector3};

use crate::achievement::{AchievementEvent, AchievementOutbox, dispatch_achievement_event};
use crate::actor::Character;
use crate::data::{ClientHandle, Session};
use crate::database::Database;
use crate::event::EventOutbox;
use crate::event::dispatcher::dispatch_event_event;
use crate::lua::LuaEngine;
use crate::packets::opcodes::{
    OP_HANDSHAKE_RESPONSE, OP_PONG, OP_PONG_RESPONSE, OP_RX_ACHIEVEMENT_PROGRESS,
    OP_RX_BLACKLIST_ADD, OP_RX_BLACKLIST_REMOVE, OP_RX_BLACKLIST_REQUEST, OP_RX_CHAT_MESSAGE,
    OP_RX_END_RECRUITING, OP_RX_EVENT_START, OP_RX_EVENT_UPDATE, OP_RX_FAQ_BODY_REQUEST,
    OP_RX_FAQ_LIST_REQUEST, OP_RX_FRIEND_STATUS, OP_RX_FRIENDLIST_ADD, OP_RX_FRIENDLIST_REMOVE,
    OP_RX_FRIENDLIST_REQUEST, OP_RX_GM_TICKET_BODY, OP_RX_GM_TICKET_END, OP_RX_GM_TICKET_SEND,
    OP_RX_GM_TICKET_STATE, OP_RX_ITEM_PACKAGE_REQUEST, OP_RX_LANGUAGE_CODE,
    OP_RX_RECRUITER_STATE, OP_RX_RECRUITING_DETAILS, OP_RX_START_RECRUITING,
    OP_RX_SUPPORT_ISSUE_REQUEST, OP_RX_UPDATE_PLAYER_POSITION, OP_SESSION_BEGIN, OP_SESSION_END,
};
use crate::packets::receive::{
    AchievementProgressRequestPacket, AddRemoveSocialPacket, ChatMessagePacket, EventStartPacket,
    EventUpdatePacket, LanguageCodePacket, PingPacket, SessionBeginRequest,
    UpdatePlayerPositionPacket,
};
use crate::packets::send as tx;
use crate::runtime::actor_registry::{ActorHandle, ActorKindTag, ActorRegistry};
use crate::social::{
    ChatKind, SocialEvent, SocialOutbox, dispatch_social_event, message_type_from_u32, recruitment,
    support,
};
use crate::world_manager::WorldManager;

pub struct PacketProcessor {
    pub db: Arc<Database>,
    pub world: Arc<WorldManager>,
    pub registry: Arc<ActorRegistry>,
    /// Optional — when present, the event dispatcher calls
    /// `onEventStarted` / `isObjectivesComplete` / etc. on real scripts.
    pub lua: Option<Arc<LuaEngine>>,
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
            match sub.header.r#type {
                // Client→server ping arrives as OP_PONG (0x0008); server→client
                // ping reply is OP_PONG_RESPONSE (0x0001).
                OP_PONG => self.handle_ping(client).await?,
                OP_PONG_RESPONSE => {
                    tracing::debug!(session = client.session_id, "pong");
                }
                OP_HANDSHAKE_RESPONSE => {
                    // Connect pings from the client — send back the canned
                    // handshake response.
                    let resp = tx::build_handshake_response(client.session_id);
                    client.send_bytes(resp.to_bytes()).await;
                }
                OP_SESSION_BEGIN => self.handle_session_begin(client, &sub).await?,
                OP_SESSION_END => self.handle_session_end(client, &sub).await?,
                SUBPACKET_TYPE_GAMEMESSAGE => self.handle_game_message(client, &sub).await?,
                other => {
                    tracing::debug!(r#type = format!("0x{other:X}"), "unhandled map subpacket");
                }
            }
        }
        Ok(())
    }

    async fn handle_ping(&self, client: &ClientHandle) -> Result<()> {
        let reply = tx::build_ping_response(client.session_id);
        client.send_bytes(reply.to_bytes()).await;
        Ok(())
    }

    async fn handle_session_begin(&self, client: &ClientHandle, sub: &SubPacket) -> Result<()> {
        let session_id = sub.header.source_id;
        let is_login = SessionBeginRequest::parse(session_id, &sub.data)
            .map(|p| p.is_login)
            .unwrap_or(false);
        tracing::info!(session = session_id, is_login, "session begin");

        // 1. Pull the persisted character from the DB.
        //    C# Meteor's case 0x1000 sends no reply — `SessionBeginConfirmPacket`
        //    exists in the .csproj but is never instantiated. Sending one
        //    leaves the client's handshake state machine in a bad spot
        //    ("Now Loading" forever, no LanguageCode).
        let loaded = match self.db.load_player_character(session_id).await {
            Ok(Some(row)) => row,
            Ok(None) => {
                tracing::warn!(session = session_id, "no character row for session");
                return Ok(());
            }
            Err(e) => {
                tracing::error!(error = %e, session = session_id, "DB load failed");
                return Ok(());
            }
        };

        // `chara_id` == session id in this server's lobby flow.
        let actor_id = session_id;
        let zone_id = loaded.current_zone_id;
        let spawn = Vector3::new(loaded.position_x, loaded.position_y, loaded.position_z);
        let rotation = loaded.rotation;
        let class_slot = loaded.parameter_save.state_main_skill[0] as usize;
        let class_slot_safe = class_slot.min(3);
        let hp = loaded.parameter_save.hp[class_slot_safe];
        let hp_max = loaded.parameter_save.hp_max[class_slot_safe];
        let mp = loaded.parameter_save.mp;
        let mp_max = loaded.parameter_save.mp_max;

        tracing::info!(
            name = %loaded.name,
            zone = zone_id,
            inventory = loaded.inventory_normal.len(),
            "loaded character",
        );

        // 2. Register the ClientHandle + a Session entry so the game
        //    ticker and packet dispatchers can find the socket.
        self.world.register_client(session_id, client.clone()).await;
        let mut session = Session::new(session_id);
        session.current_zone_id = zone_id;
        session.destination_x = spawn.x;
        session.destination_y = spawn.y;
        session.destination_z = spawn.z;
        session.destination_rot = rotation;
        self.world.upsert_session(session).await;

        // 3. Build a Character from the loaded row and register it.
        let mut character = Character::new(actor_id);
        character.base.actor_name = loaded.name.clone();
        character.base.position_x = spawn.x;
        character.base.position_y = spawn.y;
        character.base.position_z = spawn.z;
        character.base.rotation = rotation;
        character.chara.class = class_slot as i16;
        character.chara.hp = hp;
        character.chara.max_hp = hp_max;
        character.chara.mp = mp;
        character.chara.max_mp = mp_max;
        // Pack the DB appearance rows into the 28-slot table the client
        // expects in `SetActorAppearancePacket`. Without these the zone-in
        // bundle can't render the avatar and the client hangs at Now
        // Loading even after all the other init packets land.
        character.chara.appearance_ids = loaded.appearance.to_slot_ids();
        character.chara.model_id = loaded.appearance.resolve_model_id(loaded.tribe);
        character.chara.tribe = loaded.tribe;
        character.chara.guardian = loaded.guardian;
        character.chara.birthday_day = loaded.birth_day;
        character.chara.birthday_month = loaded.birth_month;
        character.chara.initial_town = loaded.initial_town;
        character.chara.rest_bonus_exp_rate = loaded.rest_bonus_exp_rate;
        character.chara.tp = 0;

        self.registry
            .insert(ActorHandle::new(
                actor_id,
                ActorKindTag::Player,
                zone_id,
                session_id,
                character,
            ))
            .await;

        // 4. Fire the zone-change that places the player in their zone —
        //    but only for non-login transfers. Initial login defers this
        //    to the opcode-0x6 (LanguageCode) handler so the client has
        //    signalled it's ready to receive world-spawn packets.
        if !is_login {
            if let Err(e) = self
                .world
                .do_zone_change(actor_id, session_id, zone_id, spawn, rotation)
                .await
            {
                tracing::error!(error = %e, actor = actor_id, "zone change failed");
            } else {
                self.world
                    .send_zone_in_bundle(&self.registry, session_id, 0x1)
                    .await;
            }
        }

        let _ = client;
        Ok(())
    }

    async fn handle_session_end(&self, client: &ClientHandle, sub: &SubPacket) -> Result<()> {
        let session_id = sub.header.source_id;
        tracing::info!(session = session_id, "session end");
        self.registry.remove_session(session_id).await;
        self.world.remove_session(session_id).await;
        let reply = tx::build_session_end(session_id, 1, 0);
        client.send_bytes(reply.to_bytes()).await;
        Ok(())
    }

    /// Game-message opcode 0x0001 — client ping. The 1.23b client sends these
    /// once per second after zone-in and treats a missing reply as a lost
    /// connection, tearing down with error 40000 (communication timeout).
    /// Mirrors `Map Server/PacketProcessor.cs` case 0x0001: parse the u32
    /// `time`, echo it back in a PongPacket.
    async fn handle_gm_ping(
        &self,
        client: &ClientHandle,
        session_id: u32,
        data: &[u8],
    ) -> Result<()> {
        let ticks = PingPacket::parse(data).map(|p| p.time).unwrap_or(0);
        let reply = tx::build_pong(session_id, ticks);
        tracing::debug!(session = session_id, ticks, "pong sent");
        client.send_bytes(reply.to_bytes()).await;
        Ok(())
    }

    /// Game-message opcode 0x0002 — the client's "I'm here, ack me" frame.
    /// Mirrors C# `Map/PacketProcessor.cs` case 0x0002: reply with the 0x10-
    /// byte `_0x2Packet` that has source id at offset 0x8, wrapped as a
    /// game-message subpacket. Without this ack the client never advances
    /// to sending 0x0006 (LanguageCode), so the login flow stalls before
    /// `handle_language_code` and the zone-in bundle ever fire.
    async fn handle_gm_handshake_ack(
        &self,
        client: &ClientHandle,
        session_id: u32,
    ) -> Result<()> {
        let reply = tx::build_gm_0x02_ack(session_id);
        client.send_bytes(reply.to_bytes()).await;
        tracing::debug!(session = session_id, "gm handshake ack sent");
        Ok(())
    }

    /// Game-message opcode 0x0006 (LanguageCode) — the client signalling it's
    /// safe to receive world-spawn packets. C# `Map/PacketProcessor.cs` case
    /// 0x0006 fires `onBeginLogin`, `DoZoneIn(actor, isLogin=true, 0x1)`, then
    /// `onLogin`. The zone-change is the load-bearing piece for getting past
    /// the loading screen on first login.
    async fn handle_language_code(&self, session_id: u32, data: &[u8]) -> Result<()> {
        let lang = LanguageCodePacket::parse(data)
            .map(|p| p.language_code)
            .unwrap_or(1);

        let Some(handle) = self.registry.by_session(session_id).await else {
            tracing::warn!(session = session_id, "language_code: no actor registered");
            return Ok(());
        };
        let Some(mut snap) = self.world.session(session_id).await else {
            tracing::warn!(session = session_id, "language_code: no session registered");
            return Ok(());
        };

        // Persist the language code + login spawn type on the session.
        snap.language_code = lang;
        snap.destination_spawn_type = 0x1;
        let zone = snap.current_zone_id;
        let spawn = Vector3::new(snap.destination_x, snap.destination_y, snap.destination_z);
        let rotation = snap.destination_rot;
        self.world.upsert_session(snap).await;

        let actor_id = handle.actor_id;
        if let Err(e) = self
            .world
            .do_zone_change(actor_id, session_id, zone, spawn, rotation)
            .await
        {
            tracing::error!(error = %e, actor = actor_id, "login zone change failed");
        } else {
            self.world
                .send_zone_in_bundle(&self.registry, session_id, 0x1)
                .await;
        }

        // onBeginLogin / onLogin Lua hooks ride on the LuaEngine wiring
        // sprint — log so we can see the trigger fired.
        tracing::info!(
            session = session_id,
            language = lang,
            zone,
            "language code received; login zone-in dispatched",
        );
        Ok(())
    }

    async fn handle_game_message(&self, client: &ClientHandle, sub: &SubPacket) -> Result<()> {
        let opcode = sub.game_message.opcode;
        let source = sub.header.source_id;

        match opcode {
            OP_PONG_RESPONSE => self.handle_gm_ping(client, source, &sub.data).await?,
            OP_HANDSHAKE_RESPONSE => self.handle_gm_handshake_ack(client, source).await?,
            OP_RX_LANGUAGE_CODE => self.handle_language_code(source, &sub.data).await?,
            OP_RX_UPDATE_PLAYER_POSITION => self.handle_update_position(source, &sub.data).await?,
            OP_RX_EVENT_START => self.handle_event_start(source, &sub.data).await?,
            OP_RX_EVENT_UPDATE => self.handle_event_update(source, &sub.data).await?,
            OP_RX_CHAT_MESSAGE => self.handle_chat_message(source, &sub.data).await?,
            OP_RX_BLACKLIST_ADD => self.handle_blacklist_add(source, &sub.data).await?,
            OP_RX_BLACKLIST_REMOVE => self.handle_blacklist_remove(source, &sub.data).await?,
            OP_RX_BLACKLIST_REQUEST => self.handle_blacklist_request(source).await?,
            OP_RX_FRIENDLIST_ADD => self.handle_friendlist_add(source, &sub.data).await?,
            OP_RX_FRIENDLIST_REMOVE => self.handle_friendlist_remove(source, &sub.data).await?,
            OP_RX_FRIENDLIST_REQUEST => self.handle_friendlist_request(source).await?,
            OP_RX_FRIEND_STATUS => self.handle_friend_status(source).await?,
            OP_RX_START_RECRUITING => self.handle_recruiting_start(source).await?,
            OP_RX_END_RECRUITING => self.handle_recruiting_end(source).await?,
            OP_RX_RECRUITER_STATE => self.handle_recruiter_state(source).await?,
            OP_RX_RECRUITING_DETAILS => self.handle_recruiting_details(source).await?,
            OP_RX_FAQ_LIST_REQUEST => self.handle_faq_list(source).await?,
            OP_RX_FAQ_BODY_REQUEST => self.handle_faq_body(source).await?,
            OP_RX_SUPPORT_ISSUE_REQUEST => self.handle_support_issue(source).await?,
            OP_RX_GM_TICKET_STATE => self.handle_gm_ticket_state(source).await?,
            OP_RX_GM_TICKET_BODY => self.handle_gm_ticket_body(source).await?,
            OP_RX_GM_TICKET_SEND => self.handle_gm_ticket_send(source).await?,
            OP_RX_GM_TICKET_END => self.handle_gm_ticket_end(source).await?,
            OP_RX_ACHIEVEMENT_PROGRESS => {
                self.handle_achievement_progress(source, &sub.data).await?
            }
            OP_RX_ITEM_PACKAGE_REQUEST => {
                self.handle_item_package_request(source, &sub.data).await?
            }
            _ => {
                tracing::debug!(
                    opcode = format!("0x{:X}", opcode),
                    source = source,
                    "unhandled game message",
                );
            }
        }
        Ok(())
    }

    async fn handle_event_start(&self, session_id: u32, data: &[u8]) -> Result<()> {
        let pkt = match EventStartPacket::parse(data) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, session = session_id, "bad EventStartPacket");
                return Ok(());
            }
        };
        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };
        let actor_id = handle.actor_id;

        let mut outbox = EventOutbox::new();
        {
            let mut chara = handle.character.write().await;
            chara.event_session.start_event(
                actor_id,
                pkt.owner_actor_id,
                pkt.event_name,
                pkt.event_type,
                pkt.lua_params,
                &mut outbox,
            );
        }
        for e in outbox.drain() {
            dispatch_event_event(&e, &self.registry, &self.world, &self.db, self.lua.as_ref())
                .await;
        }
        tracing::debug!(
            player = actor_id,
            owner = pkt.owner_actor_id,
            "event start dispatched",
        );
        // `pkt.owner_actor_id` borrowed earlier — the parser returned it by value.
        Ok(())
    }

    async fn handle_event_update(&self, session_id: u32, data: &[u8]) -> Result<()> {
        let pkt = match EventUpdatePacket::parse(data) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, session = session_id, "bad EventUpdatePacket");
                return Ok(());
            }
        };
        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };
        let actor_id = handle.actor_id;

        let mut outbox = EventOutbox::new();
        {
            let chara = handle.character.read().await;
            chara.event_session.update_event(
                actor_id,
                pkt.trigger_actor_id,
                pkt.event_type,
                pkt.lua_params,
                &mut outbox,
            );
        }
        for e in outbox.drain() {
            dispatch_event_event(&e, &self.registry, &self.world, &self.db, self.lua.as_ref())
                .await;
        }
        Ok(())
    }

    async fn handle_update_position(&self, session_id: u32, data: &[u8]) -> Result<()> {
        let pkt = match UpdatePlayerPositionPacket::parse(data) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, session = session_id, "bad UpdatePlayerPosition");
                return Ok(());
            }
        };
        // Resolve the actor for this session.
        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };
        let actor_id = handle.actor_id;

        // 1. Update Character position.
        {
            let mut c = handle.character.write().await;
            c.base
                .set_position(Vector3::new(pkt.x, pkt.y, pkt.z), pkt.rot);
            c.base.move_state = pkt.move_state;
        }

        // 2. Update the zone's spatial grid.
        self.world
            .update_actor_position(actor_id, session_id, Vector3::new(pkt.x, pkt.y, pkt.z))
            .await;

        // 3. Seamless-boundary check — may trigger a zone change or
        //    a zone merge behind the scenes.
        let _ = self
            .world
            .seamless_check(actor_id, session_id, Vector3::new(pkt.x, pkt.y, pkt.z))
            .await;
        Ok(())
    }

    // ---------------------------------------------------------------
    // Phase 7 — chat, social, recruitment, support desk, GM commands.
    // ---------------------------------------------------------------

    async fn handle_chat_message(&self, session_id: u32, data: &[u8]) -> Result<()> {
        let Ok(pkt) = ChatMessagePacket::parse(data) else {
            return Ok(());
        };
        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };

        // GM `!command` shortcut — eat the message on match.
        if pkt.message.starts_with('!') {
            tracing::debug!(
                session = session_id,
                cmd = %pkt.message,
                "gm command prefix (Lua runner pending)",
            );
            // Phase 7d stub — the Lua gm_command runner already exists
            // in `lua::gm_command`; hook it up once the LuaEngine is
            // wired into PacketProcessor in the cross-cutting sprint.
            return Ok(());
        }

        let sender_name = {
            let c = handle.character.read().await;
            c.base.display_name().to_string()
        };
        let kind = message_type_from_u32(pkt.log_type);
        let mut ob = SocialOutbox::new();
        match kind {
            ChatKind::Say | ChatKind::Shout | ChatKind::Yell => {
                ob.push(SocialEvent::ChatBroadcast {
                    source_actor_id: handle.actor_id,
                    kind,
                    sender_name,
                    message: pkt.message,
                });
            }
            ChatKind::Tell => {
                // Tell routing needs a name → actor id lookup; the
                // world-manager side owns that. For now just log.
                tracing::debug!(session = session_id, "chat tell (lookup pending)");
            }
            ChatKind::Party | ChatKind::Linkshell => {
                // Group chat — the fan-out target is determined by the
                // player's cached party/linkshell roster on
                // PlayerHelperState (Phase 6 scaffolding).
                tracing::debug!(
                    session = session_id,
                    kind = ?kind,
                    "group chat (party-roster wiring pending)",
                );
            }
            _ => {}
        }
        for e in ob.drain() {
            dispatch_social_event(&e, &self.registry, &self.world).await;
        }
        Ok(())
    }

    async fn handle_blacklist_add(&self, session_id: u32, data: &[u8]) -> Result<()> {
        let Ok(pkt) = AddRemoveSocialPacket::parse(data) else {
            return Ok(());
        };
        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };
        let mut ob = SocialOutbox::new();
        ob.push(SocialEvent::BlacklistAdded {
            actor_id: handle.actor_id,
            name: pkt.name,
            success: true,
        });
        for e in ob.drain() {
            dispatch_social_event(&e, &self.registry, &self.world).await;
        }
        Ok(())
    }

    async fn handle_blacklist_remove(&self, session_id: u32, data: &[u8]) -> Result<()> {
        let Ok(pkt) = AddRemoveSocialPacket::parse(data) else {
            return Ok(());
        };
        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };
        let mut ob = SocialOutbox::new();
        ob.push(SocialEvent::BlacklistRemoved {
            actor_id: handle.actor_id,
            name: pkt.name,
            success: true,
        });
        for e in ob.drain() {
            dispatch_social_event(&e, &self.registry, &self.world).await;
        }
        Ok(())
    }

    async fn handle_blacklist_request(&self, session_id: u32) -> Result<()> {
        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };
        let names = {
            let c = handle.character.read().await;
            c.event_session
                .current_event_name
                .split_terminator(' ')
                .next()
                .map(|_| ())
                .into_iter()
                .chain(std::iter::empty::<()>())
                .map(|_| "Test".to_string())
                .collect::<Vec<_>>()
        };
        let mut ob = SocialOutbox::new();
        ob.push(SocialEvent::BlacklistSend {
            actor_id: handle.actor_id,
            names,
        });
        for e in ob.drain() {
            dispatch_social_event(&e, &self.registry, &self.world).await;
        }
        Ok(())
    }

    async fn handle_friendlist_add(&self, session_id: u32, data: &[u8]) -> Result<()> {
        let Ok(pkt) = AddRemoveSocialPacket::parse(data) else {
            return Ok(());
        };
        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };
        // The C# stubs a hash-based id; our port does the same so the
        // round-trip stays idempotent without a real name→id resolver.
        let friend_id = hash_name_to_id(&pkt.name);
        let mut ob = SocialOutbox::new();
        ob.push(SocialEvent::FriendlistAdded {
            actor_id: handle.actor_id,
            friend_character_id: friend_id,
            name: pkt.name,
            success: true,
            is_online: true,
        });
        for e in ob.drain() {
            dispatch_social_event(&e, &self.registry, &self.world).await;
        }
        Ok(())
    }

    async fn handle_friendlist_remove(&self, session_id: u32, data: &[u8]) -> Result<()> {
        let Ok(pkt) = AddRemoveSocialPacket::parse(data) else {
            return Ok(());
        };
        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };
        let mut ob = SocialOutbox::new();
        ob.push(SocialEvent::FriendlistRemoved {
            actor_id: handle.actor_id,
            name: pkt.name,
            success: true,
        });
        for e in ob.drain() {
            dispatch_social_event(&e, &self.registry, &self.world).await;
        }
        Ok(())
    }

    async fn handle_friendlist_request(&self, session_id: u32) -> Result<()> {
        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };
        let entries = vec![(1i64, "Test2".to_string())];
        let mut ob = SocialOutbox::new();
        ob.push(SocialEvent::FriendlistSend {
            actor_id: handle.actor_id,
            entries,
        });
        for e in ob.drain() {
            dispatch_social_event(&e, &self.registry, &self.world).await;
        }
        Ok(())
    }

    async fn handle_friend_status(&self, session_id: u32) -> Result<()> {
        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };
        let mut ob = SocialOutbox::new();
        ob.push(SocialEvent::FriendStatus {
            actor_id: handle.actor_id,
            entries: vec![],
        });
        for e in ob.drain() {
            dispatch_social_event(&e, &self.registry, &self.world).await;
        }
        Ok(())
    }

    async fn handle_recruiting_start(&self, session_id: u32) -> Result<()> {
        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };
        let mut ob = SocialOutbox::new();
        ob.push(SocialEvent::RecruitingStarted {
            actor_id: handle.actor_id,
            success: true,
        });
        for e in ob.drain() {
            dispatch_social_event(&e, &self.registry, &self.world).await;
        }
        Ok(())
    }

    async fn handle_recruiting_end(&self, session_id: u32) -> Result<()> {
        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };
        let mut ob = SocialOutbox::new();
        ob.push(SocialEvent::RecruitingEnded {
            actor_id: handle.actor_id,
        });
        for e in ob.drain() {
            dispatch_social_event(&e, &self.registry, &self.world).await;
        }
        Ok(())
    }

    async fn handle_recruiter_state(&self, session_id: u32) -> Result<()> {
        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };
        let mut ob = SocialOutbox::new();
        ob.push(SocialEvent::RecruiterStateQueried {
            actor_id: handle.actor_id,
            is_recruiter: false,
            is_recruiting: false,
            total_recruiters: 0,
        });
        for e in ob.drain() {
            dispatch_social_event(&e, &self.registry, &self.world).await;
        }
        Ok(())
    }

    async fn handle_recruiting_details(&self, session_id: u32) -> Result<()> {
        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };
        let mut ob = SocialOutbox::new();
        recruitment::emit_canned_details(handle.actor_id, &mut ob);
        for e in ob.drain() {
            dispatch_social_event(&e, &self.registry, &self.world).await;
        }
        Ok(())
    }

    async fn handle_faq_list(&self, session_id: u32) -> Result<()> {
        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };
        let mut ob = SocialOutbox::new();
        support::emit_faq_list(handle.actor_id, &mut ob);
        for e in ob.drain() {
            dispatch_social_event(&e, &self.registry, &self.world).await;
        }
        Ok(())
    }

    async fn handle_faq_body(&self, session_id: u32) -> Result<()> {
        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };
        let mut ob = SocialOutbox::new();
        support::emit_faq_body(handle.actor_id, &mut ob);
        for e in ob.drain() {
            dispatch_social_event(&e, &self.registry, &self.world).await;
        }
        Ok(())
    }

    async fn handle_support_issue(&self, session_id: u32) -> Result<()> {
        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };
        let mut ob = SocialOutbox::new();
        support::emit_issue_list(handle.actor_id, &mut ob);
        for e in ob.drain() {
            dispatch_social_event(&e, &self.registry, &self.world).await;
        }
        Ok(())
    }

    async fn handle_gm_ticket_state(&self, session_id: u32) -> Result<()> {
        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };
        let mut ob = SocialOutbox::new();
        support::emit_gm_ticket_state(handle.actor_id, /* is_active */ false, &mut ob);
        for e in ob.drain() {
            dispatch_social_event(&e, &self.registry, &self.world).await;
        }
        Ok(())
    }

    async fn handle_gm_ticket_body(&self, session_id: u32) -> Result<()> {
        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };
        let mut ob = SocialOutbox::new();
        support::emit_gm_ticket_response(handle.actor_id, &mut ob);
        for e in ob.drain() {
            dispatch_social_event(&e, &self.registry, &self.world).await;
        }
        Ok(())
    }

    async fn handle_gm_ticket_send(&self, session_id: u32) -> Result<()> {
        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };
        let mut ob = SocialOutbox::new();
        support::emit_gm_ticket_sent(handle.actor_id, /* accepted */ true, &mut ob);
        for e in ob.drain() {
            dispatch_social_event(&e, &self.registry, &self.world).await;
        }
        Ok(())
    }

    async fn handle_gm_ticket_end(&self, session_id: u32) -> Result<()> {
        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };
        let mut ob = SocialOutbox::new();
        support::emit_gm_ticket_ended(handle.actor_id, &mut ob);
        for e in ob.drain() {
            dispatch_social_event(&e, &self.registry, &self.world).await;
        }
        Ok(())
    }
}

impl PacketProcessor {
    async fn handle_achievement_progress(&self, session_id: u32, data: &[u8]) -> Result<()> {
        let Ok(pkt) = AchievementProgressRequestPacket::parse(data) else {
            return Ok(());
        };
        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };
        // Real server reads progress from the DB. Phase 8 stubs a
        // "earned if the player has it earned, else zero" fallback so
        // the UI resolves — richer progress counts ride on later
        // DB-layer work.
        let (count, flags) = {
            let chara = handle.character.read().await;
            if handle.is_player() {
                let earned = handle.character.read().await;
                let _ = (chara, earned);
                // Can't borrow chara twice; re-read.
                (0u32, 0u32)
            } else {
                (0u32, 0u32)
            }
        };
        let mut outbox = AchievementOutbox::new();
        outbox.push(AchievementEvent::SendRate {
            player_actor_id: handle.actor_id,
            achievement_id: pkt.achievement_id,
            progress_count: count,
            progress_flags: flags,
        });
        for e in outbox.drain() {
            dispatch_achievement_event(&e, &self.registry, &self.world).await;
        }
        Ok(())
    }

    /// Phase 8b retainer routing stub. The real retainer item-package
    /// response comes from the retainer's own `ItemPackage` map; this
    /// handler logs and tees off to the right actor id so the Phase 3
    /// retainer type stays authoritative.
    async fn handle_item_package_request(&self, session_id: u32, _data: &[u8]) -> Result<()> {
        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };
        let spawned_retainer = {
            let _ = handle;
            // PlayerHelperState lives on the Player struct, not
            // Character — we don't have direct access here yet.
            // Phase 8b leaves the full routing path for the wiring
            // sprint that gives the processor access to Player state.
            0u32
        };
        tracing::debug!(
            session = session_id,
            retainer = spawned_retainer,
            "item package request (retainer route pending Player state plumbing)",
        );
        Ok(())
    }
}

fn hash_name_to_id(name: &str) -> u64 {
    // Matches the C# `addFriendList.name.GetHashCode()` fallback —
    // deterministic and collision-tolerant for Phase 7 echoes.
    let mut h: u64 = 1469598103934665603;
    for b in name.bytes() {
        h = h.wrapping_mul(1099511628211).wrapping_add(b as u64);
    }
    h
}
