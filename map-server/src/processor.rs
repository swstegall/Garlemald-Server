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
    OP_RX_DATA_REQUEST, OP_RX_END_RECRUITING, OP_RX_EVENT_START, OP_RX_EVENT_UPDATE,
    OP_RX_FAQ_BODY_REQUEST, OP_RX_FAQ_LIST_REQUEST, OP_RX_FRIEND_STATUS, OP_RX_FRIENDLIST_ADD,
    OP_RX_FRIENDLIST_REMOVE, OP_RX_FRIENDLIST_REQUEST, OP_RX_GM_TICKET_BODY, OP_RX_GM_TICKET_END,
    OP_RX_GM_TICKET_SEND, OP_RX_GM_TICKET_STATE, OP_RX_GROUP_CREATED,
    OP_RX_ITEM_PACKAGE_REQUEST, OP_RX_LANGUAGE_CODE, OP_RX_LOCK_TARGET, OP_RX_RECRUITER_STATE,
    OP_RX_RECRUITING_DETAILS, OP_RX_SET_TARGET, OP_RX_START_RECRUITING,
    OP_RX_SUPPORT_ISSUE_REQUEST, OP_RX_UPDATE_PLAYER_POSITION, OP_RX_ZONE_IN_COMPLETE,
    OP_SESSION_BEGIN, OP_SESSION_END,
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

/// Read a null-terminated ASCII string out of a fixed-size byte slice.
/// Used by the retail-IN dispatch arms (`OP_RX_DATA_REQUEST`,
/// `OP_RX_GROUP_CREATED`) to surface the property-path / event-name
/// strings the 1.x client embeds in those packets.
fn extract_null_terminated_ascii(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

pub struct PacketProcessor {
    pub db: Arc<Database>,
    pub world: Arc<WorldManager>,
    pub registry: Arc<ActorRegistry>,
    /// Optional — when present, the event dispatcher calls
    /// `onEventStarted` / `isObjectivesComplete` / etc. on real scripts.
    pub lua: Option<Arc<LuaEngine>>,
    /// Optional — when present, `!command` chat messages dispatch into
    /// the same typed command shim the stdin console reader uses, so
    /// in-game chat becomes an auxiliary GM console (useful when the
    /// map-server is launched with stdin redirected to /dev/null, which
    /// is the common case for `run-all.sh`-backgrounded runs).
    pub cmd: Option<Arc<crate::command_processor::CommandProcessor>>,
}

/// Derive a deterministic `group_id` for the retainer-meeting group
/// that binds a spawned retainer to its owning player. Since the
/// retainer actor id is already composite-unique via the
/// `(4 << 28) | (zone << 19) | local_id` formula in
/// `apply_spawn_my_retainer`, lifting it into u64 gives us a
/// collision-free id without a separate allocator. Tier 4 #14 B.
fn retainer_meeting_group_id(retainer_actor_id: u32) -> u64 {
    // Top 32 bits carry a sentinel so a future audit can tell
    // "this is a retainer-meeting group id" at a glance without
    // needing the surrounding context.
    (0x5200_0000u64 << 32) | retainer_actor_id as u64
}

/// One-off [`GroupResolver`](crate::group::GroupResolver) for a
/// single retainer-meeting group. The group is short-lived (created
/// on `SpawnMyRetainer`, destroyed on `DespawnMyRetainer`) so we
/// don't bother registering it with `WorldManager`; the processor
/// constructs a resolver per dispatch instead.
struct RetainerMeetingResolver {
    group_id: u64,
    player_actor_id: u32,
    player_name: String,
    retainer_actor_id: u32,
    retainer_name: String,
}

impl crate::group::GroupResolver for RetainerMeetingResolver {
    fn members(&self, group_id: u64) -> Option<Vec<u32>> {
        if group_id == self.group_id {
            Some(vec![self.player_actor_id, self.retainer_actor_id])
        } else {
            None
        }
    }
    fn kind(&self, group_id: u64) -> Option<crate::group::GroupKind> {
        if group_id == self.group_id {
            Some(crate::group::GroupKind::Retainer)
        } else {
            None
        }
    }
    fn type_id(&self, group_id: u64) -> Option<crate::group::GroupTypeId> {
        if group_id == self.group_id {
            Some(crate::group::GroupTypeId::RETAINER)
        } else {
            None
        }
    }
    fn name_of(&self, actor_id: u32) -> String {
        if actor_id == self.player_actor_id {
            self.player_name.clone()
        } else if actor_id == self.retainer_actor_id {
            self.retainer_name.clone()
        } else {
            String::new()
        }
    }
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
        // Seed level from the DB's per-class `skill_level` row so the
        // stat-baseline formula sees the right per-level multiplier at
        // login. Meteor's C# reads this from
        // `characters_class_levels.<classColumn>`; our loader writes
        // into `battle_save.skill_level[class_id]`. Falls through to 0
        // for class_slot ≥ 42 / unset class, which
        // `apply_player_stat_baseline` clamps to level 1.
        let level_from_class = loaded
            .class_levels
            .skill_level
            .get(class_slot)
            .copied()
            .unwrap_or(0);
        character.chara.level = level_from_class;
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
        // Run the Player baseline-stat seeder *before* calculate_base_stats
        // so STR/VIT/DEX/INT/MND/PIE have non-zero values at login and
        // every subsequent recalc (equip/status/trait) reads real
        // primaries. See `apply_player_stat_baseline` for the explicit-
        // placeholder caveat — real per-level growth curves weren't
        // reversed from the 1.23b client. Seed-if-zero semantics mean
        // the Hp/Mp mods just set from `characters_parametersave`
        // survive untouched.
        character.apply_player_stat_baseline();
        character.apply_player_stat_derivation();
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
        // Mount/chocobo hydration. The DB load lands them on the
        // LoadedPlayer's `ChocoboData`; mirror into CharaState so
        // the runtime chocobo helpers (`apply_issue_chocobo`,
        // `apply_send_mount_appearance`, …) can mutate via the
        // registry without routing through Player helpers.
        character.chara.has_chocobo = loaded.chocobo.has_chocobo;
        character.chara.chocobo_appearance = loaded.chocobo.chocobo_appearance;
        character.chara.chocobo_name = loaded.chocobo.chocobo_name.clone();
        // Grand Company hydration. Same motivation as the chocobo
        // fields — processor handlers mutate via the registry's
        // `Arc<RwLock<Character>>`, so runtime state lives on
        // CharaState rather than PlayerState.
        character.chara.gc_current = loaded.gc_current;
        character.chara.gc_rank_limsa = loaded.gc_limsa_rank;
        character.chara.gc_rank_gridania = loaded.gc_gridania_rank;
        character.chara.gc_rank_uldah = loaded.gc_uldah_rank;
        // Home-point hydration — same registry-reachability motivation
        // as the GC fields above; the home-point-revive dispatcher
        // reads this without a DB round-trip.
        character.chara.homepoint = loaded.homepoint;
        character.chara.homepoint_inn = loaded.homepoint_inn;
        // Hotbar hydration — mirror the loaded equipped commands into
        // CharaState so `PlayerSnapshot::hotbar` reads from the live
        // registry-reachable state. EquipAbility/UnequipAbility/
        // SwapAbilities apply paths mutate this vec in-place.
        character.chara.hotbar = loaded.hotbar.clone();
        // SNpc / Path Companion hydration — same registry-reachability
        // motivation. The SetSNpc apply path mutates these in-place +
        // persists via db.save_snpc.
        character.chara.snpc_nickname = loaded.snpc_nickname.clone();
        character.chara.snpc_skin = loaded.snpc_skin;
        character.chara.snpc_personality = loaded.snpc_personality;
        character.chara.snpc_coordinate = loaded.snpc_coordinate;
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
                Some(crate::actor::quest::Quest::from_db_row_with_npc_ls(
                    actor_aid,
                    String::new(),
                    row.sequence,
                    row.flags,
                    row.counter1,
                    row.counter2,
                    row.counter3,
                    row.npc_ls_from,
                    row.npc_ls_msg_step,
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
                // Register the director in the zone's actor registry so
                // the subsequent `event::dispatcher::dispatch_director_event_started`
                // — triggered when the client fires EventStart on the
                // director (via the login-bundle KickEvent("noticeEvent"))
                // — can resolve `zone.core.director(actor_id)`. Without
                // this, the dispatcher logs "director not on zone" and
                // the client stays at "Now Loading…" waiting for the
                // opening cutscene.
                //
                // The LuaZone:CreateDirector binding pins the director
                // local_id to 0, so the actor id is deterministic and
                // we can round-trip it into the registry idempotently.
                // `encode_director_actor_id` adds the C# `+ 2` quirk
                // — strip it back off here so `create_director_with_id`
                // re-applies the encoding correctly (otherwise the
                // round-trip drifts by 4 every CreateDirector call).
                let director_local_id =
                    (director_actor_id & 0x0007_FFFF).saturating_sub(2);
                if let Some(zone_arc) = self.world.zone(zone_actor_id).await {
                    let mut zone = zone_arc.write().await;
                    zone.core.create_director_with_id(
                        director_local_id,
                        class_path.clone(),
                        false,
                    );
                }
                tracing::info!(
                    director = director_actor_id,
                    zone = zone_actor_id,
                    class_path = %class_path,
                    "CreateDirector applied (registered in zone; will emit director spawn in zone-in bundle)"
                );
            }
            LC::EndGuildleve {
                director_actor_id,
                was_completed,
            } => {
                self.apply_end_guildleve(director_actor_id, was_completed)
                    .await;
            }
            LC::StartGuildleve { director_actor_id } => {
                self.apply_start_guildleve(director_actor_id).await;
            }
            LC::AbandonGuildleve { director_actor_id } => {
                self.apply_abandon_guildleve(director_actor_id).await;
            }
            LC::UpdateAimNumNow {
                director_actor_id,
                index,
                value,
            } => {
                self.apply_director_outbox_op(director_actor_id, "UpdateAimNumNow", |gld, ob| {
                    gld.update_aim_num_now(index, value, ob);
                })
                .await;
            }
            LC::UpdateUiState {
                director_actor_id,
                index,
                value,
            } => {
                self.apply_director_outbox_op(director_actor_id, "UpdateUIState", |gld, ob| {
                    gld.update_ui_state(index, value, ob);
                })
                .await;
            }
            LC::UpdateMarkers {
                director_actor_id,
                index,
                x,
                y,
                z,
            } => {
                self.apply_director_outbox_op(director_actor_id, "UpdateMarkers", |gld, ob| {
                    gld.update_marker(index, x, y, z, ob);
                })
                .await;
            }
            LC::SyncAllInfo { director_actor_id } => {
                self.apply_director_outbox_op(director_actor_id, "SyncAllInfo", |gld, ob| {
                    gld.sync_all(ob);
                })
                .await;
            }
            LC::StartDirectorMain {
                director_actor_id,
                class_path,
                director_name,
                spawn_immediate,
            } => {
                self.apply_start_director_main(
                    director_actor_id,
                    class_path,
                    director_name,
                    spawn_immediate,
                )
                .await;
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
            LC::DoEmote {
                actor_id,
                target_actor_id,
                emote_id,
                message_id,
            } => {
                self.apply_do_emote(actor_id, target_actor_id, emote_id, message_id)
                    .await;
            }
            LC::SetSNpc {
                player_id,
                nickname,
                actor_class_id,
                personality,
            } => {
                self.apply_set_snpc(player_id, nickname, actor_class_id, personality)
                    .await;
            }
            LC::DoClassChange { player_id, class_id } => {
                self.apply_do_class_change(player_id, class_id).await;
            }
            LC::PrepareClassChange { player_id, class_id } => {
                self.apply_prepare_class_change(player_id, class_id).await;
            }
            LC::QuestSetNpcLsFrom {
                player_id,
                quest_id,
                from,
            } => {
                self.apply_quest_set_npc_ls_from(player_id, quest_id, from)
                    .await;
            }
            LC::QuestIncrementNpcLsMsgStep {
                player_id,
                quest_id,
            } => {
                self.apply_quest_increment_npc_ls_msg_step(player_id, quest_id)
                    .await;
            }
            LC::QuestClearNpcLs {
                player_id,
                quest_id,
            } => {
                self.apply_quest_clear_npc_ls(player_id, quest_id).await;
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
            LC::SetQuestComplete {
                player_id,
                quest_id,
                flag,
            } => {
                crate::runtime::quest_apply::apply_set_quest_complete(
                    player_id, quest_id, flag, &self.registry, &self.db,
                )
                .await;
            }
            LC::AddExp {
                actor_id,
                class_id,
                exp,
            } => {
                // Route through the shared runtime helper so this path,
                // the `player:AddExp(...)` Lua command drain in
                // `runtime/quest_apply.rs`, and any GM `!giveexp` share
                // the same level-up rollover logic.
                crate::runtime::quest_apply::apply_add_exp(
                    actor_id,
                    class_id,
                    exp,
                    &self.registry,
                    &self.db,
                    Some(&self.world),
                    self.lua.as_ref(),
                )
                .await;
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
            // `onLogin` init items + every `HarvestReward` call route
            // through here. Persistence is direct-DB via `add_harvest_item`
            // (see `runtime::quest_apply::apply_add_item` for the shape);
            // the in-memory `ItemPackage` pipeline isn't wired to the
            // registry yet, so the player sees the new stack on the
            // next inventory resync.
            LC::AddItem {
                actor_id,
                item_package,
                item_id,
                quantity,
            } => {
                crate::runtime::quest_apply::apply_add_item(
                    actor_id,
                    item_package,
                    item_id,
                    quantity,
                    &self.db,
                )
                .await;
            }
            LC::AddItemToRetainer {
                retainer_id,
                item_package,
                item_id,
                quantity,
            } => {
                crate::runtime::quest_apply::apply_add_item_to_retainer(
                    retainer_id,
                    item_package,
                    item_id,
                    quantity,
                    &self.db,
                )
                .await;
            }
            LC::HandInRegionalLeve { player_id, leve_id } => {
                let _ = crate::runtime::quest_apply::apply_regional_leve_hand_in(
                    player_id,
                    leve_id,
                    &self.registry,
                    &self.db,
                    self.lua.as_ref(),
                )
                .await;
            }
            LC::AcceptRegionalLeve {
                player_id,
                leve_id,
                difficulty,
            } => {
                let _ = crate::runtime::quest_apply::apply_accept_regional_leve(
                    player_id,
                    leve_id,
                    difficulty,
                    &self.registry,
                    &self.db,
                    self.lua.as_ref(),
                )
                .await;
            }
            LC::PurchaseRetainerBazaarItem {
                buyer_id,
                retainer_id,
                server_item_id,
            } => {
                let _ = crate::runtime::quest_apply::apply_purchase_retainer_bazaar_item(
                    buyer_id,
                    retainer_id,
                    server_item_id,
                    &self.db,
                )
                .await;
            }
            LC::TryStatus {
                source_actor_id,
                target_actor_id,
                status_id,
                duration_s,
                magnitude,
                tick_ms,
                tier,
            } => {
                let _ = crate::runtime::quest_apply::apply_try_status(
                    source_actor_id,
                    target_actor_id,
                    status_id,
                    duration_s,
                    magnitude,
                    tick_ms,
                    tier,
                    &self.registry,
                    &self.db,
                    &self.world,
                    self.lua.as_ref(),
                )
                .await;
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
                self.apply_set_home_point(player_id, homepoint).await;
            }
            LC::SetHomePointInn { player_id, inn_id } => {
                self.apply_set_home_point_inn(player_id, inn_id).await;
            }
            LC::PlayerSetNpcLs {
                player_id,
                npc_ls_id,
                state,
            } => {
                self.apply_player_set_npc_ls(player_id, npc_ls_id, state)
                    .await;
            }
            LC::EquipAbility {
                player_id,
                class_id,
                command_id,
                hotbar_slot,
            } => {
                self.apply_equip_ability(player_id, class_id, command_id, hotbar_slot)
                    .await;
            }
            LC::UnequipAbility {
                player_id,
                class_id,
                hotbar_slot,
            } => {
                self.apply_unequip_ability(player_id, class_id, hotbar_slot)
                    .await;
            }
            LC::SwapAbilities {
                player_id,
                class_id,
                hotbar_slot_1,
                hotbar_slot_2,
            } => {
                self.apply_swap_abilities(player_id, class_id, hotbar_slot_1, hotbar_slot_2)
                    .await;
            }
            LC::EquipAbilityInFirstOpenSlot {
                player_id,
                class_id,
                command_id,
            } => {
                self.apply_equip_ability_in_first_open_slot(player_id, class_id, command_id)
                    .await;
            }
            LC::SetCurrentJob { player_id, job_id } => {
                self.apply_set_current_job(player_id, job_id).await;
            }
            LC::SendAppearance { actor_id } => {
                self.apply_send_appearance(actor_id).await;
            }
            LC::SavePlayTime { player_id } => {
                self.apply_save_play_time(player_id).await;
            }
            LC::SetPool {
                actor_id,
                kind,
                value,
            } => {
                self.apply_set_pool(actor_id, kind, value).await;
            }
            LC::WarpToPosition {
                actor_id,
                x,
                y,
                z,
                rotation,
                spawn_type,
            } => {
                self.apply_warp_to_position(actor_id, x, y, z, rotation, spawn_type)
                    .await;
            }
            LC::WarpToPublicArea { player_id, target } => {
                self.apply_warp_to_public_area(player_id, target).await;
            }
            LC::WarpToPrivateArea {
                player_id,
                area_class,
                area_index,
                target,
            } => {
                self.apply_warp_to_private_area(player_id, area_class, area_index, target)
                    .await;
            }
            LC::DoZoneChange {
                player_id,
                zone_id,
                private_area,
                private_area_type,
                spawn_type,
                x,
                y,
                z,
                rotation,
            } => {
                self.apply_do_zone_change(
                    player_id,
                    zone_id,
                    private_area,
                    private_area_type,
                    spawn_type,
                    x,
                    y,
                    z,
                    rotation,
                )
                .await;
            }
            LC::SpawnMyRetainer {
                player_id,
                bell_actor_id,
                bell_position,
                retainer_index,
            } => {
                self.apply_spawn_my_retainer(
                    player_id,
                    bell_actor_id,
                    bell_position,
                    retainer_index,
                )
                .await;
            }
            LC::DespawnMyRetainer { player_id } => {
                self.apply_despawn_my_retainer(player_id).await;
            }
            LC::HireRetainer {
                player_id,
                retainer_id,
            } => {
                self.apply_hire_retainer(player_id, retainer_id).await;
            }
            LC::DismissMyRetainer {
                player_id,
                retainer_id,
            } => {
                self.apply_dismiss_my_retainer(player_id, retainer_id).await;
            }
            LC::RenameRetainer {
                player_id,
                retainer_id,
                new_name,
            } => {
                self.apply_rename_retainer(player_id, retainer_id, new_name).await;
            }
            LC::AddRetainerBazaarItem {
                retainer_id,
                item_id,
                quantity,
                quality,
                price_gil,
            } => {
                self.apply_add_retainer_bazaar_item(
                    retainer_id,
                    item_id,
                    quantity,
                    quality,
                    price_gil,
                )
                .await;
            }
            LC::SetSleeping { player_id } => {
                self.apply_set_sleeping(player_id).await;
            }
            LC::StartDream { player_id, dream_id } => {
                self.apply_start_dream(player_id, dream_id).await;
            }
            LC::EndDream { player_id } => {
                self.apply_end_dream(player_id).await;
            }
            LC::Logout { player_id } => {
                self.apply_logout(player_id).await;
            }
            LC::QuitGame { player_id } => {
                self.apply_quit_game(player_id).await;
            }
            LC::IssueChocobo {
                player_id,
                appearance_id,
                name,
            } => {
                self.apply_issue_chocobo(player_id, appearance_id, name).await;
            }
            LC::StartChocoboRental { player_id, minutes } => {
                self.apply_start_chocobo_rental(player_id, minutes).await;
            }
            LC::SetMountState { player_id, state } => {
                self.apply_set_mount_state(player_id, state).await;
            }
            LC::SendMountAppearance { player_id } => {
                self.apply_send_mount_appearance(player_id).await;
            }
            LC::SetChocoboName { player_id, name } => {
                self.apply_set_chocobo_name(player_id, name).await;
            }
            LC::JoinGC { player_id, gc } => {
                self.apply_join_gc(player_id, gc).await;
            }
            LC::SetGCRank { player_id, gc, rank } => {
                self.apply_set_gc_rank(player_id, gc, rank).await;
            }
            LC::AddSeals { player_id, gc, amount } => {
                self.apply_add_seals(player_id, gc, amount).await;
            }
            LC::PromoteGC { player_id, gc } => {
                self.apply_promote_gc(player_id, gc).await;
            }
            LC::CreateContentArea {
                player_id,
                parent_zone_id,
                area_class_path,
                area_name,
                content_script,
                director_name,
                director_actor_id,
                content_area_actor_id,
            } => {
                self.apply_create_content_area(
                    player_id,
                    parent_zone_id,
                    area_class_path,
                    area_name,
                    content_script,
                    director_name,
                    director_actor_id,
                    content_area_actor_id,
                )
                .await;
            }
            LC::DoZoneChangeContent {
                player_id,
                parent_zone_id,
                area_name,
                director_actor_id,
                spawn_type,
                x,
                y,
                z,
                rotation,
            } => {
                self.apply_do_zone_change_content(
                    player_id,
                    parent_zone_id,
                    area_name,
                    director_actor_id,
                    spawn_type,
                    x,
                    y,
                    z,
                    rotation,
                )
                .await;
            }
            LC::ContentFinished {
                parent_zone_id,
                area_name,
            } => {
                tracing::info!(
                    parent_zone = parent_zone_id,
                    area = %area_name,
                    "ContentFinished applied (stub: cleanup not yet wired)",
                );
            }
            other => {
                tracing::debug!(?other, "login lua cmd (unhandled)");
            }
        }
    }

    /// Phase A of the SEQ_005 combat-tutorial path. Two responsibilities:
    ///
    /// 1. Log the content-area registration so the trace shows the
    ///    Lua chain reached this step (matching the old stub).
    /// 2. Fire the content script's `onCreate(player, contentArea,
    ///    director)` hook — which is what spawns the tutorial NPCs
    ///    (Yda + Papalymo + 3 wolves) and adds them to the player's
    ///    party + the director's member list.
    ///
    /// Phase A doesn't yet materialise a server-side
    /// `PrivateAreaContent` (instance isolation, shadowed actor lists,
    /// etc.). The `onCreate` script will hit no-op-with-logging stubs
    /// for `SpawnBattleNpcById`, `currentParty:AddMember`, `SetMod`,
    /// etc. — those stubs are in `lua/userdata.rs`. The point of
    /// running the script here is to surface every binding the
    /// tutorial needs in a single trace pass, so subsequent phases
    /// can fill them in incrementally. See
    /// `captures/seq005_unblock_plan.md` for the staged port plan.
    #[allow(clippy::too_many_arguments)]
    async fn apply_create_content_area(
        &self,
        player_id: u32,
        parent_zone_id: u32,
        area_class_path: String,
        area_name: String,
        content_script: String,
        director_name: String,
        director_actor_id: u32,
        content_area_actor_id: u32,
    ) {
        tracing::info!(
            player = format!("0x{:08X}", player_id),
            parent_zone = parent_zone_id,
            area = %area_name,
            director = %director_name,
            director_actor_id = format!("0x{:08X}", director_actor_id),
            content_area_actor_id = format!("0x{:08X}", content_area_actor_id),
            content_script = %content_script,
            "CreateContentArea applied (Phase A: lua handle live, content-script onCreate next)",
        );

        let Some(lua) = self.lua.as_ref() else {
            tracing::debug!("CreateContentArea: no LuaEngine wired — skipping onCreate");
            return;
        };
        if player_id == 0 {
            tracing::debug!(
                "CreateContentArea: player_id was 0 (caller didn't pass a LuaPlayer) — skipping onCreate",
            );
            return;
        }

        // Resolve the content script path (`scripts/lua/content/<name>.lua`).
        // Missing script → quiet skip; the stub is still applied above.
        let script_path = lua.resolver().content(&content_script);
        if !script_path.exists() {
            tracing::debug!(
                content_script = %content_script,
                script = %script_path.display(),
                "CreateContentArea: content script not on disk — skipping onCreate",
            );
            return;
        }

        // Build the player snapshot from the registry. If the player
        // isn't in the registry (rare), fall back to logging.
        let Some(handle) = self.registry.get(player_id).await else {
            tracing::warn!(
                player = format!("0x{:08X}", player_id),
                "CreateContentArea: player handle missing — skipping onCreate",
            );
            return;
        };
        let snapshot = {
            let c = handle.character.read().await;
            build_player_snapshot_from_character(&c)
        };

        // B6: capture the active content script on the leader's
        // session so the ticker can fire `onUpdate(tick, area)`
        // periodically. Cleared on logout / `ContentFinished`.
        if let Some(mut snap) = self.world.session(handle.session_id).await {
            snap.active_content_script =
                Some(crate::data::ActiveContentScript {
                    parent_zone_id,
                    area_name: area_name.clone(),
                    area_class_path: area_class_path.clone(),
                    director_name: director_name.clone(),
                    director_actor_id,
                    content_script: content_script.clone(),
                });
            self.world.upsert_session(snap).await;
        }

        // Build the LuaContentArea + LuaDirectorHandle handles. The
        // engine re-points their queues to the freshly-installed
        // script queue inside `call_content_hook`, so the placeholder
        // queues here are fine.
        let placeholder_queue = crate::lua::command::CommandQueue::new();
        let content_area = crate::lua::userdata::LuaContentArea {
            parent_zone_id,
            area_name: area_name.clone(),
            area_class_path: area_class_path.clone(),
            director_name: director_name.clone(),
            director_actor_id,
            queue: placeholder_queue.clone(),
        };
        let director = crate::lua::userdata::LuaDirectorHandle {
            name: director_name.clone(),
            actor_id: director_actor_id,
            class_path: format!("/Director/{director_name}"),
            queue: placeholder_queue,
        };

        let lua_clone = lua.clone();
        let script_path_clone = script_path.clone();
        let snapshot_clone = snapshot;
        let content_area_clone = content_area;
        let director_clone = director;
        let result = tokio::task::spawn_blocking(move || {
            lua_clone.call_content_hook(
                &script_path_clone,
                "onCreate",
                snapshot_clone,
                content_area_clone,
                director_clone,
            )
        })
        .await;
        let partial = match result {
            Ok(p) => p,
            Err(join_err) => {
                tracing::warn!(
                    player = format!("0x{:08X}", player_id),
                    error = %join_err,
                    "CreateContentArea: onCreate dispatch panicked",
                );
                return;
            }
        };
        if let Some(e) = partial.error {
            // Phase-A stubs log + no-op, so most "errors" here are
            // expected (missing bindings reported by the script).
            // Surface at debug to keep the trace clean.
            tracing::debug!(
                player = format!("0x{:08X}", player_id),
                content_script = %content_script,
                error = %e,
                "CreateContentArea: onCreate completed with error (likely missing binding — Phase A expected)",
            );
        }
        if !partial.commands.is_empty() {
            // Partition out commands that need processor-scoped
            // resources (db, sessions, client handles) the runtime
            // applier can't reach: SpawnBattleNpcById (B1) needs db
            // lookups + actor materialisation; PartyAddMember (B2)
            // needs the leader's session + client handle to broadcast
            // the group packet trio. Everything else flows through
            // the standard runtime drain.
            let mut runtime_cmds = Vec::with_capacity(partial.commands.len());
            for cmd in partial.commands {
                match cmd {
                    crate::lua::command::LuaCommand::SpawnBattleNpcById {
                        bnpc_id,
                        parent_zone_id: pz,
                        expected_actor_id,
                    } => {
                        self.apply_spawn_battle_npc_by_id(bnpc_id, pz, expected_actor_id)
                            .await;
                    }
                    crate::lua::command::LuaCommand::PartyAddMember {
                        leader_actor_id,
                        member_actor_id,
                    } => {
                        self.apply_party_add_member(leader_actor_id, member_actor_id)
                            .await;
                    }
                    crate::lua::command::LuaCommand::DirectorAddMember {
                        director_actor_id,
                        member_actor_id,
                    } => {
                        // Bind to the player whose `onCreate` chain
                        // emitted the command — they're the
                        // broadcast target for this director's
                        // group packets in the solo-tutorial case.
                        // Multi-player content groups (Phase B5+)
                        // would walk the director's player_members
                        // and broadcast to each.
                        self.apply_director_add_member(
                            player_id,
                            director_actor_id,
                            member_actor_id,
                        )
                        .await;
                    }
                    other => runtime_cmds.push(other),
                }
            }
            if !runtime_cmds.is_empty() {
                crate::runtime::quest_apply::apply_runtime_lua_commands(
                    runtime_cmds,
                    &self.registry,
                    &self.db,
                    &self.world,
                    Some(lua),
                )
                .await;
            }
        }
    }

    /// B1 of the SEQ_005 unblock plan — port of the C# in
    /// `Map Server/WorldManager.cs:514 SpawnBattleNpcById`. Joins the
    /// four `server_battlenpc_*` seed tables on `bnpc_id`, materialises
    /// a `BattleNpc` actor under the parent zone's actor list at the
    /// caller-pre-computed actor id, and broadcasts the spawn-bundle
    /// trio to nearby players via
    /// `runtime::dispatcher::spawn_bundle_fanout`.
    ///
    /// Phase B1 simplifications:
    ///   * No private-area instance isolation — the actor lands in the
    ///     parent zone's actor list. (Phase B5 wires in `PrivateAreaContent`.)
    ///   * No detection / aggro-type / kindred / mob-mod / drop-list
    ///     application — those land in subsequent passes once the
    ///     respective subsystems plumb through.
    ///   * No respawn timer — the actor sticks until explicit despawn.
    ///   * No `script_name`-driven Lua-side combat AI — the controller
    ///     stays default.
    async fn apply_spawn_battle_npc_by_id(
        &self,
        bnpc_id: u32,
        parent_zone_id: u32,
        expected_actor_id: u32,
    ) {
        // 1. Load the joined spawn DTO from the database.
        let spawn = match self.db.load_battle_npc_spawn(bnpc_id).await {
            Ok(Some(row)) => row,
            Ok(None) => {
                tracing::warn!(
                    bnpc_id,
                    parent_zone = parent_zone_id,
                    "SpawnBattleNpcById: bnpc_id not in server_battlenpc_spawn_locations",
                );
                return;
            }
            Err(e) => {
                tracing::warn!(
                    bnpc_id,
                    error = %e,
                    "SpawnBattleNpcById: db query failed",
                );
                return;
            }
        };

        // 2. Resolve the ActorClass row keyed by spawn.actor_class_id.
        //    The class carries class_path / display_name_id / event
        //    conditions — required for AddActor + ActorInstantiate.
        let actor_class = match self.db.load_actor_class(spawn.actor_class_id).await {
            Ok(Some(c)) => c,
            Ok(None) => {
                tracing::warn!(
                    bnpc_id,
                    actor_class_id = spawn.actor_class_id,
                    "SpawnBattleNpcById: actor_class not in gamedata_actor_class",
                );
                return;
            }
            Err(e) => {
                tracing::warn!(
                    bnpc_id,
                    error = %e,
                    "SpawnBattleNpcById: actor_class load failed",
                );
                return;
            }
        };

        // 3. Compute actor_number from expected_actor_id (must round-trip
        //    through the same `(4 << 28) | (zone << 19) | actor_number`
        //    formula the Lua binding used).
        let actor_number = expected_actor_id & 0x7FFFF;

        // 4. Build the BattleNpc. Pre-fill HP from the group row so the
        //    `0x0134 SetActorState` packet has the right value (combat
        //    AI math is a follow-up pass).
        let mut bnpc = crate::npc::battle_npc::BattleNpc::new(
            actor_number,
            &actor_class,
            spawn.script_name.clone(),
            parent_zone_id,
            spawn.position_x,
            spawn.position_y,
            spawn.position_z,
            spawn.rotation,
            spawn.actor_state,
            spawn.animation_id,
            None,
        );
        if spawn.hp > 0 {
            bnpc.npc.character.chara.hp = spawn.hp.min(i16::MAX as u32) as i16;
            bnpc.npc.character.chara.max_hp = spawn.hp.min(i16::MAX as u32) as i16;
        }
        if spawn.mp > 0 {
            bnpc.npc.character.chara.mp = spawn.mp.min(i16::MAX as u32) as i16;
            bnpc.npc.character.chara.max_mp = spawn.mp.min(i16::MAX as u32) as i16;
        }
        bnpc.npc.character.chara.level = spawn.min_level.clamp(1, i16::MAX as u32) as i16;

        let actor_id = bnpc.actor_id();
        if actor_id != expected_actor_id {
            tracing::warn!(
                bnpc_id,
                expected = format!("0x{:08X}", expected_actor_id),
                actual = format!("0x{:08X}", actor_id),
                "SpawnBattleNpcById: actor_id mismatch — Lua side computed differently",
            );
            // Bail rather than spawn at the wrong id; the script's
            // subsequent calls would target a phantom actor.
            return;
        }

        // 5. Insert the spatial projection into the parent zone's grid.
        let Some(zone_arc) = self.world.zone(parent_zone_id).await else {
            tracing::warn!(
                bnpc_id,
                parent_zone = parent_zone_id,
                "SpawnBattleNpcById: parent zone not loaded",
            );
            return;
        };
        {
            let mut zone = zone_arc.write().await;
            let mut ob = crate::zone::outbox::AreaOutbox::new();
            zone.core.add_actor(
                crate::zone::area::StoredActor {
                    actor_id,
                    kind: crate::zone::area::ActorKind::BattleNpc,
                    position: common::Vector3::new(
                        spawn.position_x,
                        spawn.position_y,
                        spawn.position_z,
                    ),
                    grid: (0, 0),
                    is_alive: true,
                },
                &mut ob,
            );
        }

        // 6. Register the live Character in the ActorRegistry.
        let character = bnpc.npc.character.clone();
        self.registry
            .insert(crate::runtime::actor_registry::ActorHandle::new(
                actor_id,
                crate::runtime::actor_registry::ActorKindTag::BattleNpc,
                parent_zone_id,
                /* session */ 0,
                character,
            ))
            .await;

        // 7. Fan the spawn bundle (AddActor + position + appearance +
        //    name + state + sub_state + status_all + icon + is_zoning)
        //    to every player within broadcast radius. Uses the same
        //    helper the runtime dispatcher uses for normal mob spawns.
        crate::runtime::dispatcher::spawn_bundle_fanout(
            &self.world,
            &self.registry,
            &zone_arc,
            parent_zone_id,
            actor_id,
        )
        .await;

        tracing::info!(
            bnpc_id,
            parent_zone = parent_zone_id,
            actor_id = format!("0x{:08X}", actor_id),
            actor_class_id = spawn.actor_class_id,
            script = %spawn.script_name,
            allegiance = spawn.allegiance,
            pos = ?(spawn.position_x, spawn.position_y, spawn.position_z),
            "SpawnBattleNpcById applied (B1: actor materialised + spawn bundle fanned out)",
        );
    }

    /// B2 of the SEQ_005 unblock plan — port of C#
    /// `Party::AddMember` semantics for the local-zone case (the
    /// only path the combat-tutorial scripts exercise). Updates
    /// the leader session's transient member list and re-broadcasts
    /// the GroupHeader / GroupMembersBegin / GroupMembersX08 /
    /// GroupMembersEnd sequence so the client's party-list UI
    /// shows the freshly-added ally.
    ///
    /// Phase B2 simplifications:
    ///   * No persistent server-side party state; the roster lives
    ///     on `Session.transient_party_members` and re-broadcasts
    ///     the full list every change. Cross-zone sync (which would
    ///     route through world-server's `OP_WORLD_PARTY_INVITE` →
    ///     `PartyManager::add_member` flow) is a follow-up.
    ///   * No client-side accept prompt; the new member auto-joins
    ///     (matches the tutorial use case where the allies are NPCs
    ///     with no client of their own).
    ///   * Member names default to a synthetic `bnpc_<id>` if the
    ///     actor isn't in the registry yet (rare race window).
    async fn apply_party_add_member(
        &self,
        leader_actor_id: u32,
        member_actor_id: u32,
    ) {
        let Some(leader_handle) = self.registry.get(leader_actor_id).await else {
            tracing::debug!(
                leader = format!("0x{leader_actor_id:08X}"),
                "PartyAddMember skipped — leader not in registry",
            );
            return;
        };
        let session_id = leader_handle.session_id;
        let leader_name = {
            let c = leader_handle.character.read().await;
            c.base.display_name().to_string()
        };

        // Append to transient roster. Idempotent: if the member is
        // already in the list (script double-fired AddMember) the
        // re-broadcast still produces the same packet content.
        let members_actor_ids = {
            let Some(mut snap) = self.world.session(session_id).await else {
                tracing::debug!(
                    session = session_id,
                    "PartyAddMember skipped — no session for leader",
                );
                return;
            };
            if !snap.transient_party_members.contains(&member_actor_id) {
                snap.transient_party_members.push(member_actor_id);
            }
            let ids = snap.transient_party_members.clone();
            self.world.upsert_session(snap).await;
            ids
        };

        // Build GroupMember rows: leader first, then the transient
        // adds. Look up names; default to "bnpc_<id>" placeholder if
        // the member isn't registered yet (B1's spawn happens in the
        // same drain so this should normally resolve).
        let mut members = Vec::with_capacity(1 + members_actor_ids.len());
        members.push(crate::packets::send::groups::GroupMember {
            actor_id: leader_actor_id,
            localized_name: -1,
            unknown2: 0,
            flag1: false,
            is_online: true,
            name: leader_name,
        });
        for &mid in &members_actor_ids {
            let name = if let Some(h) = self.registry.get(mid).await {
                let c = h.character.read().await;
                c.base.display_name().to_string()
            } else {
                format!("bnpc_{mid:08X}")
            };
            members.push(crate::packets::send::groups::GroupMember {
                actor_id: mid,
                localized_name: -1,
                unknown2: 0,
                flag1: false,
                is_online: true,
                name,
            });
        }

        // Build the trio. Group index uses the same solo-self flag
        // pattern `send_zone_in_bundle` uses; sequence_id is fresh.
        // Tutorial allies don't promote the player out of the
        // synthetic solo-self party — they just join it.
        const PARTY_SOLO_SELF_FLAG: u64 = 0x8000_0000_0000_0000;
        const GROUP_TYPE_PARTY: u32 = 0x2711;
        let group_index: u64 = PARTY_SOLO_SELF_FLAG | (leader_actor_id as u64);
        let zone_actor_id = leader_handle.zone_id;
        let location_code = zone_actor_id as u64;
        let sequence_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or_default();

        let mut offset = 0usize;
        let mut subs = vec![
            crate::packets::send::groups::build_group_header(
                leader_actor_id,
                location_code,
                sequence_id,
                group_index,
                GROUP_TYPE_PARTY,
                -1,
                "",
                members.len() as u32,
            ),
            crate::packets::send::groups::build_group_members_begin(
                leader_actor_id,
                location_code,
                sequence_id,
                group_index,
                members.len() as u32,
            ),
            crate::packets::send::groups::build_group_members_x08(
                leader_actor_id,
                location_code,
                sequence_id,
                &members,
                &mut offset,
            ),
            crate::packets::send::groups::build_group_members_end(
                leader_actor_id,
                location_code,
                sequence_id,
                group_index,
            ),
        ];

        let Some(client) = self.world.client(session_id).await else {
            tracing::debug!(session = session_id, "PartyAddMember skipped — no client handle");
            return;
        };
        for sub in &mut subs {
            sub.set_target_id(session_id);
            client.send_bytes(sub.to_bytes()).await;
        }

        tracing::info!(
            leader = format!("0x{leader_actor_id:08X}"),
            member = format!("0x{member_actor_id:08X}"),
            roster = members.len(),
            "PartyAddMember applied (B2: transient roster + group trio rebroadcast)",
        );
    }

    /// B4 of the SEQ_005 unblock plan — port of C#
    /// `Director::AddMember`. Appends `member_actor_id` to the
    /// player session's transient roster for `director_actor_id`,
    /// then re-broadcasts the GroupHeader / GroupMembersBegin /
    /// GroupMembersX08 / GroupMembersEnd trio keyed by the
    /// director's group id so the client's content-group UI shows
    /// the freshly-added member.
    ///
    /// Phase B4 simplification: solo-tutorial-only (broadcasts to
    /// the single `player_actor_id` argument's client). Multi-
    /// player content groups (Phase B5+) would walk the director's
    /// `player_members` set and broadcast to each.
    async fn apply_director_add_member(
        &self,
        player_actor_id: u32,
        director_actor_id: u32,
        member_actor_id: u32,
    ) {
        let Some(player_handle) = self.registry.get(player_actor_id).await else {
            tracing::debug!(
                player = format!("0x{player_actor_id:08X}"),
                director = format!("0x{director_actor_id:08X}"),
                "DirectorAddMember skipped — player not in registry",
            );
            return;
        };
        let session_id = player_handle.session_id;

        // Append to the per-director roster on Session.
        let roster = {
            let Some(mut snap) = self.world.session(session_id).await else {
                tracing::debug!(
                    session = session_id,
                    "DirectorAddMember skipped — no session",
                );
                return;
            };
            let entry = snap
                .transient_director_members
                .entry(director_actor_id)
                .or_default();
            if !entry.contains(&member_actor_id) {
                entry.push(member_actor_id);
            }
            let cloned = entry.clone();
            self.world.upsert_session(snap).await;
            cloned
        };

        // Build GroupMember rows from the roster. Resolve names from
        // the registry; placeholder for entries not yet registered
        // (rare when B1 spawns and B4 broadcasts in the same drain).
        let mut members = Vec::with_capacity(roster.len());
        for &mid in &roster {
            let name = if let Some(h) = self.registry.get(mid).await {
                let c = h.character.read().await;
                c.base.display_name().to_string()
            } else {
                format!("bnpc_{mid:08X}")
            };
            members.push(crate::packets::send::groups::GroupMember {
                actor_id: mid,
                localized_name: -1,
                unknown2: 0,
                flag1: false,
                is_online: true,
                name,
            });
        }

        // Build the trio. Group index uses the director's actor id
        // directly — the director IS the group key (no synthetic
        // solo-self flag like the player's party). C# uses the same
        // convention: `director.GetGroupId()` returns the director's
        // composite actor id.
        let group_index: u64 = director_actor_id as u64;
        let zone_actor_id = player_handle.zone_id;
        let location_code = zone_actor_id as u64;
        let sequence_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or_default();
        // Director groups use a different group_type than party.
        // C# `Director.GetGroupTypeId()` returns 30001 (0x7531) for
        // ContentGroup directors; tutorials use the same value.
        const GROUP_TYPE_CONTENT_GROUP: u32 = 30001;

        let mut offset = 0usize;
        let mut subs = vec![
            crate::packets::send::groups::build_group_header(
                player_actor_id,
                location_code,
                sequence_id,
                group_index,
                GROUP_TYPE_CONTENT_GROUP,
                -1,
                "",
                members.len() as u32,
            ),
            crate::packets::send::groups::build_group_members_begin(
                player_actor_id,
                location_code,
                sequence_id,
                group_index,
                members.len() as u32,
            ),
            crate::packets::send::groups::build_group_members_x08(
                player_actor_id,
                location_code,
                sequence_id,
                &members,
                &mut offset,
            ),
            crate::packets::send::groups::build_group_members_end(
                player_actor_id,
                location_code,
                sequence_id,
                group_index,
            ),
        ];

        let Some(client) = self.world.client(session_id).await else {
            tracing::debug!(
                session = session_id,
                "DirectorAddMember skipped — no client handle",
            );
            return;
        };
        for sub in &mut subs {
            sub.set_target_id(session_id);
            client.send_bytes(sub.to_bytes()).await;
        }

        tracing::info!(
            director = format!("0x{director_actor_id:08X}"),
            member = format!("0x{member_actor_id:08X}"),
            roster = members.len(),
            "DirectorAddMember applied (B4: roster + group trio rebroadcast)",
        );
    }

    /// Combat-tutorial / instance entry — port of C#
    /// `WorldManager.DoZoneChangeContent` (Map Server/WorldManager.cs:971).
    /// Updates the player's position to the content-area spawn coords,
    /// then emits the trio that tells the 1.x client to wipe the world
    /// and re-render: `DeleteAllActors (0x0007)` + `0x00E2(0x10)` + the
    /// standard zone-in bundle.
    ///
    /// Phase 1 simplification: we don't yet maintain a separate
    /// `PrivateAreaContent` actor list on the parent zone, so the player
    /// stays attached to the parent zone (no shadowed actors / no
    /// instance isolation). The visual effect is "world clears + player
    /// is re-spawned at the new coords"; combat-tutorial NPCs spawn into
    /// the same parent-zone scope, which matches Yda/Papalymo's existing
    /// positions until the proper instance subsystem lands.
    #[allow(clippy::too_many_arguments)]
    async fn apply_do_zone_change_content(
        &self,
        player_id: u32,
        parent_zone_id: u32,
        area_name: String,
        _director_actor_id: u32,
        spawn_type: u8,
        x: f32,
        y: f32,
        z: f32,
        rotation: f32,
    ) {
        let Some(handle) = self.registry.get(player_id).await else {
            tracing::warn!(player = player_id, "DoZoneChangeContent: actor missing");
            return;
        };
        let session_id = handle.session_id;
        let actor_id = handle.actor_id;

        // 1. Update character position so subsequent reads + the zone-in
        //    bundle's `CreateSpawnPositionPacket` see the new coords.
        {
            let mut c = handle.character.write().await;
            c.base.position_x = x;
            c.base.position_y = y;
            c.base.position_z = z;
            c.base.rotation = rotation;
            c.base.zone_id = parent_zone_id;
        }

        // 2. Update the session's destination + zone fields so the
        //    zone-in bundle pulls the right values.
        if let Some(mut snap) = self.world.session(session_id).await {
            snap.current_zone_id = parent_zone_id;
            snap.destination_zone_id = parent_zone_id;
            snap.destination_spawn_type = spawn_type;
            snap.destination_x = x;
            snap.destination_y = y;
            snap.destination_z = z;
            snap.destination_rot = rotation;
            self.world.upsert_session(snap).await;
        }

        // 3. Emit the zone-change packet trio. Order matters: client
        //    expects the world wipe first, then the 0x00E2 marker, then
        //    the zone-in payload.
        let Some(client) = self.world.client(session_id).await else {
            tracing::warn!(player = player_id, "DoZoneChangeContent: no client");
            return;
        };
        client
            .send_bytes(
                crate::packets::send::handshake::build_delete_all_actors(actor_id).to_bytes(),
            )
            .await;
        client
            .send_bytes(
                crate::packets::send::handshake::build_0xe2(actor_id, 0x10).to_bytes(),
            )
            .await;

        // 4. Replay the zone-in bundle. `send_zone_in_bundle` reads from
        //    the session + character we just updated, so the bundle
        //    spawns the player at the content-area coords.
        self.world
            .send_zone_in_bundle(&self.registry, session_id, spawn_type as u16)
            .await;

        // 5. B7 of the SEQ_005 unblock plan — fire the content
        //    script's `onZoneIn(player, contentArea, isLogin)`
        //    hook, mirroring C# `WorldManager.DoZoneChangeContent`'s
        //    final line:
        //      LuaEngine.GetInstance().CallLuaFunction(
        //          player, contentArea, "onZoneIn", true);
        //    (Map Server/WorldManager.cs:1010). Some content
        //    scripts register cutscene triggers / spawns in
        //    `onZoneIn` rather than `onCreate`; without this call
        //    those triggers never fire. We read the active content
        //    script captured on the session by Phase A's
        //    `apply_create_content_area` to know which script to
        //    target.
        let active = self
            .world
            .session(session_id)
            .await
            .and_then(|s| s.active_content_script);
        if let (Some(active), Some(lua)) = (active, self.lua.as_ref()) {
            let script_path = lua.resolver().content(&active.content_script);
            if script_path.exists() {
                let snapshot = {
                    let c = handle.character.read().await;
                    build_player_snapshot_from_character(&c)
                };
                let placeholder_queue = crate::lua::command::CommandQueue::new();
                let content_area = crate::lua::userdata::LuaContentArea {
                    parent_zone_id: active.parent_zone_id,
                    area_name: active.area_name.clone(),
                    area_class_path: active.area_class_path.clone(),
                    director_name: active.director_name.clone(),
                    director_actor_id: active.director_actor_id,
                    queue: placeholder_queue.clone(),
                };
                let director = crate::lua::userdata::LuaDirectorHandle {
                    name: active.director_name.clone(),
                    actor_id: active.director_actor_id,
                    class_path: format!("/Director/{}", active.director_name),
                    queue: placeholder_queue,
                };
                let lua_clone = lua.clone();
                let result = tokio::task::spawn_blocking(move || {
                    lua_clone.call_content_hook(
                        &script_path,
                        "onZoneIn",
                        snapshot,
                        content_area,
                        director,
                    )
                })
                .await;
                if let Ok(partial) = result {
                    if let Some(e) = partial.error {
                        tracing::debug!(
                            player = player_id,
                            content_script = %active.content_script,
                            error = %e,
                            "onZoneIn errored (likely missing binding — Phase B7 expected)",
                        );
                    }
                    if !partial.commands.is_empty() {
                        crate::runtime::quest_apply::apply_runtime_lua_commands(
                            partial.commands,
                            &self.registry,
                            &self.db,
                            &self.world,
                            self.lua.as_ref(),
                        )
                        .await;
                    }
                }
            }
        }

        tracing::info!(
            player = player_id,
            parent_zone = parent_zone_id,
            area = %area_name,
            x,
            y,
            z,
            "DoZoneChangeContent applied (B7: warp + zone-in replay + onZoneIn fired)",
        );
    }

    /// Cross-zone warp — port of C# `WorldManager.DoZoneChange`
    /// (Map Server/WorldManager.cs:855). Mirrors `apply_do_zone_change_content`'s
    /// packet flow exactly (the retail pcaps `gridania_to_coerthas.pcapng` /
    /// `from_gridania_to_blackshroud.pcapng` show the same single
    /// `0x00E2(0x10)` marker around the zone-in bundle whether the
    /// destination is a sibling zone or a content area), but uses
    /// `WorldManager::do_zone_change` to actually migrate the actor
    /// between zone registries instead of just updating in-place.
    ///
    /// Same-zone targets short-circuit the registry move and behave
    /// like a glorified `WarpToPosition` followed by a re-render —
    /// quest scripts use this idiom for "you teleport but stay in
    /// the same zone" effects (e.g. `man0g0::doNoticeEvent` warps
    /// the player to the cinematic vantage point with a fresh
    /// loading screen).
    ///
    /// `private_area`/`private_area_type` are accepted to match the
    /// Lua signature but currently unused — garlemald's `Zone` model
    /// stores private areas as children of their parent `zone_id`,
    /// and a separate `SetPrivateArea`-style packet would be needed
    /// to flip the client onto a specific private replica. Filed as
    /// a follow-up: most quest call sites pass `nil` so the public
    /// area is the right destination already.
    #[allow(clippy::too_many_arguments)]
    async fn apply_do_zone_change(
        &self,
        player_id: u32,
        zone_id: u32,
        private_area: Option<String>,
        private_area_type: u32,
        spawn_type: u8,
        x: f32,
        y: f32,
        z: f32,
        rotation: f32,
    ) {
        let Some(handle) = self.registry.get(player_id).await else {
            tracing::warn!(player = player_id, "DoZoneChange: actor missing");
            return;
        };
        let session_id = handle.session_id;
        let actor_id = handle.actor_id;
        if session_id == 0 {
            tracing::debug!(player = player_id, "DoZoneChange: no session (NPC?)");
            return;
        }

        // 1. Migrate the actor between zones (no-op if zone_id is the
        //    same as the current zone). `do_zone_change_with_private_area`
        //    also updates the session's destination + zone +
        //    private-area fields. `private_area = Some` routes the
        //    actor into that PrivateArea instance's core pool;
        //    `None` (or unknown name) goes to the parent zone's core.
        let spawn = common::Vector3::new(x, y, z);
        if let Err(e) = self
            .world
            .do_zone_change_with_private_area(
                actor_id,
                session_id,
                zone_id,
                private_area.clone(),
                private_area_type,
                spawn,
                rotation,
            )
            .await
        {
            tracing::error!(
                error = %e,
                player = player_id,
                zone = zone_id,
                ?private_area,
                "DoZoneChange: world.do_zone_change_with_private_area failed"
            );
            return;
        }

        // 2. Update the character's persistent zone_id + position
        //    (the registry move above only touches the spatial grid;
        //    the Character row's `base.zone_id` is what `send_zone_in_bundle`
        //    reads on the next login + what persists to disk).
        {
            let mut c = handle.character.write().await;
            c.base.zone_id = zone_id;
            c.base.position_x = x;
            c.base.position_y = y;
            c.base.position_z = z;
            c.base.rotation = rotation;
        }

        // 3. Carry the requested spawn_type through to the zone-in
        //    bundle so the client plays the right "you arrived" anim.
        if let Some(mut snap) = self.world.session(session_id).await {
            snap.destination_spawn_type = spawn_type;
            self.world.upsert_session(snap).await;
        }

        // 4. Emit the zone-change packet trio — same order as
        //    `apply_do_zone_change_content`.
        let Some(client) = self.world.client(session_id).await else {
            tracing::warn!(player = player_id, "DoZoneChange: no client");
            return;
        };
        client
            .send_bytes(
                crate::packets::send::handshake::build_delete_all_actors(actor_id).to_bytes(),
            )
            .await;
        client
            .send_bytes(
                crate::packets::send::handshake::build_0xe2(actor_id, 0x10).to_bytes(),
            )
            .await;

        // 5. Replay the zone-in bundle. `send_zone_in_bundle` reads
        //    from the session + character we just updated, so the
        //    bundle spawns the player at the new coords.
        self.world
            .send_zone_in_bundle(&self.registry, session_id, spawn_type as u16)
            .await;

        tracing::info!(
            player = player_id,
            zone = zone_id,
            ?private_area,
            private_area_type,
            spawn_type,
            x,
            y,
            z,
            rotation,
            "DoZoneChange applied (cross-zone warp + zone-in replay)",
        );
    }

    /// `WorldManager:WarpToPublicArea(player[, x, y, z, rot])` — quest
    /// scripts use this to send the player back to the public-area
    /// version of their current zone. With no target, uses the
    /// player's current pos (so the visible effect is just a
    /// loading-screen flicker as the private area is unwound). With
    /// a target, warps to that position.
    ///
    /// Garlemald's zone model stores private areas as children of a
    /// parent zone_id — the "public area" of zone N is just zone N
    /// itself with no private-area routing. So this is essentially
    /// a same-parent-zone DoZoneChange with `private_area=None`.
    async fn apply_warp_to_public_area(
        &self,
        player_id: u32,
        target: Option<(f32, f32, f32, f32)>,
    ) {
        let Some(handle) = self.registry.get(player_id).await else {
            tracing::warn!(player = player_id, "WarpToPublicArea: actor missing");
            return;
        };
        let (zone_id, cur_x, cur_y, cur_z, cur_rot) = {
            let c = handle.character.read().await;
            (c.base.zone_id, c.base.position_x, c.base.position_y, c.base.position_z, c.base.rotation)
        };
        let (x, y, z, rotation) = target.unwrap_or((cur_x, cur_y, cur_z, cur_rot));
        // spawn_type=2 == "warp" (matches Meteor's WarpToPublicArea
        // path which passes 2 to DoZoneChange).
        self.apply_do_zone_change(player_id, zone_id, None, 0, 2, x, y, z, rotation)
            .await;
    }

    /// `WorldManager:WarpToPrivateArea(player, area_class, area_index
    /// [, x, y, z, rot])` — quest scripts use this to instance the
    /// player into a named private-area replica (e.g. cutscene-only
    /// flashback variants like `PrivateAreaMasterPast`). Resolves
    /// the private area against the player's current parent zone
    /// then dispatches a DoZoneChange carrying the area routing.
    async fn apply_warp_to_private_area(
        &self,
        player_id: u32,
        area_class: String,
        area_index: u32,
        target: Option<(f32, f32, f32, f32)>,
    ) {
        let Some(handle) = self.registry.get(player_id).await else {
            tracing::warn!(
                player = player_id,
                %area_class,
                area_index,
                "WarpToPrivateArea: actor missing"
            );
            return;
        };
        let (zone_id, cur_x, cur_y, cur_z, cur_rot) = {
            let c = handle.character.read().await;
            (c.base.zone_id, c.base.position_x, c.base.position_y, c.base.position_z, c.base.rotation)
        };
        let (x, y, z, rotation) = target.unwrap_or((cur_x, cur_y, cur_z, cur_rot));
        self.apply_do_zone_change(
            player_id,
            zone_id,
            Some(area_class),
            area_index,
            2,
            x,
            y,
            z,
            rotation,
        )
        .await;
    }

    // =======================================================================
    // Retainer lifecycle helpers (Tier 4 #14)
    //
    // These live on the processor rather than in `runtime/quest_apply.rs`
    // because they mutate `Session` state — the session store lives on
    // `WorldManager` which the quest_apply drain doesn't hold. Once the
    // Session becomes registry-adjacent we can consolidate.
    // =======================================================================

    async fn apply_spawn_my_retainer(
        &self,
        player_id: u32,
        bell_actor_id: u32,
        bell_position: (f32, f32, f32),
        retainer_index: i32,
    ) {
        let Some(handle) = self.registry.get(player_id).await else {
            tracing::debug!(player = player_id, "SpawnMyRetainer: no actor in registry");
            return;
        };
        let session_id = handle.session_id;
        if session_id == 0 {
            tracing::debug!(player = player_id, "SpawnMyRetainer: no session (NPC?)");
            return;
        }
        let template = match self.db.load_retainer(player_id, retainer_index).await {
            Ok(Some(t)) => t,
            Ok(None) => {
                tracing::info!(
                    player = player_id,
                    idx = retainer_index,
                    "SpawnMyRetainer: character owns no retainer at this index",
                );
                return;
            }
            Err(e) => {
                tracing::warn!(
                    player = player_id,
                    idx = retainer_index,
                    err = %e,
                    "SpawnMyRetainer: DB lookup failed",
                );
                return;
            }
        };
        if template.class_path.is_empty() {
            tracing::warn!(
                player = player_id,
                retainer_id = template.id,
                actor_class_id = template.actor_class_id,
                "SpawnMyRetainer: retainer template has no actor class — `gamedata_actor_class` row missing",
            );
            return;
        }
        // Reproduce Meteor's 1-unit-toward-player offset math
        // (Player.cs:2010-2012). Read the player's snapshot once.
        // `handle.zone_id` is the registry's canonical zone — read
        // from there rather than `c.base.zone_id` because login flow
        // writes to the handle first and the Character mirror lags
        // until the next position update.
        let player_pos = {
            let c = handle.character.read().await;
            (c.base.position_x, c.base.position_y, c.base.position_z)
        };
        let zone_id = handle.zone_id;
        let (px, _py, pz) = player_pos;
        let (bx, by, bz) = bell_position;
        let dx = px - bx;
        let dz = pz - bz;
        let dist = (dx * dx + dz * dz).sqrt();
        let (pos_x, pos_z, rotation) = if dist > 0.0 {
            let ox = bx - (-dx / dist);
            let oz = bz - (-dz / dist);
            let rot = (px - ox).atan2(pz - oz);
            (ox, oz, rot)
        } else {
            (bx, bz, 0.0)
        };

        // Allocate a deterministic actor id for the retainer.
        // Mirrors Meteor's `(4 << 28 | zone << 19 | 0)` formula
        // (Npc.cs:60), but garlemald's `ActorRegistry` is shared across
        // sessions so the C# trick of reusing `local_id = 0` for every
        // retainer would collide. Stash the player's actor id in the
        // bottom 18 bits with the high bit set — the boot spawn pass
        // hands out sequential local ids starting at 1, so the
        // `0x40000` marker keeps retainer ids out of that range while
        // staying unique per (player, zone).
        let local_id = 0x40000u32 | (player_id & 0x3FFFF);
        let retainer_actor_id = (4u32 << 28) | ((zone_id & 0x1FF) << 19) | local_id;

        // Build a one-off `Character` shaped like an Npc just to
        // satisfy the `push_npc_spawn` packet emitter. We don't insert
        // it into `ActorRegistry`/`Zone` — retainers are session-
        // private (only the owner sees them) and Meteor handles
        // event-routing by checking `session.GetActor().currentSpawnedRetainer.actorId`
        // before falling back to world lookup
        // (PacketProcessor.cs:205). A future `EventStart` handler can
        // do the same against `session.spawned_retainer`.
        let actor_class = crate::npc::ActorClass::new(
            template.actor_class_id,
            template.class_path.clone(),
            0,
            0,
            "",
            0,
            0,
            0,
        );
        let mut npc = crate::npc::Npc::new(
            local_id,
            &actor_class,
            "myretainer",
            zone_id,
            pos_x,
            by,
            pos_z,
            rotation,
            0,
            0,
            Some(template.name.clone()),
        );
        npc.character.base.actor_id = retainer_actor_id;
        npc.character.chara.actor_class_id = template.actor_class_id;
        // Retail uses `_rtnre{actorId:x7}` for the wire actor name.
        npc.character.base.actor_name =
            format!("_rtnre{:07x}", retainer_actor_id);

        // Resolve the zone name for `generate_npc_actor_name` inside
        // `push_npc_spawn`. Missing zone is non-fatal — the helper
        // tolerates an empty string by using the raw class path.
        let zone_name = match self.world.zone(zone_id).await {
            Some(z) => z.read().await.core.zone_name.clone(),
            None => String::new(),
        };

        // Emit the standard NPC spawn bundle, but ONLY to the owner's
        // session. Retainers are personal-instance actors — Meteor
        // never broadcasts them via `BroadcastPacketAroundActor`, only
        // queues onto the owner's `actorInstanceList` in
        // `Session.UpdateInstance` (Session.cs:134).
        let bundle = crate::world_manager::build_retainer_spawn_bundle(
            &npc.character,
            &zone_name,
        );
        if let Some(client) = self.world.client(session_id).await {
            for mut sub in bundle {
                sub.set_target_id(session_id);
                client.send_bytes(sub.to_bytes()).await;
            }
        } else {
            tracing::debug!(
                player = player_id,
                session = session_id,
                "SpawnMyRetainer: no client handle — packets dropped (session disconnected mid-summon)",
            );
        }

        // Tier 4 #14 B — instantiate the `RetainerMeetingRelationGroup`
        // that binds this player to their summoned retainer for the
        // duration of the bell session. The group id is derived
        // deterministically from the composite retainer actor id so
        // two independent summons in parallel zones don't collide.
        // Dispatched through the shared group dispatcher so the
        // Header / Begin / MembersX02 / End bundle lands on the
        // owning client matching the pattern used for parties.
        let group_id = retainer_meeting_group_id(retainer_actor_id);
        {
            use crate::group::{GroupKind, GroupTypeId, RetainerMeetingRelationGroup};
            use crate::group::outbox::{GroupEvent, GroupOutbox};
            let mut outbox = GroupOutbox::new();
            let _group = RetainerMeetingRelationGroup::new(
                group_id,
                handle.actor_id,
                retainer_actor_id,
                &mut outbox,
            );
            let resolver = RetainerMeetingResolver {
                group_id,
                player_actor_id: handle.actor_id,
                player_name: {
                    let c = handle.character.read().await;
                    c.base.actor_name.clone()
                },
                retainer_actor_id,
                retainer_name: template.name.clone(),
            };
            for event in outbox.drain() {
                // Stamp the kind up front so `dispatch_group_event`
                // doesn't fall back to `Party` when the roster
                // branch queries `resolver.kind`.
                if let GroupEvent::GroupCreated { kind, type_id, .. } = &event {
                    debug_assert_eq!(*kind, GroupKind::Retainer);
                    debug_assert_eq!(*type_id, GroupTypeId::RETAINER);
                }
                crate::group::dispatch_group_event(
                    &event,
                    &self.registry,
                    &self.world,
                    &resolver,
                )
                .await;
            }
        }

        let Some(mut session) = self.world.session(session_id).await else {
            return;
        };
        session.spawned_retainer = Some(crate::data::SpawnedRetainer {
            retainer_id: template.id,
            actor_class_id: template.actor_class_id,
            class_path: template.class_path.clone(),
            name: template.name.clone(),
            actor_id: retainer_actor_id,
            position: (pos_x, by, pos_z),
            rotation,
            sent_spawn_packets: true,
            group_id,
        });
        self.world.upsert_session(session).await;
        let _ = bell_actor_id; // bell is the UI-side click source; the
        // relation-group is player↔retainer, not player↔bell.
        tracing::info!(
            player = player_id,
            idx = retainer_index,
            retainer_id = template.id,
            actor_id = format!("0x{:08X}", retainer_actor_id),
            name = %template.name,
            class_path = %template.class_path,
            group_id = format!("0x{:016X}", group_id),
            "SpawnMyRetainer applied (live actor + meeting group packets sent to owner session)",
        );
    }

    async fn apply_despawn_my_retainer(&self, player_id: u32) {
        let Some(handle) = self.registry.get(player_id).await else {
            return;
        };
        let session_id = handle.session_id;
        if session_id == 0 {
            return;
        }
        let Some(mut session) = self.world.session(session_id).await else {
            return;
        };
        let despawned = session.spawned_retainer.take();
        self.world.upsert_session(session).await;

        // Send `RemoveActor` to the owning session so the client drops
        // the retainer model. Mirror of Meteor's Session.cs:121-125
        // "actorInstanceList[i] is Retainer && currentSpawnedRetainer
        // == null → QueuePacket(RemoveActorPacket)" sweep.
        if let Some(snap) = &despawned
            && let Some(client) = self.world.client(session_id).await
        {
            let mut sub = tx::actor::build_remove_actor(snap.actor_id);
            sub.set_target_id(session_id);
            client.send_bytes(sub.to_bytes()).await;
        }
        // Tier 4 #14 B — tear down the `RetainerMeetingRelationGroup`
        // so the client's group table stops tracking the now-absent
        // retainer. Skip when the spawn never actually created a
        // group (group_id == 0).
        if let Some(snap) = &despawned
            && snap.group_id != 0
        {
            use crate::group::RetainerMeetingRelationGroup;
            use crate::group::outbox::GroupOutbox;
            let mut outbox = GroupOutbox::new();
            let mut group = RetainerMeetingRelationGroup::new(
                snap.group_id,
                player_id,
                snap.actor_id,
                &mut outbox,
            );
            // `RetainerMeetingRelationGroup::new` pushed a
            // `GroupCreated` event we don't care about here — drop
            // it by draining before `delete`.
            let _ = outbox.drain();
            group.delete(&mut outbox);
            let resolver = RetainerMeetingResolver {
                group_id: snap.group_id,
                player_actor_id: player_id,
                player_name: String::new(),
                retainer_actor_id: snap.actor_id,
                retainer_name: snap.name.clone(),
            };
            for event in outbox.drain() {
                crate::group::dispatch_group_event(
                    &event,
                    &self.registry,
                    &self.world,
                    &resolver,
                )
                .await;
            }
        }
        tracing::info!(
            player = player_id,
            had = despawned.is_some(),
            actor_id = ?despawned.as_ref().map(|s| format!("0x{:08X}", s.actor_id)),
            group_id = ?despawned.as_ref().map(|s| format!("0x{:016X}", s.group_id)),
            "DespawnMyRetainer applied",
        );
    }

    async fn apply_hire_retainer(&self, player_id: u32, retainer_id: u32) {
        match self.db.hire_retainer(player_id, retainer_id).await {
            Ok(true) => tracing::info!(
                player = player_id,
                retainer_id,
                "HireRetainer: fresh hire recorded",
            ),
            Ok(false) => tracing::info!(
                player = player_id,
                retainer_id,
                "HireRetainer: already hired (idempotent no-op)",
            ),
            Err(e) => tracing::warn!(
                player = player_id,
                retainer_id,
                err = %e,
                "HireRetainer: DB insert failed",
            ),
        }
    }

    // =======================================================================
    // Inn / dream helpers (Tier 4 #17)
    // =======================================================================

    /// `player:SetSleeping()` — called from `ObjectBed.lua` right
    /// before the client-facing `Logout` / `QuitGame` RPC. Resolves
    /// the player's zone to its `is_inn` flag, maps their XZ
    /// position to an inn-room code (1/2/3), and snaps the character
    /// transform to the canonical bed coord for that room. Zero-inn
    /// zones + positions outside any room are silently no-oped so
    /// GM `/bed` spawns from open fields don't teleport the player.
    async fn apply_set_sleeping(&self, player_id: u32) {
        let Some(handle) = self.registry.get(player_id).await else {
            return;
        };
        let Some(zone_arc) = self.world.zone(handle.zone_id).await else {
            return;
        };
        let is_inn = { zone_arc.read().await.core.is_inn };
        if !is_inn {
            tracing::debug!(player = player_id, "SetSleeping: not in inn zone, no-op");
            return;
        }
        let (x, y, z) = {
            let c = handle.character.read().await;
            (c.base.position_x, c.base.position_y, c.base.position_z)
        };
        let inn_code = crate::actor::inn::inn_code_from_position((x, y, z), true);
        let Some(bed) = crate::actor::inn::sleeping_position_for_inn(inn_code) else {
            tracing::debug!(
                player = player_id,
                inn_code,
                "SetSleeping: player not in any known inn room; skipping snap",
            );
            return;
        };
        {
            let mut c = handle.character.write().await;
            c.base.position_x = bed.0;
            c.base.position_y = bed.1;
            c.base.position_z = bed.2;
            c.base.rotation = bed.3;
        }
        // Mark the session as sleeping so the next login reads it.
        let session_id = handle.session_id;
        if session_id != 0
            && let Some(mut session) = self.world.session(session_id).await
        {
            session.is_sleeping = true;
            self.world.upsert_session(session).await;
        }
        tracing::info!(
            player = player_id,
            inn_code,
            pos = ?bed,
            "SetSleeping applied",
        );
    }

    async fn apply_start_dream(&self, player_id: u32, dream_id: u8) {
        let Some(handle) = self.registry.get(player_id).await else {
            return;
        };
        let session_id = handle.session_id;
        let is_inn = if let Some(zone) = self.world.zone(handle.zone_id).await {
            zone.read().await.core.is_inn
        } else {
            false
        };
        let inn_code = {
            let c = handle.character.read().await;
            crate::actor::inn::inn_code_from_position(
                (c.base.position_x, c.base.position_y, c.base.position_z),
                is_inn,
            )
        };
        if session_id != 0
            && let Some(mut session) = self.world.session(session_id).await
        {
            session.current_dream_id = Some(dream_id);
            self.world.upsert_session(session).await;
        }
        if session_id != 0
            && let Some(client) = self.world.client(session_id).await
        {
            let pkt = crate::packets::send::player::build_set_player_dream(
                handle.actor_id,
                dream_id,
                inn_code,
            );
            if let Ok(base) = common::BasePacket::create_from_subpacket(&pkt, true, false) {
                client.send_bytes(base.to_bytes()).await;
            }
        }
        tracing::info!(player = player_id, dream_id, inn_code, "StartDream applied");
    }

    /// `player:Logout()` — emit `LogoutPacket` (0x000E) to the owner's
    /// session. The client responds by closing the world connection
    /// and returning to character select. Mirrors C# `Player.Logout`
    /// (`Map Server/Actors/Chara/Player/Player.cs:861`); the
    /// `RemoveStatusEffectsByFlags(LoseOnLogout)` + `CleanupAndSave()`
    /// tail it does is deferred — the same status-cleanup gap is
    /// already noted in the post-roadmap follow-ups list, and persistent
    /// player save is currently driven by the regular DB upsert path
    /// rather than a logout-specific flush.
    async fn apply_logout(&self, player_id: u32) {
        let Some(handle) = self.registry.get(player_id).await else {
            tracing::debug!(player = player_id, "Logout: player not in registry");
            return;
        };
        let session_id = handle.session_id;
        if session_id == 0 {
            tracing::debug!(player = player_id, "Logout: no session (NPC?)");
            return;
        }
        let Some(client) = self.world.client(session_id).await else {
            tracing::debug!(
                player = player_id,
                session = session_id,
                "Logout: no client handle (already disconnected)",
            );
            return;
        };
        let pkt = tx::handshake::build_logout(handle.actor_id);
        if let Ok(base) = common::BasePacket::create_from_subpacket(&pkt, true, false) {
            client.send_bytes(base.to_bytes()).await;
        }
        tracing::info!(player = player_id, session = session_id, "Logout applied");
    }

    /// `player:QuitGame()` — emit `QuitPacket` (0x0011) to the owner's
    /// session. The client responds by terminating its process (back
    /// to launcher / desktop). Mirrors C# `Player.QuitGame`
    /// (`Map Server/Actors/Chara/Player/Player.cs:869`); same status-
    /// cleanup deferral as [`apply_logout`].
    async fn apply_quit_game(&self, player_id: u32) {
        let Some(handle) = self.registry.get(player_id).await else {
            tracing::debug!(player = player_id, "QuitGame: player not in registry");
            return;
        };
        let session_id = handle.session_id;
        if session_id == 0 {
            tracing::debug!(player = player_id, "QuitGame: no session (NPC?)");
            return;
        }
        let Some(client) = self.world.client(session_id).await else {
            tracing::debug!(
                player = player_id,
                session = session_id,
                "QuitGame: no client handle (already disconnected)",
            );
            return;
        };
        let pkt = tx::handshake::build_quit(handle.actor_id);
        if let Ok(base) = common::BasePacket::create_from_subpacket(&pkt, true, false) {
            client.send_bytes(base.to_bytes()).await;
        }
        tracing::info!(player = player_id, session = session_id, "QuitGame applied");
    }

    async fn apply_end_dream(&self, player_id: u32) {
        let Some(handle) = self.registry.get(player_id).await else {
            return;
        };
        let session_id = handle.session_id;
        let is_inn = if let Some(zone) = self.world.zone(handle.zone_id).await {
            zone.read().await.core.is_inn
        } else {
            false
        };
        let inn_code = {
            let c = handle.character.read().await;
            crate::actor::inn::inn_code_from_position(
                (c.base.position_x, c.base.position_y, c.base.position_z),
                is_inn,
            )
        };
        if session_id != 0
            && let Some(mut session) = self.world.session(session_id).await
        {
            session.current_dream_id = None;
            self.world.upsert_session(session).await;
        }
        if session_id != 0
            && let Some(client) = self.world.client(session_id).await
        {
            let pkt = crate::packets::send::player::build_set_player_dream(
                handle.actor_id,
                0,
                inn_code,
            );
            if let Ok(base) = common::BasePacket::create_from_subpacket(&pkt, true, false) {
                client.send_bytes(base.to_bytes()).await;
            }
        }
        tracing::info!(player = player_id, "EndDream applied");
    }

    // =======================================================================
    // Chocobo lifecycle helpers (Tier 4 #15)
    //
    // Session snapshot stores the live mount state, but most of the
    // mutation is on `Character::chara` (`mount_state`, `has_chocobo`,
    // `chocobo_appearance`, `chocobo_name`, `rental_expire_time`,
    // `rental_min_left`). DB persistence is through the existing
    // `issue_player_chocobo` / `change_player_chocobo_appearance` /
    // `change_player_chocobo_name` setters.
    // =======================================================================

    async fn apply_issue_chocobo(&self, player_id: u32, appearance_id: u8, name: String) {
        let Some(handle) = self.registry.get(player_id).await else {
            return;
        };
        {
            let mut c = handle.character.write().await;
            c.chara.has_chocobo = true;
            c.chara.chocobo_appearance = appearance_id;
            c.chara.chocobo_name = name.clone();
        }
        if let Err(e) = self
            .db
            .issue_player_chocobo(player_id, appearance_id, &name)
            .await
        {
            tracing::warn!(player = player_id, err = %e, "IssueChocobo: DB persist failed");
        }
        // Client-visible updates: flag + name.
        if let Some(client) = self.world.client(handle.session_id).await {
            let name_pkt =
                crate::packets::send::player::build_set_chocobo_name(handle.actor_id, &name);
            let has_pkt =
                crate::packets::send::player::build_set_has_chocobo(handle.actor_id, true);
            if let Ok(base) = common::BasePacket::create_from_subpacket(&name_pkt, true, false) {
                client.send_bytes(base.to_bytes()).await;
            }
            if let Ok(base) = common::BasePacket::create_from_subpacket(&has_pkt, true, false) {
                client.send_bytes(base.to_bytes()).await;
            }
        }
        tracing::info!(
            player = player_id,
            appearance = appearance_id,
            name = %name,
            "IssueChocobo applied",
        );
    }

    async fn apply_start_chocobo_rental(&self, player_id: u32, minutes: u8) {
        let Some(handle) = self.registry.get(player_id).await else {
            return;
        };
        let now = common::utils::unix_timestamp() as u32;
        let expire = now + (minutes as u32 * 60);
        {
            let mut c = handle.character.write().await;
            c.chara.rental_expire_time = expire;
            c.chara.rental_min_left = minutes;
        }
        tracing::info!(
            player = player_id,
            minutes,
            "StartChocoboRental applied (expire in {minutes}m)",
        );
    }

    async fn apply_set_mount_state(&self, player_id: u32, state: u8) {
        let Some(handle) = self.registry.get(player_id).await else {
            return;
        };
        {
            let mut c = handle.character.write().await;
            c.chara.mount_state = state;
        }
        // Trigger a full mount appearance broadcast so nearby players
        // see the mount swap immediately — matches Meteor's
        // `Player.SetMountState` which calls SendMountAppearance.
        self.apply_send_mount_appearance(player_id).await;
    }

    async fn apply_send_mount_appearance(&self, player_id: u32) {
        let Some(handle) = self.registry.get(player_id).await else {
            return;
        };
        let (mount_state, appearance, expire, min_left) = {
            let c = handle.character.read().await;
            (
                c.chara.mount_state,
                c.chara.chocobo_appearance,
                c.chara.rental_expire_time,
                c.chara.rental_min_left,
            )
        };
        if mount_state == 0 {
            return; // No mount — nothing to broadcast.
        }
        let pkt = match mount_state {
            1 => crate::packets::send::player::build_set_current_mount_chocobo(
                handle.actor_id,
                appearance,
                expire,
                min_left,
            ),
            2 => crate::packets::send::player::build_set_current_mount_goobbue(
                handle.actor_id,
                1,
            ),
            _ => return,
        };
        if let Ok(base) = common::BasePacket::create_from_subpacket(&pkt, true, false) {
            let bytes = base.to_bytes();
            // Self-emit — the mount owner needs the packet for their
            // own HUD regardless of whether any neighbours are
            // around to see them.
            if let Some(client) = self.world.client(handle.session_id).await {
                client.send_bytes(bytes.clone()).await;
            }
            // Fan to every nearby Player via the shared zone-grid
            // broadcast (source is auto-excluded by `actors_around`).
            if let Some(zone) = self.world.zone(handle.zone_id).await {
                let sent = crate::runtime::broadcast::broadcast_around_actor(
                    &self.world,
                    &self.registry,
                    &zone,
                    handle.actor_id,
                    bytes,
                )
                .await;
                tracing::debug!(
                    player = player_id,
                    nearby = sent,
                    "SendMountAppearance broadcast fan-out",
                );
            }
        }
    }

    async fn apply_set_chocobo_name(&self, player_id: u32, name: String) {
        let Some(handle) = self.registry.get(player_id).await else {
            return;
        };
        {
            let mut c = handle.character.write().await;
            c.chara.chocobo_name = name.clone();
        }
        if let Err(e) = self.db.change_player_chocobo_name(player_id, &name).await {
            tracing::warn!(player = player_id, err = %e, "SetChocoboName: DB persist failed");
        }
        if let Some(client) = self.world.client(handle.session_id).await {
            let pkt =
                crate::packets::send::player::build_set_chocobo_name(handle.actor_id, &name);
            if let Ok(base) = common::BasePacket::create_from_subpacket(&pkt, true, false) {
                client.send_bytes(base.to_bytes()).await;
            }
        }
        tracing::info!(player = player_id, name = %name, "SetChocoboName applied");
    }

    // =======================================================================
    // Grand Company lifecycle helpers (Tier 4 #16)
    // =======================================================================

    /// Shared helper: emit the current `SetGrandCompanyPacket` for
    /// a player whose CharaState has freshly updated GC fields. The
    /// packet is self-only (the client uses it for its own menu /
    /// nameplate rendering — other players see the GC via the
    /// propertyFlags path). Assumes the caller already mutated
    /// CharaState; just reads + emits.
    async fn emit_grand_company_packet(&self, handle: &ActorHandle) {
        let (gc, l, g, u) = {
            let c = handle.character.read().await;
            (
                c.chara.gc_current,
                c.chara.gc_rank_limsa,
                c.chara.gc_rank_gridania,
                c.chara.gc_rank_uldah,
            )
        };
        if let Some(client) = self.world.client(handle.session_id).await {
            let pkt = crate::packets::send::player::build_set_grand_company(
                handle.actor_id,
                gc,
                l,
                g,
                u,
            );
            if let Ok(base) = common::BasePacket::create_from_subpacket(&pkt, true, false) {
                client.send_bytes(base.to_bytes()).await;
            }
        }
    }

    async fn apply_join_gc(&self, player_id: u32, gc: u8) {
        let Some(handle) = self.registry.get(player_id).await else {
            return;
        };
        if !crate::actor::gc::is_valid_gc(gc) {
            tracing::debug!(player = player_id, gc, "JoinGC: invalid gc id");
            return;
        }
        // Flip CharaState and, if the per-GC rank is still the
        // "never-promoted" sentinel, leave it at `RANK_RECRUIT`
        // (127) — matches retail, which shows a newly-joined
        // character as Recruit until their first promotion.
        {
            let mut c = handle.character.write().await;
            c.chara.gc_current = gc;
            let rank_ref = match gc {
                crate::actor::gc::GC_MAELSTROM => &mut c.chara.gc_rank_limsa,
                crate::actor::gc::GC_TWIN_ADDER => &mut c.chara.gc_rank_gridania,
                crate::actor::gc::GC_IMMORTAL_FLAMES => &mut c.chara.gc_rank_uldah,
                _ => return,
            };
            if *rank_ref == 0 {
                *rank_ref = crate::actor::gc::RANK_RECRUIT;
            }
        }
        if let Err(e) = self.db.set_gc_current(player_id, gc).await {
            tracing::warn!(player = player_id, gc, err = %e, "JoinGC: DB set_gc_current failed");
        }
        // Persist the rank too — if we bumped it from 0 to 127 the
        // DB currently has 0; if it was already set we're writing
        // back the same value.
        let rank = {
            let c = handle.character.read().await;
            match gc {
                crate::actor::gc::GC_MAELSTROM => c.chara.gc_rank_limsa,
                crate::actor::gc::GC_TWIN_ADDER => c.chara.gc_rank_gridania,
                crate::actor::gc::GC_IMMORTAL_FLAMES => c.chara.gc_rank_uldah,
                _ => 0,
            }
        };
        if let Err(e) = self.db.set_gc_rank(player_id, gc, rank).await {
            tracing::warn!(player = player_id, gc, err = %e, "JoinGC: DB set_gc_rank failed");
        }
        self.emit_grand_company_packet(&handle).await;
        tracing::info!(player = player_id, gc, rank, "JoinGC applied");
    }

    async fn apply_set_gc_rank(&self, player_id: u32, gc: u8, rank: u8) {
        let Some(handle) = self.registry.get(player_id).await else {
            return;
        };
        if !crate::actor::gc::is_valid_gc(gc) {
            tracing::debug!(player = player_id, gc, "SetGCRank: invalid gc id");
            return;
        }
        {
            let mut c = handle.character.write().await;
            match gc {
                crate::actor::gc::GC_MAELSTROM => c.chara.gc_rank_limsa = rank,
                crate::actor::gc::GC_TWIN_ADDER => c.chara.gc_rank_gridania = rank,
                crate::actor::gc::GC_IMMORTAL_FLAMES => c.chara.gc_rank_uldah = rank,
                _ => return,
            }
        }
        if let Err(e) = self.db.set_gc_rank(player_id, gc, rank).await {
            tracing::warn!(player = player_id, gc, rank, err = %e, "SetGCRank: DB persist failed");
        }
        self.emit_grand_company_packet(&handle).await;
        tracing::info!(player = player_id, gc, rank, "SetGCRank applied");
    }

    /// Shared production-drain plumbing for every script-driven
    /// `director:*` mutation that needs to fan a `DirectorEvent`
    /// through `dispatch_director_event` to the player members.
    /// `op_name` is purely for tracing; `mutate` runs under a single
    /// zone write lock with a fresh `DirectorOutbox` and the
    /// guildleve director it's targeting.
    ///
    /// Quietly no-ops on:
    /// * unknown zone (already torn down),
    /// * unknown / non-guildleve director (id mismatch),
    /// * a `mutate` that doesn't push anything (e.g. an already-ended
    ///   director — `end_guildleve`'s internal idempotency).
    async fn apply_director_outbox_op<F>(
        &self,
        director_actor_id: u32,
        op_name: &'static str,
        mutate: F,
    ) where
        F: FnOnce(&mut crate::director::GuildleveDirector, &mut crate::director::DirectorOutbox),
    {
        let zone_id = (director_actor_id >> 19) & 0x1FF;
        let Some(zone_arc) = self.world.zone(zone_id).await else {
            tracing::debug!(
                director = director_actor_id,
                zone = zone_id,
                op = op_name,
                "director-outbox op skipped — zone not loaded",
            );
            return;
        };
        // Drive the director under a single write lock so the
        // outbox drain reflects exactly what `mutate` pushed (vs.
        // racing a second mutator on a different actor).
        let (events, player_members) = {
            let mut zone = zone_arc.write().await;
            let Some(gld) = zone.core.guildleve_director_mut(director_actor_id) else {
                tracing::debug!(
                    director = director_actor_id,
                    zone = zone_id,
                    op = op_name,
                    "director-outbox op skipped — guildleve director not on zone",
                );
                return;
            };
            // Snapshot the roster BEFORE running `mutate` —
            // operations like `abandon_guildleve` internally call
            // `Director::end` which clears `player_members` as part
            // of the teardown event chain. Reading after the mutate
            // would lose the recipients we need to fan packets to.
            let roster: Vec<u32> = gld.base.player_members().collect();
            let mut outbox = crate::director::DirectorOutbox::new();
            mutate(gld, &mut outbox);
            (outbox.drain(), roster)
        };

        // Drain — fires whatever packets the matching dispatcher arm
        // sends (victory music / start music / aim updates / etc).
        // Pass the live DB handle so seal-accrual on `GuildleveEnded`
        // can persist.
        for e in events {
            crate::director::dispatch_director_event(
                &e,
                &player_members,
                &self.registry,
                &self.world,
                Some(&self.db),
            )
            .await;
        }
        tracing::debug!(
            director = director_actor_id,
            zone = zone_id,
            op = op_name,
            "director-outbox op applied",
        );
    }

    /// `director:EndGuildleve(was_completed)` — closes the loop on
    /// the leve-completion seal accrual. Wraps the shared
    /// outbox-op helper with the unix-time + was_completed args
    /// `end_guildleve` needs.
    async fn apply_end_guildleve(&self, director_actor_id: u32, was_completed: bool) {
        let now_unix_s = common::utils::unix_timestamp() as u32;
        self.apply_director_outbox_op(director_actor_id, "EndGuildleve", |gld, ob| {
            gld.end_guildleve(now_unix_s, was_completed, ob);
        })
        .await;
    }

    /// `director:StartGuildleve()` — fires the leve start packet
    /// bundle (music + start text + time-limit text) plus the
    /// `GuildleveSyncAll` follow-up the helper already pushes.
    async fn apply_start_guildleve(&self, director_actor_id: u32) {
        let now_unix_s = common::utils::unix_timestamp() as u32;
        self.apply_director_outbox_op(director_actor_id, "StartGuildleve", |gld, ob| {
            gld.start_guildleve(now_unix_s, ob);
        })
        .await;
    }

    /// `director:AbandonGuildleve()` — fires the abandon-message
    /// game-message, then runs the same teardown chain as
    /// `EndGuildleve(false)` (no seal accrual on the dispatcher side
    /// because `was_completed` is false).
    async fn apply_abandon_guildleve(&self, director_actor_id: u32) {
        let now_unix_s = common::utils::unix_timestamp() as u32;
        self.apply_director_outbox_op(director_actor_id, "AbandonGuildleve", |gld, ob| {
            gld.abandon_guildleve(now_unix_s, ob);
        })
        .await;
    }

    /// `director:StartDirector(spawn_immediate)` — spawn the
    /// director's `main(thisDirector)` coroutine and run it until the
    /// first `wait()` yield. Any `director:StartGuildleve()` /
    /// `UpdateMarkers(...)` / etc. calls that happen in the initial
    /// slice (before the first `wait`) drain through the normal
    /// `apply_runtime_lua_commands` pipeline; subsequent slices run
    /// via the ticker's `lua.tick()` call on each game-loop frame.
    ///
    /// Quietly no-ops when:
    /// * no `LuaEngine` is wired (headless/test harness),
    /// * `directors/<name>.lua` isn't on disk,
    /// * the script has no `main` global (e.g. `AfterQuestWarpDirector`
    ///   only has `onEventStarted`; that path goes through the event
    ///   dispatcher instead).
    async fn apply_start_director_main(
        &self,
        director_actor_id: u32,
        class_path: String,
        director_name: String,
        spawn_immediate: bool,
    ) {
        let Some(lua) = self.lua.as_ref() else {
            tracing::debug!(
                director = director_actor_id,
                "StartDirectorMain skipped — no LuaEngine wired",
            );
            return;
        };
        // Class names resolve to scripts/lua/directors/<name>.lua via
        // the resolver; LuaDirectorHandle's `class_path` is
        // `/Director/<Name>` so the final segment is the script name.
        let script_name = director_name.clone();
        let script_path = lua.resolver().director(&script_name);
        if !script_path.exists() {
            tracing::debug!(
                director = director_actor_id,
                script = %script_path.display(),
                "StartDirectorMain skipped — script not on disk",
            );
            return;
        }

        let handle = crate::lua::userdata::LuaDirectorHandle {
            name: director_name.clone(),
            actor_id: director_actor_id,
            class_path: class_path.clone(),
            // Engine re-points to the freshly-installed queue; any
            // value here is fine, the script's `push` path will use
            // the right one.
            queue: crate::lua::command::CommandQueue::new(),
        };

        let lua_clone = lua.clone();
        let result = tokio::task::spawn_blocking(move || {
            lua_clone.spawn_director_main(&script_path, handle)
        })
        .await;
        let partial = match result {
            Ok(p) => p,
            Err(join_err) => {
                tracing::warn!(
                    director = director_actor_id,
                    error = %join_err,
                    "StartDirectorMain dispatch panicked",
                );
                return;
            }
        };
        if let Some(e) = partial.error {
            tracing::debug!(
                director = director_actor_id,
                error = %e,
                "StartDirectorMain initial resume errored",
            );
        }
        // Drain whatever the initial slice pushed (typically one or
        // two commands if `main` starts with `wait(3)` — nothing, in
        // that case — or an `EndGuildleve` if main immediately
        // completes).
        if !partial.commands.is_empty() {
            crate::runtime::quest_apply::apply_runtime_lua_commands(
                partial.commands,
                &self.registry,
                &self.db,
                &self.world,
                Some(lua),
            )
            .await;
        }
        tracing::info!(
            director = director_actor_id,
            class = %class_path,
            spawn_immediate,
            "StartDirectorMain applied — main coroutine spawned",
        );
    }

    /// `player:SetHomePoint(aetheryteId)` — `AetheryteChild.lua` calls
    /// this after the player picks a new home aetheryte. Mirrors C#
    /// `Player.SetHomePoint` (`Map Server/Actors/Chara/Player/Player.cs:1336`):
    /// updates the in-memory state and persists via
    /// `Database::save_player_home_points`. Mirrors into CharaState so
    /// `runtime::dispatcher::apply_home_point_revive` reads the new
    /// value without a DB round-trip.
    async fn apply_set_home_point(&self, player_id: u32, homepoint: u32) {
        if let Some(handle) = self.registry.get(player_id).await {
            let homepoint_inn = {
                let mut c = handle.character.write().await;
                c.chara.homepoint = homepoint;
                c.chara.homepoint_inn
            };
            if let Err(e) = self
                .db
                .save_player_home_points(player_id, homepoint, homepoint_inn)
                .await
            {
                tracing::warn!(
                    player = player_id,
                    homepoint,
                    err = %e,
                    "SetHomePoint: DB persist failed",
                );
                return;
            }
        } else {
            // Offline persist path — Lua can't realistically hit this
            // (the player runs the script), but keep the DB write as a
            // safety net so a stray `SetHomePoint` from a non-player
            // hook doesn't silently drop. Inn id stays at whatever the
            // DB already holds.
            let inn = match self.db.load_player_character(player_id).await {
                Ok(Some(p)) => p.homepoint_inn,
                _ => 0,
            };
            if let Err(e) = self
                .db
                .save_player_home_points(player_id, homepoint, inn)
                .await
            {
                tracing::warn!(
                    player = player_id,
                    homepoint,
                    err = %e,
                    "SetHomePoint (offline): DB persist failed",
                );
                return;
            }
        }
        tracing::info!(player = player_id, homepoint, "SetHomePoint applied");
    }

    /// `player:SetHomePointInn(innId)` — companion to `SetHomePoint`
    /// that mutates only the inn-room id. Reads the current homepoint
    /// from the live Character, writes the inn id, then persists both
    /// (the DB API is one-shot for both fields). 6 call sites in
    /// dft + populace inn-keeper scripts.
    async fn apply_set_home_point_inn(&self, player_id: u32, inn_id: u8) {
        if let Some(handle) = self.registry.get(player_id).await {
            let homepoint = {
                let mut c = handle.character.write().await;
                c.chara.homepoint_inn = inn_id;
                c.chara.homepoint
            };
            if let Err(e) = self
                .db
                .save_player_home_points(player_id, homepoint, inn_id)
                .await
            {
                tracing::warn!(
                    player = player_id,
                    inn_id,
                    err = %e,
                    "SetHomePointInn: DB persist failed",
                );
                return;
            }
        } else {
            // Offline-fallback safety net — Lua callers can't realistically
            // hit this, but keep the persist path consistent with
            // `apply_set_home_point`'s offline branch.
            let homepoint = match self.db.load_player_character(player_id).await {
                Ok(Some(p)) => p.homepoint,
                _ => 0,
            };
            if let Err(e) = self
                .db
                .save_player_home_points(player_id, homepoint, inn_id)
                .await
            {
                tracing::warn!(
                    player = player_id,
                    inn_id,
                    err = %e,
                    "SetHomePointInn (offline): DB persist failed",
                );
                return;
            }
        }
        tracing::info!(player = player_id, inn_id, "SetHomePointInn applied");
    }

    /// `player:SetNpcLs(id, state)` / `player:AddNpcLs(id)` /
    /// `quest:NewNpcLsMsg(from)` apply path. State decode mirrors
    /// the C# `Player.SetNpcLs` switch (Map Server/Actors/Chara/Player/Player.cs):
    ///
    ///  0 = NPCLS_GONE     → (false, false) — not in player's collection
    ///  1 = NPCLS_INACTIVE → (false, true)  — owned, no glow
    ///  2 = NPCLS_ACTIVE   → (true, false)  — owned, calling (post-read)
    ///  3 = NPCLS_ALERT    → (true, true)   — owned, glow + calling
    ///
    /// 1.x's `npc_ls_id` is 1-based on the wire (1..=40); the DB row
    /// is 0-based, so we decrement before persisting. The matching
    /// `playerWork.npcLinkshellChat{Calling,Extra}[N]` SetActorProperty
    /// fan-out is deferred — those paths aren't in the property
    /// registry yet, so the client won't see the icon flip until
    /// they're plumbed through.
    async fn apply_player_set_npc_ls(&self, player_id: u32, npc_ls_id: u32, state: u8) {
        if !(1..=40).contains(&npc_ls_id) {
            tracing::debug!(
                player = player_id,
                npc_ls_id,
                state,
                "SetNpcLs: id out of valid range (1..=40)",
            );
            return;
        }
        let (is_calling, is_extra) = match state {
            0 => (false, false), // NPCLS_GONE
            1 => (false, true),  // NPCLS_INACTIVE
            2 => (true, false),  // NPCLS_ACTIVE
            3 => (true, true),   // NPCLS_ALERT
            _ => {
                tracing::debug!(
                    player = player_id,
                    npc_ls_id,
                    state,
                    "SetNpcLs: unknown state code",
                );
                return;
            }
        };
        let zero_based = npc_ls_id - 1;

        // C# `Player.AddNpcLs` first-add gate: if the player didn't
        // own this NpcLs (both flags false OR the row didn't exist),
        // fire the canonical "<NpcLs> linkpearl obtained." toast on
        // the GONE → owned transition. We probe BEFORE the upsert so
        // a re-add of an already-owned LS doesn't double-fire.
        let was_owned: bool = match self.db.load_npc_ls_state(player_id, zero_based).await {
            Ok(Some((c, e))) => c || e, // any flag true = owned
            Ok(None) | Err(_) => false, // missing row OR error → treat as not-owned
        };

        if let Err(e) = self
            .db
            .save_npc_ls(player_id, zero_based, is_calling, is_extra)
            .await
        {
            tracing::warn!(
                player = player_id,
                npc_ls_id,
                state,
                err = %e,
                "SetNpcLs: DB persist failed",
            );
            return;
        }
        tracing::debug!(
            player = player_id,
            npc_ls_id,
            state,
            is_calling,
            is_extra,
            "SetNpcLs persisted",
        );

        // First-add toast — fire only when transitioning from "not
        // owned" to "owned". State 0 (GONE) is not an "ownership"
        // state itself, so we also gate on the new state being
        // anything-but-GONE (otherwise SetNpcLs(id, GONE) on a
        // never-owned LS would fire spuriously).
        let now_owned = is_calling || is_extra;
        if !was_owned && now_owned {
            if let Some(handle) = self.registry.get(player_id).await {
                if let Some(client) = self.world.client(handle.session_id).await {
                    let pkt = crate::packets::send::misc::build_text_sheet_no_source_auto(
                        handle.actor_id,
                        crate::packets::send::misc::WORLD_MASTER_ACTOR_ID,
                        /* text_id */ 25118,
                        crate::packets::send::misc::MESSAGE_TYPE_SYSTEM,
                        &[common::luaparam::LuaParam::UInt32(npc_ls_id)],
                        /* prefer_alt */ false,
                    );
                    client.send_bytes(pkt.to_bytes()).await;
                    tracing::debug!(
                        player = player_id,
                        npc_ls_id,
                        "SetNpcLs first-add: 25118 'linkpearl obtained' toast fired",
                    );
                }
            }
        }
    }

    /// `player:EquipAbility(classId, commandId, hotbarSlot, _)` —
    /// persist a single hotbar slot to DB. C#
    /// `Player.EquipAbility` decrements `hotbarSlot` by `commandBorder`
    /// (32) before saving the 0-based DB row; we mirror that math
    /// here. The in-memory hotbar snapshot + the
    /// `charaWork.command[N]` SetActorProperty fan-out are deferred
    /// — the next character load picks the row up.
    async fn apply_equip_ability(
        &self,
        player_id: u32,
        class_id: u8,
        command_id: u32,
        hotbar_slot: u16,
    ) {
        const COMMAND_BORDER: u16 = 0x20;
        let zero_based_slot = hotbar_slot.saturating_sub(COMMAND_BORDER);
        if let Err(e) = self
            .db
            .equip_ability(player_id, class_id, zero_based_slot, command_id, 0)
            .await
        {
            tracing::warn!(
                player = player_id, class_id, command_id, hotbar_slot,
                err = %e,
                "EquipAbility: DB persist failed",
            );
            return;
        }
        // Mirror the in-memory CharaState hotbar so subsequent
        // PlayerSnapshot builds (and FindFirstCommandSlotById /
        // charaWork.command[N] reads) see the new equip
        // immediately, not just after next character load. C# wire
        // mask: `0xA0F00000 | command_id`.
        if let Some(handle) = self.registry.get(player_id).await {
            let mut c = handle.character.write().await;
            let masked = command_id | 0xA0F00000;
            // Replace existing entry at this slot, or push.
            if let Some(entry) = c
                .chara
                .hotbar
                .iter_mut()
                .find(|e| e.hotbar_slot == zero_based_slot)
            {
                entry.command_id = masked;
                entry.recast_time = 0;
            } else {
                c.chara.hotbar.push(crate::gamedata::HotbarEntry {
                    hotbar_slot: zero_based_slot,
                    command_id: masked,
                    recast_time: 0,
                });
            }
        }
        tracing::info!(
            player = player_id,
            class_id,
            command_id,
            hotbar_slot,
            "EquipAbility persisted + snapshot mirror",
        );

        // Fan out the canonical "<command> equipped" toast.
        // Mirror C# `Player.EquipAbility`'s
        // `SendGameMessage(WorldMaster, 30603, 0x20, 0, commandId)`.
        if let Some(handle) = self.registry.get(player_id).await {
            if let Some(client) = self.world.client(handle.session_id).await {
                let pkt = crate::packets::send::misc::build_text_sheet_no_source_auto(
                    handle.actor_id,
                    crate::packets::send::misc::WORLD_MASTER_ACTOR_ID,
                    /* text_id */ 30603,
                    crate::packets::send::misc::MESSAGE_TYPE_SYSTEM,
                    &[
                        common::luaparam::LuaParam::UInt32(0),
                        common::luaparam::LuaParam::UInt32(command_id),
                    ],
                    /* prefer_alt */ false,
                );
                client.send_bytes(pkt.to_bytes()).await;
            }
        }
    }

    /// `player:UnequipAbility(slot)` — DELETE the hotbar row for the
    /// player's current class + slot. C# decrements `slot` by 1 (its
    /// scripts pass 1-indexed slots) plus `commandBorder`; the
    /// scripts that hit this binding (EquipAbilityCommand.lua) already
    /// pre-massage the slot index before calling, so we accept a raw
    /// 0-based slot.
    async fn apply_unequip_ability(&self, player_id: u32, class_id: u8, hotbar_slot: u16) {
        // Capture the soon-to-be-dropped command_id from the in-memory
        // hotbar snapshot — needed to build the 30604 toast below.
        // C# wire format strips the `0xA0F00000` mask via XOR; we do
        // the same here so the LuaParam carries the raw command id.
        let unmasked_command_id: u32 = if let Some(handle) = self.registry.get(player_id).await {
            let c = handle.character.read().await;
            c.chara
                .hotbar
                .iter()
                .find(|e| e.hotbar_slot == hotbar_slot)
                .map(|e| e.command_id ^ 0xA0F00000)
                .unwrap_or(0)
        } else {
            0
        };

        if let Err(e) = self
            .db
            .unequip_ability(player_id, class_id, hotbar_slot)
            .await
        {
            tracing::warn!(
                player = player_id, class_id, hotbar_slot,
                err = %e,
                "UnequipAbility: DB persist failed",
            );
            return;
        }
        // Mirror the snapshot hotbar drop + capture handle for the
        // toast fan-out below.
        let session_id = if let Some(handle) = self.registry.get(player_id).await {
            let mut c = handle.character.write().await;
            c.chara.hotbar.retain(|e| e.hotbar_slot != hotbar_slot);
            handle.session_id
        } else {
            0
        };
        tracing::info!(
            player = player_id,
            class_id,
            hotbar_slot,
            "UnequipAbility persisted + snapshot mirror",
        );

        // Fan out the canonical "<command> unequipped" toast — only
        // when there was a command in the slot (matches C#'s
        // `if (printMessage && commandId != 0)` gate).
        if unmasked_command_id != 0 && session_id != 0 {
            if let Some(client) = self.world.client(session_id).await {
                let pkt = crate::packets::send::misc::build_text_sheet_no_source_auto(
                    player_id,
                    crate::packets::send::misc::WORLD_MASTER_ACTOR_ID,
                    /* text_id */ 30604,
                    crate::packets::send::misc::MESSAGE_TYPE_SYSTEM,
                    &[
                        common::luaparam::LuaParam::UInt32(0),
                        common::luaparam::LuaParam::UInt32(unmasked_command_id),
                    ],
                    /* prefer_alt */ false,
                );
                client.send_bytes(pkt.to_bytes()).await;
            }
        }
    }

    /// `player:SwapAbilities(slot1, slot2)` — exchange two hotbar
    /// slots. Round-trips through `db.load_hotbar` to read the
    /// current commands then re-writes both rows.
    async fn apply_swap_abilities(
        &self,
        player_id: u32,
        class_id: u8,
        hotbar_slot_1: u16,
        hotbar_slot_2: u16,
    ) {
        const COMMAND_BORDER: u16 = 0x20;
        let zero_1 = hotbar_slot_1.saturating_sub(COMMAND_BORDER);
        let zero_2 = hotbar_slot_2.saturating_sub(COMMAND_BORDER);
        let entries = match self.db.load_hotbar(player_id, class_id).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    player = player_id, class_id,
                    err = %e,
                    "SwapAbilities: DB load failed",
                );
                return;
            }
        };
        let cmd_1 = entries
            .iter()
            .find(|e| e.hotbar_slot == zero_1)
            .map(|e| (e.command_id, e.recast_time))
            .unwrap_or((0, 0));
        let cmd_2 = entries
            .iter()
            .find(|e| e.hotbar_slot == zero_2)
            .map(|e| (e.command_id, e.recast_time))
            .unwrap_or((0, 0));
        if let Err(e) = self
            .db
            .equip_ability(player_id, class_id, zero_1, cmd_2.0, cmd_2.1)
            .await
        {
            tracing::warn!(
                player = player_id, class_id, slot = zero_1,
                err = %e,
                "SwapAbilities: DB write slot1 failed",
            );
            return;
        }
        if let Err(e) = self
            .db
            .equip_ability(player_id, class_id, zero_2, cmd_1.0, cmd_1.1)
            .await
        {
            tracing::warn!(
                player = player_id, class_id, slot = zero_2,
                err = %e,
                "SwapAbilities: DB write slot2 failed",
            );
            return;
        }
        // Mirror the snapshot hotbar swap so subsequent reads
        // (FindFirstCommandSlotById, charaWork.command[N]) see
        // the new slot mapping immediately.
        if let Some(handle) = self.registry.get(player_id).await {
            let mut c = handle.character.write().await;
            for entry in c.chara.hotbar.iter_mut() {
                if entry.hotbar_slot == zero_1 {
                    entry.command_id = cmd_2.0;
                    entry.recast_time = cmd_2.1;
                } else if entry.hotbar_slot == zero_2 {
                    entry.command_id = cmd_1.0;
                    entry.recast_time = cmd_1.1;
                }
            }
        }
        tracing::info!(
            player = player_id,
            class_id,
            slot_1 = hotbar_slot_1,
            slot_2 = hotbar_slot_2,
            "SwapAbilities persisted (both slots swapped) + snapshot mirror",
        );
    }

    /// `player:EquipAbilityInFirstOpenSlot(classId, commandId)` —
    /// composite: find the first empty slot via
    /// `db.find_first_command_slot`, then `equip_ability` there.
    async fn apply_equip_ability_in_first_open_slot(
        &self,
        player_id: u32,
        class_id: u8,
        command_id: u32,
    ) {
        let slot = match self.db.find_first_command_slot(player_id, class_id).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    player = player_id, class_id,
                    err = %e,
                    "EquipAbilityInFirstOpenSlot: find_first_command_slot failed",
                );
                return;
            }
        };
        // The hotbar holds 30 commands; reject if the helper returned
        // an out-of-range index (means the bar is full).
        if slot >= 30 {
            tracing::debug!(
                player = player_id,
                class_id,
                command_id,
                "EquipAbilityInFirstOpenSlot: hotbar full",
            );
            return;
        }
        if let Err(e) = self
            .db
            .equip_ability(player_id, class_id, slot, command_id, 0)
            .await
        {
            tracing::warn!(
                player = player_id, class_id, command_id, slot,
                err = %e,
                "EquipAbilityInFirstOpenSlot: DB persist failed",
            );
            return;
        }
        // Mirror the snapshot hotbar push.
        if let Some(handle) = self.registry.get(player_id).await {
            let mut c = handle.character.write().await;
            let masked = command_id | 0xA0F00000;
            if let Some(entry) = c.chara.hotbar.iter_mut().find(|e| e.hotbar_slot == slot) {
                entry.command_id = masked;
                entry.recast_time = 0;
            } else {
                c.chara.hotbar.push(crate::gamedata::HotbarEntry {
                    hotbar_slot: slot,
                    command_id: masked,
                    recast_time: 0,
                });
            }
        }
        tracing::info!(
            player = player_id,
            class_id,
            command_id,
            slot,
            "EquipAbilityInFirstOpenSlot persisted + snapshot mirror",
        );

        // Sibling auto-fire of the EquipAbility 30603 toast — same
        // wire shape, same C# precedent (Player.EquipAbility passes
        // `printMessage = true` from EquipAbilityInFirstOpenSlot).
        if let Some(handle) = self.registry.get(player_id).await {
            if let Some(client) = self.world.client(handle.session_id).await {
                let pkt = crate::packets::send::misc::build_text_sheet_no_source_auto(
                    handle.actor_id,
                    crate::packets::send::misc::WORLD_MASTER_ACTOR_ID,
                    /* text_id */ 30603,
                    crate::packets::send::misc::MESSAGE_TYPE_SYSTEM,
                    &[
                        common::luaparam::LuaParam::UInt32(0),
                        common::luaparam::LuaParam::UInt32(command_id),
                    ],
                    /* prefer_alt */ false,
                );
                client.send_bytes(pkt.to_bytes()).await;
            }
        }
    }

    /// `player:SavePlayTime()` — persist the player's play_time so
    /// the `player.lua::onLogin` first-login marker
    /// (`GetPlayTime(false) == 0` → "new player") flips after the
    /// first run. The accumulating last-play-time-update +
    /// elapsed-seconds math lives on the `Player` wrapper
    /// (`actor::player::Player::get_play_time(true)`); the
    /// registry only carries `Character` so we can't reach
    /// `player.play_time` directly from here. Round-trip through
    /// the DB: load current value, bump by 1 second (so the
    /// new-player check fails), persist. Real elapsed-time
    /// accumulation lands when we plumb `PlayerState` access
    /// through the registry.
    async fn apply_save_play_time(&self, player_id: u32) {
        let current = match self.db.load_player_character(player_id).await {
            Ok(Some(p)) => p.play_time,
            _ => 0,
        };
        let new_value = current.saturating_add(1).max(1);
        if let Err(e) = self
            .db
            .save_player_play_time(player_id, new_value)
            .await
        {
            tracing::warn!(
                player = player_id, play_time = new_value,
                err = %e,
                "SavePlayTime: DB persist failed",
            );
            return;
        }
        tracing::debug!(
            player = player_id,
            play_time = new_value,
            "SavePlayTime persisted (registry-side accumulation deferred)",
        );
    }

    /// `player:SendAppearance()` / `actor:SendAppearance()` —
    /// rebroadcast 0x00D6 SetActorAppearancePacket from the actor's
    /// current `chara.model_id` + `chara.appearance_ids` (28-slot
    /// equipment table). Same fan-out shape as DoEmote: send to
    /// self if player, broadcast to in-zone neighbours so all
    /// witnesses see the new gear.
    async fn apply_send_appearance(&self, actor_id: u32) {
        let Some(handle) = self.registry.get(actor_id).await else {
            tracing::debug!(actor = actor_id, "SendAppearance: actor not in registry");
            return;
        };
        let (model_id, appearance_ids) = {
            let c = handle.character.read().await;
            (c.chara.model_id, c.chara.appearance_ids)
        };
        let bytes = crate::packets::send::actor::build_set_actor_appearance(
            actor_id,
            model_id,
            &appearance_ids,
        )
        .to_bytes();
        crate::runtime::dispatcher::send_to_self_if_player(
            &self.registry,
            &self.world,
            actor_id,
            bytes.clone(),
        )
        .await;
        crate::runtime::dispatcher::broadcast_to_neighbours(
            &self.world,
            &self.registry,
            actor_id,
            bytes,
        )
        .await;
        tracing::info!(
            actor = actor_id,
            model_id,
            "SendAppearance applied + 0x00D6 broadcast",
        );
    }

    /// `player:SetCurrentJob(jobId)` — flips the player's
    /// `current_job` field, broadcasts `SetCurrentJobPacket` (0x01A4)
    /// to the player + neighbours so the nameplate flips, and
    /// re-loads the hotbar from DB for the new job. C#
    /// `Player.SetCurrentJob` (Map Server/Actors/Chara/Player/Player.cs:1300).
    async fn apply_set_current_job(&self, player_id: u32, job_id: u8) {
        let Some(handle) = self.registry.get(player_id).await else {
            tracing::debug!(player = player_id, job_id, "SetCurrentJob: actor missing");
            return;
        };
        let actor_id = handle.actor_id;
        {
            let mut c = handle.character.write().await;
            c.chara.current_job = job_id as u16;
        }
        let bytes = crate::packets::send::player::build_set_current_job(actor_id, job_id as u32)
            .to_bytes();
        crate::runtime::dispatcher::send_to_self_if_player(
            &self.registry,
            &self.world,
            actor_id,
            bytes.clone(),
        )
        .await;
        crate::runtime::dispatcher::broadcast_to_neighbours(
            &self.world,
            &self.registry,
            actor_id,
            bytes,
        )
        .await;
        tracing::info!(
            player = player_id,
            job_id,
            "SetCurrentJob applied + 0x01A4 broadcast (hotbar reload deferred to next character load)",
        );
    }

    /// `player:SetHP/SetMaxHP/SetMP/SetMaxMP/SetTP(value)` — direct
    /// pool setter used by GM `setmaxhp` / `setmaxmp` commands and
    /// by quest scripts that need to override player pools without
    /// running the recalc-stats pipeline.
    ///
    /// For SetMaxHP / SetMaxMP we ALSO heal current HP / MP up to
    /// the new max if the player was at-or-below the old max — this
    /// matches Meteor's `Player.SetMaxHP` "set max + heal to full"
    /// behaviour that the GM commands script around.
    ///
    /// Broadcasts a single `charaWork/stateAtQuicklyForAll` bundle
    /// (chara + player variants) so the owner self-HUD and neighbour
    /// nameplate HP bars update immediately. Works on any actor (not
    /// player-only — bnpc HP setters round-trip the same path).
    async fn apply_set_pool(
        &self,
        actor_id: u32,
        kind: crate::lua::command::SetPoolKind,
        value: i32,
    ) {
        use crate::lua::command::SetPoolKind;
        let Some(handle) = self.registry.get(actor_id).await else {
            tracing::debug!(actor = actor_id, "SetPool: actor not in registry");
            return;
        };
        let value_i16 = value.clamp(0, i16::MAX as i32) as i16;
        let value_u16 = value.clamp(0, u16::MAX as i32) as u16;
        let post_pools = {
            let mut c = handle.character.write().await;
            match kind {
                SetPoolKind::Hp => {
                    c.chara.hp = value_i16.min(c.chara.max_hp);
                }
                SetPoolKind::MaxHp => {
                    let old_max = c.chara.max_hp;
                    c.chara.max_hp = value_i16;
                    // Heal-to-full when the player was at/under the
                    // old cap — Meteor's setmaxhp behaviour.
                    if c.chara.hp >= old_max || c.chara.hp == 0 {
                        c.chara.hp = value_i16;
                    } else {
                        c.chara.hp = c.chara.hp.min(value_i16);
                    }
                }
                SetPoolKind::Mp => {
                    c.chara.mp = value_i16.min(c.chara.max_mp);
                }
                SetPoolKind::MaxMp => {
                    let old_max = c.chara.max_mp;
                    c.chara.max_mp = value_i16;
                    if c.chara.mp >= old_max || c.chara.mp == 0 {
                        c.chara.mp = value_i16;
                    } else {
                        c.chara.mp = c.chara.mp.min(value_i16);
                    }
                }
                SetPoolKind::Tp => {
                    c.chara.tp = value_u16;
                }
            }
            (
                c.chara.hp.max(0) as u16,
                c.chara.max_hp.max(0) as u16,
                c.chara.mp.max(0) as u16,
                c.chara.max_mp.max(0) as u16,
                c.chara.tp,
            )
        };
        let (hp, hp_max, mp, mp_max, tp) = post_pools;
        let mut subs =
            crate::packets::send::actor::build_chara_state_at_quickly_for_all(
                actor_id, hp, hp_max, mp, mp_max, tp,
            );
        // Players also get the player-variant bundle (extra fields:
        // class slot + main-skill level). Bnpcs don't need it; the
        // chara variant alone updates their nameplate HP bar.
        if handle.is_player() {
            let (class_slot, main_skill_level) = {
                let c = handle.character.read().await;
                (c.chara.class.max(0) as u8, c.chara.level.max(1) as u16)
            };
            subs.extend(
                crate::packets::send::actor::build_player_state_at_quickly_for_all(
                    actor_id,
                    hp,
                    hp_max,
                    class_slot,
                    main_skill_level,
                ),
            );
        }
        for sub in subs {
            let bytes = sub.to_bytes();
            crate::runtime::dispatcher::send_to_self_if_player(
                &self.registry,
                &self.world,
                actor_id,
                bytes.clone(),
            )
            .await;
            crate::runtime::dispatcher::broadcast_to_neighbours(
                &self.world,
                &self.registry,
                actor_id,
                bytes,
            )
            .await;
        }
        tracing::info!(
            actor = actor_id,
            ?kind,
            value,
            hp, hp_max, mp, mp_max, tp,
            "SetPool applied + broadcast"
        );
    }

    /// Same-zone teleport. Called by both `WorldManager:WarpToPosition`
    /// and `WorldManager:DoPlayerMoveInZone` (the latter just supplies
    /// its own spawn_type). Mirrors the same-zone branch of the GM
    /// `!warp` command (`command_processor::handle_warp`):
    ///
    ///   1. Mutate `c.base.position_x/y/z/rotation` so subsequent
    ///      packets read the new pose.
    ///   2. Refresh `session.destination_x/y/z/rot/spawn_type` so any
    ///      follow-up zone-in bundle starts from the warped location.
    ///   3. Emit `SetActorPosition` to the owning client so the player
    ///      visibly snaps to the target — `is_zoning_player=false`
    ///      because we're not crossing the loading-screen boundary.
    ///
    /// Cross-zone warps need the full `DoZoneChange` flow (loading
    /// screen + zone-change packets), which isn't wired yet — see
    /// the `WarpToPublicArea` / `WarpToPrivateArea` arms above.
    async fn apply_warp_to_position(
        &self,
        actor_id: u32,
        x: f32,
        y: f32,
        z: f32,
        rotation: f32,
        spawn_type: u8,
    ) {
        let Some(handle) = self.registry.get(actor_id).await else {
            tracing::debug!(actor = actor_id, "WarpToPosition: actor not in registry");
            return;
        };
        let session_id = handle.session_id;
        {
            let mut c = handle.character.write().await;
            c.base.position_x = x;
            c.base.position_y = y;
            c.base.position_z = z;
            c.base.rotation = rotation;
        }
        if let Some(mut session) = self.world.session(session_id).await {
            session.destination_x = x;
            session.destination_y = y;
            session.destination_z = z;
            session.destination_rot = rotation;
            session.destination_spawn_type = spawn_type;
            self.world.upsert_session(session).await;
        }
        if let Some(client) = self.world.client(session_id).await {
            let pkt = crate::packets::send::build_set_actor_position(
                actor_id,
                actor_id as i32,
                x,
                y,
                z,
                rotation,
                spawn_type.into(),
                false,
            );
            client.send_bytes(pkt.to_bytes()).await;
            tracing::info!(
                actor = actor_id,
                x, y, z, rotation, spawn_type,
                "WarpToPosition applied + SetActorPosition emitted"
            );
        } else {
            tracing::debug!(
                actor = actor_id,
                "WarpToPosition: no client handle (offline) — pose updated but no packet sent"
            );
        }
    }

    /// `player:DoEmote(targetActorId, emoteId, messageId)` —
    /// fans out the canonical 0x00E1 ActorDoEmotePacket. Sent to
    /// the actor themself (so they see their own animation) and
    /// broadcast to in-zone neighbours (so witnesses see it). Same
    /// fan-out shape as SetPool / SetActorPosition.
    async fn apply_do_emote(
        &self,
        actor_id: u32,
        target_actor_id: u32,
        emote_id: u32,
        message_id: u32,
    ) {
        if self.registry.get(actor_id).await.is_none() {
            tracing::debug!(actor = actor_id, "DoEmote: actor not in registry");
            return;
        }
        let bytes = crate::packets::send::actor::build_actor_do_emote(
            actor_id,
            emote_id,
            target_actor_id,
            message_id,
        )
        .to_bytes();
        crate::runtime::dispatcher::send_to_self_if_player(
            &self.registry,
            &self.world,
            actor_id,
            bytes.clone(),
        )
        .await;
        crate::runtime::dispatcher::broadcast_to_neighbours(
            &self.world,
            &self.registry,
            actor_id,
            bytes,
        )
        .await;
        tracing::info!(
            actor = actor_id,
            target = target_actor_id,
            emote_id,
            message_id,
            "DoEmote applied + broadcast",
        );
    }

    /// `player:SetSNpc(nickname, actorClassId, classType)` apply
    /// path. Mirrors C# `Player.SetSNpc` (Map Server/Actors/Chara/
    /// Player/Player.cs):
    /// - SNpcNickname = nickname (raw string)
    /// - SNpcSkin = (actorClassId - 1070000) cast to u8
    /// - SNpcPersonality = `classType` (we skip C#'s race-index
    ///   switch derivation since the cinematic doesn't expose the
    ///   intermediate value and the script callers pass the
    ///   already-resolved personality directly)
    /// SNpcCoordinate is preserved (SetSNpc doesn't write it).
    async fn apply_set_snpc(
        &self,
        player_id: u32,
        nickname: String,
        actor_class_id: u32,
        personality: u8,
    ) {
        let Some(handle) = self.registry.get(player_id).await else {
            tracing::debug!(player = player_id, "SetSNpc: actor missing");
            return;
        };
        // C# `(byte)(actorClassId - 1070000)` — actorClassId 0 case
        // would underflow; clamp to 0.
        let skin = actor_class_id.saturating_sub(1_070_000) as u8;
        let coordinate = {
            let mut c = handle.character.write().await;
            c.chara.snpc_nickname = nickname.clone();
            c.chara.snpc_skin = skin;
            c.chara.snpc_personality = personality;
            c.chara.snpc_coordinate
        };
        if let Err(e) = self
            .db
            .save_snpc(player_id, nickname.clone(), skin, personality, coordinate)
            .await
        {
            tracing::warn!(
                player = player_id,
                actor_class_id, personality, err = %e,
                "SetSNpc: DB persist failed",
            );
            return;
        }
        tracing::info!(
            player = player_id,
            actor_class_id,
            skin,
            personality,
            "SetSNpc applied",
        );
    }

    /// `player:DoClassChange(classId)` apply — minimum-viable port
    /// of C# `Player.DoClassChange`. The C# method is mostly stub
    /// comments (`// load hotbars`, `// Calculate stats`, etc.);
    /// the only fully-implemented ceremony steps are status-effect
    /// removal + first-time-class init. Garlemald does the
    /// structural minimum:
    ///   1. Update `chara.class` to the new class id (so the
    ///      next snapshot read sees the new active class).
    ///   2. Reload the hotbar from DB for the new class via
    ///      `db.load_hotbar` + mirror to `chara.hotbar` so
    ///      `FindFirstCommandSlotById` and the
    ///      `charaWork.command[N]` accessor see the new class's
    ///      equipped commands.
    ///   3. Broadcast 0x01A4 SetCurrentJobPacket so neighbours'
    ///      nameplates flip (mirrors apply_set_current_job).
    ///
    /// Status-effect removal (LoseOnClassChange flag) + stat
    /// recalc + SendCharaExpInfo are deferred — neither is in
    /// meteor-decomp's authoritative API surface and the
    /// underlying mechanics aren't fully ported. Documented
    /// deviation per
    /// `feedback_meteor_decomp_authoritative_for_engine_bindings.md`.
    async fn apply_do_class_change(&self, player_id: u32, class_id: u8) {
        let Some(handle) = self.registry.get(player_id).await else {
            tracing::debug!(player = player_id, class_id, "DoClassChange: actor missing");
            return;
        };
        let actor_id = handle.actor_id;

        // Reload hotbar BEFORE updating chara.class so a partial
        // failure (DB load fails) leaves the player on their old
        // class with intact hotbar.
        let new_hotbar = match self.db.load_hotbar(player_id, class_id).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    player = player_id, class_id, err = %e,
                    "DoClassChange: db.load_hotbar failed",
                );
                return;
            }
        };
        {
            let mut c = handle.character.write().await;
            c.chara.class = class_id as i16;
            c.chara.hotbar = new_hotbar;
        }

        // Broadcast 0x01A4 — same packet shape as SetCurrentJob;
        // the client's per-actor class-id field is reused for
        // both the "active class" and "active job" indicators.
        let bytes = crate::packets::send::player::build_set_current_job(actor_id, class_id as u32)
            .to_bytes();
        crate::runtime::dispatcher::send_to_self_if_player(
            &self.registry,
            &self.world,
            actor_id,
            bytes.clone(),
        )
        .await;
        crate::runtime::dispatcher::broadcast_to_neighbours(
            &self.world,
            &self.registry,
            actor_id,
            bytes,
        )
        .await;

        tracing::info!(
            player = player_id,
            class_id,
            "DoClassChange applied (chara.class + hotbar reload + 0x01A4 broadcast; status-effect removal + stat recalc deferred)",
        );
    }

    /// `player:PrepareClassChange(classId)` apply — C# precursor
    /// that calls `SendCharaExpInfo()`. Garlemald doesn't have
    /// SendCharaExpInfo wired (no opcode builder, not a real
    /// engine binding per meteor-decomp); log + no-op. The
    /// EquipCommand.lua flow that calls Prepare→Do treats the
    /// pair atomically anyway — Prepare being a no-op doesn't
    /// break the script flow.
    async fn apply_prepare_class_change(&self, player_id: u32, class_id: u8) {
        tracing::debug!(
            player = player_id,
            class_id,
            "PrepareClassChange captured (SendCharaExpInfo not wired — no opcode builder)",
        );
    }

    /// `quest:GetData():SetNpcLsFrom(from)` and the
    /// `LuaQuestHandle::NewNpcLsMsg` first step.
    /// Mutates the live Quest's `data.npc_ls_from`, then persists to
    /// the migration-050 column. Silently no-ops if the player isn't
    /// in the registry or the quest isn't in their journal.
    async fn apply_quest_set_npc_ls_from(&self, player_id: u32, quest_id: u32, from: u32) {
        let Some(handle) = self.registry.get(player_id).await else {
            return;
        };
        let slot = {
            let mut c = handle.character.write().await;
            let Some(slot) = c.quest_journal.slot_of(quest_id) else {
                return;
            };
            if let Some(q) = c.quest_journal.slots[slot].as_mut() {
                q.set_npc_ls_from(from);
            }
            slot as i32
        };
        if let Err(e) = self
            .db
            .save_quest_npc_ls(player_id, slot, from, /* msg_step preserved */ {
                let c = handle.character.read().await;
                c.quest_journal
                    .get(quest_id)
                    .map(|q| q.get_npc_ls_msg_step())
                    .unwrap_or(0)
            })
            .await
        {
            tracing::warn!(
                player = player_id, quest = quest_id, from, err = %e,
                "QuestSetNpcLsFrom: DB persist failed",
            );
        }
    }

    /// `quest:GetData():IncrementNpcLsMsgStep()` and the
    /// `LuaQuestHandle::ReadNpcLsMsg` first step.
    async fn apply_quest_increment_npc_ls_msg_step(&self, player_id: u32, quest_id: u32) {
        let Some(handle) = self.registry.get(player_id).await else {
            return;
        };
        let (slot, npc_ls_from, new_step) = {
            let mut c = handle.character.write().await;
            let Some(slot) = c.quest_journal.slot_of(quest_id) else {
                return;
            };
            let (from, step) = if let Some(q) = c.quest_journal.slots[slot].as_mut() {
                let step = q.inc_npc_ls_msg_step();
                (q.get_npc_ls_from(), step)
            } else {
                return;
            };
            (slot as i32, from, step)
        };
        if let Err(e) = self
            .db
            .save_quest_npc_ls(player_id, slot, npc_ls_from, new_step)
            .await
        {
            tracing::warn!(
                player = player_id, quest = quest_id, err = %e,
                "QuestIncrementNpcLsMsgStep: DB persist failed",
            );
        }
    }

    /// `quest:GetData():ClearNpcLs()` and the
    /// `LuaQuestHandle::EndOfNpcLsMsgs` last step.
    async fn apply_quest_clear_npc_ls(&self, player_id: u32, quest_id: u32) {
        let Some(handle) = self.registry.get(player_id).await else {
            return;
        };
        let slot = {
            let mut c = handle.character.write().await;
            let Some(slot) = c.quest_journal.slot_of(quest_id) else {
                return;
            };
            if let Some(q) = c.quest_journal.slots[slot].as_mut() {
                q.clear_npc_ls();
            }
            slot as i32
        };
        if let Err(e) = self.db.save_quest_npc_ls(player_id, slot, 0, 0).await {
            tracing::warn!(
                player = player_id, quest = quest_id, err = %e,
                "QuestClearNpcLs: DB persist failed",
            );
        }
    }

    async fn apply_add_seals(&self, player_id: u32, gc: u8, amount: i32) {
        if !crate::actor::gc::is_valid_gc(gc) {
            tracing::debug!(player = player_id, gc, "AddSeals: invalid gc id");
            return;
        }
        match self.db.add_seals(player_id, gc, amount).await {
            Ok(total) => tracing::info!(
                player = player_id,
                gc,
                delta = amount,
                total,
                "AddSeals applied",
            ),
            Err(e) => tracing::warn!(
                player = player_id,
                gc,
                err = %e,
                "AddSeals: DB persist failed",
            ),
        }
    }

    /// `player:PromoteGC(gc)` — atomic seal-spend + rank-bump.
    /// Mirrors the post-`eventDoRankUp` tail of Meteor's
    /// `PopulaceCompanyOfficer.lua` flow. Refuses (logs at `info` and
    /// returns without any DB write) when:
    /// * `gc` isn't a valid GC id (1/2/3),
    /// * the player isn't in the registry (offline / NPC),
    /// * the player isn't enlisted in `gc` (`chara.gc_current != gc`),
    /// * current rank has no `next_rank` (already at/past 1.23b cap of 31),
    /// * seal balance is below `gc_promotion_cost(current)`.
    /// On success: spends `cost` seals via `db.add_seals(-cost)`,
    /// bumps the per-GC rank field on `CharaState` to `next_rank`,
    /// persists the rank via `db.set_gc_rank`, and emits
    /// `SetGrandCompanyPacket` so the client sees the new rank.
    async fn apply_promote_gc(&self, player_id: u32, gc: u8) {
        if !crate::actor::gc::is_valid_gc(gc) {
            tracing::debug!(player = player_id, gc, "PromoteGC: invalid gc id");
            return;
        }
        let Some(handle) = self.registry.get(player_id).await else {
            tracing::debug!(player = player_id, "PromoteGC: player not in registry");
            return;
        };
        // Read current enrollment + rank + (for tier-shift gates) the
        // completed-quest set under a single read lock.
        let (enrolled_gc, current_rank, completed_quests) = {
            let c = handle.character.read().await;
            let rank = match gc {
                crate::actor::gc::GC_MAELSTROM => c.chara.gc_rank_limsa,
                crate::actor::gc::GC_TWIN_ADDER => c.chara.gc_rank_gridania,
                crate::actor::gc::GC_IMMORTAL_FLAMES => c.chara.gc_rank_uldah,
                _ => 0,
            };
            let completed: std::collections::HashSet<u32> =
                c.quest_journal.iter_completed().collect();
            (c.chara.gc_current, rank, completed)
        };
        if enrolled_gc != gc {
            tracing::info!(
                player = player_id,
                gc,
                enrolled = enrolled_gc,
                "PromoteGC refused: player not enlisted in target GC",
            );
            return;
        }
        let Some(next_rank) = crate::actor::gc::next_rank(current_rank) else {
            tracing::info!(
                player = player_id,
                gc,
                current_rank,
                "PromoteGC refused: already at or past STORY_RANK_CAP",
            );
            return;
        };
        // Tier-shift gate — Corporal → Sergeant Third Class (17 → 21)
        // and Chief Sergeant → Second Lieutenant (27 → 31) require
        // their respective per-GC story quest to be completed before
        // the dialog branch even offers the promotion. Refuse here
        // even if seal balance + cap checks would otherwise pass — the
        // script's `eventTalkQuestUncomplete()` dialog the comment
        // header at `PopulaceCompanyOfficer.lua:20` describes is the
        // client-visible counterpart.
        if let Some(gate_quest) = crate::actor::gc::tier_shift_quest(current_rank, gc)
            && !completed_quests.contains(&gate_quest)
        {
            tracing::info!(
                player = player_id,
                gc,
                current_rank,
                gate_quest,
                "PromoteGC refused: tier-shift story quest not completed",
            );
            return;
        }
        let cost = crate::actor::gc::gc_promotion_cost(current_rank);
        if cost <= 0 {
            tracing::info!(
                player = player_id,
                gc,
                current_rank,
                "PromoteGC refused: no promotion cost defined for current rank",
            );
            return;
        }
        let balance = match self.db.get_seals(player_id, gc).await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(
                    player = player_id,
                    gc,
                    err = %e,
                    "PromoteGC: DB get_seals failed",
                );
                return;
            }
        };
        if balance < cost {
            tracing::info!(
                player = player_id,
                gc,
                current_rank,
                cost,
                balance,
                "PromoteGC refused: insufficient seal balance",
            );
            return;
        }
        // Spend seals first — `add_seals` clamps the post-deposit
        // total at 0 so even if we later fail to bump the rank the
        // player isn't double-charged on retry. The rank-bump path
        // sticks with our existing AddSeals semantics.
        if let Err(e) = self.db.add_seals(player_id, gc, -cost).await {
            tracing::warn!(
                player = player_id,
                gc,
                cost,
                err = %e,
                "PromoteGC: DB seal deduction failed",
            );
            return;
        }
        // Bump CharaState first so the SetGrandCompanyPacket emit
        // (which reads CharaState) reflects the new rank without
        // racing the DB write.
        {
            let mut c = handle.character.write().await;
            match gc {
                crate::actor::gc::GC_MAELSTROM => c.chara.gc_rank_limsa = next_rank,
                crate::actor::gc::GC_TWIN_ADDER => c.chara.gc_rank_gridania = next_rank,
                crate::actor::gc::GC_IMMORTAL_FLAMES => c.chara.gc_rank_uldah = next_rank,
                _ => {}
            }
        }
        if let Err(e) = self.db.set_gc_rank(player_id, gc, next_rank).await {
            tracing::warn!(
                player = player_id,
                gc,
                next_rank,
                err = %e,
                "PromoteGC: DB set_gc_rank failed (CharaState already updated; will reconcile on next login)",
            );
        }
        self.emit_grand_company_packet(&handle).await;
        // Rank-up animation broadcast — `eventDoRankUp` plays the
        // promotion fanfare on the promoting client itself, but
        // nearby players never hear / see it because `callClientFunction`
        // only targets the issuing player. Emit a server-side
        // `PlayAnimationOnActor` (0x00DA) carrying the canonical
        // teleport-fanfare animation id (`0x4000FFB`, used by
        // `TeleportCommand.lua` for the teleport-in flourish — the
        // closest documented "scene transition" effect we have, and a
        // plausible salute placeholder until a dedicated GC-salute id
        // is sourced) so neighbours witness the rank-up moment too.
        // Wraps both the self-send and the nearby-player fan-out
        // through the shared `broadcast_around_actor` helper, matching
        // the chocobo `SendMountAppearance` pattern at
        // `apply_send_mount_appearance:1719-1745`.
        const RANKUP_ANIMATION_ID: u32 = 0x4000_FFB;
        let sub = tx::actor::build_play_animation_on_actor(
            handle.actor_id,
            RANKUP_ANIMATION_ID,
        );
        if let Ok(base) = common::BasePacket::create_from_subpacket(&sub, true, false) {
            let bytes = base.to_bytes();
            // Self-emit so the promoting player sees the salute
            // regardless of how far from any neighbour they are.
            if let Some(client) = self.world.client(handle.session_id).await {
                client.send_bytes(bytes.clone()).await;
            }
            if let Some(zone) = self.world.zone(handle.zone_id).await {
                let sent = crate::runtime::broadcast::broadcast_around_actor(
                    &self.world,
                    &self.registry,
                    &zone,
                    handle.actor_id,
                    bytes,
                )
                .await;
                tracing::debug!(
                    player = player_id,
                    nearby = sent,
                    "PromoteGC: rank-up animation broadcast fan-out",
                );
            }
        }
        tracing::info!(
            player = player_id,
            gc,
            current_rank,
            next_rank,
            cost,
            "PromoteGC applied",
        );
    }

    async fn apply_add_retainer_bazaar_item(
        &self,
        retainer_id: u32,
        item_id: u32,
        quantity: i32,
        quality: u8,
        price_gil: i32,
    ) {
        match self
            .db
            .add_retainer_bazaar_item(retainer_id, item_id, quantity, quality, price_gil)
            .await
        {
            Ok(server_item_id) => {
                tracing::info!(
                    retainer_id,
                    item_id,
                    quantity,
                    quality,
                    price_gil,
                    server_item_id,
                    "AddRetainerBazaarItem applied",
                );
            }
            Err(e) => {
                tracing::warn!(
                    retainer_id,
                    item_id,
                    quantity,
                    quality,
                    price_gil,
                    err = %e,
                    "AddRetainerBazaarItem: DB upsert failed",
                );
            }
        }
    }

    async fn apply_dismiss_my_retainer(&self, player_id: u32, retainer_id: u32) {
        // Delete the ownership row first; if the dismissed retainer
        // is currently spawned, also clear the session snapshot so a
        // subsequent `SpawnMyRetainer` can't re-reference the stale id.
        let deleted = match self.db.dismiss_retainer(player_id, retainer_id).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    player = player_id,
                    retainer_id,
                    err = %e,
                    "DismissMyRetainer: DB delete failed",
                );
                return;
            }
        };
        if let Some(handle) = self.registry.get(player_id).await {
            let session_id = handle.session_id;
            if session_id != 0
                && let Some(mut session) = self.world.session(session_id).await
            {
                if let Some(r) = &session.spawned_retainer
                    && r.retainer_id == retainer_id
                {
                    session.spawned_retainer = None;
                    self.world.upsert_session(session).await;
                }
            }
        }
        tracing::info!(
            player = player_id,
            retainer_id,
            deleted,
            "DismissMyRetainer applied",
        );
    }

    /// Tier 4 #14 E — persist a retainer rename via
    /// [`Database::rename_retainer`] (writes the per-character
    /// `customName` column). If the renamed retainer is currently
    /// spawned, also refresh the in-memory `SpawnedRetainer.name`
    /// so the same session's future reads (e.g. the
    /// `GetSpawnedRetainer():GetName()` chain) see the new name
    /// without a re-summon.
    async fn apply_rename_retainer(
        &self,
        player_id: u32,
        retainer_id: u32,
        new_name: String,
    ) {
        if new_name.trim().is_empty() {
            tracing::debug!(
                player = player_id,
                retainer_id,
                "RenameRetainer: empty name rejected",
            );
            return;
        }
        match self
            .db
            .rename_retainer(player_id, retainer_id, new_name.clone())
            .await
        {
            Ok(true) => tracing::info!(
                player = player_id,
                retainer_id,
                new_name = %new_name,
                "RenameRetainer applied",
            ),
            Ok(false) => {
                tracing::info!(
                    player = player_id,
                    retainer_id,
                    "RenameRetainer: no ownership row — retainer not hired",
                );
                return;
            }
            Err(e) => {
                tracing::warn!(
                    player = player_id,
                    retainer_id,
                    err = %e,
                    "RenameRetainer: DB update failed",
                );
                return;
            }
        }

        // Refresh the session's live snapshot if this retainer is
        // currently out. Otherwise nothing to do — subsequent
        // `SpawnMyRetainer` calls will re-read via `load_retainer`,
        // which now COALESCEs in the `customName`.
        let Some(handle) = self.registry.get(player_id).await else {
            return;
        };
        let session_id = handle.session_id;
        if session_id == 0 {
            return;
        }
        if let Some(mut session) = self.world.session(session_id).await {
            if let Some(r) = session.spawned_retainer.as_mut()
                && r.retainer_id == retainer_id
            {
                r.name = new_name;
                self.world.upsert_session(session).await;
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
        // Mirror Meteor's `QuestState.UpdateState()` (Map Server/Actors/
        // Quest/QuestState.cs:UpdateState) — re-run the script's
        // `onStateChange(sequence)` hook so it can re-evaluate flag-
        // dependent `quest:SetENpc(...)` calls, then drain stale entries
        // and broadcast clears for ENPCs the new state didn't re-register.
        //
        // Without the re-run, scripts that toggle ENPC visibility from
        // `onTalk` (e.g. `man0g0::seq000_onTalk` flips Yda → off + Papalymo
        // → on by setting `FLAG_SEQ000_MINITUT0` and trailing
        // `quest:UpdateENPCs()`) only got the stale-drain half — Yda never
        // went off, Papalymo never went on, and the player got stuck after
        // the talk-tutorial cinematic with nothing else to interact with.
        let sequence = {
            let c = handle.character.read().await;
            c.quest_journal.get(quest_id).map(|q| q.get_sequence())
        };
        if let Some(sequence) = sequence {
            // Swap the ENPC maps BEFORE the hook runs so `apply_quest_set_enpc`
            // sees a clean `current` and the `old` set captures the previous
            // state for diffing — same pattern as `apply_quest_start_sequence`.
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
        }
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

        tracing::info!(
            player = player_id,
            npc_class = enpc.actor_class_id,
            npc_actor = format!("0x{:08X}", npc_actor_id),
            quest_flag = enpc.quest_flag_type,
            talk = enpc.is_talk_enabled,
            push = enpc.is_push_enabled,
            emote = enpc.is_emote_enabled,
            "broadcast_quest_enpc_update",
        );

        let subpackets = crate::packets::send::build_actor_event_status_packets(
            npc_actor_id,
            &conditions,
            enpc.is_talk_enabled,
            enpc.is_emote_enabled,
            Some(enpc.is_push_enabled),
            /* notice_enabled */ true,
        );
        for mut sub in subpackets {
            // 1.x client silently drops event-related subpackets whose
            // SubPacketHeader.target_id != receiving actor's session id
            // (same gotcha that `dispatch_event_event` for RunEventFunction
            // documents). Without setting it the SetEventStatus + quest-
            // graphic broadcasts evaporate on the wire — visible symptom:
            // after `man0g0::seq000_onTalk` swaps Yda → off / Papalymo → on,
            // Papalymo's talk-arrow icon never appears and the player gets
            // stuck with no clickable next NPC.
            sub.set_target_id(player_id);
            client.send_bytes(sub.to_bytes()).await;
        }
        let mut graphic = crate::packets::send::build_set_actor_quest_graphic(
            npc_actor_id,
            enpc.quest_flag_type,
        );
        graphic.set_target_id(player_id);
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
        for mut sub in subpackets {
            // Same target_id requirement as `broadcast_quest_enpc_update`.
            sub.set_target_id(player_id);
            client.send_bytes(sub.to_bytes()).await;
        }
        let mut graphic = crate::packets::send::build_set_actor_quest_graphic(npc_actor_id, 0);
        graphic.set_target_id(player_id);
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

        // Fan out the canonical "<Quest> added to journal" toast.
        // Mirror C# `WorldManager.AddQuest`'s
        // `SendGameMessage(WorldMaster, 25224, 0x20, questId)`. Routed
        // through the auto-tier text-sheet builder; receiver = the
        // owning client only (no broadcast — this is a personal
        // system message).
        if let Some(client) = self.world.client(handle.session_id).await {
            let pkt = crate::packets::send::misc::build_text_sheet_no_source_auto(
                handle.actor_id,
                crate::packets::send::misc::WORLD_MASTER_ACTOR_ID,
                /* text_id */ 25224,
                crate::packets::send::misc::MESSAGE_TYPE_SYSTEM,
                &[common::luaparam::LuaParam::UInt32(quest_id)],
                /* prefer_alt */ false,
            );
            client.send_bytes(pkt.to_bytes()).await;
        }

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

        // Fan out the canonical "<Quest> complete!" toast.
        // Mirror C# `Quest.OnComplete`'s
        // `SendGameMessage(WorldMaster, 25086, 0x20, GetQuestId())`.
        if let Some(client) = self.world.client(handle.session_id).await {
            let pkt = crate::packets::send::misc::build_text_sheet_no_source_auto(
                handle.actor_id,
                crate::packets::send::misc::WORLD_MASTER_ACTOR_ID,
                /* text_id */ 25086,
                crate::packets::send::misc::MESSAGE_TYPE_SYSTEM,
                &[common::luaparam::LuaParam::UInt32(quest_id)],
                /* prefer_alt */ false,
            );
            client.send_bytes(pkt.to_bytes()).await;
        }
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

        // Fan out the canonical "<Quest> abandoned." toast.
        // Mirror C# `WorldManager.AbandonQuest`'s
        // `SendGameMessage(this, WorldMaster, 25236, 0x20, abandoned.GetQuestId())`.
        if let Some(client) = self.world.client(handle.session_id).await {
            let pkt = crate::packets::send::misc::build_text_sheet_no_source_auto(
                handle.actor_id,
                crate::packets::send::misc::WORLD_MASTER_ACTOR_ID,
                /* text_id */ 25236,
                crate::packets::send::misc::MESSAGE_TYPE_SYSTEM,
                &[common::luaparam::LuaParam::UInt32(quest_id)],
                /* prefer_alt */ false,
            );
            client.send_bytes(pkt.to_bytes()).await;
        }
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
                .map(|q| (
                    q.get_sequence(),
                    q.get_flags(),
                    q.get_counter(0),
                    q.get_counter(1),
                    q.get_counter(2),
                    q.get_npc_ls_from(),
                    q.get_npc_ls_msg_step(),
                ))
                .unwrap_or((0, 0, 0, 0, 0, 0, 0));
            let handle = crate::lua::LuaQuestHandle {
                player_id: snapshot.actor_id,
                quest_id,
                has_quest: c.quest_journal.has(quest_id),
                sequence: quest.0,
                flags: quest.1,
                counters: [quest.2, quest.3, quest.4],
                npc_ls_from: quest.5,
                npc_ls_msg_step: quest.6,
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

    /// Variant of [`Self::fire_quest_hook`] for hooks that fire while a
    /// client-initiated event is open (`onTalk` / `onPush` / `onEmote` /
    /// `onCommand`). The hook body's tail typically does
    /// `callClientFunction(player, "delegateEvent", …)` followed by
    /// `player:EndEvent()` — both produce event-flavoured `LuaCommand`s
    /// (`RunEventFunction` / `EndEvent`) that `apply_login_lua_command`
    /// has no arm for.
    ///
    /// To make those packets actually reach the client, we snapshot the
    /// player's `EventSession` (set by `handle_event_start`'s preceding
    /// `start_event` call) and translate the event-flavoured commands
    /// into an `EventOutbox`, then drain through `dispatch_event_event`
    /// — same pattern as `dispatch_director_event_started` and
    /// `apply_quest_on_notice`.
    ///
    /// After dispatching, auto-resume any `_WAIT_EVENT`-parked coroutine
    /// the hook spun up via `callClientFunction`'s
    /// `coroutine.yield("_WAIT_EVENT", player)`. The resume drains the
    /// post-yield `player:EndEvent()` and any trailing `quest:UpdateENPCs()`
    /// — without this the coroutine sits forever waiting for an
    /// `EventUpdate` the 1.x client never sends for cutscene completion.
    async fn fire_quest_event_hook(
        &self,
        handle: &ActorHandle,
        quest_id: u32,
        hook_name: &'static str,
        extra_args: Vec<crate::lua::QuestHookArg>,
    ) {
        let Some(engine) = self.lua.as_ref() else {
            return;
        };
        let Some(script_name) = engine.catalogs().quest_script_name(quest_id) else {
            return;
        };
        let script_path = engine.resolver().quest(&script_name);
        if !script_path.exists() {
            return;
        }

        let (snapshot, quest_handle) = {
            let c = handle.character.read().await;
            if !c.quest_journal.has(quest_id) {
                return;
            }
            let snap = build_player_snapshot_from_character(&c);
            let q = c.quest_journal.get(quest_id).expect("has");
            let qh = crate::lua::LuaQuestHandle {
                player_id: snap.actor_id,
                quest_id,
                has_quest: true,
                sequence: q.get_sequence(),
                flags: q.get_flags(),
                counters: [q.get_counter(0), q.get_counter(1), q.get_counter(2)],
            npc_ls_from: q.get_npc_ls_from(),
            npc_ls_msg_step: q.get_npc_ls_msg_step(),
                queue: crate::lua::command::CommandQueue::new(),
            };
            (snap, qh)
        };

        let engine_clone = engine.clone();
        let script_path_clone = script_path.clone();
        let result = tokio::task::spawn_blocking(move || {
            engine_clone.call_quest_hook(
                &script_path_clone,
                hook_name,
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
                    "quest event-hook dispatch panicked",
                );
                return;
            }
        };
        if let Some(e) = result.error {
            tracing::debug!(
                error = %e,
                quest = quest_id,
                hook = hook_name,
                "quest event-hook errored",
            );
        }
        if result.commands.is_empty() {
            return;
        }

        // Bridge step — translate event-flavoured commands into the
        // EventOutbox so cinematic packets reach the client.
        let event_session_snapshot = {
            let c = handle.character.read().await;
            c.event_session.clone()
        };
        let mut outbox = crate::event::outbox::EventOutbox::new();
        crate::event::lua_bridge::translate_lua_commands_into_outbox(
            &result.commands,
            &event_session_snapshot,
            &mut outbox,
        );
        for e in outbox.drain() {
            Box::pin(crate::event::dispatcher::dispatch_event_event(
                &e,
                &self.registry,
                &self.world,
                &self.db,
                self.lua.as_ref(),
            ))
            .await;
        }
        // Drain non-event commands through the login-command pipeline
        // (quest-flag mutates, AddExp, UpdateENPCs, etc.).
        for cmd in result.commands {
            Box::pin(self.apply_login_lua_command(handle, cmd)).await;
        }

        // DON'T auto-resume here. The opening cinematic auto-resume
        // (`apply_quest_on_notice`) is needed because the OpeningDirector's
        // notice cinematic doesn't reliably elicit an `EventUpdate` from
        // the client. For interactive talk/push cinematics, the 1.x
        // client *does* send `0x012E EventUpdate` when the cinematic
        // ends — that path lands in `dispatch_event_updated` which calls
        // `lua.fire_player_event(...)` and resumes the parked coroutine
        // properly, with `EndEvent` going out *after* the cinematic has
        // visibly completed.
        //
        // Auto-resuming here drains the rest of the coroutine
        // (data:SetFlag → player:EndEvent → quest:UpdateENPCs)
        // immediately, which queues the `EndEvent` packet ~1 frame
        // after `RunEventFunction`. The client then receives EndEvent
        // *during* the cinematic playback, which leaves the client's
        // event-input layer in a state that silently drops every
        // subsequent `EventStart` from clicks on other NPCs (verified
        // 2026-04-25: after `processTtrNomal003` finishes, neither Yda
        // nor Papalymo's clicks produce inbound `0x012D` even though
        // both are talk-enabled with `target_id` correctly set).
        //
        // The parked coroutine stays in the scheduler; the client's
        // EventUpdate at cinematic-end resumes it via
        // `dispatch_event_updated` → `LuaEngine::fire_player_event`,
        // which then drains the trailing `EndEvent` + `UpdateENPCs`
        // back through the same EventOutbox bridge + apply pipeline
        // (see `LuaEngine::dispatch_post_resume_commands` once that's
        // wired — for now the resume path is responsible for emitting
        // the post-cinematic packets).
        let _ = engine;
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
            // Retail-IN opcodes that the 1.x client emits regularly but
            // that garlemald previously dropped via the catch-all `_`
            // arm. Promoted to explicit log-and-drop here so they
            // surface in tracing instead of being invisible. Counts
            // are from the 56-capture retail audit
            // (`captures/retail_pcap_gap_analysis.md`).
            OP_RX_ZONE_IN_COMPLETE => {
                // 24 events/session. Wiki: "Unknown 0x007"; semantics
                // per Meteor: client signals it's safe to receive
                // world-spawn packets after zone-in init. Today
                // garlemald uses `OP_RX_LANGUAGE_CODE` (0x0006) as
                // the deferred trigger, but 0x0007 is its successor —
                // promotion to explicit dispatch here keeps the
                // existing language-code path authoritative while
                // surfacing 0x0007 events for future feature work
                // (e.g. retail uses this as the "DoZoneIn complete"
                // signal alongside or instead of 0x0006).
                tracing::debug!(
                    source = source,
                    "RX 0x0007 zone-in-complete signal (no-op pending dedicated handler)",
                );
            }
            OP_RX_LOCK_TARGET => {
                // 66 events/session. Wiki: "Target Locked". Client
                // sends this when the player target-locks an actor
                // (Tab-Tab in 1.x). Garlemald's targeting today is
                // partially server-side fictional; this explicit
                // dispatch makes the client-side intent visible.
                let target_id = if sub.data.len() >= 4 {
                    u32::from_le_bytes(sub.data[..4].try_into().unwrap())
                } else {
                    0
                };
                tracing::debug!(
                    source = source,
                    target = format!("0x{:08X}", target_id),
                    "RX 0x00CC target-locked",
                );
            }
            OP_RX_SET_TARGET => {
                // 118 events/session — most-frequent IN gap. Wiki:
                // "Target Selected". Client sends this on
                // soft-target / hover-select. Project Meteor parses
                // it via `SetTargetPacket` and uses
                // `attackTarget != 0xE0000000` to drive auto-attack
                // engage state (`PacketProcessor.cs:175`).
                let attack_target = if sub.data.len() >= 4 {
                    u32::from_le_bytes(sub.data[..4].try_into().unwrap())
                } else {
                    0
                };
                tracing::debug!(
                    source = source,
                    attack_target = format!("0x{:08X}", attack_target),
                    "RX 0x00CD target-selected",
                );
            }
            OP_RX_DATA_REQUEST => {
                // 44 events/session. Same opcode as outbound
                // KickEvent — direction disambiguates. Client asks
                // for a GAM-property refresh by path; payload at
                // body[0..4] is u32 target_actor_id, body[4..24] is
                // a null-padded ASCII property path
                // (e.g. "charaWork/exp"), body[24..32] is variable
                // trailing data.
                let prop_path = if sub.data.len() >= 24 {
                    extract_null_terminated_ascii(&sub.data[4..24])
                } else {
                    String::new()
                };
                tracing::debug!(
                    source = source,
                    property = %prop_path,
                    "RX 0x012F data-request (no-op pending property-refresh handler)",
                );
            }
            OP_RX_GROUP_CREATED => {
                // 270 events/session — highest-volume IN gap. Same
                // opcode as outbound GenericData. Client signals it
                // has spawned a new monster group / actor and wants
                // the server to register `/_init` event handlers.
                //
                // Captured retail body shape (every 0x0133 IN record
                // in the 56-capture survey, none ambiguous):
                //   body[0..8]  = u64 actor or monster-group id
                //                 (synthetic 0x2680… prefix for mob
                //                 groups)
                //   body[8..14] = ASCII "/_init"
                //   body[14..40] = 26 bytes of zero
                //
                // The captures don't disambiguate the string field's
                // declared width — every captured string fits in 7
                // bytes including the NUL, so a 16/24/32-byte field
                // would all look identical on the wire when followed
                // by zero-padding. We extract through the full body
                // and stop at the first NUL: defensive against the
                // unknown true field size, correct for the captured
                // strings, and harmless if the trailing region turns
                // out to be reserved rather than padding (it's
                // always zero in practice).
                let event_name = if sub.data.len() >= 8 {
                    extract_null_terminated_ascii(&sub.data[8..])
                } else {
                    String::new()
                };
                let group_id = if sub.data.len() >= 8 {
                    u64::from_le_bytes(sub.data[..8].try_into().unwrap())
                } else {
                    0
                };
                tracing::debug!(
                    source = source,
                    group_id = format!("0x{:016X}", group_id),
                    event = %event_name,
                    "RX 0x0133 group-created (no-op pending event-init handler)",
                );
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

    // (helper used by the retail-IN arms above; trims at the first NUL.)

    /// Pmeteor's `RequestQuestJournalCommand` static-actor id —
    /// `0xA0F00000 | 0x5E93`. The 1.x client sends `EventStart` against
    /// this actor with `event_name="commandRequest"` whenever the player
    /// opens a journal entry, expecting a `qtdata` reply with the quest's
    /// sequence + journalInfo.
    const REQUEST_QUEST_JOURNAL_COMMAND: u32 = 0xA0F0_5E93;

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
        let event_name_for_match = pkt.event_name.clone();
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

        // Fire the per-quest event hook based on the EventStart's
        // `event_type`. Meteor's convention is to fire for *every* active
        // quest and let the script filter by NPC class id + sequence —
        // pre-filtering on `QuestState.current` membership would drop
        // scripts that haven't populated their ENPC list yet (many stub
        // quests, tutorial cleanup paths, etc.).
        //
        // Mirrors `PopulaceStandard.lua::doQuestEvent`'s eventType switch:
        //   * 1 → `quest:OnTalk(player, npc)`
        //   * 2 → `quest:OnPush(player, npc, eventName)`
        //   * 3 → `quest:OnEmote(player, npc, eventName)`
        //   * 0 → `quest:OnCommand(player, npc, eventName)`
        //
        // The 1.x client fires eventType=2 itself when the player walks
        // into a `SetPushEventConditionWithCircle` radius — this is the
        // hook that lets quests like `man0g0::onPush` fire the
        // `processTtrNomal002` cinematic when the player closes on Yda.
        if let Some(hook_name) = match pkt.event_type {
            1 => Some("onTalk"),
            2 => Some("onPush"),
            3 => Some("onEmote"),
            0 => Some("onCommand"),
            _ => None,
        } {
            self.fire_quest_hook_for_active_quests(&handle, owner_actor_id, hook_name).await;
        }

        // RequestQuestJournalCommand handler — when the client opens a
        // quest's journal entry it sends EventStart targeting the
        // `RequestQuestJournalCommand` static actor (id `0xA0F05E93`)
        // with eventName `"commandRequest"`. Pmeteor's
        // `commands/RequestQuestJournalCommand.lua` responds by calling
        // `quest:GetJournalInformation()` and queueing a
        // `SendDataPacket("requestedData", "qtdata", questId, sequence,
        // …journalInfo)` (opcode 0x0133), then `EndEvent`. Without the
        // qtdata response the 1.x journal pane shows the quest name from
        // sqpack data but no description / sequence summary, leaving the
        // entry blank for the user.
        //
        // We don't have a full command-actor scripting framework yet, so
        // this is a hardcoded handler: detect the magic actor id +
        // eventName, walk the player's journal, and emit one qtdata
        // packet per active quest with the default-empty journalInfo
        // (man0g0 + most opener quests don't override
        // `getJournalInformation`).
        if owner_actor_id == Self::REQUEST_QUEST_JOURNAL_COMMAND
            && event_name_for_match == "commandRequest"
        {
            self.send_quest_journal_data(&handle, session_id).await;
        }

        tracing::debug!(
            player = actor_id,
            owner = owner_actor_id,
            event_type = pkt.event_type,
            "event start dispatched",
        );
        Ok(())
    }

    /// Mirror of pmeteor's `RequestQuestJournalCommand.lua::onEventStarted`
    /// — emit one `0x0133 GenericDataPacket(["requestedData", "qtdata",
    /// questId, sequence])` per active quest, then a single `EndEvent`.
    /// The default empty journalInfo is fine for the man0g0 opener path
    /// (and most quests that don't override `getJournalInformation`).
    async fn send_quest_journal_data(&self, handle: &ActorHandle, session_id: u32) {
        let Some(client) = self.world.client(session_id).await else {
            return;
        };
        let actor_id = handle.actor_id;

        let active_quests: Vec<(u32, u32)> = {
            let c = handle.character.read().await;
            c.quest_journal
                .slots
                .iter()
                .flatten()
                .map(|q| (q.quest_id(), q.get_sequence()))
                .collect()
        };

        for (quest_id, sequence) in active_quests {
            // Match pmeteor's exact param shape: [String, String, Int32,
            // Int32, Nil] — pmeteor's lua tail does
            // `unpack(journalInfo)` after the questId/sequence ints, and
            // even with `journalInfo == {}` C# pads at least one Nil into
            // the packet so the client's reader sees a 5-param payload.
            let params = vec![
                common::luaparam::LuaParam::String("requestedData".to_string()),
                common::luaparam::LuaParam::String("qtdata".to_string()),
                common::luaparam::LuaParam::Int32(quest_id as i32),
                common::luaparam::LuaParam::Int32(sequence as i32),
                common::luaparam::LuaParam::Nil,
            ];
            let mut pkt = crate::packets::send::player::build_generic_data(actor_id, &params);
            // 1.x client silently drops event-flavoured subpackets where
            // SubPacketHeader.target_id != receiving actor's session id.
            // Pmeteor's queue dispatcher stamps `target_id = player.Id`
            // for all queued packets; garlemald's `build_generic_data`
            // leaves it 0, which makes the client ignore the qtdata
            // payload and the journal pane never populates the
            // description. Same gotcha as `broadcast_quest_enpc_update`.
            pkt.set_target_id(actor_id);
            client.send_bytes(pkt.to_bytes()).await;
            tracing::debug!(
                player = actor_id,
                quest = quest_id,
                sequence = sequence,
                "RequestQuestJournalCommand → qtdata sent",
            );
        }

        // Pmeteor's lua tail calls `player:EndEvent()` after queueing the
        // qtdata packets, regardless of whether any quest matched. Match
        // that — without an EndEvent the client sits with an open event
        // session and the journal-pane request never completes.
        let mut end = crate::packets::send::events::build_end_event(
            actor_id,
            Self::REQUEST_QUEST_JOURNAL_COMMAND,
            "commandRequest",
            0,
        );
        end.set_target_id(actor_id);
        client.send_bytes(end.to_bytes()).await;
    }

    /// Look up the NPC's live state and fire `<hook_name>(player, quest, npc)`
    /// once per active quest in the player's journal. Properly bridges any
    /// event-flavoured commands the hook emits (`RunEventFunction` /
    /// `EndEvent` / `KickEvent`) into the `EventOutbox` so cinematic
    /// packets reach the client — without this, the quest's
    /// `callClientFunction(...)` lines would queue their commands but
    /// they'd be silently dropped at `apply_login_lua_command`.
    ///
    /// No-ops if the NPC isn't in the registry, or the player has no
    /// active quests.
    async fn fire_quest_hook_for_active_quests(
        &self,
        handle: &ActorHandle,
        npc_actor_id: u32,
        hook_name: &'static str,
    ) {
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
            // quest-side fan-out loop.
            return;
        };

        for quest_id in active_quest_ids {
            self.fire_quest_event_hook(
                handle,
                quest_id,
                hook_name,
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

        // 4. Proximity-push dispatch is now CLIENT-SIDE. The
        //    `SetPushEventConditionWithCircle` packets emitted in the
        //    spawn bundle, combined with the corrected `SetEventStatus`
        //    wire format (UInt32 enabled flag + correct outwards bits),
        //    let the 1.x client track proximity locally and fire
        //    `EventStart(eventType=2, owner=npc, eventName="pushDefault")`
        //    when the player walks into the circle. That EventStart
        //    lands in `handle_event_start` below.
        //
        //    Earlier in this branch we ran a server-side
        //    `kick_quest_proximity_pushes` that emitted
        //    `KickEventPacket("pushDefault")` to force the same flow,
        //    because the SetEventStatus packet was malformed and the
        //    client never tracked proximity. Once the wire format was
        //    fixed (UInt32 not Byte) and the broadcast started actually
        //    enabling the push trigger, both paths started firing —
        //    one EventStart per client-side trigger AND one per
        //    server-side kick — which spammed the same `processTtrNomal002`
        //    cinematic ~30 times per second. Letting the client own
        //    proximity is the cleaner answer.

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

        // GM `!command` shortcut — eat the message on match. When we
        // have a CommandProcessor handle, dispatch the message verbatim
        // through the same typed shim the stdin console reader uses.
        // This turns in-game chat into an auxiliary GM console — useful
        // since `run-all.sh`-backgrounded map-servers have stdin tied
        // to /dev/null so the stdin path is dead in practice.
        if pkt.message.starts_with('!') {
            let line = pkt.message[1..].to_string();
            tracing::info!(
                session = session_id,
                cmd = %line,
                "gm command from chat",
            );
            if let Some(cmd) = &self.cmd {
                match cmd.run(&line).await {
                    Ok(response) if !response.is_empty() => {
                        tracing::info!(%response, "command result");
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!(error = %e, "gm command failed");
                    }
                }
            } else {
                tracing::warn!(
                    "gm command requested via chat but CommandProcessor is not wired",
                );
            }
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
        // Hotbar mirrored to CharaState at session-begin (see the
        // `character.chara.hotbar = loaded.hotbar.clone()` line in
        // the LoadedPlayer hydration above). Registry-reachable +
        // mutable by the EquipAbility/UnequipAbility/SwapAbilities
        // apply paths.
        hotbar: c.chara.hotbar.clone(),
        command_border: 0x20,
        // SNpc / Path Companion scratchpad mirror.
        snpc_nickname: c.chara.snpc_nickname.clone(),
        snpc_skin: c.chara.snpc_skin,
        snpc_personality: c.chara.snpc_personality,
        snpc_coordinate: c.chara.snpc_coordinate,
        mount_state: c.chara.mount_state,
        has_chocobo: c.chara.has_chocobo,
        chocobo_appearance: c.chara.chocobo_appearance,
        chocobo_name: c.chara.chocobo_name.clone(),
        rental_expire_time: c.chara.rental_expire_time,
        rental_min_left: c.chara.rental_min_left,
        gc_current: c.chara.gc_current,
        gc_rank_limsa: c.chara.gc_rank_limsa,
        gc_rank_gridania: c.chara.gc_rank_gridania,
        gc_rank_uldah: c.chara.gc_rank_uldah,
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
        // Login snapshot never has a retainer spawned — the tutorial
        // hook runs before the player has even hit the world map.
        spawned_retainer: None,
        // Dream/sleeping state is session-scoped; the caller
        // overlays via `PlayerSnapshot::set_inn_state` if it has
        // session access.
        current_dream_id: None,
        is_sleeping: false,
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
            npc_ls_from: q.get_npc_ls_from(),
            npc_ls_msg_step: q.get_npc_ls_msg_step(),
        })
        .collect();
    snapshot.completed_quests = c.quest_journal.iter_completed().collect();
    snapshot
}

#[cfg(test)]
mod retail_in_dispatch_tests {
    use super::*;

    /// Property-path extraction matches what the 0x012F handler logs
    /// (captured `action_and_traits.pcapng` 0x012F record #1 carries
    /// "charaWork/exp" at body offset 4..24, null-padded).
    #[test]
    fn extract_null_terminated_handles_short_string() {
        let mut bytes = [0u8; 20];
        bytes[..13].copy_from_slice(b"charaWork/exp");
        assert_eq!(extract_null_terminated_ascii(&bytes), "charaWork/exp");
    }

    /// The 0x0133 captured "/_init" string lives at body[8..24]
    /// (16 bytes), with the rest null-padded.
    #[test]
    fn extract_null_terminated_handles_init_string() {
        let mut bytes = [0u8; 16];
        bytes[..6].copy_from_slice(b"/_init");
        assert_eq!(extract_null_terminated_ascii(&bytes), "/_init");
    }

    /// No null terminator → string spans the entire slice.
    #[test]
    fn extract_null_terminated_no_terminator() {
        let bytes = [b'A'; 8];
        assert_eq!(extract_null_terminated_ascii(&bytes), "AAAAAAAA");
    }

    /// Empty input.
    #[test]
    fn extract_null_terminated_empty() {
        assert_eq!(extract_null_terminated_ascii(&[]), "");
    }
}
