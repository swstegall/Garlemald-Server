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
        // `characters_parametersave` stores a single hp/hpMax value (not
        // per-class), and `load_parameter_save` writes it into `hp[0]`
        // regardless of current class — matching C# `LoadPlayerCharacter`
        // in Project Meteor's `Map Server/Database.cs:858`. Reading
        // `hp[class_slot]` for a non-PUG character hit the default-zero
        // slots, delivering `hp=0 hpMax=0` to the client and flipping its
        // CharaBase into a death-nameplate path that indexes the
        // uninitialised death-depictor config — the nil-index at
        // `DepictionJudge:judgeNameplate() line 900`. Always read index 0.
        let _ = class_slot_safe;
        let hp_max = loaded.parameter_save.hp_max[0];
        let mp_max = loaded.parameter_save.mp_max;
        // Seed the ModifierMap with the DB's stored max HP / MP so
        // `Character::calculate_base_stats` (port of C#
        // `Character.CalculateBaseStats` in `chara.rs`) has non-zero
        // `Modifier::Hp` / `Modifier::Mp` values to project into the
        // character's HP/MP pools. For Project Meteor the equivalent
        // wiring lives in equip/trait handlers that accumulate stats
        // into the modifier map; we're not there yet, so the lobby's
        // `characters_parametersave` row (`hp=1900 hpMax=1000`) is the
        // single source of truth at login. Current HP and MP are then
        // set by `calculate_base_stats` from the Hp/Mp modifiers, so we
        // don't need to plumb them through the processor separately.
        let hp = hp_max;
        let mp = mp_max;

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
        // Seed the battle-modifier map with the DB max values, then run
        // `calculate_base_stats` — port of C# `Character.CalculateBaseStats`
        // (`actor/chara.rs:113`) which reads `Modifier::Hp` / `HpPercent`
        // / `Mp` / `MpPercent` and projects them onto the char's HP/MP
        // pools. For a fresh Project-Meteor-style login the modifier map
        // is otherwise empty, so without this seed `calculate_base_stats`
        // would leave HP/MP at zero and the client would snap into
        // death-nameplate mode during its first `_onUpdateWork` tick.
        // The `hp`/`mp`/`max_hp`/`max_mp` assignments below are redundant
        // with what `calculate_base_stats` writes, but they keep the
        // character's pools consistent if any future refactor bypasses
        // the recalc path.
        character.chara.hp = hp;
        character.chara.max_hp = hp_max;
        character.chara.mp = mp;
        character.chara.max_mp = mp_max;
        character.chara.mods.set(
            crate::actor::modifier::Modifier::Hp,
            hp_max as f64,
        );
        character.chara.mods.set(
            crate::actor::modifier::Modifier::Mp,
            mp_max as f64,
        );
        character.calculate_base_stats();
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

        // Hydrate the quest journal from the DB. `loaded.quest_scenario`
        // holds the active-slot rows (sequence/flags/counters) and the
        // separate bitfield column feeds the 2048-bit completion set.
        // Previously this data was loaded but dropped on the floor because
        // the runtime Player's helpers.quest_journal wasn't reachable from
        // the processor — now that `quest_journal` lives on Character the
        // zone-in bundle and any Lua hook see the real state.
        for row in &loaded.quest_scenario {
            let slot = row.slot as usize;
            if slot >= 16 {
                continue;
            }
            let actor_aid = crate::actor::quest::quest_actor_id(row.quest_id);
            character.quest_journal.slots[slot] =
                Some(crate::actor::quest::Quest::from_db_row(
                    actor_aid,
                    String::new(),
                    row.sequence,
                    row.flags,
                    row.counter1,
                    row.counter2,
                    row.counter3,
                ));
        }
        match self.db.load_completed_quests(actor_id).await {
            Ok(bs) => character.quest_journal.completed = bs,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    actor = actor_id,
                    "load_completed_quests failed; starting with empty bitfield",
                );
            }
        }

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
                // The login-director branch in `scripts/lua/player.lua` is
                // gated on `HasQuest(110001) == true`, but the matching
                // `AddQuest(110001)` in the first half of `onBeginLogin` is
                // commented out in Meteor's upstream source — so the
                // director branch is dead code on a canonical Asdf-style
                // login and no OpeningDirector gets created. A previous
                // port of this handler seeded the tutorial quest here to
                // "make the director branch fire," which spawned an extra
                // Director actor and flipped the Player's ScriptBind
                // LuaParam list to the 9-param with-director variant.
                // The client's `DepictionJudge:judgeNameplate` then hit a
                // nil field ~10s in and bounced the session with the
                // EventStart-wrapped Lua error report we saw earlier.
                // Feed the Lua call the real snapshot.
                let snapshot = {
                    let c = handle.character.read().await;
                    build_player_snapshot_for_login(&c)
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
    ///
    /// Marked `pub(crate)` so integration tests can drive the full
    /// command pipeline directly — the real server only reaches this
    /// from `handle_session_begin` / `onZoneIn` drain paths.
    pub(crate) async fn apply_login_lua_command(
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
                self.apply_add_quest(player_id, quest_id).await;
            }
            LC::CompleteQuest {
                player_id,
                quest_id,
            } => {
                self.apply_complete_quest(player_id, quest_id).await;
            }
            LC::AbandonQuest {
                player_id,
                quest_id,
            } => {
                self.apply_abandon_quest(player_id, quest_id).await;
            }
            LC::QuestClearData {
                player_id,
                quest_id,
            } => {
                self.apply_quest_mutation(player_id, quest_id, |q| q.clear_data())
                    .await;
            }
            LC::QuestClearFlags {
                player_id,
                quest_id,
            } => {
                self.apply_quest_mutation(player_id, quest_id, |q| q.clear_flags())
                    .await;
            }
            LC::QuestSetFlag {
                player_id,
                quest_id,
                bit,
            } => {
                self.apply_quest_mutation(player_id, quest_id, |q| q.set_flag(bit))
                    .await;
            }
            LC::QuestClearFlag {
                player_id,
                quest_id,
                bit,
            } => {
                self.apply_quest_mutation(player_id, quest_id, |q| q.clear_flag(bit))
                    .await;
            }
            LC::QuestSetCounter {
                player_id,
                quest_id,
                idx,
                value,
            } => {
                self.apply_quest_mutation(player_id, quest_id, |q| {
                    q.set_counter(idx as usize, value)
                })
                .await;
            }
            LC::QuestIncCounter {
                player_id,
                quest_id,
                idx,
            } => {
                self.apply_quest_mutation(player_id, quest_id, |q| {
                    q.inc_counter(idx as usize);
                })
                .await;
            }
            LC::QuestDecCounter {
                player_id,
                quest_id,
                idx,
            } => {
                self.apply_quest_mutation(player_id, quest_id, |q| {
                    q.dec_counter(idx as usize);
                })
                .await;
            }
            LC::QuestStartSequence {
                player_id,
                quest_id,
                sequence,
            } => {
                self.apply_quest_start_sequence(player_id, quest_id, sequence)
                    .await;
            }
            LC::QuestSetEnpc {
                player_id,
                quest_id,
                actor_class_id,
                quest_flag_type,
                is_talk_enabled,
                is_push_enabled,
                is_emote_enabled,
                is_spawned,
            } => {
                self.apply_quest_set_enpc(
                    player_id,
                    quest_id,
                    actor_class_id,
                    quest_flag_type,
                    is_talk_enabled,
                    is_push_enabled,
                    is_emote_enabled,
                    is_spawned,
                )
                .await;
            }
            LC::QuestUpdateEnpcs {
                player_id,
                quest_id,
            } => {
                self.apply_quest_update_enpcs(player_id, quest_id).await;
            }
            LC::AddExp {
                actor_id,
                class_id,
                exp,
            } => {
                if exp == 0 {
                    return;
                }
                let class_slot = class_id as usize;
                let new_exp = {
                    let mut c = handle.character.write().await;
                    if class_slot >= c.battle_save.skill_point.len() {
                        tracing::warn!(class = class_id, "AddExp: class_id out of range");
                        return;
                    }
                    let updated = c.battle_save.skill_point[class_slot].saturating_add(exp).max(0);
                    c.battle_save.skill_point[class_slot] = updated;
                    updated
                };
                if let Err(e) = self.db.set_exp(actor_id, class_id, new_exp).await {
                    tracing::warn!(
                        actor = actor_id,
                        class = class_id,
                        err = %e,
                        "AddExp: DB persist failed",
                    );
                }
                tracing::info!(
                    actor = actor_id,
                    class = class_id,
                    delta = exp,
                    total = new_exp,
                    "AddExp applied",
                );
                // Level-up detection + level-up packet emission + XP-gained
                // client message deferred — needs the per-class level-curve
                // table (Meteor's `Player.GetLevelThreshold(...)`) which
                // hasn't been ported yet.
            }
            LC::AddGil { actor_id, amount } => {
                if amount == 0 {
                    return;
                }
                match self.db.add_gil(actor_id, amount).await {
                    Ok(total) => {
                        tracing::info!(
                            actor = actor_id,
                            delta = amount,
                            total,
                            "AddGil applied",
                        );
                        // Currency-package inventory refresh packet emission
                        // deferred — the next zone-in / explicit inventory
                        // resync reflects the new balance.
                    }
                    Err(e) => {
                        tracing::warn!(
                            actor = actor_id,
                            delta = amount,
                            err = %e,
                            "AddGil: DB persist failed",
                        );
                    }
                }
            }
            LC::Die { actor_id } => {
                let Some(zone) = self.world.zone(handle.zone_id).await else {
                    return;
                };
                crate::runtime::dispatcher::apply_die(
                    actor_id,
                    &self.registry,
                    &self.world,
                    &zone,
                )
                .await;
            }
            LC::Revive { actor_id } => {
                let Some(zone) = self.world.zone(handle.zone_id).await else {
                    return;
                };
                crate::runtime::dispatcher::apply_revive(
                    actor_id,
                    &self.registry,
                    &self.world,
                    &zone,
                )
                .await;
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

    // =======================================================================
    // Quest-mutation helpers (ported from Meteor's `Quest.cs` /
    // `QuestData.cs` runtime surface)
    // =======================================================================

    /// Resolve a player's active quest, run `mutate`, and — if the quest
    /// ended up dirty — persist the new `(sequence, flags, counters)`
    /// tuple to `characters_quest_scenario`. The dirty flag is cleared
    /// after the write so the next mutation reliably flips it again.
    ///
    /// No-ops if the player isn't live in the registry or doesn't have
    /// the quest in their journal (matches Meteor: mutations on a missing
    /// quest are silently ignored rather than panicking).
    async fn apply_quest_mutation<F>(&self, player_id: u32, quest_id: u32, mutate: F)
    where
        F: FnOnce(&mut crate::actor::quest::Quest),
    {
        let Some(handle) = self.registry.get(player_id).await else {
            tracing::debug!(
                player = player_id,
                quest = quest_id,
                "quest mutation skipped — player not in registry",
            );
            return;
        };
        let save_tuple = {
            let mut c = handle.character.write().await;
            let Some(slot) = c.quest_journal.slot_of(quest_id) else {
                tracing::debug!(
                    player = player_id,
                    quest = quest_id,
                    "quest mutation skipped — quest not in journal",
                );
                return;
            };
            let Some(q) = c.quest_journal.slots[slot].as_mut() else {
                return;
            };
            mutate(q);
            if q.is_dirty() {
                let sequence = q.get_sequence();
                let flags = q.get_flags();
                let counters = [q.get_counter(0), q.get_counter(1), q.get_counter(2)];
                let actor_id = q.actor_id;
                q.clear_dirty();
                Some((slot as i32, actor_id, sequence, flags, counters))
            } else {
                None
            }
        };
        if let Some((slot, actor_id, sequence, flags, [c1, c2, c3])) = save_tuple
            && let Err(e) = self
                .db
                .save_quest(player_id, slot, actor_id, sequence, flags, c1, c2, c3)
                .await
        {
            tracing::warn!(
                error = %e,
                player = player_id,
                quest = quest_id,
                "quest save failed",
            );
        }
    }

    /// `quest:StartSequence(sequence)` — bump the sequence number,
    /// persist, then run the ENPC diff pattern Meteor uses in
    /// `QuestState.UpdateState`: swap `current` → `old`, fire
    /// `onStateChange` (which re-registers surviving ENPCs via
    /// `quest:SetENpc(...)`), then drain whatever's left in `old` as
    /// clear-broadcasts.
    async fn apply_quest_start_sequence(&self, player_id: u32, quest_id: u32, sequence: u32) {
        self.apply_quest_mutation(player_id, quest_id, |q| q.start_sequence(sequence))
            .await;
        let Some(handle) = self.registry.get(player_id).await else {
            return;
        };
        // Swap the ENPC maps BEFORE the hook runs so `apply_quest_set_enpc`
        // sees a clean `current` and can correctly diff against `old`.
        {
            let mut c = handle.character.write().await;
            if let Some(q) = c.quest_journal.get_mut(quest_id) {
                q.state.begin_sequence_swap();
            }
        }

        self.fire_quest_hook(
            &handle,
            quest_id,
            "onStateChange",
            vec![crate::lua::QuestHookArg::Int(sequence as i64)],
        )
        .await;

        // Anything still in `old` after the hook is an ENPC the new
        // sequence didn't re-register — emit a clear for each.
        let stale: Vec<crate::actor::quest::QuestEnpc> = {
            let mut c = handle.character.write().await;
            match c.quest_journal.get_mut(quest_id) {
                Some(q) => q.state.drain_stale_enpcs().collect(),
                None => Vec::new(),
            }
        };
        for enpc in stale {
            self.broadcast_quest_enpc_clear(player_id, enpc).await;
        }
    }

    /// `quest:SetENpc(...)` handler. Mutates the live `QuestState`,
    /// then — if the `AddEnpcOutcome` reports a state change worth
    /// broadcasting — emits the matching event-status + quest-graphic
    /// packets to the player.
    async fn apply_quest_set_enpc(
        &self,
        player_id: u32,
        quest_id: u32,
        actor_class_id: u32,
        quest_flag_type: u8,
        is_talk_enabled: bool,
        is_push_enabled: bool,
        is_emote_enabled: bool,
        is_spawned: bool,
    ) {
        let Some(handle) = self.registry.get(player_id).await else {
            return;
        };
        let enpc = crate::actor::quest::QuestEnpc::new(
            actor_class_id,
            quest_flag_type,
            is_spawned,
            is_talk_enabled,
            is_emote_enabled,
            is_push_enabled,
        );
        let outcome = {
            let mut c = handle.character.write().await;
            let Some(q) = c.quest_journal.get_mut(quest_id) else {
                return;
            };
            q.state.add_enpc(enpc)
        };
        match outcome {
            crate::actor::quest::AddEnpcOutcome::Unchanged => {
                // Matches Meteor: silent when the ENPC carried over with
                // identical flags (no packet churn on sequences that just
                // re-register the same active list).
            }
            crate::actor::quest::AddEnpcOutcome::New(snapshot)
            | crate::actor::quest::AddEnpcOutcome::Updated(snapshot) => {
                self.broadcast_quest_enpc_update(player_id, snapshot).await;
            }
        }
    }

    /// `quest:UpdateENPCs()` handler — drain the stale half of the
    /// diff (ENPCs left over from the previous sequence that weren't
    /// re-registered) and emit a clear broadcast for each.
    async fn apply_quest_update_enpcs(&self, player_id: u32, quest_id: u32) {
        let Some(handle) = self.registry.get(player_id).await else {
            return;
        };
        let stale: Vec<crate::actor::quest::QuestEnpc> = {
            let mut c = handle.character.write().await;
            match c.quest_journal.get_mut(quest_id) {
                Some(q) => q.state.drain_stale_enpcs().collect(),
                None => Vec::new(),
            }
        };
        for enpc in stale {
            self.broadcast_quest_enpc_clear(player_id, enpc).await;
        }
    }

    /// Resolve the NPC by actor-class id inside the player's zone, then
    /// queue [`build_actor_event_status_packets`] + [`build_set_actor_quest_graphic`]
    /// against the player's session. No-ops when the NPC isn't live or
    /// the player has no active session (e.g. a scripted test harness).
    async fn broadcast_quest_enpc_update(
        &self,
        player_id: u32,
        enpc: crate::actor::quest::QuestEnpc,
    ) {
        let Some(player_handle) = self.registry.get(player_id).await else {
            return;
        };
        let session_id = player_handle.session_id;
        if session_id == 0 {
            return;
        }
        let Some(client) = self.world.client(session_id).await else {
            return;
        };

        let zone_id = player_handle.zone_id;
        let Some(npc_handle) = self
            .find_npc_by_class_id(zone_id, enpc.actor_class_id)
            .await
        else {
            tracing::debug!(
                player = player_id,
                class_id = enpc.actor_class_id,
                "quest ENPC broadcast skipped — no live NPC with that class id in zone",
            );
            return;
        };

        let (npc_actor_id, conditions) = {
            let c = npc_handle.character.read().await;
            (c.base.actor_id, c.base.event_conditions.clone())
        };

        let subpackets = crate::packets::send::build_actor_event_status_packets(
            npc_actor_id,
            &conditions,
            enpc.is_talk_enabled,
            enpc.is_emote_enabled,
            Some(enpc.is_push_enabled),
            /* notice_enabled */ true,
        );
        for sub in subpackets {
            client.send_bytes(sub.to_bytes()).await;
        }
        let graphic = crate::packets::send::build_set_actor_quest_graphic(
            npc_actor_id,
            enpc.quest_flag_type,
        );
        client.send_bytes(graphic.to_bytes()).await;
    }

    /// Clear-broadcast counterpart of [`broadcast_quest_enpc_update`].
    /// Emits every event-condition with `enabled=false` and the
    /// quest-graphic icon set to 0 so the client drops the marker.
    async fn broadcast_quest_enpc_clear(
        &self,
        player_id: u32,
        enpc: crate::actor::quest::QuestEnpc,
    ) {
        let Some(player_handle) = self.registry.get(player_id).await else {
            return;
        };
        let session_id = player_handle.session_id;
        if session_id == 0 {
            return;
        }
        let Some(client) = self.world.client(session_id).await else {
            return;
        };
        let zone_id = player_handle.zone_id;
        let Some(npc_handle) = self
            .find_npc_by_class_id(zone_id, enpc.actor_class_id)
            .await
        else {
            return;
        };
        let (npc_actor_id, conditions) = {
            let c = npc_handle.character.read().await;
            (c.base.actor_id, c.base.event_conditions.clone())
        };

        let subpackets = crate::packets::send::build_actor_event_status_packets(
            npc_actor_id,
            &conditions,
            /* talk */ false,
            /* emote */ false,
            /* push */ Some(false),
            /* notice */ false,
        );
        for sub in subpackets {
            client.send_bytes(sub.to_bytes()).await;
        }
        let graphic = crate::packets::send::build_set_actor_quest_graphic(npc_actor_id, 0);
        client.send_bytes(graphic.to_bytes()).await;
    }

    /// Linear scan of the zone's actor roster for an NPC whose
    /// `actor_class_id` matches `class_id`. Quest scripts typically
    /// register 2-8 ENPCs per sequence so per-call O(n) isn't a hot
    /// path; a proper index on `ActorRegistry` can come later if needed.
    async fn find_npc_by_class_id(&self, zone_id: u32, class_id: u32) -> Option<ActorHandle> {
        let actors = self.registry.actors_in_zone(zone_id).await;
        for h in actors {
            let matches = {
                let c = h.character.read().await;
                c.chara.actor_class_id == class_id
            };
            if matches {
                return Some(h);
            }
        }
        None
    }

    /// `player:AddQuest(id)` — allocate a free slot, build a fresh
    /// `Quest`, persist the initial row, and fire the Lua `onStart`
    /// hook (the first of Meteor's five quest callbacks). Hook-emitted
    /// commands are applied via `apply_login_lua_command`.
    async fn apply_add_quest(&self, player_id: u32, quest_id: u32) {
        let Some(handle) = self.registry.get(player_id).await else {
            return;
        };
        let save_tuple = {
            let mut c = handle.character.write().await;
            if c.quest_journal.has(quest_id) {
                tracing::debug!(
                    player = player_id,
                    quest = quest_id,
                    "AddQuest skipped — quest already in journal",
                );
                return;
            }
            if c.quest_journal.is_completed(quest_id) {
                tracing::debug!(
                    player = player_id,
                    quest = quest_id,
                    "AddQuest skipped — quest already completed",
                );
                return;
            }
            let actor_id = crate::actor::quest::quest_actor_id(quest_id);
            let name = self
                .lua
                .as_ref()
                .and_then(|e| e.catalogs().quest_script_name(quest_id))
                .unwrap_or_default();
            let quest = crate::actor::quest::Quest::new(actor_id, name);
            let Some(slot) = c.quest_journal.add(quest) else {
                tracing::warn!(
                    player = player_id,
                    quest = quest_id,
                    "AddQuest failed — journal full (16 slots)",
                );
                return;
            };
            (slot as i32, actor_id)
        };
        let (slot, actor_id) = save_tuple;
        if let Err(e) = self
            .db
            .save_quest(player_id, slot, actor_id, 0, 0, 0, 0, 0)
            .await
        {
            tracing::warn!(
                error = %e,
                player = player_id,
                quest = quest_id,
                "AddQuest DB persist failed",
            );
        }
        tracing::info!(player = player_id, quest = quest_id, slot, "AddQuest applied");
        self.fire_quest_hook(&handle, quest_id, "onStart", vec![])
            .await;
    }

    /// `player:CompleteQuest(id)` — fire `onFinish(player, quest, true)`
    /// first so the script sees the quest still in-journal, then remove
    /// the scenario row and set the completion bit.
    async fn apply_complete_quest(&self, player_id: u32, quest_id: u32) {
        let Some(handle) = self.registry.get(player_id).await else {
            return;
        };
        // Fire onFinish before we tear the quest down so the hook can still
        // read `quest:GetData()` counters / flags via its snapshot.
        self.fire_quest_hook(
            &handle,
            quest_id,
            "onFinish",
            vec![crate::lua::QuestHookArg::Bool(true)],
        )
        .await;

        let removed_slot = {
            let mut c = handle.character.write().await;
            let slot = c.quest_journal.slot_of(quest_id);
            c.quest_journal.complete(quest_id);
            slot.map(|s| s as i32)
        };
        if let Some(slot) = removed_slot {
            if let Err(e) = self.db.remove_quest(player_id, quest_id).await {
                tracing::warn!(
                    error = %e,
                    player = player_id,
                    quest = quest_id,
                    slot,
                    "CompleteQuest: scenario-row delete failed",
                );
            }
        }
        if let Err(e) = self.db.complete_quest(player_id, quest_id).await {
            tracing::warn!(
                error = %e,
                player = player_id,
                quest = quest_id,
                "CompleteQuest: bitstream save failed",
            );
        }
        tracing::info!(
            player = player_id,
            quest = quest_id,
            "CompleteQuest applied",
        );
    }

    /// `player:AbandonQuest(id)` / `player:RemoveQuest(id)` — drop the
    /// active slot and fire `onFinish(player, quest, false)` so scripts
    /// can distinguish completion from abandonment via the boolean arg.
    async fn apply_abandon_quest(&self, player_id: u32, quest_id: u32) {
        let Some(handle) = self.registry.get(player_id).await else {
            return;
        };
        // Fire onFinish first (same reasoning as CompleteQuest).
        self.fire_quest_hook(
            &handle,
            quest_id,
            "onFinish",
            vec![crate::lua::QuestHookArg::Bool(false)],
        )
        .await;

        let had = {
            let mut c = handle.character.write().await;
            c.quest_journal.remove(quest_id).is_some()
        };
        if !had {
            tracing::debug!(
                player = player_id,
                quest = quest_id,
                "AbandonQuest skipped — quest not in journal",
            );
            return;
        }
        if let Err(e) = self.db.remove_quest(player_id, quest_id).await {
            tracing::warn!(
                error = %e,
                player = player_id,
                quest = quest_id,
                "AbandonQuest DB delete failed",
            );
        }
        tracing::info!(
            player = player_id,
            quest = quest_id,
            "AbandonQuest applied",
        );
    }

    /// Build a `PlayerSnapshot` + `LuaQuestHandle`, invoke the named
    /// hook on `scripts/lua/quests/<prefix>/<name>.lua`, and drain the
    /// emitted `LuaCommand`s through `apply_login_lua_command` so the
    /// side effects land in the same Rust-side pipeline player scripts
    /// already use.
    ///
    /// No-ops when:
    /// * `self.lua` is `None` (test harnesses that don't wire Lua)
    /// * the quest id isn't in the `gamedata_quests` catalog (so the
    ///   class name can't be resolved, so there's no script to run)
    /// * the resolved script path doesn't exist on disk
    ///
    /// A Lua-side error inside the hook is logged but not propagated —
    /// quest progression mustn't hard-fail on a scripting bug.
    async fn fire_quest_hook(
        &self,
        handle: &ActorHandle,
        quest_id: u32,
        hook_name: &str,
        extra_args: Vec<crate::lua::QuestHookArg>,
    ) {
        let Some(engine) = self.lua.as_ref() else {
            return;
        };
        let Some(script_name) = engine.catalogs().quest_script_name(quest_id) else {
            tracing::debug!(
                quest = quest_id,
                hook = hook_name,
                "quest hook skipped — quest id not in gamedata_quests catalog",
            );
            return;
        };
        let script_path = engine.resolver().quest(&script_name);
        if !script_path.exists() {
            tracing::debug!(
                quest = quest_id,
                hook = hook_name,
                path = %script_path.display(),
                "quest hook skipped — no script on disk",
            );
            return;
        }

        // Snapshot both the Player view and the live Quest state from a
        // single Character read so the hook sees a coherent frame.
        let (snapshot, quest_handle) = {
            let c = handle.character.read().await;
            let snapshot = build_player_snapshot_from_character(&c);
            let quest = c
                .quest_journal
                .get(quest_id)
                .map(|q| (q.get_sequence(), q.get_flags(), q.get_counter(0), q.get_counter(1), q.get_counter(2)))
                .unwrap_or((0, 0, 0, 0, 0));
            let handle = crate::lua::LuaQuestHandle {
                player_id: snapshot.actor_id,
                quest_id,
                has_quest: c.quest_journal.has(quest_id),
                sequence: quest.0,
                flags: quest.1,
                counters: [quest.2, quest.3, quest.4],
                queue: crate::lua::command::CommandQueue::new(),
            };
            (snapshot, handle)
        };

        let engine_clone = engine.clone();
        let script_path_clone = script_path.clone();
        let hook_name_owned = hook_name.to_string();
        // `call_quest_hook` is synchronous and can block (Lua scripts
        // often take milliseconds to tens of ms). Run it on the tokio
        // blocking pool so we don't stall the reactor thread.
        let result = tokio::task::spawn_blocking(move || {
            engine_clone.call_quest_hook(
                &script_path_clone,
                &hook_name_owned,
                snapshot,
                quest_handle,
                extra_args,
            )
        })
        .await;

        let result = match result {
            Ok(r) => r,
            Err(join_err) => {
                tracing::warn!(
                    error = %join_err,
                    quest = quest_id,
                    hook = hook_name,
                    "quest hook dispatch panicked",
                );
                return;
            }
        };
        if let Some(e) = result.error {
            tracing::debug!(
                error = %e,
                quest = quest_id,
                hook = hook_name,
                "quest hook errored; applying partial commands",
            );
        } else {
            tracing::debug!(
                quest = quest_id,
                hook = hook_name,
                commands = result.commands.len(),
                "quest hook fired",
            );
        }
        // Hook-emitted commands recurse back through the command
        // pipeline — `apply_login_lua_command` can re-invoke
        // `apply_add_quest` → `fire_quest_hook`, so the compiler needs
        // an explicit indirection point to bound the future size.
        for cmd in result.commands {
            Box::pin(self.apply_login_lua_command(handle, cmd)).await;
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

        // Client Lua error tunnel — the 1.x client re-purposes EventStart
        // with `unknown == 0x39800010` to ship a Lua stack trace up to
        // the server (Meteor `EventStartPacket.cs` has the commented-out
        // branch). Surface the trace in the log and stop — there's no
        // event to dispatch and calling `start_event` on the session
        // would record a phantom "owner actor missing" entry.
        if let Some(err_text) = pkt.client_script_error.as_deref() {
            tracing::warn!(
                session = session_id,
                error_index = pkt.trigger_actor_id,
                error_num = pkt.owner_actor_id,
                lua_error = %err_text,
                "client Lua error reported via EventStart tunnel",
            );
            return Ok(());
        }

        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };
        let actor_id = handle.actor_id;

        let owner_actor_id = pkt.owner_actor_id;
        let mut outbox = EventOutbox::new();
        {
            let mut chara = handle.character.write().await;
            chara.event_session.start_event(
                actor_id,
                owner_actor_id,
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

        // Fire `onTalk(player, quest, npc)` on every active quest the
        // player holds. Meteor's convention is to fire for *every* quest
        // and let the script filter by NPC class id + sequence — trying
        // to pre-filter on `QuestState.current` membership would drop
        // scripts that haven't populated their ENPC list yet (many stub
        // quests, tutorial cleanup paths, etc.).
        self.fire_on_talk_for_active_quests(&handle, owner_actor_id).await;

        tracing::debug!(
            player = actor_id,
            owner = owner_actor_id,
            "event start dispatched",
        );
        Ok(())
    }

    /// Look up the NPC's live state and fire `onTalk(player, quest, npc)`
    /// once per active quest in the player's journal. No-ops if the NPC
    /// isn't in the registry, or the player has no active quests.
    async fn fire_on_talk_for_active_quests(&self, handle: &ActorHandle, npc_actor_id: u32) {
        let active_quest_ids: Vec<u32> = {
            let c = handle.character.read().await;
            c.quest_journal
                .slots
                .iter()
                .flatten()
                .map(|q| q.quest_id())
                .collect()
        };
        if active_quest_ids.is_empty() {
            return;
        }
        let Some(npc_spec) = self.build_npc_spec(npc_actor_id).await else {
            // Not a registered actor (e.g. director-owned kicks) — the
            // event went through the normal dispatch; we just skip the
            // quest-side onTalk loop.
            return;
        };

        for quest_id in active_quest_ids {
            self.fire_quest_hook(
                handle,
                quest_id,
                "onTalk",
                vec![crate::lua::QuestHookArg::Npc(npc_spec.clone())],
            )
            .await;
        }
    }

    /// Snapshot the NPC's registry entry into a `Send`-friendly spec the
    /// quest-hook dispatcher can materialise as a `LuaNpc` userdata on
    /// the blocking pool. Returns `None` if the actor isn't live.
    async fn build_npc_spec(&self, actor_id: u32) -> Option<crate::lua::LuaNpcSpec> {
        let npc_handle = self.registry.get(actor_id).await?;
        let c = npc_handle.character.read().await;
        Some(crate::lua::LuaNpcSpec {
            actor_id: c.base.actor_id,
            name: c.base.actor_name.clone(),
            class_name: c.base.class_name.clone(),
            class_path: c.base.class_path.clone(),
            // `unique_id` isn't stored on BaseActor yet — Meteor's
            // equivalent comes from the spawn-row `uniqueId` column.
            // Scripts that read `npc:GetUniqueId()` will see an empty
            // string until the spawn pipeline starts populating it.
            unique_id: String::new(),
            zone_id: c.base.zone_id,
            zone_name: String::new(),
            state: c.base.current_main_state,
            pos: (c.base.position_x, c.base.position_y, c.base.position_z),
            rotation: c.base.rotation,
            actor_class_id: c.chara.actor_class_id,
            quest_graphic: 0,
        })
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
            dispatch_social_event(&e, &self.registry, &self.world, &self.db).await;
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
            dispatch_social_event(&e, &self.registry, &self.world, &self.db).await;
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
            dispatch_social_event(&e, &self.registry, &self.world, &self.db).await;
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
            dispatch_social_event(&e, &self.registry, &self.world, &self.db).await;
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
            dispatch_social_event(&e, &self.registry, &self.world, &self.db).await;
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
            dispatch_social_event(&e, &self.registry, &self.world, &self.db).await;
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
            dispatch_social_event(&e, &self.registry, &self.world, &self.db).await;
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
            dispatch_social_event(&e, &self.registry, &self.world, &self.db).await;
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
            dispatch_social_event(&e, &self.registry, &self.world, &self.db).await;
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
            dispatch_social_event(&e, &self.registry, &self.world, &self.db).await;
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
            dispatch_social_event(&e, &self.registry, &self.world, &self.db).await;
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
            dispatch_social_event(&e, &self.registry, &self.world, &self.db).await;
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
            dispatch_social_event(&e, &self.registry, &self.world, &self.db).await;
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
            dispatch_social_event(&e, &self.registry, &self.world, &self.db).await;
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
            dispatch_social_event(&e, &self.registry, &self.world, &self.db).await;
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
            dispatch_social_event(&e, &self.registry, &self.world, &self.db).await;
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
            dispatch_social_event(&e, &self.registry, &self.world, &self.db).await;
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
            dispatch_social_event(&e, &self.registry, &self.world, &self.db).await;
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
            dispatch_social_event(&e, &self.registry, &self.world, &self.db).await;
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
        active_quest_states: Vec::new(),
        unlocked_aetherytes: Vec::new(),
        traits: Vec::new(),
        inventory: Vec::new(),
        login_director_actor_id: c.chara.login_director_actor_id,
    }
}

/// Variant of [`build_player_snapshot_for_login`] for the quest-hook
/// dispatch path. Populates `active_quests` / `completed_quests` /
/// `active_quest_states` from the live `Character::quest_journal` so
/// the `LuaPlayer` passed into `onStart`/`onFinish`/`onStateChange`
/// returns accurate values for `HasQuest` / `IsQuestCompleted` /
/// `GetFreeQuestSlot` and so `LuaQuestHandle` getters resolve against
/// real sequence/flags/counters.
fn build_player_snapshot_from_character(c: &Character) -> crate::lua::userdata::PlayerSnapshot {
    let mut snapshot = build_player_snapshot_for_login(c);
    snapshot.active_quests = c
        .quest_journal
        .slots
        .iter()
        .flatten()
        .map(|q| q.quest_id())
        .collect();
    snapshot.active_quest_states = c
        .quest_journal
        .slots
        .iter()
        .flatten()
        .map(|q| crate::lua::QuestStateSnapshot {
            quest_id: q.quest_id(),
            sequence: q.get_sequence(),
            flags: q.get_flags(),
            counters: [q.get_counter(0), q.get_counter(1), q.get_counter(2)],
        })
        .collect();
    snapshot.completed_quests = c.quest_journal.iter_completed().collect();
    snapshot
}
