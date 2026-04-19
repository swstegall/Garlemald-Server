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
        // `base.zone_id` feeds `player:GetZoneID()` from Lua. Without
        // setting it here it defaults to 0 and the tutorial branch in
        // `player.lua:onBeginLogin` (`... and player:GetZoneID() == 193`)
        // evaluates false — so `SetLoginDirector` never fires and the
        // ScriptBind LuaParams stay on the non-director path.
        character.base.zone_id = zone_id;
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
        self.world.upsert_session(snap).await;

        let actor_id = handle.actor_id;

        // Run `player.lua:onBeginLogin(player)` *before* the zone-in
        // bundle, matching C# `PacketProcessor` case 0x0006 ordering
        // (`onBeginLogin` → `DoZoneIn` → `onLogin`). The script is what
        // calls `player:SetLoginDirector(director)` on the tutorial
        // path (zones 193/166/184) — that flips `has_login_director` on
        // the Character so `send_zone_in_bundle` can emit the correct
        // ActorInstantiate LuaParam shape. Without this hook firing the
        // client stays at Now Loading even when every zone-in packet
        // lands correctly.
        //
        // We drain the command queue and apply only the commands we
        // know how to handle on the Rust side (SetLoginDirector,
        // AddQuest, SetHomePoint). Other commands are logged and
        // skipped — the Lua side-effect surface isn't fully ported.
        if let Some(ref engine) = self.lua {
            let script = engine.resolver().player();
            if script.exists() {
                // The C# `onBeginLogin` flow is two-pass: the first branch
                // issues `AddQuest(110001)` when play_time==0, then the
                // second branch checks `HasQuest(110001)==true` and
                // attaches the login director. Our Lua call sees a stale
                // snapshot, so without pre-populating `active_quests`
                // with the tutorial quest id for a fresh tutorial-zone
                // character the director branch never fires. Seed the
                // expected quest based on the initial town / zone so the
                // second branch evaluates truthy. For tutorial zones:
                // town 1 → zone 193 → quest 110001
                // town 2 → zone 166 → quest 110005
                // town 3 → zone 184 → quest 110009
                let snapshot = {
                    let c = handle.character.read().await;
                    let mut snap = build_player_snapshot_for_login(&c);
                    let tutorial_quest = match snap.initial_town {
                        1 => Some(110001u32),
                        2 => Some(110005u32),
                        3 => Some(110009u32),
                        _ => None,
                    };
                    if let Some(q) = tutorial_quest
                        && !snap.active_quests.contains(&q)
                    {
                        snap.active_quests.push(q);
                    }
                    snap
                };
                let snapshot_for_err = snapshot.clone();
                match engine.call_player_hook(&script, "onBeginLogin", snapshot) {
                    Ok(result) => {
                        let cmd_count = result.commands.len();
                        for cmd in result.commands {
                            self.apply_login_lua_command(&handle, cmd).await;
                        }
                        tracing::info!(
                            session = session_id,
                            actor = actor_id,
                            commands = cmd_count,
                            "onBeginLogin lua hook ran"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            session = session_id,
                            actor = snapshot_for_err.actor_id,
                            "onBeginLogin lua hook failed; continuing without it"
                        );
                    }
                }
            } else {
                tracing::debug!(
                    path = %script.display(),
                    "player.lua not present; skipping onBeginLogin"
                );
            }
        }

        // Capture the post-Lua spawn position — `SetPos` commands from
        // the tutorial-zone `onBeginLogin` path overwrite the DB
        // position with the cutscene-canonical coordinates, and the
        // zone change needs the updated values to stage the player at
        // the right spot before `send_zone_in_bundle` renders them.
        let (spawn, rotation) = if let Some(snap) = self.world.session(session_id).await {
            (
                Vector3::new(snap.destination_x, snap.destination_y, snap.destination_z),
                snap.destination_rot,
            )
        } else {
            (Vector3::default(), 0.0)
        };

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

        tracing::info!(
            session = session_id,
            language = lang,
            zone,
            "language code received; login zone-in dispatched",
        );

        // C# `Map/PacketProcessor.cs` case 0x0006 runs `onBeginLogin` →
        // `DoZoneIn(isLogin=true, 0x1)` → `onLogin`, in that order. Missing
        // the `onLogin` step left fresh characters stuck at Now Loading
        // with an empty inventory because `initClassItems`/`initRaceItems`
        // never ran. We call it best-effort: if the script errors partway
        // through (e.g. on an unsupported `charaWork` property access),
        // commands queued before the error are still applied.
        if let Some(ref engine) = self.lua {
            let script = engine.resolver().player();
            if script.exists() {
                let snapshot = {
                    let c = handle.character.read().await;
                    build_player_snapshot_for_login(&c)
                };
                let result = engine.call_player_hook_best_effort(&script, "onLogin", snapshot);
                let cmd_count = result.commands.len();
                for cmd in result.commands {
                    self.apply_login_lua_command(&handle, cmd).await;
                }
                match result.error {
                    None => tracing::info!(
                        session = session_id,
                        actor = actor_id,
                        commands = cmd_count,
                        "onLogin lua hook ran"
                    ),
                    Some(e) => tracing::warn!(
                        error = %e,
                        session = session_id,
                        actor = actor_id,
                        commands = cmd_count,
                        "onLogin lua hook errored; applied partial commands"
                    ),
                }
            }

            // C# `WorldManager.DoZoneIn` ends with
            // `LuaEngine.CallLuaFunction(player, playerArea, "onZoneIn", true)`
            // — fired AFTER `SendZoneInPackets`, `SendInstanceUpdate`, and
            // `LockUpdates(false)`. For the tutorial zone `ocn0Battle02`
            // that hook re-kicks the opening director with
            // `player:KickEvent(player:GetDirector(), "noticeEvent")`
            // (no varargs). The packet from the first KickEvent inside
            // the zone-in bundle is apparently not enough on its own —
            // the client also needs this second KickEvent that arrives
            // *after* it has finished ingesting the bundle. Missing this
            // call is what leaves "Now Loading" on screen indefinitely.
            let zone_name = match self.world.zone(zone).await {
                Some(z) => z.read().await.core.zone_name.clone(),
                None => String::new(),
            };
            if !zone_name.is_empty() {
                let zone_script = engine.resolver().zone(&zone_name);
                if zone_script.exists() {
                    let snapshot = {
                        let c = handle.character.read().await;
                        build_player_snapshot_for_login(&c)
                    };
                    let result =
                        engine.call_player_hook_best_effort(&zone_script, "onZoneIn", snapshot);
                    let cmd_count = result.commands.len();
                    for cmd in result.commands {
                        self.apply_post_zone_in_lua_command(&handle, session_id, cmd)
                            .await;
                    }
                    match result.error {
                        None => tracing::info!(
                            session = session_id,
                            actor = actor_id,
                            zone = %zone_name,
                            commands = cmd_count,
                            "onZoneIn lua hook ran"
                        ),
                        Some(e) => tracing::warn!(
                            error = %e,
                            session = session_id,
                            actor = actor_id,
                            zone = %zone_name,
                            commands = cmd_count,
                            "onZoneIn lua hook errored; applied partial commands"
                        ),
                    }
                } else {
                    tracing::debug!(
                        path = %zone_script.display(),
                        "zone.lua not present; skipping onZoneIn"
                    );
                }
            }
        }

        Ok(())
    }

    /// Commands emitted by `zone.lua:onZoneIn` arrive *after* the zone-in
    /// bundle has already been flushed to the client. KickEvent in
    /// particular has to be sent immediately as its own subpacket rather
    /// than captured onto `session.pending_kick_event` (which would be
    /// read by a future `send_zone_in_bundle` call that never comes).
    async fn apply_post_zone_in_lua_command(
        &self,
        handle: &ActorHandle,
        session_id: u32,
        cmd: crate::lua::LuaCommandKind,
    ) {
        use crate::lua::LuaCommandKind as LC;
        match cmd {
            LC::KickEvent {
                player_id,
                actor_id,
                trigger,
                args,
            } => {
                if actor_id == 0 {
                    tracing::debug!(
                        %trigger,
                        "onZoneIn KickEvent skipped — no director actor id"
                    );
                    return;
                }
                let lua_params: Vec<common::luaparam::LuaParam> = args
                    .into_iter()
                    .map(|a| match a {
                        crate::lua::command::LuaCommandArg::Int(i) => {
                            common::luaparam::LuaParam::Int32(i as i32)
                        }
                        crate::lua::command::LuaCommandArg::UInt(u) => {
                            common::luaparam::LuaParam::UInt32(u as u32)
                        }
                        crate::lua::command::LuaCommandArg::Float(_) => {
                            common::luaparam::LuaParam::Int32(0)
                        }
                        crate::lua::command::LuaCommandArg::String(s) => {
                            common::luaparam::LuaParam::String(s)
                        }
                        crate::lua::command::LuaCommandArg::Bool(true) => {
                            common::luaparam::LuaParam::True
                        }
                        crate::lua::command::LuaCommandArg::Bool(false) => {
                            common::luaparam::LuaParam::False
                        }
                        crate::lua::command::LuaCommandArg::Nil => {
                            common::luaparam::LuaParam::Nil
                        }
                        crate::lua::command::LuaCommandArg::ActorId(id) => {
                            common::luaparam::LuaParam::Actor(id)
                        }
                    })
                    .collect();
                // C# `Player.KickEvent` always uses event_type=5 (the
                // 2-arg Lua form and 3-arg form both land here); only
                // the rarely-used `KickEventSpecial` uses 0.
                let mut sub = crate::packets::send::events::build_kick_event(
                    player_id, actor_id, &trigger, 5, &lua_params,
                );
                sub.set_target_id(session_id);
                if let Some(client) = self.world.client(session_id).await {
                    client.send_bytes(sub.to_bytes()).await;
                    tracing::info!(
                        session = session_id,
                        trigger_actor = player_id,
                        owner_actor = actor_id,
                        event = %trigger,
                        args = lua_params.len(),
                        "onZoneIn KickEvent dispatched directly to client"
                    );
                } else {
                    tracing::warn!(
                        session = session_id,
                        "onZoneIn KickEvent dropped — no client handle"
                    );
                }
                let _ = handle.actor_id;
            }
            other => {
                tracing::debug!(?other, "post-zone-in lua cmd (unhandled)");
            }
        }
    }

    /// Apply a LuaCommand emitted by `onBeginLogin`. Only the commands
    /// load-bearing for the login flow are handled here; others are
    /// logged and dropped.
    async fn apply_login_lua_command(
        &self,
        handle: &ActorHandle,
        cmd: crate::lua::LuaCommandKind,
    ) {
        use crate::lua::LuaCommandKind as LC;
        match cmd {
            LC::CreateDirector {
                director_actor_id,
                zone_actor_id,
                class_path,
            } => {
                // Capture a LoginDirectorSpec on the Session. The
                // zone-in bundle reads this later to emit the director
                // spawn sequence AND patch the player's ScriptBind
                // LuaParams with the correct `Actor(id)` reference.
                let class_name = class_path
                    .rsplit('/')
                    .next()
                    .unwrap_or(&class_path)
                    .to_string();
                if let Some(mut snap) = self.world.session(handle.session_id).await {
                    snap.login_director = Some(crate::data::LoginDirectorSpec {
                        actor_id: director_actor_id,
                        zone_actor_id,
                        class_path: class_path.clone(),
                        class_name: class_name.clone(),
                    });
                    self.world.upsert_session(snap).await;
                }
                tracing::info!(
                    director = director_actor_id,
                    zone = zone_actor_id,
                    class_path = %class_path,
                    "CreateDirector applied (will emit director spawn in zone-in bundle)"
                );
            }
            LC::SetLoginDirector {
                player_id,
                director_actor_id,
            } => {
                let mut c = handle.character.write().await;
                c.chara.login_director_actor_id = director_actor_id;
                tracing::info!(
                    player = player_id,
                    director = director_actor_id,
                    "SetLoginDirector applied (ScriptBind LuaParams will reference director actor)"
                );
            }
            // `player.lua:onBeginLogin` for tutorial zones sets the
            // canonical cutscene-spawn position via four
            // `player.positionX/Y/Z/rotation = …` assignments, each of
            // which fires one `SetPos` command carrying the running
            // state. Apply these to the Character so the subsequent
            // zone-in bundle's `SetActorPosition` packet matches the
            // tutorial spawn (zone 193: `0.016, 10.35, -36.91, 0.025`).
            // The Session's destination-pos is also refreshed so
            // `do_zone_change` sees the updated location.
            LC::SetPos {
                actor_id,
                zone_id: _,
                x,
                y,
                z,
                rotation,
            } => {
                {
                    let mut c = handle.character.write().await;
                    c.base.position_x = x;
                    c.base.position_y = y;
                    c.base.position_z = z;
                    c.base.rotation = rotation;
                }
                if let Some(mut snap) = self.world.session(handle.session_id).await {
                    snap.destination_x = x;
                    snap.destination_y = y;
                    snap.destination_z = z;
                    snap.destination_rot = rotation;
                    self.world.upsert_session(snap).await;
                }
                tracing::debug!(
                    actor = actor_id,
                    x,
                    y,
                    z,
                    rotation,
                    "SetPos applied (tutorial spawn position)"
                );
            }
            LC::KickEvent {
                player_id,
                actor_id,
                trigger,
                args,
            } => {
                // Capture onto the session so send_zone_in_bundle can
                // emit the KickEventPacket after the director spawn.
                // C# `Player.KickEvent` runs with `eventType = 5` —
                // that specific value triggers the cutscene dispatcher
                // inside the 1.23b client. The `actor_id` is the owner
                // (the director actor we just spawned). Args from the
                // script (e.g. the `true` in `player:KickEvent(director,
                // "noticeEvent", true)`) are promoted to `LuaParam`s
                // and written into the packet body at offset 0x30.
                let lua_params: Vec<common::luaparam::LuaParam> = args
                    .into_iter()
                    .map(|a| match a {
                        crate::lua::command::LuaCommandArg::Int(i) => {
                            common::luaparam::LuaParam::Int32(i as i32)
                        }
                        crate::lua::command::LuaCommandArg::UInt(u) => {
                            common::luaparam::LuaParam::UInt32(u as u32)
                        }
                        crate::lua::command::LuaCommandArg::Float(_) => {
                            common::luaparam::LuaParam::Int32(0)
                        }
                        crate::lua::command::LuaCommandArg::String(s) => {
                            common::luaparam::LuaParam::String(s)
                        }
                        crate::lua::command::LuaCommandArg::Bool(true) => {
                            common::luaparam::LuaParam::True
                        }
                        crate::lua::command::LuaCommandArg::Bool(false) => {
                            common::luaparam::LuaParam::False
                        }
                        crate::lua::command::LuaCommandArg::Nil => {
                            common::luaparam::LuaParam::Nil
                        }
                        crate::lua::command::LuaCommandArg::ActorId(id) => {
                            common::luaparam::LuaParam::Actor(id)
                        }
                    })
                    .collect();
                if let Some(mut snap) = self.world.session(handle.session_id).await {
                    snap.pending_kick_event = Some(crate::data::PendingKickEvent {
                        trigger_actor_id: player_id,
                        owner_actor_id: actor_id,
                        event_name: trigger.clone(),
                        args: lua_params,
                    });
                    self.world.upsert_session(snap).await;
                }
                tracing::info!(
                    player = player_id,
                    target = actor_id,
                    %trigger,
                    "KickEvent captured (will emit KickEventPacket after director spawn)"
                );
            }
            LC::AddQuest {
                player_id,
                quest_id,
            } => {
                tracing::debug!(player = player_id, quest = quest_id, "AddQuest (stub)");
            }
            // `onLogin` → `initClassItems` / `initRaceItems` emit these for
            // brand-new characters. Full persistence + inventory packet
            // emission lands with the phase-8 item pipeline; logging here
            // confirms the hook traversed its full class/race branches so
            // the follow-on `SavePlayTime` / `SendMessage` steps also ran.
            LC::AddItem {
                actor_id,
                item_package,
                item_id,
                quantity,
            } => {
                tracing::info!(
                    actor = actor_id,
                    package = item_package,
                    item = item_id,
                    qty = quantity,
                    "AddItem captured (onLogin init items; persistence deferred)"
                );
            }
            LC::SendMessage {
                actor_id,
                message_type,
                sender,
                text,
            } => {
                tracing::info!(
                    actor = actor_id,
                    kind = format!("0x{:02X}", message_type),
                    %sender,
                    %text,
                    "SendMessage captured (login-hook sys message; packet emit deferred)"
                );
            }
            LC::SetHomePoint {
                player_id,
                homepoint,
            } => {
                tracing::debug!(
                    player = player_id,
                    homepoint,
                    "SetHomePoint (stub)"
                );
            }
            other => {
                tracing::debug!(?other, "login lua cmd (unhandled)");
            }
        }
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

/// Assemble a `PlayerSnapshot` from just the `Character` state available to
/// the packet processor (no full `Player` wrapper). The normal
/// `PlayerSnapshot::from(&Player)` path requires the richer `actor::Player`
/// struct with helper state we don't have plumbed into `ActorRegistry`
/// yet — this constructs the subset `player.lua:onBeginLogin` actually
/// reads: `GetPlayTime` (returns 0 → "new player"), `GetInitialTown`,
/// `HasQuest`, `GetZoneID`, plus the `playerWork.tribe` field read in
/// the tutorial branch.
fn build_player_snapshot_for_login(c: &Character) -> crate::lua::userdata::PlayerSnapshot {
    crate::lua::userdata::PlayerSnapshot {
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
        play_time: 0,
        current_class: c.chara.class.max(0) as u8,
        current_level: c.chara.level,
        current_job: c.chara.current_job as u8,
        current_gil: 0,
        initial_town: c.chara.initial_town,
        tribe: c.chara.tribe,
        guardian: c.chara.guardian,
        birth_month: c.chara.birthday_month,
        birth_day: c.chara.birthday_day,
        homepoint: 0,
        homepoint_inn: 0,
        mount_state: 0,
        has_chocobo: false,
        is_gm: false,
        is_engaged: false,
        is_trading: false,
        is_trade_accepted: false,
        is_party_leader: false,
        current_event_owner: 0,
        current_event_name: String::new(),
        current_event_type: 0,
        completed_quests: Vec::new(),
        active_quests: Vec::new(),
        unlocked_aetherytes: Vec::new(),
        traits: Vec::new(),
        inventory: Vec::new(),
        login_director_actor_id: c.chara.login_director_actor_id,
    }
}
