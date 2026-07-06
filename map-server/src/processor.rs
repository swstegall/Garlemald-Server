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
    OP_RX_GM_TICKET_SEND, OP_RX_GM_TICKET_STATE, OP_RX_GROUP_CREATED, OP_RX_ITEM_PACKAGE_REQUEST,
    OP_RX_LANGUAGE_CODE, OP_RX_LOCK_TARGET, OP_RX_RECRUITER_STATE, OP_RX_RECRUITING_DETAILS,
    OP_RX_SET_TARGET, OP_RX_START_RECRUITING, OP_RX_SUPPORT_ISSUE_REQUEST,
    OP_RX_UPDATE_PLAYER_POSITION, OP_RX_ZONE_IN_COMPLETE, OP_SESSION_BEGIN, OP_SESSION_END,
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
                    common::packet_diagnostics::log_unknown_subpacket("map", "map", &sub);
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
        // Restore the saved private area (a quest Echo / PrivateAreaMasterPast
        // instance) so a relog INSIDE one lands the player back in it rather
        // than the public zone. Without this a player who saved in man0l1's
        // Fisherman's-Guild echo (PrivateAreaMasterPast type 5, zone 230)
        // reloads into public zone 230 — they see the public guild populace but
        // the echo-only quest NPCs (e.g. Sisipu, class 1000155, which has no
        // public spawn row) are absent and the quest can't advance. The login
        // zone-change below threads this through. (Garlemald-Server #46.)
        let login_private_area =
            (!loaded.current_private_area.is_empty()).then(|| loaded.current_private_area.clone());
        let login_private_area_type = loaded.current_private_area_type;
        session.current_private_area_name = login_private_area.clone();
        session.current_private_area_level = login_private_area_type;
        session.destination_x = spawn.x;
        session.destination_y = spawn.y;
        session.destination_z = spawn.z;
        session.destination_rot = rotation;
        // LOGIN window opens here and closes on the client's first
        // RX 0x0007: any warp drained in between (the login arm re-runs
        // quest onStateChange AFTER dispatching zone-in bundle #1, and
        // a rescue arm may emit WarpToPublicArea) is parked on
        // `deferred_login_warp` instead of firing a second world-load
        // under the still-initializing client (#46 round 4; see
        // `Session::defer_warps_until_zone_in_ack`). This upsert is a
        // fresh `Session::new`, so a relog can never inherit a stale
        // `reload_in_flight` / deferred-warp latch from a crashed
        // predecessor session.
        session.defer_warps_until_zone_in_ack = true;
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
        character
            .chara
            .mods
            .set(crate::actor::modifier::Modifier::Hp, hp_max as f64);
        character
            .chara
            .mods
            .set(crate::actor::modifier::Modifier::Mp, mp_max as f64);
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
        // Play-time hydration — the DB round-trip already worked
        // (`SavePlayTime` persists, `load_player_character` reads
        // `playTime`), but the value was dropped here, so the login
        // snapshot's `GetPlayTime(false)` stayed 0 and `player.lua::
        // onLogin` re-ran its first-login branch (message + duplicate
        // starter kit) every login. (Garlemald-Server #46.)
        character.chara.play_time = loaded.play_time;
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
        // Attuned-aetheryte hydration (`characters_aetherytes`,
        // migration 068 — Garlemald-Server #46, round 5). Feeds
        // `PlayerSnapshot::unlocked_aetherytes` so the
        // `HasAetheryteNodeUnlocked` gates (AetheryteParent.lua menu,
        // TeleportCommand.lua destination check) survive a relog.
        // Loaded directly here rather than through `LoadedPlayer`
        // (the DTO lives in gamedata.rs) — same direct-DB shape as
        // `load_completed_quests` below.
        match self.db.load_character_aetherytes(actor_id).await {
            Ok(ids) => character.chara.unlocked_aetherytes = ids.into_iter().collect(),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    actor = actor_id,
                    "load_character_aetherytes failed; starting with empty attunement set",
                );
            }
        }
        // Hotbar hydration — mirror the loaded equipped commands into
        // CharaState so `PlayerSnapshot::hotbar` reads from the live
        // registry-reachable state. EquipAbility/UnequipAbility/
        // SwapAbilities apply paths mutate this vec in-place.
        character.chara.hotbar = loaded.hotbar.clone();
        // Owned NPC linkshells — hydrated for the zone-in pearl re-emit
        // (Garlemald-Server #46). The PlayerSetNpcLs apply path keeps
        // the DB row authoritative; this mirror is read-only at zone-in.
        character.chara.npc_linkshells = loaded.npc_linkshells.clone();
        // Levequest journal hydration — mirror the loaded regional/local
        // guildleve slots into CharaState so the zone-in `/_init` bundle
        // re-emits them (previously loaded into LoadedPlayer, then dropped).
        character.chara.guildleves_local = loaded.guildleves_local.clone();
        character.chara.guildleves_regional = loaded.guildleves_regional.clone();
        // Equipped-title hydration — mirror `currentTitle` so the zone-in
        // bundle can emit `SetPlayerTitle`; the DB column already loaded into
        // `loaded.current_title` but was never applied to the runtime actor.
        character.chara.current_title = loaded.current_title;
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
                    row.counter4,
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

        // Per-class levels + skill points → in-memory battle_save (slot
        // = class id), so the `/_init` dump and the AddExp rollover see
        // persisted progression across relogs instead of a fresh
        // level-1/0-SP character. The active class's level also feeds
        // `chara.level` (the displayed level + stat-pipeline input) —
        // without this, tutorial EXP earned before a warp rendered as a
        // level-down to 1/0 at the next zone-in.
        match self.db.load_class_levels_and_exp(actor_id).await {
            Ok(save) => {
                // The DB-side struct and the runtime battle_save differ
                // in array length — copy the overlapping class-id slots.
                for (i, v) in save.skill_point.iter().enumerate() {
                    if let Some(slot) = character.battle_save.skill_point.get_mut(i) {
                        *slot = *v;
                    }
                }
                for (i, v) in save.skill_level.iter().enumerate() {
                    if let Some(slot) = character.battle_save.skill_level.get_mut(i) {
                        *slot = *v;
                    }
                }
                let active = if character.chara.current_job > 0 {
                    character.chara.current_job as usize
                } else {
                    character.chara.class.max(0) as usize
                };
                if let Some(lvl) = character.battle_save.skill_level.get(active).copied()
                    && lvl > 0
                {
                    character.chara.level = lvl;
                }
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    actor = actor_id,
                    "load_class_levels_and_exp failed; starting at level 1",
                );
            }
        }

        // Hydrate persisted status effects into the runtime container.
        // `load_character_status_effects` already filled
        // `loaded.status_effects`, but that Vec was previously dropped on the
        // floor — long-lived effects (food buffs, scripted quest effects)
        // vanished every relog. Re-apply through the same `add_status_effect`
        // path the runtime uses; the emitted events (gain toast, recalc) are
        // discarded because no client socket is attached yet and the zone-in
        // property bundle re-emits the `charaWork.status[]` /
        // `statusShownTime[]` arrays. Mirrors C# `Player.Load` re-applying the
        // `SavePlayerStatusEffects` rows.
        if !loaded.status_effects.is_empty() {
            let now_ms = common::utils::unix_timestamp() as u64 * 1000;
            let mut hydrate_outbox = crate::status::StatusOutbox::new();
            for entry in &loaded.status_effects {
                let mut effect = crate::status::StatusEffect::new(
                    actor_id,
                    entry.status_id,
                    entry.magnitude as f64,
                    entry.tick,
                    entry.duration,
                    entry.tier,
                    now_ms,
                );
                effect.extra = entry.extra as f64;
                // No login "you gain the effect of X" spam — the effects are
                // being restored, not freshly applied.
                effect.silent_on_gain = true;
                character.status_effects.add_status_effect(
                    effect,
                    actor_id,
                    now_ms,
                    crate::status::DEFAULT_GAIN_TEXT_ID,
                    &mut hydrate_outbox,
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
                .do_zone_change_with_private_area(
                    actor_id,
                    session_id,
                    zone_id,
                    login_private_area.clone(),
                    login_private_area_type,
                    spawn,
                    rotation,
                )
                .await
            {
                tracing::error!(error = %e, actor = actor_id, "zone change failed");
            } else {
                self.world
                    .send_zone_in_bundle(
                        &self.registry,
                        &self.db,
                        self.lua.as_ref(),
                        session_id,
                        0x1,
                        // Fresh-connection arrival — like login, retail
                        // sends no mass-delete trio (nothing to clean).
                        /* commit_keep_list */
                        false,
                    )
                    .await;
            }
        }

        let _ = client;
        Ok(())
    }

    async fn handle_session_end(&self, client: &ClientHandle, sub: &SubPacket) -> Result<()> {
        let session_id = sub.header.source_id;
        tracing::info!(session = session_id, "session end");
        // Purge the leaving player's parked coroutines BEFORE the
        // registry forgets the session → actor mapping. The scheduler's
        // park map is keyed by player actor id and was previously only
        // purged via ContentFinished, so a `_WAIT_EVENT` park (an NPC
        // talk mid-`callClientFunction`) survived teardown and fired
        // against post-relog state — the live Charlys→Hobriaut hijack
        // (a stale Charlys continuation resumed by a Hobriaut talk,
        // silently draining QuestIncCounter + AddGil(2000) + EndEvent).
        // Same `purge_owner` sweep the ContentFinished teardown uses
        // (runtime/quest_apply.rs). (Garlemald-Server #46.)
        if let Some(handle) = self.registry.by_session(session_id).await
            && let Some(lua) = self.lua.as_ref()
        {
            let purged = lua
                .scheduler()
                .lock()
                .map(|mut s| s.purge_owner(handle.actor_id))
                .unwrap_or(0);
            if purged > 0 {
                tracing::warn!(
                    session = session_id,
                    player = handle.actor_id,
                    purged_coroutines = purged,
                    "session end purged parked coroutines — a talk/cutscene was mid-flight at disconnect",
                );
            }
        }
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
    async fn handle_gm_handshake_ack(&self, client: &ClientHandle, session_id: u32) -> Result<()> {
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
        let (spawn, rotation, login_private_area, login_private_area_type) =
            if let Some(snap) = self.world.session(session_id).await {
                (
                    Vector3::new(snap.destination_x, snap.destination_y, snap.destination_z),
                    snap.destination_rot,
                    // Restored in handle_session_begin from the saved
                    // currentPrivateArea — route the login zone-in into the
                    // saved Echo / private-area instance so relogging inside
                    // one (e.g. man0l1's FSH-guild echo) lands the player back
                    // in it with its echo-only NPCs. (Garlemald-Server #46.)
                    snap.current_private_area_name.clone(),
                    snap.current_private_area_level,
                )
            } else {
                (Vector3::default(), 0.0, None, 0)
            };

        if let Err(e) = self
            .world
            .do_zone_change_with_private_area(
                actor_id,
                session_id,
                zone,
                login_private_area,
                login_private_area_type,
                spawn,
                rotation,
            )
            .await
        {
            tracing::error!(error = %e, actor = actor_id, "login zone change failed");
        } else {
            self.world
                .send_zone_in_bundle(
                    &self.registry,
                    &self.db,
                    self.lua.as_ref(),
                    session_id,
                    0x1,
                    // Login — retail's login.pcapng carries no
                    // mass-delete trio at all.
                    /* commit_keep_list */
                    false,
                )
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
            // `LockUpdates(false)`. No shipped zone.lua currently defines
            // an `onZoneIn` (the resolver's not-present branch below is
            // the normal path), but the hook is kept for content parity
            // with the C# pipeline.
            //
            // CAUTION (Garlemald-Server #25): do NOT re-add a
            // `KickEvent(..., "noticeEvent")` to a zone.lua onZoneIn. The
            // legacy `ocn0Battle02` zone.lua did exactly that, on the
            // belief that the bundle kick alone left "Now Loading" up —
            // a diagnosis that predates the director_actor_id +2 and
            // playerWork.questScenario fixes which made the bundle kick
            // land. With both kicks in place the Limsa client answered
            // with TWO EventStarts, the second parked onNotice coroutine
            // replaced the first, only one EndEvent went out, and the
            // never-closed first event left the client input-locked (the
            // SEQ_000 WASD softlock). Upstream's `quest_system` branch
            // deleted that zone.lua too (pmeteor 26fd79be); the single
            // bundle kick from player.lua onBeginLogin is sufficient,
            // as Gridania/Ul'dah always demonstrated.
            self.run_zone_on_zone_in_hook(&handle, session_id, zone)
                .await;
        }

        // Arm quest ENPC state on login / relog. A continuous playthrough sets
        // up each sequence's talk/push ENPC flags via StartSequence ->
        // onStateChange (and quest:UpdateENPCs on talk). But a RELOG loads the
        // quest at its saved sequence/counters WITHOUT a StartSequence, so
        // onStateChange never re-runs: the ENPC flags (e.g. man0l1 SEQ_007's
        // MSK_TRIGGER `pushDefault` + Isandorel-off at subseqMSK==1) are never
        // armed, AND `state.current` stays empty — so the is_quest_enpc guard
        // in the dispatcher's NPC release paths (`event/dispatcher.rs::
        // dispatch_npc_event_started`, inert + no-script releases) misfires
        // and sends a premature EndEvent out from under the first talk's
        // just-parked cutscene ("first talk did nothing"). Re-run
        // onStateChange for every active quest now that the zone-in NPCs are
        // spawned (find_npc_by_class_id can resolve them). apply_quest_update
        // _enpcs is idempotent — begin_sequence_swap + onStateChange + diff
        // broadcast + stale-clear — so a freshly-StartSequence'd opener (the
        // man0l0 boat) just re-derives the same state (no spurious re-broadcast
        // / clear). Mirrors Meteor re-establishing quest ENPC state per zone-in.
        // (Garlemald-Server #46 — SEQ_007 relog soft-lock + premature resume.)
        let active_quest_ids: Vec<u32> = {
            let c = handle.character.read().await;
            c.quest_journal
                .slots
                .iter()
                .flatten()
                .map(|q| q.quest_id())
                .collect()
        };
        for quest_id in active_quest_ids {
            self.apply_quest_update_enpcs(actor_id, quest_id).await;
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
                        crate::lua::command::LuaCommandArg::Nil => common::luaparam::LuaParam::Nil,
                        crate::lua::command::LuaCommandArg::ActorId(id) => {
                            common::luaparam::LuaParam::Actor(id)
                        }
                    })
                    .collect();
                // C# `Player.KickEvent` always uses event_type=5 (the
                // 2-arg Lua form and 3-arg form both land here); only
                // the rarely-used `KickEventSpecial` uses 0.
                let mut sub = crate::packets::send::events::build_kick_event(
                    player_id,
                    actor_id,
                    &trigger,
                    5,
                    &lua_params,
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

    /// Fire `zone.lua:onZoneIn` for `zone_id` against this player and
    /// drain the resulting commands through
    /// `apply_post_zone_in_lua_command`. Shared by the login arm (C#
    /// `WorldManager.DoZoneIn`'s tail, WorldManager.cs:1468) and the
    /// seamless-flip arm in `handle_update_position` (C#
    /// `DoSeamlessZoneChange`'s tail, WorldManager.cs:947 — pmeteor
    /// runs the DESTINATION zone's onZoneIn on every boundary flip;
    /// garlemald previously only ran it at login). No-op without a Lua
    /// engine or when the zone ships no zone.lua (the common case).
    /// (Garlemald-Server #46, round 4.)
    async fn run_zone_on_zone_in_hook(&self, handle: &ActorHandle, session_id: u32, zone_id: u32) {
        let Some(engine) = self.lua.as_ref() else {
            return;
        };
        let zone_name = match self.world.zone(zone_id).await {
            Some(z) => z.read().await.core.zone_name.clone(),
            None => String::new(),
        };
        if zone_name.is_empty() {
            return;
        }
        let zone_script = engine.resolver().zone(&zone_name);
        if !zone_script.exists() {
            tracing::debug!(
                path = %zone_script.display(),
                "zone.lua not present; skipping onZoneIn"
            );
            return;
        }
        let snapshot = {
            let c = handle.character.read().await;
            build_player_snapshot_for_login(&c)
        };
        let result = engine.call_player_hook_best_effort(&zone_script, "onZoneIn", snapshot);
        let cmd_count = result.commands.len();
        for cmd in result.commands {
            self.apply_post_zone_in_lua_command(handle, session_id, cmd)
                .await;
        }
        match result.error {
            None => tracing::info!(
                session = session_id,
                actor = handle.actor_id,
                zone = %zone_name,
                commands = cmd_count,
                "onZoneIn lua hook ran"
            ),
            Some(e) => tracing::warn!(
                error = %e,
                session = session_id,
                actor = handle.actor_id,
                zone = %zone_name,
                commands = cmd_count,
                "onZoneIn lua hook errored; applied partial commands"
            ),
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
                let director_local_id = (director_actor_id & 0x0007_FFFF).saturating_sub(2);
                if let Some(zone_arc) = self.world.zone(zone_actor_id).await {
                    let mut zone = zone_arc.write().await;
                    zone.core
                        .create_director_with_id(director_local_id, class_path.clone(), false);
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
                class_path,
                class_name,
            } => {
                {
                    let mut c = handle.character.write().await;
                    c.chara.login_director_actor_id = director_actor_id;
                }
                // Also rebuild session.login_director with the NEW
                // director's spec so the next zone-in bundle's
                // director-spawn packets reference this director (not
                // the stale OpeningDirector spec captured at
                // onBeginLogin). Smoke-revealed bug fixed in commit
                // (this) — previously the bundle would emit packets
                // for the OpeningDirector at every warp regardless
                // of mid-session SetLoginDirector calls.
                if !class_path.is_empty()
                    && director_actor_id != 0
                    && let Some(mut snap) = self.world.session(handle.session_id).await
                {
                    let zone_actor_id = snap.current_zone_id;
                    snap.login_director = Some(crate::data::LoginDirectorSpec {
                        actor_id: director_actor_id,
                        zone_actor_id,
                        class_path: class_path.clone(),
                        class_name: class_name.clone(),
                    });
                    self.world.upsert_session(snap).await;
                }
                tracing::info!(
                    player = player_id,
                    director = director_actor_id,
                    %class_path,
                    %class_name,
                    "SetLoginDirector applied (chara.login_director_actor_id + session.login_director both refreshed)"
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
                // Capture onto the session so `send_zone_in_bundle` can
                // emit the KickEventPacket at the END of the bundle —
                // AFTER the player + director + nearby NPCs have all been
                // re-spawned post-warp.
                //
                // Per meteor-decomp `event_kick_receiver_decomp.md`, the
                // client's `KickClientOrderEventReceiver::Receive` slot
                // 2 silently drops kicks whose owner actor either isn't
                // in the registry OR has `actor[+0x5c] == 0`. The flag is
                // set when the actor's spawn-packet sequence completes.
                // Sending the kick BEFORE the spawn (or before
                // `DeleteAllActors` wipes the actor list) loses it.
                //
                // C# `Player.KickEvent` always uses event_type=5 (the
                // 2-arg Lua form and 3-arg form both land here); only
                // the rarely-used `KickEventSpecial` uses 0. The
                // `actor_id` is the owner (the director). Args from the
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
                        crate::lua::command::LuaCommandArg::Nil => common::luaparam::LuaParam::Nil,
                        crate::lua::command::LuaCommandArg::ActorId(id) => {
                            common::luaparam::LuaParam::Actor(id)
                        }
                    })
                    .collect();
                if let Some(mut snap) = self.world.session(handle.session_id).await {
                    let kick = crate::data::PendingKickEvent {
                        trigger_actor_id: player_id,
                        owner_actor_id: actor_id,
                        event_name: trigger.clone(),
                        args: lua_params,
                    };
                    // #46 escort R1 — a kick captured AFTER the content
                    // warp already emitted its wipe pair in the same
                    // login-scoped burst (`reload_in_flight` set, client
                    // ack outstanding) has NO safe emitter left this
                    // drain: the zone-in bundle already flushed, and a
                    // bundle-end emission would ride AFTER
                    // DeleteAllActors — the wire-proven client-side drop
                    // (session 53943). Park it on the content slot; the
                    // client's RX 0x0007 content-warp ack releases it
                    // (`handle_zone_in_complete`). Kicks captured BEFORE
                    // any warp in the burst (the Baderon SEQ_003 staging,
                    // the SEQ_005 doExitDoor burst) see
                    // `reload_in_flight == false` and keep the proven
                    // `pending_kick_event` route unchanged.
                    if snap.reload_in_flight {
                        snap.pending_content_kick_event = Some(kick);
                        self.world.upsert_session(snap).await;
                        tracing::info!(
                            player = player_id,
                            target = actor_id,
                            %trigger,
                            "KickEvent parked until content-warp zone-in ack (RX 0x0007 — reload in flight)"
                        );
                    } else {
                        snap.pending_kick_event = Some(kick);
                        self.world.upsert_session(snap).await;
                        tracing::info!(
                            player = player_id,
                            target = actor_id,
                            %trigger,
                            "KickEvent captured (will emit at end of zone-in bundle, after all spawns)"
                        );
                    }
                }
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
            LC::DoClassChange {
                player_id,
                class_id,
            } => {
                self.apply_do_class_change(player_id, class_id).await;
            }
            LC::PrepareClassChange {
                player_id,
                class_id,
            } => {
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
                    player_id,
                    quest_id,
                    flag,
                    &self.registry,
                    &self.db,
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
                // Shared applier: DB persist + the currency-package
                // delta bracket and 25246 "You obtain" toast to the
                // owning client (Garlemald-Server #46).
                crate::runtime::quest_apply::apply_add_gil(
                    actor_id,
                    amount,
                    &self.registry,
                    Some(&self.world),
                    &self.db,
                )
                .await;
            }
            LC::EarnAchievement {
                actor_id,
                achievement_id,
                points,
            } => {
                // Shared applier: persist + earned toast + points/latest
                // re-sync through the achievement dispatcher.
                crate::runtime::quest_apply::apply_earn_achievement(
                    actor_id,
                    achievement_id,
                    points,
                    &self.registry,
                    &self.world,
                    &self.db,
                )
                .await;
            }
            LC::SetTitle { actor_id, title_id } => {
                crate::runtime::quest_apply::apply_set_title(
                    actor_id,
                    title_id,
                    &self.registry,
                    &self.world,
                    &self.db,
                )
                .await;
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
                    self.lua.as_ref(),
                    Some(&self.db),
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
            // through here. Persistence is direct-DB via `add_harvest_item`;
            // NORMAL adds now also emit a live no-wipe single-package
            // refresh to the owning client (see
            // `runtime::quest_apply::apply_add_item`), so the bag renders
            // the new stack mid-session without a re-zone. Non-NORMAL
            // packages (key items etc.) stay DB-only until their per-table
            // persistence lands.
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
                    &self.registry,
                    Some(&self.world),
                    &self.db,
                )
                .await;
            }
            LC::RemoveItem {
                actor_id,
                item_package,
                catalog_id,
                quantity,
            } => {
                crate::runtime::quest_apply::apply_remove_item(
                    actor_id,
                    item_package,
                    catalog_id,
                    quantity,
                    &self.registry,
                    Some(&self.world),
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
                    Some(&self.world),
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
                crate::runtime::quest_apply::apply_send_message(
                    actor_id,
                    message_type,
                    &sender,
                    &text,
                    &self.registry,
                    &self.world,
                )
                .await;
            }
            LC::SendGameMessage {
                actor_id,
                text_owner_id,
                text_id,
                log_type,
                params,
            } => {
                crate::runtime::quest_apply::apply_send_game_message(
                    actor_id,
                    text_owner_id,
                    text_id,
                    log_type,
                    &params,
                    &self.registry,
                    &self.world,
                )
                .await;
            }
            // The NPC-linkshell narration line, drained on the onNpcLS
            // fan-out (which routes through this login applier).
            // (Garlemald-Server #46 live test.)
            LC::SendGameMessageLocalizedDisplayName {
                player_id,
                text_owner_actor_id,
                text_id,
                log_type,
                display_id,
                params,
            } => {
                crate::runtime::quest_apply::apply_send_game_message_localized_display_name(
                    player_id,
                    text_owner_actor_id,
                    text_id,
                    log_type,
                    display_id,
                    &params,
                    &self.registry,
                    &self.world,
                )
                .await;
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
            // First-touch aetheryte attunement — the live path is the
            // runtime drain (aetheryte touches are NPC events), but the
            // login applier mirrors the arm for hook symmetry, matching
            // how `SendMessage` / `SetHomePoint` exist on both paths.
            // (Garlemald-Server #46, round 5.)
            LC::UnlockAetheryte {
                player_id,
                aetheryte_id,
            } => {
                crate::runtime::quest_apply::apply_unlock_aetheryte(
                    player_id,
                    aetheryte_id,
                    &self.registry,
                    &self.db,
                    &self.world,
                )
                .await;
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
                self.apply_rename_retainer(player_id, retainer_id, new_name)
                    .await;
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
            LC::StartDream {
                player_id,
                dream_id,
            } => {
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
                self.apply_issue_chocobo(player_id, appearance_id, name)
                    .await;
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
            LC::SetGCRank {
                player_id,
                gc,
                rank,
            } => {
                self.apply_set_gc_rank(player_id, gc, rank).await;
            }
            LC::AddSeals {
                player_id,
                gc,
                amount,
            } => {
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
                crate::runtime::quest_apply::apply_content_finished(
                    parent_zone_id,
                    &area_name,
                    &self.registry,
                    &self.world,
                    self.lua.as_ref(),
                )
                .await;
            }
            // Event-flavoured commands (`RunEventFunction`, `EndEvent`)
            // emitted via the login-scoped pipeline get bridged through
            // the EventOutbox + dispatch_event_event. Mirrors
            // `fire_quest_event_hook` (processor.rs:5996+) so the SEQ_005
            // director-coroutine cinematic body actually reaches the wire
            // instead of dropping at this catch-all. (KickEvent has its
            // own arm above — captures onto session for post-warp
            // emission, so we don't include it here.)
            //
            // Why this exists: the man0g0 SEQ_005 cinematic (see pmeteor
            // `directors/Quest/QuestDirectorMan0g001.lua::onEventStarted`)
            // calls `callClientFunction(player, "delegateEvent", ...)`
            // followed by `player:EndEvent()` repeatedly to advance
            // through tutorial stages. Both translate to LuaCommand::
            // RunEventFunction / EndEvent on the server side. Before
            // this arm landed, they fell through to the silent
            // `tracing::debug!(?other, "login lua cmd (unhandled)")`
            // branch and the cinematic body never ran (post-warp byte-
            // diff vs pmeteor capture confirmed: pmeteor sends
            // 0x0130 RunEventFunction + 0x0131 EndEvent post-warp,
            // garlemald sent zero of either).
            //
            // The translator + dispatcher are already known-good — they
            // serve `fire_quest_event_hook` and `apply_quest_on_notice`.
            // This arm just plumbs the login-scoped path into the same
            // bridge.
            cmd @ (LC::RunEventFunction { .. } | LC::EndEvent { .. }) => {
                let player_id = handle.actor_id;
                let event_session_snapshot = {
                    let c = handle.character.read().await;
                    c.event_session.clone()
                };
                let mut outbox = crate::event::outbox::EventOutbox::new();
                crate::event::lua_bridge::translate_lua_commands_into_outbox(
                    std::slice::from_ref(&cmd),
                    &event_session_snapshot,
                    &mut outbox,
                );
                let drained: Vec<_> = outbox.drain();
                let event_count = drained.len();
                for e in drained {
                    Box::pin(crate::event::dispatcher::dispatch_event_event(
                        &e,
                        &self.registry,
                        &self.world,
                        &self.db,
                        self.lua.as_ref(),
                    ))
                    .await;
                }
                tracing::debug!(
                    player = player_id,
                    events = event_count,
                    cmd = ?cmd,
                    "login lua cmd routed via EventOutbox bridge",
                );
            }
            // Equip starting gear at login. `player.lua::equipClassItems`
            // (called from `onLogin`) does `player:GetEquipment():Set(...)`,
            // which pushes `EquipFromPackage`. The equip applier lives in the
            // runtime drain (`apply_runtime_lua_command`); the login drain
            // otherwise dropped this as "unhandled", so the class weapon was
            // never actually equipped (it sat in the bag) and a Gladiator could
            // not draw it → F / Active-mode stayed inert. Route it through the
            // runtime applier so the equip + 0x014E refresh actually run.
            // (Garlemald-Server #28.)
            cmd @ LC::EquipFromPackage { .. } => {
                crate::runtime::quest_apply::apply_runtime_lua_command(
                    cmd,
                    &self.registry,
                    &self.db,
                    &self.world,
                    self.lua.as_ref(),
                )
                .await;
            }
            // `player:SendDataPacket(n)` from a quest/NPC hook drained on
            // the login path — most importantly `endTutorialMode` =
            // SendDataPacket(7), fired from man0l1/man0g1 onNpcLS. The
            // runtime drain owns this command (apply_send_data_packet)
            // but the login drain had no arm, so the tutorial-mode exit
            // was silently dropped on the onNpcLS fan-out, leaving the
            // client masked. (Garlemald-Server #46 live test.)
            cmd @ LC::SendDataPacket { .. } => {
                crate::runtime::quest_apply::apply_runtime_lua_command(
                    cmd,
                    &self.registry,
                    &self.db,
                    &self.world,
                    self.lua.as_ref(),
                )
                .await;
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
            snap.active_content_script = Some(crate::data::ActiveContentScript {
                parent_zone_id,
                area_name: area_name.clone(),
                area_class_path: area_class_path.clone(),
                director_name: director_name.clone(),
                director_actor_id,
                content_area_actor_id,
                content_script: content_script.clone(),
                // Pre-warp window: content NPCs not yet AddActor'd on the
                // client; roster broadcasts stay suppressed until
                // `apply_do_zone_change_content` finishes the warp bundle.
                warp_complete: false,
                // Filled in by the spawn appliers as onCreate's spawns
                // land; consumed by the ContentFinished teardown.
                spawned_actor_ids: Vec::new(),
            });
            // Re-arm the content-warp ack gate for THIS instance. The
            // field doc always promised "reset to false when a new
            // content warp dispatches" but no code performed it, so a
            // RETRY entry (escort duty failed → SEQ_048 rollback →
            // Zephyr push again) inherited the previous instance's
            // `true`: the ticker's onUpdate driver ran during the
            // pre-warp window (driving escort NPCs at a client
            // mid-transition) and the pre-ack KickEvent park window
            // never opened. First-time flows are unchanged (the flag
            // is already false). (#46 escort R1.)
            snap.content_warp_acked = false;
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
            // C2b — onCreate doesn't iterate area rosters; B1 will
            // populate them on subsequent onUpdate ticks via the
            // ticker path.
            players: Vec::new(),
            allies: Vec::new(),
            monsters: Vec::new(),
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
            //
            // PartyAddMember is coalesced (#46 round 5): roster
            // updates apply per-command, but the GroupHeader/Begin/
            // X08/End trio is emitted ONCE per leader after the loop
            // — a script pass adding several allies used to ship one
            // trio per member, and the intermediate-roster trios have
            // no retail analogue (retail emits exactly one trio per
            // composition, under a fresh group id).
            let mut runtime_cmds = Vec::with_capacity(partial.commands.len());
            let mut party_trio_leaders: Vec<u32> = Vec::new();
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
                    crate::lua::command::LuaCommand::SpawnActor {
                        zone_id: sz,
                        actor_class_id,
                        unique_id,
                        x,
                        y,
                        z,
                        rotation,
                        expected_actor_id,
                    } => {
                        // Content-area SpawnActor — needs db lookups
                        // (actor_class + appearance) that the runtime
                        // applier can't reach. Same partition rationale
                        // as SpawnBattleNpcById above.
                        self.apply_spawn_actor(
                            sz,
                            actor_class_id,
                            unique_id,
                            x,
                            y,
                            z,
                            rotation,
                            expected_actor_id,
                        )
                        .await;
                    }
                    crate::lua::command::LuaCommand::PartyAddMember {
                        leader_actor_id,
                        member_actor_id,
                    } => {
                        crate::runtime::quest_apply::apply_party_add_member_roster(
                            leader_actor_id,
                            member_actor_id,
                            &self.registry,
                            &self.world,
                        )
                        .await;
                        if !party_trio_leaders.contains(&leader_actor_id) {
                            party_trio_leaders.push(leader_actor_id);
                        }
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
            // One trio per leader for the batch's final composition
            // (see the coalescing note above the partition loop).
            for leader_actor_id in party_trio_leaders {
                crate::runtime::quest_apply::emit_party_group_trio(
                    leader_actor_id,
                    &self.registry,
                    &self.world,
                )
                .await;
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

        // NOTE: the content director's spawn-packet emission lives in
        // `send_zone_in_bundle` (reads `session.active_content_script`),
        // NOT here. Earlier iteration tried to emit them at content-area
        // creation time but that fired BEFORE the warp completed and
        // double-spawned for content scripts that also call
        // `SetLoginDirector(director)` (e.g. man0g0's Quest director),
        // crashing Wine. Deferring to `send_zone_in_bundle` lets the
        // existing defensive guard (`if Some(active.director_actor_id)
        // != login_director_spec.as_ref().map(|s| s.actor_id)`) skip
        // the duplicate emission cleanly.
    }

    /// B1 of the SEQ_005 unblock plan — port of the C# in
    /// `Map Server/WorldManager.cs:514 SpawnBattleNpcById`. Joins the
    /// four `server_battlenpc_*` seed tables on `bnpc_id`, materialises
    /// a `BattleNpc` actor under the parent zone's actor list at the
    /// caller-pre-computed actor id, and broadcasts the spawn-bundle
    /// trio to nearby players via
    /// `runtime::dispatcher::spawn_bundle_fanout`.
    ///
    /// Phase B1 simplifications, partially closed by Phase C1 (combat AI
    /// landing 2026-05-14):
    ///   * No private-area instance isolation — the actor lands in the
    ///     parent zone's actor list. (Phase B5 wires in `PrivateAreaContent`.)
    ///   * ✅ C1: detection_type / neutral / kindred_type / respawn /
    ///     drop_list applied from the joined DTO; a `Controller`
    ///     (BattleNpc or Ally kind based on allegiance) is attached to
    ///     the actor's `AIContainer` so the AI tick loop can drive
    ///     aggro and target acquisition. Mob-mod summer (pool / genus /
    ///     spawn rows) still deferred.
    ///   * No `script_name`-driven Lua-side combat AI — the controller
    ///     drives detection / engagement, but scripted overrides (e.g.
    ///     `bloodthirsty_wolf.lua` custom AI hooks) aren't wired yet.
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
        // Give every script-spawned BattleNpc real HP. The seed `hp` column is
        // 0 for many groups (incl. the SEQ_005 tutorial wolves/Yda/Papalymo),
        // and garlemald's spawn path — unlike pmeteor's CalculateBaseStats() —
        // never derives HP from genus/level, so those actors would spawn at
        // hp=0 → `is_dead()` → rendered downed and unable to fight. When the
        // seed gives no HP, fall back to a level-scaled default so the actor is
        // alive. (Garlemald-Server #28; pmeteor parity TODO: full stat derive.)
        let resolved_hp: u32 = if spawn.hp > 0 {
            spawn.hp
        } else {
            // `saturating_mul` guards against a malformed seed row with an
            // absurd `min_level` overflowing u32 (panic in debug / wrap in
            // release); the result is clamped to i16 below regardless.
            spawn.min_level.max(1).saturating_mul(100).max(100)
        };
        bnpc.npc.character.chara.hp = resolved_hp.min(i16::MAX as u32) as i16;
        bnpc.npc.character.chara.max_hp = resolved_hp.min(i16::MAX as u32) as i16;
        if spawn.mp > 0 {
            bnpc.npc.character.chara.mp = spawn.mp.min(i16::MAX as u32) as i16;
            bnpc.npc.character.chara.max_mp = spawn.mp.min(i16::MAX as u32) as i16;
        }
        bnpc.npc.character.chara.level = spawn.min_level.clamp(1, i16::MAX as u32) as i16;

        // Stamp the visual model + equipment from `gamedata_actor_appearance`,
        // exactly as the populace `apply_spawn_actor` path does. Without this
        // the BattleNpc keeps the constructor defaults (model_id = 0, all 28
        // appearance slots = 0), so the zone-in 0x00D6 SetActorAppearance ships
        // an empty model: creature mobs (wolves) have no mesh to draw and are
        // invisible, and humanoid allies (Yda/Papalymo) fall back to an empty
        // skeleton that renders as a knocked-out/downed pose. (Garlemald #28.)
        if let Ok(Some(app)) = self.db.load_npc_appearance(spawn.actor_class_id).await {
            let (model_id, slots) = app.pack();
            bnpc.npc.character.chara.model_id = model_id;
            bnpc.npc.character.chara.appearance_ids = slots;
        }

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

        // 4b. Phase C1 — apply combat metadata from the joined DTO
        //     (detection / neutral / kindred / pool / genus / drop-list /
        //     respawn) and attach the AI Controller so the ticker drives
        //     aggro and engagement. See `BattleNpc::apply_spawn_metadata`
        //     for the full per-field mapping + the Meteor parity notes.
        //
        //     #28 S2.4 — pool-job casters (currentJob 22 THM / 23 CNJ)
        //     get their loop spell pre-resolved out of the boot-time
        //     battle-command catalog. The seed pools carry no spellListId,
        //     so the tutorial caster (Papalymo) uses THM lvl-1 `thunder`
        //     27313 (range 20, cast 2000 ms, recast 6 s) — report C §5.
        const CASTER_DEFAULT_SPELL_ID: u16 = 27313;
        let caster_spell = if matches!(spawn.current_job, 22 | 23) {
            self.lua
                .as_ref()
                .and_then(|l| {
                    l.catalogs()
                        .battle_commands
                        .read()
                        .ok()
                        .and_then(|m| m.get(&CASTER_DEFAULT_SPELL_ID).cloned())
                })
                .map(|gd| gd.to_battle_command())
        } else {
            None
        };
        if matches!(spawn.current_job, 22 | 23) && caster_spell.is_none() {
            tracing::warn!(
                bnpc_id,
                spell_id = CASTER_DEFAULT_SPELL_ID,
                "SpawnBattleNpcById: caster pool job but spell not in catalog — \
                 caster will stand back without casting",
            );
        }
        bnpc.apply_spawn_metadata(&spawn, bnpc_id, actor_id, caster_spell);

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
            let spawn_pos =
                common::Vector3::new(spawn.position_x, spawn.position_y, spawn.position_z);
            zone.core.add_actor(
                crate::zone::area::StoredActor {
                    actor_id,
                    kind: crate::zone::area::ActorKind::BattleNpc,
                    position: spawn_pos,
                    grid: (0, 0),
                    is_alive: true,
                },
                &mut ob,
            );
            // Project into the correct spatial-grid cell. `add_actor` stores
            // grid (0,0); `actors_around` / `broadcast_around_actor` read the
            // grid cell, not the raw position — so without this the NPC sits in
            // cell (0,0) and the post-warp content reveal's spawn_bundle_fanout
            // finds no nearby player and the NPC never renders (until its first
            // MoveActorToPosition happens to fix the cell). (Garlemald-Server #46.)
            zone.core
                .update_actor_position(actor_id, spawn_pos, &mut ob);
        }

        // 6. Register the live Character in the ActorRegistry.
        let mut character = bnpc.npc.character.clone();
        // #28 S0.5 — content-spawned NPCs are one-shots: opt them out of
        // the ticker's default 30 s BNpc respawn. A tutorial wolf
        // respawning at full HP mid-fight would keep the all-wolves-dead
        // gate from ever firing (seed `respawnTime` is 0 for all five
        // anyway). Detection: the spawn fires from a content script's
        // onCreate, which runs after `active_content_script` is set on
        // the owning session for this parent zone.
        let content_spawn = {
            let mut found = false;
            for mut snap in self.world.all_sessions().await {
                if let Some(active) = snap.active_content_script.as_mut()
                    && active.parent_zone_id == parent_zone_id
                {
                    // #28 S1.3 — record the spawn on the owning content so
                    // the ContentFinished teardown despawns actors the
                    // script never AddMember'd into the director roster.
                    if !active.spawned_actor_ids.contains(&actor_id) {
                        active.spawned_actor_ids.push(actor_id);
                        self.world.upsert_session(snap).await;
                    }
                    found = true;
                    break;
                }
            }
            found
        };
        character.chara.respawn_disabled = content_spawn;
        // Phase C1/C2b — tag ally-allegiance BNpcs as `Ally` in the
        // registry. C# Meteor instantiates `Ally` as a separate
        // subclass of `BattleNpc` (`Map Server/Actors/Chara/Npc/Ally.cs`);
        // garlemald folds it into the same struct + Controller kind,
        // but downstream classifiers (the content-area `GetAllies`
        // partition, future ally-only logic) need the tag to read
        // intent without locking the Character to peek at the
        // controller.
        let kind_tag = if spawn.allegiance == 1 {
            crate::runtime::actor_registry::ActorKindTag::Ally
        } else {
            crate::runtime::actor_registry::ActorKindTag::BattleNpc
        };
        self.registry
            .insert(crate::runtime::actor_registry::ActorHandle::new(
                actor_id,
                kind_tag,
                parent_zone_id,
                /* session */ 0,
                character,
            ))
            .await;

        // 7. SKIP the immediate spawn_bundle_fanout for content-area
        //    spawns. The 5 NPCs (yda/papalymo/3 mobs) spawn during
        //    SimpleContent30010.lua's onCreate, which fires BEFORE
        //    the player warps. Broadcasting AddActor + 9 follow-up
        //    state packets per NPC into the player's CURRENT zone
        //    view (50 packets total) leaks the content-area NPCs
        //    into the public Gridania area where the player still
        //    is.
        //
        //    Pmeteor's reference capture
        //    (`captures/pmeteor-quest/20260426-160210-gridania-manual3/`,
        //    line 31753..31928 = the SEQ_005 warp window) shows
        //    ZERO actor-spawn packets pre-kick: pmeteor's
        //    `contentArea:SpawnActor` only adds the actor to the
        //    content-area's data structure, deferring wire emission
        //    until the player warps in. Garlemald previously fanned
        //    out 6 spawn bursts pre-kick (5 NPCs + a director) on
        //    top of the kick — the pre-kick burst diff vs pmeteor
        //    is the ONE remaining structural delta after fixing the
        //    duplicate-kick + content-trio + KickEvent-field bugs.
        //
        //    Hypothesis under test: those leaked spawns are why the
        //    post-warp kick (`OUT 0x012F` at the right bytes)
        //    silently no-ops in the client and never echoes
        //    `IN 0x012D EventStart`. Skip the broadcast — keep the
        //    actor in the registry (so AI / engagement / combat
        //    work) but stop polluting the client's content-area
        //    state machine.
        //
        //    Trade-off: until a deferred-emit-on-zone-in path is
        //    wired, the client won't render the NPCs visually
        //    post-warp. Acceptable for the kick-test; the cinematic
        //    body fires from `delegateEvent processTtrBtl001` which
        //    is purely client-side rendering.
        let _ = zone_arc; // keep handle so registry insert works
        let _ = spawn.script_name.clone(); // suppress unused warning
        // crate::runtime::dispatcher::spawn_bundle_fanout(
        //     &self.world,
        //     &self.registry,
        //     &zone_arc,
        //     parent_zone_id,
        //     actor_id,
        // )
        // .await;

        tracing::info!(
            bnpc_id,
            parent_zone = parent_zone_id,
            actor_id = format!("0x{:08X}", actor_id),
            actor_class_id = spawn.actor_class_id,
            allegiance = spawn.allegiance,
            "SpawnBattleNpcById applied — wire fan-out SKIPPED to keep \
             content-area NPCs out of the player's pre-warp view",
        );
    }

    /// Port of C# `Area::SpawnActor(classId, uniqueId, x, y, z, rot)`
    /// (`Map Server/Actors/Area/Area.cs:528`). Loads the actor class +
    /// appearance from the gamedata tables, constructs a populace
    /// `Npc` at `expected_actor_id` (deterministic from the Lua side),
    /// inserts into the parent zone's spatial grid + actor registry,
    /// and lets the next `send_zone_in_bundle` fan its 10-packet spawn
    /// bundle to the post-warp player via the standard NPC neighbour
    /// loop.
    ///
    /// Used by content-area `onCreate` scripts (e.g.
    /// `scripts/lua/content/SimpleContent30010.lua:30` spawning the
    /// SEQ_005 "openingstoper" event-trigger actor at actor_id
    /// `0x40080006` — verified against pmeteor's reference capture
    /// `captures/pmeteor-quest/20260426-160210-gridania-manual3/`).
    ///
    /// Like `apply_spawn_battle_npc_by_id`, this SKIPS the immediate
    /// `spawn_bundle_fanout` for content-area spawns — those NPCs are
    /// spawned pre-warp during `onCreate` but the player hasn't warped
    /// into the content area yet, so fanning their spawn bundle into
    /// the player's current (public) view would leak the content-area
    /// NPCs. The post-warp `send_zone_in_bundle` picks them up via
    /// `actors_around(50.0)` and emits the bundle then.
    #[allow(clippy::too_many_arguments)]
    async fn apply_spawn_actor(
        &self,
        zone_id: u32,
        actor_class_id: u32,
        unique_id: String,
        x: f32,
        y: f32,
        z: f32,
        rotation: f32,
        expected_actor_id: u32,
    ) {
        // 1. Resolve the actor class row — required for the
        //    AddActor + ScriptBind bundle.
        let actor_class = match self.db.load_actor_class(actor_class_id).await {
            Ok(Some(c)) => c,
            Ok(None) => {
                tracing::warn!(
                    actor_class_id,
                    zone = zone_id,
                    unique_id = %unique_id,
                    "SpawnActor: actor_class not in gamedata_actor_class",
                );
                return;
            }
            Err(e) => {
                tracing::warn!(
                    actor_class_id,
                    error = %e,
                    "SpawnActor: actor_class load failed",
                );
                return;
            }
        };

        // 2. Round-trip the actor_number out of the Lua-side composite
        //    formula `(4 << 28) | (zone << 19) | ((actor_number + 5) & 0x7FFFF)`.
        //
        //    Garlemald's `Npc::new(actor_number, ...)` (npc/npc.rs)
        //    composes the actor id as `(4 << 28) | (zone << 19) |
        //    (actor_number & 0x7FFFF)` with NO `+ 5` ctor quirk —
        //    pmeteor's `+5` is folded into the Lua-side formula
        //    instead. So the actor_number we hand to Npc::new is the
        //    raw bottom 19 bits of the composite id, no subtraction.
        //
        //    Pre-fix this subtracted 5 on the apply side as well,
        //    producing `actor_id mismatch — Lua side computed
        //    differently expected="0x4536049C" actual="0x45360497"`
        //    in the SpawnActor log and the openingstoper actor was
        //    never registered (early-return on the mismatch check
        //    below).
        let raw_actor_number = expected_actor_id & 0x7FFFF;

        // 3. Build the Npc.
        let mut npc = crate::npc::npc::Npc::new(
            raw_actor_number,
            &actor_class,
            unique_id.clone(),
            zone_id,
            x,
            y,
            z,
            rotation,
            0,
            0,
            None,
        );

        // 4. Stamp model + appearance from gamedata_actor_appearance,
        //    matching the bulk spawner's behaviour. Missing rows leave
        //    the all-zero defaults; `SetActorAppearancePacket` will
        //    fire but with model_id=0 — fine for invisible event
        //    triggers like openingstoper.
        if let Ok(Some(app)) = self.db.load_npc_appearance(actor_class_id).await {
            let (model_id, slots) = app.pack();
            npc.character.chara.model_id = model_id;
            npc.character.chara.appearance_ids = slots;
        }

        let actor_id = npc.actor_id();
        if actor_id != expected_actor_id {
            tracing::warn!(
                expected = format!("0x{:08X}", expected_actor_id),
                actual = format!("0x{:08X}", actor_id),
                unique_id = %unique_id,
                "SpawnActor: actor_id mismatch — Lua side computed differently",
            );
            return;
        }
        npc.generate_actor_name(raw_actor_number);

        // 5. Insert spatial projection into the parent zone grid.
        let Some(zone_arc) = self.world.zone(zone_id).await else {
            tracing::warn!(
                zone = zone_id,
                actor_id = format!("0x{actor_id:08X}"),
                "SpawnActor: parent zone not loaded",
            );
            return;
        };
        {
            let mut zone = zone_arc.write().await;
            let mut ob = crate::zone::outbox::AreaOutbox::new();
            zone.core.add_actor(
                crate::zone::area::StoredActor {
                    actor_id,
                    kind: crate::zone::area::ActorKind::Npc,
                    position: common::Vector3::new(x, y, z),
                    grid: (0, 0),
                    is_alive: true,
                },
                &mut ob,
            );
        }

        // 6. Register the live Character in the ActorRegistry.
        let character = npc.character.clone();
        self.registry
            .insert(crate::runtime::actor_registry::ActorHandle::new(
                actor_id,
                crate::runtime::actor_registry::ActorKindTag::Npc,
                zone_id,
                /* session */ 0,
                character,
            ))
            .await;

        // 6b. #28 S1.3 — content-area spawns (openingstoper et al.) are
        //     recorded on the owning session's ActiveContentScript so the
        //     ContentFinished teardown can despawn them: the director
        //     roster alone misses SpawnActor'd trigger NPCs the script
        //     never AddMember'd.
        for mut snap in self.world.all_sessions().await {
            if let Some(active) = snap.active_content_script.as_mut()
                && active.parent_zone_id == zone_id
            {
                if !active.spawned_actor_ids.contains(&actor_id) {
                    active.spawned_actor_ids.push(actor_id);
                    self.world.upsert_session(snap).await;
                }
                break;
            }
        }

        // 7. SKIP immediate spawn_bundle_fanout. Same reasoning as
        //    apply_spawn_battle_npc_by_id: the content-area spawns
        //    happen pre-warp during onCreate. The player still sits in
        //    the public-area view at this point; fanning AddActor +
        //    follow-ups would leak the trigger into the wrong view.
        //    Post-warp `send_zone_in_bundle` picks the actor up via
        //    `actors_around(50.0)` and emits the standard NPC spawn
        //    bundle (including event-condition packets if the class
        //    has parsed `eventConditions`).
        let _ = zone_arc;

        tracing::info!(
            zone = zone_id,
            actor_id = format!("0x{:08X}", actor_id),
            actor_class_id,
            unique_id = %unique_id,
            pos = ?(x, y, z),
            "SpawnActor applied — actor inserted into zone + registry, \
             spawn bundle deferred to post-warp send_zone_in_bundle",
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

        // Append to the per-director roster on Session. NO PER-CALL
        // GROUP-TRIO BROADCAST — pmeteor's `ContentGroup.AddMember`
        // (Map Server/Actors/Group/ContentGroup.cs:67-78) only
        // re-emits the GroupHeader/Begin/X/End trio when
        // `isStarted == true` (set by `Start()`, called via
        // `StartContentGroup`). Per the pmeteor pcap byte-diff
        // (2026-05-15, captures/pmeteor-quest/20260426-160210-
        // gridania-manual3/), the SEQ_005 onCreate's seven AddMember
        // calls produce ZERO group trio packets pre-warp; the trio is
        // only emitted ONCE by the pre-warp `apply_do_zone_change_content`
        // batch. Garlemald was emitting the trio per AddMember
        // call (8 trios total), and the 1.x client's content-group
        // state machine apparently resets on each GroupHeader — so
        // the rapid-fire trios churned the state machine and the
        // post-warp KickEvent silently dropped because the client's
        // group state was never stable.
        //
        // The roster update is the only side effect here. The pre-warp
        // emission in `apply_do_zone_change_content` reads
        // `transient_director_members[director_actor_id]` to build the
        // single trio it sends — that's where the wire emission lives.
        let roster_len = {
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
            let len = entry.len();
            self.world.upsert_session(snap).await;
            len
        };

        tracing::info!(
            director = format!("0x{director_actor_id:08X}"),
            member = format!("0x{member_actor_id:08X}"),
            roster = roster_len,
            "DirectorAddMember applied (roster updated; trio deferred to pre-warp emission)",
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
        // Unused since the same-map (0x16) escort reveal branch — the only
        // consumer of this id — was removed; kept in the signature to mirror
        // C# `WorldManager.DoZoneChangeContent`. (Garlemald-Server #46.)
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
        //    Also purge LOSE_ON_ZONING status effects — content-area
        //    transitions are loading-screen warps from the client's
        //    perspective and lose the same buff set retail's regular
        //    zone changes drop.
        let mut status_outbox = crate::status::StatusOutbox::new();
        {
            let mut c = handle.character.write().await;
            c.base.position_x = x;
            c.base.position_y = y;
            c.base.position_z = z;
            c.base.rotation = rotation;
            c.base.zone_id = parent_zone_id;
            c.status_effects.remove_by_flag(
                crate::status::StatusEffectFlags::LOSE_ON_ZONING,
                &mut status_outbox,
            );
        }
        self.drain_status_outbox(status_outbox).await;

        // 2. Run the SAME zone-change migration helper that
        //    `apply_do_zone_change` (cross-zone) uses. Both wipe-+-
        //    rebuild flows need it. For a same-zone same-private-area
        //    warp (no private_area name, parent_zone_id unchanged), the
        //    helper's branches fall through: `zone_changed = false`,
        //    `private_area_changed = false`, so neither of the
        //    remove branches fires; the dest-add re-inserts the actor
        //    at the new spawn coords with grid=(0,0). The session's
        //    `is_updates_locked = true / false` pair brackets the
        //    operation, suppressing any in-flight broadcast paths
        //    that might race with the warp.
        //
        //    Without this step the client never echoes `RX 0x0007
        //    zone-in-complete` and the loading screen hangs forever
        //    after the second-Yda-talk warp — the cross-zone path
        //    works because it goes through this helper.
        let spawn = Vector3::new(x, y, z);
        if let Err(e) = self
            .world
            .do_zone_change_with_private_area(
                actor_id,
                session_id,
                parent_zone_id,
                /* private_area_name */ None,
                /* private_area_level */ 0,
                spawn,
                rotation,
            )
            .await
        {
            tracing::error!(
                error = %e,
                player = player_id,
                zone = parent_zone_id,
                "DoZoneChangeContent: do_zone_change_with_private_area failed",
            );
            return;
        }

        // Project the PLAYER into the correct spatial-grid cell. The helper
        // re-inserts the player at grid (0,0); broadcast_around_actor (the
        // post-warp content reveal) keys on the grid CELL, so without this the
        // player sits in cell (0,0) while the escort NPCs are at the gate, and
        // the reveal's spawn_bundle_fanout finds no nearby player → the NPCs
        // never render. Normal warps self-correct via the deferred bundle / the
        // player's first position update, but the instant spawnType-0x16 warp
        // reveals immediately on RX 0x0007, before that happens. (Garlemald #46.)
        if let Some(zone_arc) = self.world.zone(parent_zone_id).await {
            let mut zw = zone_arc.write().await;
            let mut ob = crate::zone::outbox::AreaOutbox::new();
            zw.core.update_actor_position(actor_id, spawn, &mut ob);
        }

        // Rebind the player's REGISTRY handle zone to the content zone.
        // The per-frame combat ticker filters actors by the frozen
        // `ActorHandle.zone_id` (`actor_registry::actors_in_zone`, driven
        // by `ticker::tick_zone`) and hands `ai_container::update` only
        // THAT zone as its `ActorArena`. We already wrote
        // `Character.base.zone_id = parent_zone_id` and migrated the
        // spatial grid above, but the registry handle is still pinned to
        // wherever the player last crossed a seamless boundary. Without
        // this rebind the player is ticked in the wrong zone's arena, so
        // `arena.get(<content mob>)` returns `None`, `in_range` is false,
        // and every auto-attack swing is dropped (`ai_container.rs`
        // "swing ready but skipped … target_in_arena=false") → no TP →
        // "active mode" errors → the killing blow is never credited to the
        // player → no EXP. Mirrors the seamless path's `reassign_zone`
        // call on `SeamlessResult::ZoneChanged`. (Garlemald-Server #199.)
        self.registry.reassign_zone(actor_id, parent_zone_id).await;

        // 3. Update the session's spawn_type / destination fields. The
        //    helper above set zone + xyz/rot but not the spawn_type
        //    arg the bundle uses. Also latch `reload_in_flight` — the
        //    content warp is the other immediate wipe+0x10 emitter
        //    (same shape as `quest_apply::apply_do_zone_change`'s
        //    resident-geometry branch), so a stale pre-Now-Loading
        //    0x00CA would otherwise overwrite the warped position /
        //    stream phantom partner-zone NPCs. Cleared by the client's
        //    RX 0x0007 zone-in-complete. (Garlemald-Server #46, round 4.)
        if let Some(mut snap) = self.world.session(session_id).await {
            snap.destination_spawn_type = spawn_type;
            snap.reload_in_flight = true;
            self.world.upsert_session(snap).await;
        }

        // 3. Emit the zone-change packet trio. Order matters: client
        //    expects the world wipe first, then the 0x00E2 marker, then
        //    the zone-in payload.
        let Some(client) = self.world.client(session_id).await else {
            tracing::warn!(player = player_id, "DoZoneChangeContent: no client");
            return;
        };

        // 3a. PRE-WARP emission: content group trio + KickEvent.
        //
        // Pmeteor's reference capture (`captures/pmeteor-quest/
        // 20260426-160210-gridania-manual3/`) shows the SEQ_005 warp
        // bundle in this order at offset 15:54:31.108:
        //
        //   [13] OUT 0x017c GroupHeader        (content group, content director's roster)
        //   [14] OUT 0x017d GroupBegin
        //   [15] OUT 0x0183 ContentMembersX08
        //   [16] OUT 0x017e GroupEnd
        //   [17] OUT 0x012f KickEvent          ← KICK fires HERE (pre-warp)
        //   [18] OUT 0x0166 text-sheet
        //   [19] OUT 0x0007 DeleteAllActors    ← WARP starts
        //   [20] OUT 0x00e2 warp marker
        //
        // The client buffers the kick during warp processing and echoes
        // back IN 0x012D EventStart for the content director ~2.28
        // seconds later (verified at line 33975 of the pcap), which
        // then triggers the cinematic body server-side.
        //
        // Garlemald previously emitted the KickEvent at the END of the
        // post-warp zone-in bundle on the theory that the client gates
        // KickEvent on `actor[+0x5c] != 0` (per meteor-decomp note) —
        // but pmeteor's working flow disproves that ordering: the kick
        // can target an actor the client doesn't have spawned yet, as
        // long as the GROUP roster (017C/D/F/E trio) is registered
        // beforehand. The trio survives DeleteAllActors (groups are
        // separate from the actor list), and the kick gets resolved
        // post-warp once the spawn bundle materialises the new actors.
        let mut emitted_pre_warp_kick = false;
        if let Some(snap) = self.world.session(session_id).await
            && let Some(active) = snap.active_content_script.as_ref()
            && let Some(kick) = snap.pending_kick_event.as_ref()
        {
            // Only flip the order when the kick targets THIS content
            // director (ie this is the SEQ_005-style flow). For other
            // warps with no captured kick (or a kick targeting a
            // different actor), keep the post-warp ordering — those
            // were already working under the deferred-emission model.
            if kick.owner_actor_id == active.director_actor_id {
                // Pmeteor's content-group id is `0x3000_0000_0000_0000 |
                // counter` (WorldManager.cs:1077: `groupIndexId =
                // groupIndexId | 0x3000000000000000;` followed by
                // `groupIndexId++` per group). The high nibble 0x3 is
                // the content-group bitfield marker; the lower 60 bits
                // are a per-server-process counter starting at 1.
                //
                // Garlemald previously used `director_actor_id as u64`
                // (= 0x65300003 for the SEQ_005 director) which has the
                // wrong bitfield prefix entirely (0x6 = director, not
                // 0x3 = content group). The 1.x client uses the high
                // nibble to dispatch the group via the right index
                // table — wrong prefix → wrong table → group invisible
                // to the client's content-group state.
                //
                // For the SEQ_005 case we hard-code counter=1 (only
                // ever one content group active at a time per session
                // in the tutorial flow). A real registry of per-session
                // group counters lives elsewhere (transient_director_members
                // is keyed by director_actor_id, so we don't yet have
                // a content-group counter).
                let group_index: u64 = 0x3000_0000_0000_0000u64 | 1u64;
                let location_code = parent_zone_id as u64;
                let sequence_id = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or_default();
                // Build the GroupMember rows from the director's
                // transient roster (populated by DirectorAddMember
                // earlier in the doContentArea chain).
                let roster: Vec<u32> = snap
                    .transient_director_members
                    .get(&active.director_actor_id)
                    .cloned()
                    .unwrap_or_default();
                let mut members: Vec<crate::packets::send::groups::GroupMember> =
                    Vec::with_capacity(roster.len());
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
                // C# Group.cs:49 — `ContentGroup_SimpleContentGroup24B = 30006`,
                // which is the value `ContentGroup.GetTypeId()` returns and what
                // pmeteor sends in the GroupHeader's type_id field for the
                // SEQ_005 director's group (verified vs
                // captures/pmeteor-quest/20260426-160210-gridania-manual3/ at
                // line 31878 — bytes [0x50..0x53] = 0x00007536 = 30006).
                // Garlemald previously used 30001 which is `GuildleveGroup`
                // (Group.cs:44) — the wrong type — and the 1.x client
                // dispatched the group through the guildleve path instead of
                // the content path.
                const GROUP_TYPE_CONTENT_GROUP: u32 = 30006;
                let mut offset = 0usize;
                let pre_warp_subs = vec![
                    // SetActorProperty(charaWork.currentContentGroup =
                    // group_type_id) — port of pmeteor
                    // `Character.SetCurrentContentGroup(group)`
                    // (Map Server/Actors/Chara/Character.cs:207). Tells
                    // the client "your content group is now type 30006",
                    // which the kick receiver dispatches against. Without
                    // this, the client has no content-group context and
                    // the post-warp KickEvent silently drops at the
                    // dispatch table. Murmur2 hash of the dotted path
                    // matches pmeteor's bytes (verified at line 31866 of
                    // the SEQ_005 reference capture: `89 E1 F9 0D` LE =
                    // 0x0DF9E189).
                    crate::packets::send::actor::build_set_actor_property_u32(
                        actor_id,
                        "charaWork/currentContentGroup",
                        common::utils::murmur_hash2("charaWork.currentContentGroup", 0),
                        GROUP_TYPE_CONTENT_GROUP,
                    ),
                    crate::packets::send::groups::build_group_header(
                        actor_id,
                        location_code,
                        sequence_id,
                        group_index,
                        GROUP_TYPE_CONTENT_GROUP,
                        -1,
                        "",
                        members.len() as u32,
                    ),
                    crate::packets::send::groups::build_group_members_begin(
                        actor_id,
                        location_code,
                        sequence_id,
                        group_index,
                        members.len() as u32,
                    ),
                    crate::packets::send::groups::build_content_members_x08(
                        actor_id,
                        location_code,
                        sequence_id,
                        &members,
                        &mut offset,
                    ),
                    crate::packets::send::groups::build_group_members_end(
                        actor_id,
                        location_code,
                        sequence_id,
                        group_index,
                    ),
                    // Then the KickEvent itself, AFTER the group trio
                    // (matching pmeteor's [13..17] ordering).
                    crate::packets::send::events::build_kick_event(
                        kick.trigger_actor_id,
                        kick.owner_actor_id,
                        &kick.event_name,
                        5,
                        &kick.args,
                    ),
                ];
                for mut sub in pre_warp_subs {
                    sub.set_target_id(session_id);
                    client.send_bytes(sub.to_bytes()).await;
                }
                emitted_pre_warp_kick = true;
                tracing::info!(
                    player = player_id,
                    director = format!("0x{:08X}", active.director_actor_id),
                    roster = members.len(),
                    event = %kick.event_name,
                    "DoZoneChangeContent: emitted PRE-warp content group trio + KickEvent"
                );
            }
        }
        // Clear pending_kick_event so send_zone_in_bundle's end-of-bundle
        // emission doesn't re-fire it. Idempotent — a no-op if we didn't
        // pre-emit.
        if emitted_pre_warp_kick && let Some(mut snap) = self.world.session(session_id).await {
            snap.pending_kick_event = None;
            self.world.upsert_session(snap).await;
        }

        // #46 escort R1 — any pending kick the pre-warp branch did NOT
        // consume (owner ≠ this content director, or no active content
        // script) must not fall through to `send_zone_in_bundle`'s
        // end-of-bundle emission: on the CONTENT warp path that emission
        // lands AFTER DeleteAllActors in the same flush, and the client
        // silently drops a 0x012F whose owner it just wiped (wire-proven,
        // session 53943 — the director's onEventStarted never ran and the
        // escort duty could never complete). Move it to the post-ack slot;
        // the client's RX 0x0007 zone-in-complete releases it, by which
        // point the director actor rode the zone-in bundle and is
        // client-known. Login/normal-warp bundles are NOT affected — this
        // move happens only on the content-warp emitter.
        if !emitted_pre_warp_kick
            && let Some(mut snap) = self.world.session(session_id).await
            && let Some(kick) = snap.pending_kick_event.take()
        {
            tracing::info!(
                player = player_id,
                owner = format!("0x{:08X}", kick.owner_actor_id),
                event = %kick.event_name,
                "DoZoneChangeContent: pending kick re-parked for post-ack emission (RX 0x0007)"
            );
            snap.pending_content_kick_event = Some(kick);
            self.world.upsert_session(snap).await;
        }

        // pmeteor's content-warp burst opens with a "you have entered an
        // instance" system message (`SendGameMessage(WorldMaster, 34108,
        // 0x20)`, WorldManager.cs:999) immediately before DeleteAllActors.
        // Byte parity with the reference capture
        // (captures/pmeteor-quest/20260426-160210-gridania-manual3,
        // map-packets.log ~31938): header source AND body textOwner are both
        // WorldMaster 0x5FF80001 — the client dispatches game messages
        // through per-actor receivers keyed on the header source, so
        // sourcing this from the player would route it to the wrong
        // receiver class.
        {
            let mut msg = crate::packets::send::misc::build_text_sheet_no_source_x28(
                crate::packets::send::misc::WORLD_MASTER_ACTOR_ID,
                crate::packets::send::misc::WORLD_MASTER_ACTOR_ID,
                34108,
                0x20,
            );
            msg.set_target_id(session_id);
            client.send_bytes(msg.to_bytes()).await;
        }

        // The content-warp reload does a REAL scene reload: DeleteAllActors +
        // the 0x00E2(0x10) force-reload latch, then the zone-in bundle (which
        // carries the content NPCs). ALL live content warps take this single
        // path now — man0l0 deck tutorial, man0g0 SEQ_005, the man0u0 /
        // battle-zone triggers, and the man0l1 escort (re-homed cross-map to
        // zone 129 so the SetMap is a genuine off-disk load). The earlier
        // escort-only spawnType branches (7 = WARP_LIGHT teleport respawn,
        // 0x16 = same-map in-place reveal) were dead — no caller emits them
        // since the escort moved to the 0x10 cross-map warp — and were removed
        // (history: commit ebe7ecf / captures/issue28-rca/04-decomp-unlock.md).
        //
        // The decompiled client (FUN_0058cca0, case 0x00E2) sets the
        // MapLayoutElement force-reload latch [+0xbc]=1 for any subcode except
        // 0x15/0x16; the SetMap handler then schedules the reload. WITHOUT the
        // latch a same-region SetMap takes the no-op arm and "Now Loading"
        // hangs forever — exactly the man0l0 regression. Both subpackets MUST
        // be target_id-tagged or the world-server proxy drops them.
        // (Garlemald-Server #28/#46.)
        {
            let mut wipe = crate::packets::send::handshake::build_delete_all_actors(actor_id);
            wipe.set_target_id(session_id);
            client.send_bytes(wipe.to_bytes()).await;
            let mut e2 = crate::packets::send::handshake::build_0xe2(actor_id, 0x10);
            e2.set_target_id(session_id);
            client.send_bytes(e2.to_bytes()).await;
        }

        // Immediate bundle, commit_keep_list = false: the bare wipe above is
        // load-bearing for the same-zone content transition, so no trailing
        // keep-list commit is added on top (the pmeteor-verified shape). The
        // content NPCs ride this bundle.
        self.world
            .send_zone_in_bundle(
                &self.registry,
                &self.db,
                self.lua.as_ref(),
                session_id,
                spawn_type as u16,
                /* commit_keep_list */ false,
            )
            .await;

        // The bundle has now AddActor'd + state-synced the content NPCs on the
        // client; lift the pre-warp suppression of roster broadcasts
        // (`ActiveContentScript::warp_complete`) and unpark the onUpdate driver
        // (`content_warp_acked`). No NPC reveal is deferred to the client's
        // RX-0x0007 echo any more — that deferred-reveal path existed only for
        // the removed same-map (0x16) escort and was deleted with it.
        if let Some(mut snap) = self.world.session(session_id).await {
            if let Some(active) = snap.active_content_script.as_mut() {
                active.warp_complete = true;
            }
            snap.content_warp_acked = true;
            self.world.upsert_session(snap).await;
        }

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
                    // C2b — onZoneIn doesn't iterate area rosters
                    // (onUpdate does, via the ticker path).
                    players: Vec::new(),
                    allies: Vec::new(),
                    monsters: Vec::new(),
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

                // 6. Resume the parked director coroutine, if one was
                //    spawned pre-warp via `StartDirectorMain` and parked
                //    on a `coroutine.yield("_WAIT_EVENT", player)`.
                //
                //    Why this exists: man0g0 SEQ_005's pre-warp setup
                //    (`scripts/lua/quests/man/man0g0.lua::doContentArea`)
                //    runs:
                //
                //      contentArea:CreateContentArea(...)   -- spawns NPCs
                //      director:AddMember(starterPlayer)
                //      director:StartDirector(false)         -- spawns main coroutine
                //      player:KickEvent(director, "noticeEvent", true)
                //      GetWorldManager():DoZoneChangeContent(...)
                //
                //    The KickEvent emits a 0x012F packet pre-warp, which
                //    the client should normally echo back as 0x012F
                //    EventStart. `handle_event_start` (processor.rs:6630)
                //    then resumes the parked coroutine so it runs the
                //    cinematic body (`callClientFunction(player,
                //    "delegateEvent", ...)` → 0x0130 RunEventFunction +
                //    `player:EndEvent()` → 0x0131 EndEvent).
                //
                //    But the post-warp byte-diff vs pmeteor capture
                //    (captures/pmeteor-quest/20260426-160210-gridania-manual3/)
                //    shows the client NEVER echoes EventStart after the
                //    DoZoneChangeContent warp — so `handle_event_start`
                //    never fires, the coroutine sits parked, and the
                //    cinematic body never runs (verified: 0x0130 +
                //    0x0131 missing post-warp).
                //
                //    The fix is to drive the resume here, mirroring the
                //    "implicit EventStart" that pmeteor's
                //    `WorldManager.DoZoneChangeContent` final
                //    `LuaEngine.GetInstance().CallLuaFunction(player,
                //    contentArea, "onZoneIn", true)` produces — the
                //    onZoneIn hook above is the surface, and resuming
                //    the parked director coroutine is the side effect
                //    pmeteor's engine fires implicitly. We thread it
                //    explicitly here.
                //
                //    The resumed coroutine's `RunEventFunction` /
                //    `EndEvent` commands flow through
                //    `apply_runtime_lua_commands` →
                //    `apply_login_lua_command`'s EventOutbox bridge
                //    arm (commit `8de33cd`), which dispatches them to
                //    the wire as 0x0130 / 0x0131. So the cinematic
                //    body finally lands in the post-warp packet
                //    stream.
                if let Some(resumed) = lua.fire_player_event_and_drain(player_id, &[]) {
                    let resumed_count = resumed.len();
                    if !resumed.is_empty() {
                        crate::runtime::quest_apply::apply_runtime_lua_commands(
                            resumed,
                            &self.registry,
                            &self.db,
                            &self.world,
                            self.lua.as_ref(),
                        )
                        .await;
                    }
                    tracing::info!(
                        player = player_id,
                        commands = resumed_count,
                        "DoZoneChangeContent: resumed parked director coroutine",
                    );
                } else {
                    tracing::debug!(
                        player = player_id,
                        "DoZoneChangeContent: no parked director coroutine to resume",
                    );
                }
            }
        }

        // NO server-side cinematic kickoff here (removed).
        //
        // Packet-log evidence (packetlogs/map-packets.log, the region-103
        // reload diagnostic) settled this: once the warp actually
        // completes, the client DOES autonomously post
        // `IN 0x012D EventStart` for the content director 0x65300003
        // (event_name="noticeEvent") ~5s post-warp ([904] in that capture).
        // The prior "client never fires EventStart" belief was from runs
        // where the warp never completed.
        //
        // The previous workaround dispatched the director's
        // `onEventStarted` HERE, mid-warp — which emitted the cinematic
        // `RunEventFunction (delegateEvent processTtrBtl001)` at warp time
        // ([889] @ 12:30:15.6), ~5s BEFORE the client was ready. The
        // still-loading client dropped it; and because the director
        // coroutine was now parked past its `callClientFunction` +
        // `player:EndEvent()`, the client's REAL EventStart ([904] @
        // 12:30:20.5) merely RESUMED the parked coroutine → straight to
        // `EndEvent` ([905]) with no cinematic body. Net: the cinematic
        // fired into the void and the ready client only ever saw EndEvent.
        //
        // Fix: do nothing at warp time. Let the client's real EventStart
        // flow through `handle_event_start` →
        // `dispatch_event_start_to_content_director`, which does a FRESH
        // `onEventStarted` dispatch (no parked coroutine to resume) and so
        // emits the cinematic `RunEventFunction` at [904] time, when the
        // client is actually ready to render it.

        tracing::info!(
            player = player_id,
            parent_zone = parent_zone_id,
            area = %area_name,
            x,
            y,
            z,
            "DoZoneChangeContent applied (warp + zone-in replay; cinematic deferred to client EventStart)",
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
        // Body extracted to `quest_apply::apply_do_zone_change` (#28
        // S1.2) so the runtime drain (ticker / signal resumes — the
        // SEQ_005 director's final warp-out) can reach it without an
        // `Arc<PacketProcessor>`; this arm just delegates.
        crate::runtime::quest_apply::apply_do_zone_change(
            player_id,
            zone_id,
            private_area,
            private_area_type,
            spawn_type,
            x,
            y,
            z,
            rotation,
            &self.registry,
            &self.db,
            &self.world,
            self.lua.as_ref(),
        )
        .await;
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
            (
                c.base.zone_id,
                c.base.position_x,
                c.base.position_y,
                c.base.position_z,
                c.base.rotation,
            )
        };
        let (x, y, z, rotation) = target.unwrap_or((cur_x, cur_y, cur_z, cur_rot));
        // spawn_type=15 — pmeteor's WarpToPublicArea passes 15 to
        // DoZoneChange (WorldManager.cs:935-939); the earlier comment
        // claiming it passes 2 was wrong, and the value feeds the 0x00CE
        // SetActorPosition tail. (Garlemald-Server #28 review.)
        self.apply_do_zone_change(player_id, zone_id, None, 0, 15, x, y, z, rotation)
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
            (
                c.base.zone_id,
                c.base.position_x,
                c.base.position_y,
                c.base.position_z,
                c.base.rotation,
            )
        };
        let (x, y, z, rotation) = target.unwrap_or((cur_x, cur_y, cur_z, cur_rot));
        // spawn_type=15 — matches pmeteor's WarpToPrivateArea
        // (WorldManager.cs:925-928).
        self.apply_do_zone_change(
            player_id,
            zone_id,
            Some(area_class),
            area_index,
            15,
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
        npc.character.base.actor_name = format!("_rtnre{:07x}", retainer_actor_id);

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
        let bundle = crate::world_manager::build_retainer_spawn_bundle(&npc.character, &zone_name);
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
            use crate::group::outbox::{GroupEvent, GroupOutbox};
            use crate::group::{GroupKind, GroupTypeId, RetainerMeetingRelationGroup};
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
                crate::group::dispatch_group_event(&event, &self.registry, &self.world, &resolver)
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
                crate::group::dispatch_group_event(&event, &self.registry, &self.world, &resolver)
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

    /// `player:Logout()` — purge status effects flagged `LoseOnLogout`,
    /// then emit `LogoutPacket` (0x000E) to the owner's session. The
    /// client responds by closing the world connection and returning
    /// to character select. Mirrors C# `Player.Logout`
    /// (`Map Server/Actors/Chara/Player/Player.cs:861`). The
    /// `CleanupAndSave()` tail Meteor does is still deferred — persistent
    /// player save is driven by the regular DB upsert path rather than
    /// a logout-specific flush.
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
        self.purge_status_effects_on_disconnect(&handle).await;
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

    /// `player:QuitGame()` — purge status effects flagged `LoseOnLogout`,
    /// then emit `QuitPacket` (0x0011) to the owner's session. The
    /// client responds by terminating its process (back to launcher /
    /// desktop). Mirrors C# `Player.QuitGame`
    /// (`Map Server/Actors/Chara/Player/Player.cs:869`); same
    /// `CleanupAndSave()` deferral as [`apply_logout`]. Retail treats
    /// QuitGame as a stronger Logout for cleanup purposes, so both
    /// fire the LoseOnLogout purge identically.
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
        self.purge_status_effects_on_disconnect(&handle).await;
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

    /// Shared `RemoveStatusEffectsByFlags(LoseOnLogout)` tail used by
    /// both Logout and QuitGame. Mirrors Meteor's `Player.Cleanup` —
    /// drops every effect tagged for logout removal and drains the
    /// resulting status events (slot-clear packets + `onLose` Lua
    /// hooks + recalc) before the client connection drops.
    async fn purge_status_effects_on_disconnect(&self, handle: &crate::runtime::ActorHandle) {
        let mut outbox = crate::status::StatusOutbox::new();
        {
            let mut c = handle.character.write().await;
            c.status_effects.remove_by_flag(
                crate::status::StatusEffectFlags::LOSE_ON_LOGOUT,
                &mut outbox,
            );
            // Persist the survivors so long-lived effects come back on the
            // next login (the DbSave arm snapshots the container after the
            // logout-losing effects have been stripped). Mirrors C#
            // `Player.CleanupAndSave` → `Database.SavePlayerStatusEffects`.
            c.status_effects.save_to_db(&mut outbox);
        }
        self.drain_status_outbox(outbox).await;
    }

    /// Drain a status outbox through `dispatch_status_event` against
    /// the processor's registry / world / db / catalogs. Common tail
    /// for any caller (disconnect, zone change, …) that purges effects
    /// in-memory and needs the wire/save/recalc events to fan out.
    /// When the Lua engine isn't attached the drain is dropped — the
    /// in-memory mutation has already landed, which is the
    /// load-bearing half for the common disconnect case where the
    /// client is about to lose its socket anyway.
    async fn drain_status_outbox(&self, mut outbox: crate::status::StatusOutbox) {
        let Some(lua_ref) = self.lua.as_ref() else {
            return;
        };
        for evt in outbox.drain() {
            crate::runtime::dispatcher::dispatch_status_event(
                &evt,
                &self.registry,
                &self.world,
                &self.db,
                lua_ref.catalogs(),
            )
            .await;
        }
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
            let pkt =
                crate::packets::send::player::build_set_player_dream(handle.actor_id, 0, inn_code);
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
            2 => crate::packets::send::player::build_set_current_mount_goobbue(handle.actor_id, 1),
            _ => return,
        };
        // Raw subpacket bytes — the map-server writer task owns BasePacket
        // framing (`wrap_subpackets_in_basepacket`); pre-framing here both
        // double-wrapped the frame AND hid the zero `target_id` from the
        // stamp helper, so the packet died at the world-server proxy.
        {
            let mut self_bytes = pkt.to_bytes();
            // Self-emit — the mount owner needs the packet for their
            // own HUD regardless of whether any neighbours are
            // around to see them.
            if let Some(client) = self.world.client(handle.session_id).await {
                common::subpacket::SubPacket::stamp_target_id_if_zero(
                    &mut self_bytes,
                    handle.session_id,
                );
                client.send_bytes(self_bytes).await;
            }
            // Fan to every nearby Player via the shared zone-grid
            // broadcast (source is auto-excluded by `actors_around`).
            if let Some(zone) = self.world.zone(handle.zone_id).await {
                let sent = crate::runtime::broadcast::broadcast_around_actor(
                    &self.world,
                    &self.registry,
                    &zone,
                    handle.actor_id,
                    pkt.to_bytes(),
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
            let pkt = crate::packets::send::player::build_set_chocobo_name(handle.actor_id, &name);
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
            let pkt =
                crate::packets::send::player::build_set_grand_company(handle.actor_id, gc, l, g, u);
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
                // Setting home implies attunement — mirror of the pure
                // `Player::set_home_point` helper's in-memory insert,
                // on the registry-reachable set the snapshots actually
                // read. Persisted below alongside the homepoint.
                // (Garlemald-Server #46, round 5.)
                c.chara.unlocked_aetherytes.insert(homepoint);
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
            // Durable half of the implied attunement (INSERT OR IGNORE,
            // `characters_aetherytes` migration 068) — the pure helper
            // has no DB handle in scope, so the apply arm owns this.
            if let Err(e) = self
                .db
                .insert_character_aetheryte(player_id, homepoint)
                .await
            {
                tracing::warn!(
                    player = player_id,
                    homepoint,
                    err = %e,
                    "SetHomePoint: attunement persist failed",
                );
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
            // Implied attunement, offline flavour (same INSERT OR
            // IGNORE as the online branch above).
            if let Err(e) = self
                .db
                .insert_character_aetheryte(player_id, homepoint)
                .await
            {
                tracing::warn!(
                    player = player_id,
                    homepoint,
                    err = %e,
                    "SetHomePoint (offline): attunement persist failed",
                );
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

    /// Reply to the client's `work/achieveAetheryte` 0x012F work-sync
    /// request — pmeteor-parity interim (Player.cs:1182-1190
    /// `SendAchievedAetheryte`): an ALL-TRUE `Bitstream(512, true)`
    /// sliced to the requested bit window, i.e. "every aetheryte
    /// achieved". pmeteor ships this exact fake, so the client's
    /// achievement-flavoured aetheryte bits have never been real on any
    /// Meteor-derived server; the real teleport enforcement is the
    /// server-authoritative `HasAetheryteNodeUnlocked` gate in
    /// TeleportCommand.lua / AetheryteParent.lua backed by
    /// `characters_aetherytes`.
    ///
    /// TODO(#46 round 5): the per-bit mapping aetheryte-class-id →
    /// bit-index is unmapped; once known, slice a real bitset built from
    /// `CharaState::unlocked_aetherytes` instead of all-TRUE.
    ///
    /// Wire shape (pmeteor `SetActorPropetyPacket` bitfield mode,
    /// 0x0137):
    ///   [0]     runningByteTotal
    ///   [1]     slice length (`AddBitfield` uses payload len as the
    ///           type byte)
    ///   [2..6]  murmur2("work.event_achieve_aetheryte")
    ///   […]     slice bytes (bit-packed from..=to window with a
    ///           trailing 0x03 page byte — `Bitstream.GetSlice`)
    ///   […]     target marker `0x82 + 5 + len(target)`, 0x09,
    ///           u16 from, u16 to, ASCII "work/achieveAetheryte"
    async fn send_achieved_aetheryte(&self, player_id: u32, from: u16, to: u16) {
        const TARGET: &str = "work/achieveAetheryte";
        const BITFIELD_BITS: u16 = 512;
        let Some(handle) = self.registry.get(player_id).await else {
            return;
        };
        let Some(client) = self.world.client(handle.session_id).await else {
            return;
        };
        // Defensive clamps — C# would throw on an inverted/oversized
        // window; a hostile client shouldn't be able to panic the sim.
        let to = to.min(BITFIELD_BITS - 1);
        let from = from.min(to);
        // Port of `Bitstream::GetSlice(from, to)` for all-true data:
        // slice length is (to-from)/8 (+1 when the window has a partial
        // byte) + 1 trailing page byte (0x03). Full bytes are 0xFF; C#
        // only writes the partial byte when it lands exactly at len-2
        // (bug-for-bug faithful: a window like from=0,to=8 drops its
        // 9th bit on the floor, same as pmeteor).
        let span = (to - from) as usize;
        let mut slice = vec![0u8; span / 8 + usize::from(!span.is_multiple_of(8)) + 1];
        let last = slice.len() - 1;
        slice[last] = 0x03;
        let total_bits = span + 1;
        let full_bytes = total_bits / 8;
        for b in slice.iter_mut().take(full_bytes) {
            *b = 0xFF;
        }
        let partial_bits = total_bits % 8;
        if partial_bits != 0 && slice.len() >= 2 && full_bytes == slice.len() - 2 {
            slice[full_bytes] = (1u8 << partial_bits) - 1;
        }
        // Assemble the 0x0137 body in bitfield mode. The
        // ActorPropertyPacketBuilder doesn't speak bitfield targets
        // (its `done()` seals with the plain `0x82+len` marker), so
        // the body is laid out manually per the shape above.
        let mut data = crate::packets::send::body(0xA8);
        let id = common::utils::murmur_hash2("work.event_achieve_aetheryte", 0);
        let mut cur = 1usize;
        data[cur] = slice.len() as u8;
        data[cur + 1..cur + 5].copy_from_slice(&id.to_le_bytes());
        cur += 5;
        data[cur..cur + slice.len()].copy_from_slice(&slice);
        cur += slice.len();
        data[cur] = 0x82u8 + 5 + TARGET.len() as u8;
        data[cur + 1] = 0x09;
        data[cur + 2..cur + 4].copy_from_slice(&from.to_le_bytes());
        data[cur + 4..cur + 6].copy_from_slice(&to.to_le_bytes());
        cur += 6;
        data[cur..cur + TARGET.len()].copy_from_slice(TARGET.as_bytes());
        cur += TARGET.len();
        // runningByteTotal counts everything after the header byte.
        data[0] = (cur - 1) as u8;
        let mut sub = SubPacket::new(
            crate::packets::opcodes::OP_SET_ACTOR_PROPERTY,
            player_id,
            data,
        );
        sub.set_target_id(handle.session_id);
        client.send_bytes(sub.to_bytes()).await;
        tracing::debug!(
            player = player_id,
            from,
            to,
            "work/achieveAetheryte all-TRUE bitfield sent (pmeteor-parity interim)",
        );
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
        // Delegate to the shared free-fn so the flashing-pearl flow is
        // identical on the login drain and the runtime/resume drain.
        // (Garlemald-Server #46 live test round 2.)
        crate::runtime::quest_apply::apply_player_set_npc_ls(
            player_id,
            npc_ls_id,
            state,
            &self.registry,
            &self.db,
            &self.world,
        )
        .await;
    }

    /// `player:EquipAbility(classId, commandId, hotbarSlot, _)` —
    /// persist a single hotbar slot to DB. C#
    /// `Player.EquipAbility` decrements `hotbarSlot` by `commandBorder`
    /// (32) before saving the 0-based DB row; we mirror that math
    /// here. The in-memory hotbar snapshot + the
    /// `charaWork.command[N]` SetActorProperty fan-out are deferred
    /// — the next character load picks the row up.
    /// pmeteor `Player.UpdateHotbar(slots)` — push one slot's live
    /// command + recast state to the owning client after an
    /// Equip/Unequip/Swap applier, so hotbar edits render without
    /// re-zoning. Thin delegate: the body moved to
    /// `runtime::quest_apply::send_hotbar_slot_update` so the level-up
    /// auto-equip path (`equip_abilities_at_level`) shares the exact
    /// same wire shape. (#28 S3.1, hoisted for #46 round 2.)
    async fn send_hotbar_slot_update(&self, player_id: u32, slot0: u16) {
        crate::runtime::quest_apply::send_hotbar_slot_update(
            player_id,
            slot0,
            &self.registry,
            &self.world,
            self.lua.as_ref(),
        )
        .await;
    }

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
        if let Some(handle) = self.registry.get(player_id).await
            && let Some(client) = self.world.client(handle.session_id).await
        {
            let mut pkt = crate::packets::send::misc::build_text_sheet_no_source_auto(
                // Header source = WorldMaster (the client dispatches by
                // header source; it must be an always-present static
                // actor, never the player — Garlemald-Server #28 crash RCA).
                crate::packets::send::misc::WORLD_MASTER_ACTOR_ID,
                crate::packets::send::misc::WORLD_MASTER_ACTOR_ID,
                /* text_id */ 30603,
                crate::packets::send::misc::MESSAGE_TYPE_SYSTEM,
                &[
                    common::luaparam::LuaParam::UInt32(0),
                    common::luaparam::LuaParam::UInt32(command_id),
                ],
                /* prefer_alt */ false,
            );
            pkt.set_target_id(handle.session_id);
            client.send_bytes(pkt.to_bytes()).await;
        }

        // Live hotbar refresh (pmeteor `EquipAbility` tail: UpdateHotbar).
        self.send_hotbar_slot_update(player_id, zero_based_slot)
            .await;
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
        if unmasked_command_id != 0
            && session_id != 0
            && let Some(client) = self.world.client(session_id).await
        {
            let mut pkt = crate::packets::send::misc::build_text_sheet_no_source_auto(
                // Header source = WorldMaster (the client dispatches by
                // header source; it must be an always-present static
                // actor, never the player — Garlemald-Server #28 crash RCA).
                crate::packets::send::misc::WORLD_MASTER_ACTOR_ID,
                crate::packets::send::misc::WORLD_MASTER_ACTOR_ID,
                /* text_id */ 30604,
                crate::packets::send::misc::MESSAGE_TYPE_SYSTEM,
                &[
                    common::luaparam::LuaParam::UInt32(0),
                    common::luaparam::LuaParam::UInt32(unmasked_command_id),
                ],
                /* prefer_alt */ false,
            );
            pkt.set_target_id(session_id);
            client.send_bytes(pkt.to_bytes()).await;
        }

        // Live hotbar refresh — the emptied slot disables client-side.
        self.send_hotbar_slot_update(player_id, hotbar_slot).await;
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

        // Live hotbar refresh for both slots.
        self.send_hotbar_slot_update(player_id, zero_1).await;
        self.send_hotbar_slot_update(player_id, zero_2).await;
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
        if let Some(handle) = self.registry.get(player_id).await
            && let Some(client) = self.world.client(handle.session_id).await
        {
            let mut pkt = crate::packets::send::misc::build_text_sheet_no_source_auto(
                // Header source = WorldMaster (the client dispatches by
                // header source; it must be an always-present static
                // actor, never the player — Garlemald-Server #28 crash RCA).
                crate::packets::send::misc::WORLD_MASTER_ACTOR_ID,
                crate::packets::send::misc::WORLD_MASTER_ACTOR_ID,
                /* text_id */ 30603,
                crate::packets::send::misc::MESSAGE_TYPE_SYSTEM,
                &[
                    common::luaparam::LuaParam::UInt32(0),
                    common::luaparam::LuaParam::UInt32(command_id),
                ],
                /* prefer_alt */ false,
            );
            pkt.set_target_id(handle.session_id);
            client.send_bytes(pkt.to_bytes()).await;
        }

        // Live hotbar refresh (pmeteor `EquipAbility` tail: UpdateHotbar).
        self.send_hotbar_slot_update(player_id, slot).await;
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
        if let Err(e) = self.db.save_player_play_time(player_id, new_value).await {
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
        let bytes =
            crate::packets::send::player::build_set_current_job(actor_id, job_id as u32).to_bytes();
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
        let mut subs = crate::packets::send::actor::build_chara_state_at_quickly_for_all(
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
            hp,
            hp_max,
            mp,
            mp_max,
            tp,
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
            let mut pkt = crate::packets::send::build_set_actor_position(
                actor_id,
                actor_id as i32,
                x,
                y,
                z,
                rotation,
                spawn_type.into(),
                false,
            );
            // Untargeted subpackets are dropped by the world-server proxy
            // fan-out — without the tag this in-zone warp never moved the
            // client. (Garlemald-Server #28.)
            pkt.set_target_id(session_id);
            client.send_bytes(pkt.to_bytes()).await;
            tracing::info!(
                actor = actor_id,
                x,
                y,
                z,
                rotation,
                spawn_type,
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
    ///   SNpcCoordinate is preserved (SetSNpc doesn't write it).
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
        let mut new_hotbar = match self.db.load_hotbar(player_id, class_id).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    player = player_id, class_id, err = %e,
                    "DoClassChange: db.load_hotbar failed",
                );
                return;
            }
        };
        // First-time-class init — pmeteor `DoClassChange`
        // (Player.cs:1268-1272): `if (charaWork.battleSave.skillLevel
        // [classId-1] <= 0) { UpdateClassLevel(classId, 1);
        // EquipAbilitiesAtLevel(classId, 1); }` — a class that has never
        // been played starts at level 1 with its level-1 kit
        // auto-equipped. Gate on BOTH the empty hotbar AND a 0/absent
        // characters_class_levels entry so a deliberately emptied bar on
        // an already-played class doesn't re-deal the starter kit.
        // (#46 round 2.)
        if new_hotbar.is_empty() {
            let db_level = self
                .db
                .load_class_levels_and_exp(player_id)
                .await
                .map(|s| s.skill_level.get(class_id as usize).copied().unwrap_or(0))
                .unwrap_or(0);
            if db_level <= 0 {
                // pmeteor `UpdateClassLevel(classId, 1)` — persisting the
                // level also closes this init gate for future changes.
                if let Err(e) = self.db.set_level(player_id, class_id, 1).await {
                    tracing::warn!(
                        player = player_id, class_id, err = %e,
                        "DoClassChange: first-time set_level failed",
                    );
                }
                {
                    let mut c = handle.character.write().await;
                    if let Some(slot) = c.battle_save.skill_level.get_mut(class_id as usize) {
                        *slot = 1;
                    }
                }
                // Auto-equip the level-1 kit (class bar + job mirror).
                // `chara.class` still holds the OLD class here, so the
                // helper takes the DB-slot path (no live-mirror / wire
                // writes) — the reload right below picks the rows up and
                // the full-bar refresh at the tail ships them.
                if let Some(lua) = self.lua.as_ref() {
                    crate::runtime::quest_apply::equip_abilities_at_level(
                        player_id,
                        class_id,
                        1,
                        &self.registry,
                        &self.db,
                        Some(&*self.world),
                        lua,
                    )
                    .await;
                    match self.db.load_hotbar(player_id, class_id).await {
                        Ok(v) => new_hotbar = v,
                        Err(e) => tracing::warn!(
                            player = player_id, class_id, err = %e,
                            "DoClassChange: post-init hotbar reload failed",
                        ),
                    }
                }
            }
        }
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

        // Full-bar refresh — pmeteor `DoClassChange` tail
        // (Player.cs:1276+ `Database.LoadHotbar(this)` + the Set Hotbar
        // Commands property blocks / `UpdateHotbar`): push all 30 slots
        // so the client swaps the visible bar to the new class —
        // occupied slots render, stale old-class slots blank out
        // (absent mirror entries emit the disable shape). Self-only,
        // target stamped inside the helper. Before this the reload only
        // touched the in-memory mirror and the client kept drawing the
        // previous class's bar until re-zoning. (#46 round 2.)
        for slot0 in 0..crate::runtime::quest_apply::HOTBAR_SLOTS {
            self.send_hotbar_slot_update(player_id, slot0).await;
        }

        tracing::info!(
            player = player_id,
            class_id,
            "DoClassChange applied (chara.class + hotbar reload + first-time kit init + 0x01A4 broadcast + full-bar refresh; status-effect removal + stat recalc deferred)",
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
        crate::runtime::quest_apply::apply_quest_set_npc_ls_from(
            player_id,
            quest_id,
            from,
            &self.registry,
            &self.db,
            &self.world,
        )
        .await;
    }

    /// `quest:GetData():IncrementNpcLsMsgStep()` and the
    /// `LuaQuestHandle::ReadNpcLsMsg` first step.
    async fn apply_quest_increment_npc_ls_msg_step(&self, player_id: u32, quest_id: u32) {
        crate::runtime::quest_apply::apply_quest_increment_npc_ls_msg_step(
            player_id,
            quest_id,
            &self.registry,
            &self.db,
        )
        .await;
    }

    /// `quest:GetData():ClearNpcLs()` and the
    /// `LuaQuestHandle::EndOfNpcLsMsgs` last step.
    async fn apply_quest_clear_npc_ls(&self, player_id: u32, quest_id: u32) {
        crate::runtime::quest_apply::apply_quest_clear_npc_ls(
            player_id,
            quest_id,
            &self.registry,
            &self.db,
        )
        .await;
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
    ///   On success: spends `cost` seals via `db.add_seals(-cost)`,
    ///   bumps the per-GC rank field on `CharaState` to `next_rank`,
    ///   persists the rank via `db.set_gc_rank`, and emits
    ///   `SetGrandCompanyPacket` so the client sees the new rank.
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
        const RANKUP_ANIMATION_ID: u32 = 0x0400_0FFB;
        let sub = tx::actor::build_play_animation_on_actor(handle.actor_id, RANKUP_ANIMATION_ID);
        // Raw subpacket bytes — the writer task owns BasePacket framing;
        // pre-framing double-wrapped AND hid the zero `target_id` from the
        // stamp helper, so the packet died at the world-server proxy.
        {
            let mut self_bytes = sub.to_bytes();
            // Self-emit so the promoting player sees the salute
            // regardless of how far from any neighbour they are.
            if let Some(client) = self.world.client(handle.session_id).await {
                common::subpacket::SubPacket::stamp_target_id_if_zero(
                    &mut self_bytes,
                    handle.session_id,
                );
                client.send_bytes(self_bytes).await;
            }
            if let Some(zone) = self.world.zone(handle.zone_id).await {
                let sent = crate::runtime::broadcast::broadcast_around_actor(
                    &self.world,
                    &self.registry,
                    &zone,
                    handle.actor_id,
                    sub.to_bytes(),
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
                && let Some(r) = &session.spawned_retainer
                && r.retainer_id == retainer_id
            {
                session.spawned_retainer = None;
                self.world.upsert_session(session).await;
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
    async fn apply_rename_retainer(&self, player_id: u32, retainer_id: u32, new_name: String) {
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
        if let Some(mut session) = self.world.session(session_id).await
            && let Some(r) = session.spawned_retainer.as_mut()
            && r.retainer_id == retainer_id
        {
            r.name = new_name;
            self.world.upsert_session(session).await;
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
                let counters = [
                    q.get_counter(0),
                    q.get_counter(1),
                    q.get_counter(2),
                    q.get_counter(3),
                ];
                let actor_id = q.actor_id;
                q.clear_dirty();
                Some((slot as i32, actor_id, sequence, flags, counters))
            } else {
                None
            }
        };
        if let Some((slot, actor_id, sequence, flags, [c1, c2, c3, c4])) = save_tuple
            && let Err(e) = self
                .db
                .save_quest(player_id, slot, actor_id, sequence, flags, c1, c2, c3, c4)
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
    #[allow(clippy::too_many_arguments)]
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

        // Session-resolved zone — the ActorHandle's zone_id is frozen at
        // registration (`reassign_zone` has no production callers), so a
        // post-warp UpdateENPCs (the SEQ_010 Tkebbe talk flow in zone
        // 155) would otherwise search the login zone and skip every
        // broadcast. (Garlemald-Server #28, req 4.)
        let (zone_id, requester_area) = match self.world.session(session_id).await {
            Some(s) if s.current_zone_id != 0 => (
                s.current_zone_id,
                s.current_private_area_name
                    .clone()
                    .map(|n| (n, s.current_private_area_level)),
            ),
            _ => (player_handle.zone_id, None),
        };
        let Some(npc_handle) = self
            .find_npc_by_class_id(zone_id, enpc.actor_class_id, requester_area.as_ref())
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
        let mut graphic =
            crate::packets::send::build_set_actor_quest_graphic(npc_actor_id, enpc.quest_flag_type);
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
        // Session-resolved zone + area (see broadcast_quest_enpc_update).
        let (zone_id, requester_area) = match self.world.session(session_id).await {
            Some(s) if s.current_zone_id != 0 => (
                s.current_zone_id,
                s.current_private_area_name
                    .clone()
                    .map(|n| (n, s.current_private_area_level)),
            ),
            _ => (player_handle.zone_id, None),
        };
        let Some(npc_handle) = self
            .find_npc_by_class_id(zone_id, enpc.actor_class_id, requester_area.as_ref())
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
    /// Area-aware ENPC resolution — prefer the copy whose private-area
    /// pool matches the requester's routing, fall back to a zone-root
    /// copy. Mirrors `quest_apply::find_npc_by_class_id` (several city
    /// NPCs are seeded both at the zone root and inside a private-area
    /// phase under the same class id; first-match was HashMap-order
    /// nondeterministic). (Garlemald-Server #28.)
    async fn find_npc_by_class_id(
        &self,
        zone_id: u32,
        class_id: u32,
        requester_area: Option<&(String, u32)>,
    ) -> Option<ActorHandle> {
        let actors = self.registry.actors_in_zone(zone_id).await;
        let mut root_match: Option<ActorHandle> = None;
        for h in actors {
            let matches = {
                let c = h.character.read().await;
                c.chara.actor_class_id == class_id
            };
            if !matches {
                continue;
            }
            match (&h.private_area, requester_area) {
                (Some(npc_area), Some(req)) if npc_area.as_ref() == req => return Some(h),
                (None, None) => return Some(h),
                // Root copy — fallback for a private-area player.
                (None, Some(_)) if root_match.is_none() => root_match = Some(h),
                _ => {}
            }
        }
        root_match
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
            .save_quest(player_id, slot, actor_id, 0, 0, 0, 0, 0, 0)
            .await
        {
            tracing::warn!(
                error = %e,
                player = player_id,
                quest = quest_id,
                "AddQuest DB persist failed",
            );
        }
        tracing::info!(
            player = player_id,
            quest = quest_id,
            slot,
            "AddQuest applied"
        );

        // Fan out the canonical "<Quest> added to journal" toast.
        // Mirror C# `WorldManager.AddQuest`'s
        // `SendGameMessage(WorldMaster, 25224, 0x20, questId)`. Routed
        // through the auto-tier text-sheet builder; receiver = the
        // owning client only (no broadcast — this is a personal
        // system message).
        if let Some(client) = self.world.client(handle.session_id).await {
            let mut pkt = crate::packets::send::misc::build_text_sheet_no_source_auto(
                // Header source = WorldMaster (the client dispatches by
                // header source; it must be an always-present static
                // actor, never the player — Garlemald-Server #28 crash RCA).
                crate::packets::send::misc::WORLD_MASTER_ACTOR_ID,
                crate::packets::send::misc::WORLD_MASTER_ACTOR_ID,
                /* text_id */ 25224,
                crate::packets::send::misc::MESSAGE_TYPE_SYSTEM,
                &[common::luaparam::LuaParam::UInt32(quest_id)],
                /* prefer_alt */ false,
            );
            pkt.set_target_id(handle.session_id);
            client.send_bytes(pkt.to_bytes()).await;
        }

        self.fire_quest_hook(&handle, quest_id, "onStart", vec![])
            .await;
    }

    /// `player:CompleteQuest(id)` — fire `onFinish(player, quest, true)`
    /// first so the script sees the quest still in-journal, then land
    /// the shared completion core (journal/DB teardown, journal
    /// wire-clear, 25086 toast, ENPC "!" clears) via
    /// [`crate::runtime::quest_apply::finish_complete_quest`] — one body
    /// for both drain paths so the login-scoped and live-talk turn-ins
    /// can't drift. (Garlemald-Server #46.)
    async fn apply_complete_quest(&self, player_id: u32, quest_id: u32) {
        let Some(handle) = self.registry.get(player_id).await else {
            return;
        };
        // Idempotence guard — pmeteor Player.cs:1804 `if (HasQuest(id))`:
        // a completed quest's turn-in must never re-fire (double-click,
        // replayed drain, script re-call after completion) — no second
        // onFinish, no repeat toast / DB writes. (Garlemald-Server #46 —
        // Treasures of the Main infinite gil/EXP turn-in.)
        let in_journal = {
            let c = handle.character.read().await;
            c.quest_journal.has(quest_id)
        };
        if !in_journal {
            tracing::debug!(
                player = player_id,
                quest = quest_id,
                "CompleteQuest skipped — quest not in journal",
            );
            return;
        }
        // Fire onFinish before we tear the quest down so the hook can still
        // read `quest:GetData()` counters / flags via its snapshot.
        self.fire_quest_hook(
            &handle,
            quest_id,
            "onFinish",
            vec![crate::lua::QuestHookArg::Bool(true)],
        )
        .await;
        crate::runtime::quest_apply::finish_complete_quest(
            &handle,
            player_id,
            quest_id,
            &self.registry,
            &self.db,
            &self.world,
        )
        .await;
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
        tracing::info!(player = player_id, quest = quest_id, "AbandonQuest applied",);

        // Fan out the canonical "<Quest> abandoned." toast.
        // Mirror C# `WorldManager.AbandonQuest`'s
        // `SendGameMessage(this, WorldMaster, 25236, 0x20, abandoned.GetQuestId())`.
        if let Some(client) = self.world.client(handle.session_id).await {
            let mut pkt = crate::packets::send::misc::build_text_sheet_no_source_auto(
                // Header source = WorldMaster (the client dispatches by
                // header source; it must be an always-present static
                // actor, never the player — Garlemald-Server #28 crash RCA).
                crate::packets::send::misc::WORLD_MASTER_ACTOR_ID,
                crate::packets::send::misc::WORLD_MASTER_ACTOR_ID,
                /* text_id */ 25236,
                crate::packets::send::misc::MESSAGE_TYPE_SYSTEM,
                &[common::luaparam::LuaParam::UInt32(quest_id)],
                /* prefer_alt */ false,
            );
            pkt.set_target_id(handle.session_id);
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
                .map(|q| {
                    (
                        q.get_sequence(),
                        q.get_flags(),
                        q.get_counter(0),
                        q.get_counter(1),
                        q.get_counter(2),
                        q.get_counter(3),
                        q.get_npc_ls_from(),
                        q.get_npc_ls_msg_step(),
                    )
                })
                .unwrap_or((0, 0, 0, 0, 0, 0, 0, 0));
            let handle = crate::lua::LuaQuestHandle {
                player_id: snapshot.actor_id,
                quest_id,
                has_quest: c.quest_journal.has(quest_id),
                sequence: quest.0,
                flags: quest.1,
                counters: [quest.2, quest.3, quest.4, quest.5],
                npc_ls_from: quest.6,
                npc_ls_msg_step: quest.7,
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
            // warn (not debug): an argument-shape mismatch killing a hook
            // mid-arm while its queued commands still apply was invisible
            // at debug level — the #46 infinite-turn-in root cause.
            tracing::warn!(
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
                counters: [
                    q.get_counter(0),
                    q.get_counter(1),
                    q.get_counter(2),
                    q.get_counter(3),
                ],
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
            // warn — same tripwire rationale as `fire_quest_hook` (#46).
            tracing::warn!(
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
        //
        // `RunEventFunction` / `EndEvent` are ALREADY on the wire from the
        // EventOutbox bridge immediately above (`translate_lua_commands_into
        // _outbox` matches exactly those two variants and `dispatch_event_
        // event` sends them). `apply_login_lua_command`'s own RunEventFunction
        // /EndEvent arm (processor.rs ~1847) re-runs the IDENTICAL translate+
        // dispatch, so draining them here a second time double-emits the
        // cutscene RPC: the 1.x client plays the cinematic twice and posts two
        // EventUpdates, which corrupts its modal event layer — the man0l0
        // Rostnsthal walk-up softlock (each `onPush` sent `processTtrNomal002`
        // twice; after a couple of approaches the client went modal-silent).
        // Skip those two here; the bridge owns them. This mirrors the existing
        // KickEvent dedup (KickEvent is INTENTIONALLY excluded from the bridge
        // — see lua_bridge.rs — and owned by the login arm; here the ownership
        // is the reverse). Every other command (flag mutates, UpdateENPCs,
        // SendMessage, …) still needs the login applier. (Garlemald-Server #46.)
        for cmd in result.commands {
            if matches!(
                cmd,
                crate::lua::command::LuaCommand::RunEventFunction { .. }
                    | crate::lua::command::LuaCommand::EndEvent { .. }
            ) {
                continue;
            }
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
            OP_RX_ZONE_IN_COMPLETE => self.handle_zone_in_complete(source).await,
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
                // "Target Selected". `SetTargetPacket` (pmeteor
                // `PacketProcessor.cs:173`):
                //   body[0..4] = actorID      — the selected (soft) target
                //   body[4..8] = attackTarget — 0xE0000000 when there is no
                //                               locked attack target, else a
                //                               real actor id (auto-attack on).
                // pmeteor sets `currentTarget = actorID` and
                // `isAutoAttackEnabled = attackTarget != 0xE0000000`. The prior
                // garlemald code mislabelled body[0..4] as `attackTarget` and
                // only logged it. (Garlemald-Server #28.)
                let selected_target = if sub.data.len() >= 4 {
                    u32::from_le_bytes(sub.data[..4].try_into().unwrap())
                } else {
                    0
                };
                let attack_target = if sub.data.len() >= 8 {
                    u32::from_le_bytes(sub.data[4..8].try_into().unwrap())
                } else {
                    Self::SET_TARGET_NONE
                };
                let auto_attack = attack_target != Self::SET_TARGET_NONE;
                tracing::debug!(
                    source = source,
                    selected = format!("0x{:08X}", selected_target),
                    attack_target = format!("0x{:08X}", attack_target),
                    auto_attack,
                    "RX 0x00CD target-selected",
                );
                self.apply_player_set_target(source, selected_target, auto_attack)
                    .await;
            }
            OP_RX_DATA_REQUEST => {
                // 44 events/session. Same opcode as outbound
                // KickEvent — direction disambiguates. Client asks
                // for a GAM-property refresh by path. pmeteor
                // `WorkSyncRequestPacket.cs` decodes:
                //   body[0..4]  = u32 target actor id
                //   body[4]     = 0x09 marker → BITFIELD request:
                //                 body[5..7] = u16 `from` bit index,
                //                 body[7..9] = u16 `to` bit index,
                //                 null-terminated path at body[9..]
                //   otherwise   → plain request, path at body[4..]
                // The previous fixed body[4..24] extraction predated
                // the marker discovery — bitfield requests (the
                // "work/achieveAetheryte" family) decoded as a
                // "\t…" garbage path and never matched. (Garlemald-
                // Server #46, round 5.)
                let is_bitfield = sub.data.len() >= 9 && sub.data[4] == 0x09;
                let (bit_from, bit_to, path_start) = if is_bitfield {
                    (
                        u16::from_le_bytes(sub.data[5..7].try_into().unwrap()),
                        u16::from_le_bytes(sub.data[7..9].try_into().unwrap()),
                        9usize,
                    )
                } else {
                    (0, 0, 4usize)
                };
                let prop_path = if sub.data.len() > path_start {
                    extract_null_terminated_ascii(&sub.data[path_start..])
                } else {
                    String::new()
                };
                match prop_path.as_str() {
                    "work/achieveAetheryte" => {
                        tracing::debug!(
                            source = source,
                            from = bit_from,
                            to = bit_to,
                            "RX 0x012F work-sync: achieveAetheryte",
                        );
                        self.send_achieved_aetheryte(source, bit_from, bit_to).await;
                    }
                    _ => {
                        tracing::debug!(
                            source = source,
                            property = %prop_path,
                            "RX 0x012F data-request (no-op pending property-refresh handler)",
                        );
                    }
                }
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
                    "RX 0x0133 group-created",
                );
                // Reply with the content-group `/_init` SynchGroupWorkValues
                // packet. Mirrors pmeteor's `WorldManager.SendGroupInit` →
                // `ContentGroup.SendInitWorkValues` (Map Server/
                // WorldManager.cs:1640-1654 + Group/ContentGroup.cs:105).
                //
                // Without this reply, the client's content-group state
                // machine sits forever waiting for the director property,
                // so the cinematic body (RunEventFunction sequence) never
                // fires and "Now Loading" never clears post-warp.
                //
                // The director_actor_id is the lower 32 bits of group_id —
                // the client encodes the director as a u64 group key
                // matching the director's actor id (verified from the
                // captured man0g0 SEQ_005 hang: client sent
                // `group_id=0x0000000065300003` which IS the
                // QuestDirectorMan0g001 actor id).
                //
                // Only respond for CONTENT-DIRECTOR group inits, NOT
                // for player-work / mob-group / social-group inits.
                //
                // Discriminator: the high u32 of group_id distinguishes
                // group classes:
                //   - 0x00000000__XXXXXXXX → content director (LEGACY format,
                //                            pre-0x3000 prefix; low u32 IS the
                //                            director actor id; e.g. man0g0
                //                            QuestDirectorMan0g001 = 0x65300003)
                //   - 0x30000000__XXXXXXXX → content group (post-93eb62b
                //                            format; low u32 is a per-session
                //                            group counter starting at 1; the
                //                            director_actor_id must be looked
                //                            up via session.active_content_script.
                //                            This is what apply_do_zone_change_
                //                            content emits in 3a; the client
                //                            echoes the same group_id back in
                //                            its IN 0x0133)
                //   - 0x80000000__XXXXXXXX → player-work group (low u32 is the
                //                            player actor id, high bit flags
                //                            it as a player-side group)
                //   - 0x2680XXXX__XXXXXXXX → mob/monster group (synthetic
                //                            id with 0x2680 prefix per the
                //                            captured-bytes comment above)
                //
                // The 0x3000-prefix branch is the SEQ_005-blocking case
                // (Phase 9 #8c follow-up): until 2026-05-15 the dispatcher
                // filtered `high == 0` only, silently dropped the client's
                // 0x3000_0000_0000_0001 echo, and the missing OUT 0x017A
                // SynchGroupWorkValues reply stalled the client's
                // WorkSyncUpdater — the director never got marked
                // event-ready, IN 0x012D for director never fired, the
                // cinematic body never started.
                //
                // Earlier iteration responded to ALL 0x0133 with event="/_init"
                // and crashed Wine when the OpeningDirector path's player-work
                // group sent group_id=0x8000000000000001 (player actor id 1
                // misinterpreted as a director). Hence the explicit branch
                // gating below.
                let high = (group_id >> 32) as u32;
                if event_name == "/_init" {
                    let director_actor_id: Option<u32> = if high == 0 {
                        // Legacy format: low u32 IS the director id
                        Some((group_id & 0xFFFF_FFFF) as u32)
                    } else if (high & 0xF000_0000) == 0x3000_0000 {
                        // Content-group format: look up the active
                        // content script's director from the session.
                        // The low u32 is a per-session counter, NOT the
                        // director id, so we can't derive it from the
                        // group_id alone.
                        self.world
                            .session(source)
                            .await
                            .and_then(|s| s.active_content_script)
                            .map(|a| a.director_actor_id)
                    } else {
                        // Player-work, mob, etc — not for us to reply to.
                        None
                    };
                    if let Some(director_actor_id) = director_actor_id {
                        let mut sub = crate::packets::send::groups
                            ::build_synch_group_work_values_content_init(
                                source,
                                group_id,
                                director_actor_id,
                            );
                        sub.set_target_id(source);
                        client.send_bytes(sub.to_bytes()).await;
                        tracing::info!(
                            source = source,
                            group_id = format!("0x{:016X}", group_id),
                            director = format!("0x{:08X}", director_actor_id),
                            "RX 0x0133 → emitted SynchGroupWorkValues /_init reply",
                        );
                    }
                }
            }
            _ => {
                tracing::debug!(
                    opcode = format!("0x{:X}", opcode),
                    source = source,
                    "unhandled game message",
                );
                common::packet_diagnostics::log_unknown_game_message("map", "map", sub);
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

    /// `NpcLinkshellChatCommand` static-actor id — `0xA0F00000 | 0x5E95`
    /// (decoded from `staticactors.bin`). The client fires `EventStart`
    /// against this actor when the player opens an NPC-linkshell chat
    /// (the flashing linkpearl); the first integer LuaParam is the
    /// `npcLsId`. pmeteor routes it to `Player.HandleNpcLs`, which finds
    /// the active quest whose `npcLsFrom == npcLsId` and fires its
    /// `onNpcLS(player, quest, from, msgStep)` hook. (Garlemald-Server
    /// #46 live test — the man0l1 SEQ_003 Path-Companion progression +
    /// endTutorialMode ride this path.)
    const NPC_LINKSHELL_CHAT_COMMAND: u32 = 0xA0F0_5E95;

    // `pub(crate)` so the #28 S3.2 integration test can drive a synthetic
    // retail-shaped 0x012D through the same parse + dispatch path the
    // socket reader uses.
    pub(crate) async fn handle_event_start(&self, session_id: u32, data: &[u8]) -> Result<()> {
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

        // DIAGNOSTIC (Garlemald-Server #46 walk-up): log EVERY inbound
        // EventStart the client sends, BEFORE any dispatch/early-return, so a
        // live test definitively shows whether the client emits a `talkDefault`
        // (event_type=1) for an NPC the player is trying to talk to — vs. only
        // emitting `pushDefault` (type=2) / `noticeEvent` (type=5). If no
        // type=1 ever appears for the target, the block is the client not
        // sending the talk (modal / non-talkable); if it appears but nothing
        // happens, the block is server-side dispatch.
        tracing::info!(
            session = session_id,
            owner = format!("0x{:08X}", pkt.owner_actor_id),
            event_type = pkt.event_type,
            event_name = %pkt.event_name,
            "RX EventStart (inbound, pre-dispatch)",
        );

        let Some(handle) = self.registry.by_session(session_id).await else {
            return Ok(());
        };
        let actor_id = handle.actor_id;

        let owner_actor_id = pkt.owner_actor_id;

        // pmeteor `LuaEngine.cs::EventStarted` parity — a STRICT resume-or-
        // fresh split: if the player has a `_WAIT_EVENT`-parked coroutine,
        // this EventStart resumes it (zero args, matching pmeteor's bare
        // `coroutine.Resume()`); otherwise fall through to the fresh
        // `onEventStarted` dispatch below. Never both — the old shape ran
        // the fresh dispatch AND a trailing parked-resume arm
        // (`dispatch_event_start_to_npc`), which resumed the just-parked
        // menu coroutine with zero args so the camp Aetheryte menu opened
        // and instantly closed (0x0130 + 0x0131 in one flush).
        //
        // Unlike pmeteor, the resume is gated on the park's stamped event
        // owner: pmeteor resumes ANY parked coroutine for the player, which
        // is how a stale Charlys talk surviving a relog got resumed by a
        // Hobriaut talk — silently draining QuestIncCounter + AddGil(2000)
        // + EndEvent. Owner mismatch ⇒ discard the stale park and dispatch
        // fresh; owner UNKNOWN (parked outside the stamping scopes, e.g. a
        // pre-warp director park from the DoZoneChangeContent drain) ⇒
        // leave it alone and let `dispatch_event_start_to_content_director`'s
        // own resume arm decide. (Garlemald-Server #46.)
        //
        // Command static actors + journal/NpcLs (0xA0F0xxxx) are exempt
        // from the gate entirely: their menu round-trips resume via
        // EventUpdate, and the pre-#46 dispatch skipped them with this
        // same mask. Running them through the gate would let a hotbar /
        // emote press DISCARD a stamped director park mid-flight (e.g.
        // SEQ_005's processTtrBtl001 while the ACTIVEMODE popup is up —
        // the game is NOT client-modal there), softlocking the tutorial
        // until relog.
        let is_command_static_actor = (owner_actor_id & 0xFFF0_0000) == 0xA0F0_0000;
        if let Some(lua) = self.lua.as_ref().filter(|_| !is_command_static_actor) {
            let parked_owner = lua
                .scheduler()
                .lock()
                .ok()
                .and_then(|s| s.parked_event_owner(actor_id));
            match parked_owner {
                Some(parked) if parked == owner_actor_id => {
                    // pmeteor `Player.StartEvent` (Player.cs:2174) stamps
                    // currentEventOwner/Name/Type BEFORE the resume-or-
                    // dispatch split — mirror the field writes WITHOUT
                    // `start_event`'s outbox row (the row is what triggers
                    // the fresh dispatch).
                    {
                        let mut chara = handle.character.write().await;
                        chara.event_session.current_event_owner = owner_actor_id;
                        chara.event_session.current_event_name = pkt.event_name.clone();
                        chara.event_session.current_event_type = pkt.event_type;
                    }
                    // Keep any re-park from the continuation stamped with
                    // the same owner.
                    let _owner_ctx = crate::lua::scheduler::CoroutineScheduler::event_owner_scope(
                        lua.scheduler(),
                        actor_id,
                        owner_actor_id,
                    );
                    let resumed = lua
                        .fire_player_event_and_drain(actor_id, &[])
                        .filter(|c| !c.is_empty());
                    tracing::debug!(
                        player = actor_id,
                        owner = format!("0x{owner_actor_id:08X}"),
                        event_name = %pkt.event_name,
                        commands = resumed.as_ref().map(|c| c.len()).unwrap_or(0),
                        "EventStart resumed the owner's parked coroutine (pmeteor resume arm)",
                    );
                    if let Some(cmds) = resumed {
                        self.apply_resumed_event_commands(&handle, cmds).await;
                    }
                    return Ok(());
                }
                Some(parked) if parked != crate::lua::scheduler::EVENT_OWNER_UNKNOWN => {
                    tracing::warn!(
                        player = actor_id,
                        parked_owner = format!("0x{parked:08X}"),
                        new_owner = format!("0x{owner_actor_id:08X}"),
                        event_name = %pkt.event_name,
                        "stale parked coroutine displaced by new event — discarded before fresh dispatch",
                    );
                    if let Ok(mut s) = lua.scheduler().lock() {
                        s.discard_parked_event(actor_id);
                    }
                }
                // Nothing parked, or parked-but-unattributable: fresh
                // dispatch as before.
                _ => {}
            }
        }

        // Any `_WAIT_EVENT` park produced by the dispatch fan-out below
        // (the EventStarted outbox → NPC/director dispatch, the quest-hook
        // fan-out, command scripts) belongs to THIS event — stamp it with
        // the owner so the resume gate above can match (or displace) it on
        // the next EventStart. RAII: the early returns below (hotbar,
        // NpcLs) clear the stamp on drop.
        let _event_owner_ctx = self.lua.as_ref().map(|lua| {
            crate::lua::scheduler::CoroutineScheduler::event_owner_scope(
                lua.scheduler(),
                actor_id,
                owner_actor_id,
            )
        });

        let event_name_for_match = pkt.event_name.clone();
        // Snapshot the EventStart payload before the fields are moved into
        // `start_event`. The director-onEventStarted branch below replays
        // the event_name + event_type + lua_params into the Lua coroutine
        // (mirrors pmeteor `LuaEngine.cs::EventStarted`).
        let event_name_for_director = pkt.event_name.clone();
        let event_type_for_director = pkt.event_type;
        let lua_params_for_director = pkt.lua_params.clone();
        // Same snapshot for the command-static-actor dispatch below.
        let event_name_for_cmd = pkt.event_name.clone();
        let event_type_for_cmd = pkt.event_type;
        let lua_params_for_cmd = pkt.lua_params.clone();
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

        // Hotbar skill press — retail sends `EventStart` with eventName
        // `commandDefault` and the command's static-actor id
        // (`0xA0F00000 | id`) as the owner (D §1.1, combat_skills.pcapng;
        // the real target rides in the type-6 Actor LuaParam). Resolve the
        // masked id against the battle-command catalog and execute inline —
        // the on-disk command scripts are 4-liners with no
        // WeaponSkill/Ability/Cast bindings (plan R5). The route is
        // exclusive, mirroring pmeteor's `PacketProcessor` 0x012D which
        // targets only the command static actor: no quest/director fan-out
        // for a skill press. The two ActivateCommand ids ride eventName
        // `commandForced` and stay on the command-script dispatch below;
        // unresolved 0xA0F0xxxx owners (journal command etc.) fall through
        // unchanged. (#28 S3.2.)
        if (owner_actor_id & 0xFFF0_0000) == 0xA0F0_0000 && event_name_for_match == "commandDefault"
        {
            let masked_cmd = (owner_actor_id & 0xFFFF) as u16;
            let command = self.lua.as_ref().and_then(|l| {
                l.catalogs()
                    .battle_commands
                    .read()
                    .ok()
                    .and_then(|m| m.get(&masked_cmd).cloned())
            });
            if let Some(command) = command {
                self.dispatch_hotbar_command(&handle, session_id, &lua_params_for_cmd, command)
                    .await;
                return Ok(());
            }
        }

        // Content-director `onEventStarted` dispatch. When the EventStart's
        // `owner_actor_id` matches the player's `active_content_script`
        // director, route into the director's `onEventStarted(player,
        // director, eventType, eventName, ...)` hook. Without this the
        // SEQ_005 combat-tutorial cinematic body never runs — the
        // `KickEvent("noticeEvent")` that `man0g0::doContentArea` sends
        // pre-warp results in the client posting `EventStart(eventType=5)`
        // post-warp, but the existing quest-hook switch below only matches
        // `eventType` 0–3 so the dispatch falls through and the director's
        // `onEventStarted` body (which calls `callClientFunction(...,
        // "processTtrBtl001", ...)` to play the cinematic) is never run.
        //
        // Mirrors pmeteor `LuaEngine.cs::EventStarted` (lines 651-682):
        // `if (mSleepingOnPlayerEvent.ContainsKey(player.Id)) resume the
        // parked coroutine; else if (target is Director) Director.OnEventStart;
        // else CallLuaFunction(player, target, "onEventStarted", …)`. The
        // owner-stamped resume arm at the top of this handler covers
        // attributed parks; this path keeps its OWN resume arm for
        // UNSTAMPED director parks (pre-warp parks from the
        // DoZoneChangeContent drain land outside the stamping scopes) plus
        // the fresh dispatch; the NPC fallback is the quest-hook fan-out
        // further down.
        self.dispatch_event_start_to_content_director(
            &handle,
            session_id,
            owner_actor_id,
            event_name_for_director,
            event_type_for_director,
            lua_params_for_director,
        )
        .await;

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
            self.fire_quest_hook_for_active_quests(
                &handle,
                owner_actor_id,
                hook_name,
                &event_name_for_cmd,
            )
            .await;
        }

        // Non-quest NPC / object interaction — the TARGET actor's OWN
        // `onEventStarted` script (the aetheryte's teleport/homepoint/leve
        // menu in AetheryteParent.lua, base populace dialogue, etc.) rides
        // the `start_event` → `EventStarted` outbox row above, dispatched by
        // `event/dispatcher.rs::dispatch_npc_event_started`. There used to
        // be a SECOND dispatch here (`dispatch_event_start_to_npc`) whose
        // parked-resume arm resumed the coroutine the outbox dispatch had
        // JUST parked — zero args, so menu scripts saw a nil choice and fell
        // through to `player:EndEvent()`: the client got the menu-open
        // 0x0130 and the 0x0131 EndEvent in the same flush and the camp
        // Aetheryte menu instantly closed. pmeteor `LuaEngine.cs::
        // EventStarted` is a strict resume-OR-fresh if/else — the resume
        // arm lives at the top of this handler now, and the fresh dispatch
        // is single-path through the outbox. (Garlemald-Server #46.)

        // SEQ-005 content-tutorial handshake (UNRESOLVED — breadcrumb for
        // the next attempt). Packet-diff against the working pmeteor capture
        // captures/pmeteor-quest/20260426-160210-gridania-manual3 shows the
        // real post-warp sequence is:
        //   IN  EventStart type=101 "talkDefault"  @15:54:31.414  (precursor)
        //   IN  EventStart type=5   "noticeEvent"  @15:54:33.388  (for the
        //                                           CONTENT director 0x653000xx)
        //   OUT RunEventFunction event_name="noticeEvent" @15:54:33.388
        //                                           (onNotice → processTtrNomal001withHQ)
        //   ... tutorial fight runs with the noticeEvent left OPEN ...
        //   OUT EndEvent "noticeEvent" @15:56:16    (~2 min later, post-fight)
        // i.e. onNotice fires on the type-5 noticeEvent, NOT type-101, and
        // the notice EndEvent must NOT be sent until the fight ends.
        //
        // The blocker: garlemald's client posts the type-101 precursor but
        // NEVER posts the type-5 noticeEvent for the content director (it
        // does post type-5 for the OPENING director, which works). This is
        // the documented client-side kick-priming gap — the content
        // director's receiver isn't primed, so its `noticeEvent` kick is
        // silently dropped. Firing onNotice on the type-101 here (a previous
        // attempt) is WRONG: it sends the cinematic + a premature notice
        // EndEvent the retail client never sees at that point, and still
        // doesn't unblock. See meteor-decomp/docs/seq005_kick_gate_analysis.md
        // and the reference_ffxiv_1x_kick_priming_lua_binding memory.

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
            self.send_quest_journal_data(&handle, session_id, &lua_params_for_cmd)
                .await;
        }

        // NPC linkshell chat — the client opened the flashing linkpearl.
        // Mirror pmeteor `Player.HandleNpcLs` (Player.cs:1975-1986):
        // find the active quest whose `npcLsFrom == npcLsId` and fire its
        // `onNpcLS(player, quest, from, msgStep)` hook (which drains the
        // Path-Companion messages, advances the sequence, and — at
        // man0l1 SEQ_003 — calls endTutorialMode). Routed by owner id,
        // like the journal command above; the on-disk
        // commands/NpcLinkshellChatCommand.lua is a stale revision and is
        // deliberately bypassed. (Garlemald-Server #46 live test.)
        if owner_actor_id == Self::NPC_LINKSHELL_CHAT_COMMAND {
            // Diagnostic (Garlemald-Server #46): the NPC-linkshell read
            // reaches here but `handle_npc_ls_chat` couldn't find the
            // npcLsId param. Dump the raw EventStart payload + the parsed
            // params so the exact wire encoding of the clicked linkshell id
            // is visible in one live test.
            tracing::debug!(
                owner = format_args!("{owner_actor_id:#010x}"),
                event = %event_name_for_cmd,
                event_type = event_type_for_cmd,
                params = ?lua_params_for_cmd,
                "NpcLs chat EventStart received",
            );
            self.handle_npc_ls_chat(&handle, &lua_params_for_cmd).await;
            return Ok(());
        }

        // Generic client-command dispatch — run `commands/<Name>.lua::
        // onEventStarted` for a command static actor. This is how the client's
        // active-mode toggle (F / sword → ActivateCommand) reaches the script
        // that calls `player.Engage(...)` + `sendSignal("playerActive")`,
        // un-parking the combat-tutorial director. Generalizes the journal
        // one-off above. (Garlemald-Server #28.)
        if let Some(command_name) = Self::command_script_name(owner_actor_id) {
            // Harvest commands are node-scoped: the command static actor
            // (0xA0F0xxxx) is identical for every gather node, so resolve
            // the node the player actually struck. The client `SetTarget`s
            // the node before invoking the command, so its soft target
            // (`current_target`) IS the clicked node; map that live actor
            // id back to the `(zoneId, uniqueId)` key `DummyCommand.lua`
            // feeds `GetGatherNodeMetadata`. `None` for every other
            // command (they don't read the commandActor's identity).
            // (Wave 3 gather partial.)
            let command_actor_identity = if Self::is_gather_command(owner_actor_id) {
                let target = { handle.character.read().await.chara.current_target };
                self.world.gather_node_identity(target).await
            } else {
                None
            };
            self.dispatch_command_script(
                &handle,
                owner_actor_id,
                command_name,
                event_name_for_cmd,
                event_type_for_cmd,
                lua_params_for_cmd,
                command_actor_identity,
            )
            .await;
        }

        tracing::debug!(
            player = actor_id,
            owner = owner_actor_id,
            event_type = pkt.event_type,
            "event start dispatched",
        );
        Ok(())
    }

    /// EventStart router for content directors — pmeteor `LuaEngine.cs::
    /// EventStarted` lines 651-682. If the player has an active content
    /// script and the EventStart's owner is that content's director, run
    /// the director's `onEventStarted` (or resume a parked coroutine
    /// waiting on `_WAIT_EVENT`). Translates emitted event-flavoured
    /// commands through `EventOutbox` + `dispatch_event_event` and applies
    /// the rest through the runtime drain.
    async fn dispatch_event_start_to_content_director(
        &self,
        handle: &ActorHandle,
        session_id: u32,
        owner_actor_id: u32,
        event_name: String,
        event_type: u8,
        lua_params: Vec<common::luaparam::LuaParam>,
    ) {
        let Some(active) = self
            .world
            .session(session_id)
            .await
            .and_then(|s| s.active_content_script)
        else {
            return;
        };
        if owner_actor_id != active.director_actor_id {
            return;
        }
        let Some(lua) = self.lua.as_ref() else {
            return;
        };

        let actor_id = handle.actor_id;

        // First, try to resume a parked `_WAIT_EVENT` coroutine. When the
        // director's `onEventStarted` body has already run once and parked
        // on a `callClientFunction(...)` yield, a subsequent EventStart
        // from the client's cinematic completion should resume *that*
        // coroutine, not start a fresh dispatch. Pmeteor's
        // `mSleepingOnPlayerEvent` check is the same gate.
        let resumed = lua.fire_player_event_and_drain(actor_id, &[]);
        let commands = match resumed {
            Some(cmds) if !cmds.is_empty() => {
                tracing::debug!(
                    player = actor_id,
                    director = owner_actor_id,
                    commands = cmds.len(),
                    "EventStart resumed parked director coroutine",
                );
                cmds
            }
            _ => {
                // Fresh dispatch — load the director script and run
                // `onEventStarted` in a coroutine.
                let script_path = lua.resolver().director(&active.director_name);
                if !script_path.exists() {
                    tracing::debug!(
                        director = owner_actor_id,
                        director_name = %active.director_name,
                        script = %script_path.display(),
                        "EventStart for content director skipped — script not on disk",
                    );
                    return;
                }
                let snapshot = {
                    let c = handle.character.read().await;
                    build_player_snapshot_from_character(&c)
                };
                let director_handle = crate::lua::userdata::LuaDirectorHandle {
                    name: active.director_name.clone(),
                    actor_id: active.director_actor_id,
                    class_path: active.area_class_path.clone(),
                    queue: crate::lua::command::CommandQueue::new(),
                };
                let lua_clone = lua.clone();
                let script_path_clone = script_path.clone();
                let event_name_clone = event_name.clone();
                let lua_params_clone = lua_params;
                let result = tokio::task::spawn_blocking(move || {
                    lua_clone.call_director_on_event_started(
                        &script_path_clone,
                        snapshot,
                        director_handle,
                        event_name_clone,
                        event_type,
                        lua_params_clone,
                    )
                })
                .await;
                let partial = match result {
                    Ok(p) => p,
                    Err(join_err) => {
                        tracing::warn!(
                            director = owner_actor_id,
                            error = %join_err,
                            "director onEventStarted dispatch panicked",
                        );
                        return;
                    }
                };
                if let Some(e) = partial.error {
                    tracing::debug!(
                        director = owner_actor_id,
                        director_name = %active.director_name,
                        error = %e,
                        "director onEventStarted errored; applying partial commands",
                    );
                } else {
                    tracing::debug!(
                        director = owner_actor_id,
                        director_name = %active.director_name,
                        event_name = %event_name,
                        commands = partial.commands.len(),
                        "director onEventStarted fired",
                    );
                }
                partial.commands
            }
        };

        Box::pin(self.apply_event_script_commands(handle, commands)).await;
    }

    /// Apply a script's drained `LuaCommand`s. Event-flavoured commands
    /// (`RunEventFunction` / `EndEvent` / `KickEvent` / …) are translated
    /// through the `EventOutbox` + dispatcher so cinematic packets reach the
    /// wire (without this they hit `apply_runtime_lua_command`'s catch-all and
    /// are dropped); the rest go through the runtime; and any `SendSignal`
    /// resumes the coroutines parked on `waitForSignal(name)` (e.g. the combat
    /// tutorial director on "playerActive") and applies THEIR commands,
    /// recursively. Shared by the content-director dispatch and the
    /// command-static-actor dispatch. Mirrors `apply_quest_on_notice` +
    /// `fire_quest_event_hook`. (Garlemald-Server #28.)
    /// Delegates to the shared `quest_apply::apply_event_script_commands`
    /// (also used by the ticker's per-owner coroutine drains). Order is
    /// load-bearing on the wire: pmeteor sends `SendDataPacket(9)`
    /// (startTutorialMode — a runtime/0x0133 command) BEFORE the
    /// `processTtrBtl001` cinematic (RunEventFunction/0x0130), while later
    /// it sends the tutorial-widget SendDataPackets AFTER their cinematic —
    /// the shared drain interleaves per-command to preserve that.
    async fn apply_event_script_commands(
        &self,
        handle: &ActorHandle,
        commands: Vec<crate::lua::command::LuaCommand>,
    ) {
        crate::runtime::quest_apply::apply_event_script_commands(
            handle,
            commands,
            &self.registry,
            &self.db,
            &self.world,
            self.lua.as_ref(),
        )
        .await;
    }

    /// Apply the burst drained from a resumed `_WAIT_EVENT` coroutine.
    /// Login-scoped bursts (content warps / quest handoffs / director
    /// staging / logout — see [`Self::is_login_scoped_burst`] for the
    /// per-variant rationale) MUST route through the login applier;
    /// everything else takes the shared event-script drain. Shared by
    /// the EventStart resume gate and the EventUpdate resume path so
    /// both client wake-ups route a continuation identically.
    async fn apply_resumed_event_commands(
        &self,
        handle: &ActorHandle,
        cmds: Vec<crate::lua::command::LuaCommand>,
    ) {
        if Self::is_login_scoped_burst(&cmds) {
            // #46 escort R2 — enforce the EndEvent-before-warp wire
            // invariant on THIS path too. The shared event-script drain
            // below already hoists via `hoist_end_events_before_warps`,
            // but login-scoped bursts bypassed it — so a script tail of
            // `DoZoneChangeContent(...)` → `player:EndEvent()` (the
            // startMan0l1Content escort shape, session 53943) shipped
            // 0x00E2 → 0x0131 and the EndEvent landed inside the
            // client's Now-Loading window, losing the `_onPostEvent`
            // teardown (desktopWidgetMode-16 menu mask). The hoister's
            // `warp_family_player` matcher includes `DoZoneChangeContent`,
            // so the same reorder covers the content warp.
            let cmds = crate::runtime::quest_apply::hoist_end_events_before_warps(cmds);
            for cmd in cmds {
                Box::pin(self.apply_login_lua_command(handle, cmd)).await;
            }
        } else {
            Box::pin(self.apply_event_script_commands(handle, cmds)).await;
        }
    }

    /// Command static-actor ids that dispatch to a `commands/<Name>.lua`
    /// script via `onEventStarted`. Decoded from `staticactors.bin`
    /// (`… | 0xA0F00000`). ActivateCommand is the active/passive (draw/sheathe)
    /// toggle the client sends on F / the sword icon — two ids for
    /// activate/deactivate, both routed to the one script which branches on
    /// `player.currentMainState`. (Garlemald-Server #28.)
    const ACTIVATE_COMMAND_A: u32 = 0xA0F0_5209;
    const ACTIVATE_COMMAND_B: u32 = 0xA0F0_520A;

    /// Main-menu System command static actors, decoded from
    /// `staticactors.bin` (`id | 0xA0F00000`): the client fires an
    /// `EventStart` against these when the player clicks Exit / Teleport
    /// / Return in the main menu. Without a dispatch arm the press falls
    /// through `command_script_name` and the button does nothing — the
    /// live-test "Exit game / Teleport don't work" report.
    /// `TeleportCommand` (24220) backs BOTH the Teleport menu
    /// (`isTeleport == 0`) and Return (`isTeleport == 1`); the on-disk
    /// `commands/{LogoutCommand,TeleportCommand}.lua` scripts already
    /// drive the `delegateCommand` confirm round-trip, which parks on
    /// `_WAIT_EVENT` and resumes on the client's `0x012E EventUpdate`
    /// exactly like quest `delegateEvent`. (Garlemald-Server #46 live
    /// test.)
    const LOGOUT_COMMAND: u32 = 0xA0F0_5E9B;
    const TELEPORT_COMMAND: u32 = 0xA0F0_5E9C;

    /// `EmoteStandardCommand` static actor — the client fires an EventStart
    /// against this (eventName "commandRequest") for every one-shot emote from
    /// the emote menu / `/bow` etc., carrying the emote id (101=Surprised ..
    /// 105=Bow ..) + a showText flag in the LuaParams. Without a dispatch arm
    /// the press falls through `command_script_name` and the emote never plays
    /// outside a quest's scripted `player:DoEmote`. Id observed in the live
    /// EventStart log. (Garlemald-Server #46.)
    const EMOTE_STANDARD_COMMAND: u32 = 0xA0F0_5E26;

    /// Harvest command static actors — the client fires an `EventStart`
    /// against one of these (eventName `"commandRequest"`) when the
    /// player picks Mine / Log / Fish / Quarry / Harvest / Spearfish on a
    /// gathering node. The masked low half is the harvest command id
    /// (`22002..=22007`, [`crate::gathering::HARVEST_TYPE_MINE`] …), and
    /// the static-actor id is `id | 0xA0F00000` like every other command
    /// actor. All six route to the one `commands/DummyCommand.lua` script,
    /// which branches on the node's resolved `harvestType` internally.
    /// Without a dispatch arm the press falls through `command_script_name`
    /// and no minigame ever opens. (Wave 3 gather partial.)
    const GATHER_COMMAND_MASK: u32 = 0xA0F0_0000;

    /// Is `owner_actor_id` one of the six harvest command static actors?
    fn is_gather_command(owner_actor_id: u32) -> bool {
        (owner_actor_id & 0xFFF0_0000) == Self::GATHER_COMMAND_MASK
            && crate::gathering::is_valid_harvest_type(owner_actor_id & 0xFFFF)
    }

    /// SetTarget's `attackTarget` "no attack target" sentinel — the value the
    /// 1.x client writes when the player has no locked combat target (pmeteor
    /// `SetTargetPacket.attackTarget` "Usually 0xE0000000"). Same constant as
    /// `actor_battle::NO_ENMITY_TARGET`. (Garlemald-Server #28.)
    const SET_TARGET_NONE: u32 = 0xE000_0000;

    /// Does a resumed quest/director coroutine burst (drained on the
    /// 0x012E EventUpdate) need the LOGIN command applier rather than the
    /// runtime drain? `apply_runtime_lua_command` has no arms for the
    /// content-area / quest-handoff / director-staging commands and would
    /// silently drop them (its `_ => false` catch-all); only
    /// `apply_login_lua_command` creates content areas, registers
    /// directors, captures the post-warp KickEvent into the zone-in
    /// bundle, etc. Each variant here marks a burst that MUST route
    /// through login:
    ///  * `CreateContentArea` / `DoZoneChangeContent` — the SEQ_005
    ///    content warp (man0l0 doExitDoor);
    ///  * `AddQuest` / `CompleteQuest` — the quest-handoff burst
    ///    (man0l0 Hob → man0l1);
    ///  * `Logout` / `QuitGame` — the Exit-game confirm;
    ///  * `SetLoginDirector` — the after-quest-warp director staging
    ///    (man0l1 Baderon → AfterQuestWarpDirector): without login
    ///    routing the director is never created/registered/spawned and
    ///    its `noticeEvent` kick is direct-dispatched to an unspawned
    ///    owner the client drops, so the post-warp cutscene desktop-
    ///    widget mode never clears and the menu / linkpearl / aetheryte
    ///    stay dead. `SetLoginDirector` appears ONLY in director-staged
    ///    warp bursts, so it's a precise tell with no false positives.
    ///    (Round-3 live test — Baderon breaks the menu.)
    fn is_login_scoped_burst(cmds: &[crate::lua::command::LuaCommand]) -> bool {
        use crate::lua::command::LuaCommand;
        cmds.iter().any(|c| {
            matches!(
                c,
                LuaCommand::CreateContentArea { .. }
                    | LuaCommand::DoZoneChangeContent { .. }
                    | LuaCommand::AddQuest { .. }
                    | LuaCommand::CompleteQuest { .. }
                    | LuaCommand::Logout { .. }
                    | LuaCommand::QuitGame { .. }
                    | LuaCommand::SetLoginDirector { .. }
            )
        })
    }

    fn command_script_name(owner_actor_id: u32) -> Option<&'static str> {
        match owner_actor_id {
            Self::ACTIVATE_COMMAND_A | Self::ACTIVATE_COMMAND_B => Some("ActivateCommand"),
            Self::LOGOUT_COMMAND => Some("LogoutCommand"),
            Self::TELEPORT_COMMAND => Some("TeleportCommand"),
            Self::EMOTE_STANDARD_COMMAND => Some("EmoteStandardCommand"),
            id if Self::is_gather_command(id) => Some("DummyCommand"),
            _ => None,
        }
    }

    /// Run `commands/<Name>.lua::onEventStarted` for a client command static
    /// actor and apply its commands (incl. any `sendSignal`). Generalizes the
    /// hardcoded journal command. (Garlemald-Server #28.)
    #[allow(clippy::too_many_arguments)]
    async fn dispatch_command_script(
        &self,
        handle: &ActorHandle,
        command_actor_id: u32,
        command_name: &'static str,
        event_name: String,
        event_type: u8,
        lua_params: Vec<common::luaparam::LuaParam>,
        // Identity `(zone_id, unique_id)` of the physical actor the command
        // was invoked on, when the dispatch resolved it (harvest commands:
        // the clicked gather node). Stamped onto the `commandActor`
        // userdata so `DummyCommand.lua` can key `GetGatherNodeMetadata`
        // off the node the player actually struck. `None` for every
        // command that isn't node-scoped. (Wave 3 gather partial.)
        command_actor_identity: Option<(u32, String)>,
    ) {
        let Some(lua) = self.lua.as_ref() else {
            return;
        };
        let script_path = lua.resolver().command(command_name);
        if !script_path.exists() {
            tracing::warn!(
                command = command_name,
                owner = command_actor_id,
                script = %script_path.display(),
                "command script not on disk — skipping dispatch",
            );
            return;
        }
        let snapshot = {
            let c = handle.character.read().await;
            build_player_snapshot_from_character(&c)
        };
        let lua_clone = lua.clone();
        let script_path_clone = script_path.clone();
        let result = tokio::task::spawn_blocking(move || {
            lua_clone.call_command_on_event_started(
                &script_path_clone,
                snapshot,
                command_actor_id,
                event_name,
                event_type,
                lua_params,
                command_actor_identity,
            )
        })
        .await;
        let partial = match result {
            Ok(p) => p,
            Err(join_err) => {
                tracing::warn!(
                    command = command_name,
                    error = %join_err,
                    "command onEventStarted dispatch panicked",
                );
                return;
            }
        };
        if let Some(e) = partial.error {
            tracing::warn!(
                command = command_name,
                error = %e,
                "command onEventStarted errored; applying partial commands",
            );
        } else {
            tracing::debug!(
                command = command_name,
                owner = command_actor_id,
                commands = partial.commands.len(),
                "command onEventStarted fired",
            );
        }
        let commands = self.flush_battle_states_on_sheathe(partial.commands).await;
        Box::pin(self.apply_event_script_commands(handle, commands)).await;
    }

    /// Sheathe-side battle flush — pmeteor `AIContainer.InternalDisengage`
    /// (AIContainer.cs:351-363). `commands/ActivateCommand.lua` sheathes
    /// via `player.Disengage(0x0000)`, which only pushes
    /// `ChangeState{PASSIVE}` — without this pre-pass the in-flight
    /// AttackState plus any queued weapon-skill/cast survives the sheathe
    /// and keeps resolving swings forever (live log 2026-07-04: sheathe at
    /// 01:12:12, auto-attack still landing at 01:13:12). For every
    /// ChangeState-to-PASSIVE in a command burst whose actor is mid-battle:
    /// flush the ai_container, drop the soft target (pmeteor
    /// `ChangeTarget(null)`), and route the resulting
    /// `BattleEvent::Disengage` through the dispatcher — its arm already
    /// emits the PASSIVE state trio + enmity-gem clear + hate clear +
    /// ResetHead — then consume the ChangeState command (applying it too
    /// would double-play the sheathe animation). Non-engaged stance flips
    /// fall through to `quest_apply::apply_change_state` unchanged. (#46.)
    pub(crate) async fn flush_battle_states_on_sheathe(
        &self,
        commands: Vec<crate::lua::command::LuaCommand>,
    ) -> Vec<crate::lua::command::LuaCommand> {
        use crate::lua::command::LuaCommand;
        let mut kept = Vec::with_capacity(commands.len());
        for cmd in commands {
            let actor_id = match &cmd {
                LuaCommand::ChangeState {
                    actor_id,
                    main_state: crate::actor::MAIN_STATE_PASSIVE,
                } => *actor_id,
                _ => {
                    kept.push(cmd);
                    continue;
                }
            };
            let Some(target_handle) = self.registry.get(actor_id).await else {
                kept.push(cmd);
                continue;
            };
            let Some(zone_arc) = self.world.zone(target_handle.zone_id).await else {
                kept.push(cmd);
                continue;
            };
            let mut battle_outbox = crate::battle::outbox::BattleOutbox::new();
            let engaged = {
                let mut c = target_handle.character.write().await;
                if c.ai_container.state_stack().is_empty() {
                    false
                } else {
                    c.ai_container.internal_disengage(&mut battle_outbox);
                    c.chara.current_target = crate::actor::INVALID_ACTORID;
                    true
                }
            };
            if !engaged {
                kept.push(cmd);
                continue;
            }
            tracing::debug!(
                actor = format!("0x{actor_id:08X}"),
                "sheathe flushed engaged battle states (InternalDisengage)",
            );
            for ev in battle_outbox.drain() {
                crate::runtime::dispatcher::dispatch_battle_event(
                    &ev,
                    &self.registry,
                    &self.world,
                    &zone_arc,
                    self.lua.as_ref(),
                    Some(&self.db),
                )
                .await;
            }
        }
        kept
    }

    /// One X01 error row — pmeteor `WeaponSkillState.errorResult` shape
    /// (`CommandResult(owner.Id, textId, 0)` flushed via
    /// `DoBattleAction(skill.id, 0, errorResult)`): animation 0, the
    /// pressed command id, the player as the row target, and the error
    /// text in `worldMasterTextId`. Self-only — the validation error is
    /// the actor's own feedback line. (#28 S3.2.)
    async fn send_command_error_result(
        &self,
        session_id: u32,
        actor_id: u32,
        command_id: u16,
        text_id: u16,
    ) {
        let Some(client) = self.world.client(session_id).await else {
            return;
        };
        let row = crate::packets::send::actor_battle::CommandResult {
            target_id: actor_id,
            worldmaster_text_id: text_id,
            ..Default::default()
        };
        let mut pkt = crate::packets::send::actor_battle::build_command_result_x01(
            actor_id, 0, command_id, &row,
        );
        pkt.set_target_id(session_id);
        client.send_bytes(pkt.to_bytes()).await;
    }

    /// End the event-session the press's `start_event` opened — exactly
    /// one 0x0131 per IN 0x012D (retail shape). Routed through the
    /// session + event dispatcher so the packet rides the standard
    /// target-stamped send. (#28 S3.2.)
    async fn end_command_event(&self, handle: &ActorHandle) {
        let mut outbox = EventOutbox::new();
        {
            let mut c = handle.character.write().await;
            c.event_session.end_event(handle.actor_id, &mut outbox);
        }
        for e in outbox.drain() {
            dispatch_event_event(&e, &self.registry, &self.world, &self.db, self.lua.as_ref())
                .await;
        }
    }

    /// Inline execution of a hotbar skill press (#28 S3.2) — the Rust port
    /// of pmeteor's 4-line command scripts (`AttackWeaponSkill.lua` /
    /// `Ability.lua` / `AttackMagic.lua`) + `Player.CanUse`
    /// (Player.cs:2809-2877), routed by the catalog's `commandType`
    /// (2 → weaponskill, 3 → ability, 4 → spell) instead of shipping
    /// staticactors.bin. Exactly one EndEvent answers every press;
    /// validation failures additionally carry one X01 error row (or the
    /// 32503 system line for the active-mode gate). Completion damage
    /// flows through the existing ticker → `ResolveAction` →
    /// `resolve_action` pipeline; S3.3 handles costs/recast there.
    pub(crate) async fn dispatch_hotbar_command(
        &self,
        handle: &ActorHandle,
        session_id: u32,
        lua_params: &[common::luaparam::LuaParam],
        command: crate::gamedata::BattleCommand,
    ) {
        let actor_id = handle.actor_id;

        // Retail rides the real target in the type-6 Actor param — the
        // Int32 slot pmeteor's script signature names `targetActor` is 0
        // on the wire, which is why pmeteor falls through to
        // `currentTarget` (Player.cs:2685-2701). Same fallback here.
        let param_target = lua_params.iter().find_map(|p| match p {
            common::luaparam::LuaParam::Actor(id) if *id != 0 => Some(*id),
            _ => None,
        });

        let (main_state, my_x, my_y, my_z, mp, tp, current_target, can_change, recast_end) = {
            let c = handle.character.read().await;
            (
                c.base.current_main_state,
                c.base.position_x,
                c.base.position_y,
                c.base.position_z,
                c.chara.mp,
                c.chara.tp,
                c.chara.current_target,
                c.ai_container.can_change_state(),
                c.chara
                    .hotbar
                    .iter()
                    .find(|e| (e.command_id & 0xFFFF) as u16 == command.id)
                    .map(|e| e.recast_time)
                    .unwrap_or(0),
            )
        };

        // Active-mode gate (commands/AttackWeaponSkill.lua:15-19):
        // "You are not in active mode" as the WorldMaster system line.
        if main_state != crate::actor::MAIN_STATE_ACTIVE {
            if let Some(client) = self.world.client(session_id).await {
                let mut pkt = crate::packets::send::misc::build_text_sheet_no_source_x28(
                    crate::packets::send::misc::WORLD_MASTER_ACTOR_ID,
                    crate::packets::send::misc::WORLD_MASTER_ACTOR_ID,
                    32503,
                    crate::packets::send::misc::MESSAGE_TYPE_SYSTEM,
                );
                pkt.set_target_id(session_id);
                client.send_bytes(pkt.to_bytes()).await;
            }
            self.end_command_event(handle).await;
            return;
        }

        // Target resolution + the pmeteor `IsValidTarget` null arm
        // (32511 "Target does not exist"). A dead target takes the same
        // line — the lvl-1 kit is enemy-single-target only.
        let target_id = param_target.unwrap_or(current_target);
        let target_handle = if target_id != 0 && target_id != crate::actor::INVALID_ACTORID {
            self.registry.get(target_id).await
        } else {
            None
        };
        let target_state = match &target_handle {
            Some(t) => {
                let c = t.character.read().await;
                c.is_alive()
                    .then_some((c.base.position_x, c.base.position_y, c.base.position_z))
            }
            None => None,
        };
        let Some((t_x, t_y, t_z)) = target_state else {
            self.send_command_error_result(session_id, actor_id, command.id, 32511)
                .await;
            self.end_command_event(handle).await;
            return;
        };

        // pmeteor `Player.WeaponSkill/Ability/Cast`: a busy state stack
        // (mid-cast) rejects with the same wait-a-moment line, then
        // `Player.CanUse` runs the ordered checklist (Player.cs:2809-2877).
        let now_unix = common::utils::unix_timestamp() as u32;
        let dist_xz = ((t_x - my_x).powi(2) + (t_z - my_z).powi(2)).sqrt();
        let half_height = command.range_height as f32 / 2.0;
        let error_text = if !can_change || recast_end > now_unix {
            Some(32535) // "Please wait a moment and try again."
        } else if dist_xz > command.range {
            Some(32539) // "The target is too far away."
        } else if dist_xz < command.min_range {
            Some(32538) // "The target is too close."
        } else if t_y - my_y > half_height {
            Some(32540) // "The target is too far above you."
        } else if my_y - t_y > half_height {
            Some(32541) // "The target is too far below you."
        } else if command.mp_cost as i32 > mp as i32 {
            Some(32545) // "You do not have enough MP."
        } else if command.tp_cost as i32 > tp as i32 {
            Some(32546) // "You do not have enough TP."
        } else {
            None
        };
        if let Some(text_id) = error_text {
            self.send_command_error_result(session_id, actor_id, command.id, text_id)
                .await;
            self.end_command_event(handle).await;
            return;
        }

        // Engage-if-not (command-script line: `if not IsEngaged() then
        // Engage(target)`) + the skill state push. Shared battle clock —
        // the ticker drives completion in this domain (#28 S0.2).
        let now_ms = crate::runtime::clock::server_now_ms();
        let pushed = {
            let mut c = handle.character.write().await;
            if !c.ai_container.is_engaged() {
                let delay = c.get_attack_delay_ms();
                c.ai_container.internal_engage(target_id, now_ms, delay);
                // Hate seed — same rationale as `apply_actor_engage`:
                // without it a controller-less engage has no most-hated
                // entry for downstream reads. (#28.)
                c.hate.update_hate(target_id, 1);
            }
            let bc = command.to_battle_command();
            match command.command_type {
                t if t == crate::battle::CommandType::WEAPON_SKILL.bits() as i16 => {
                    c.ai_container.internal_weapon_skill(target_id, bc, now_ms)
                }
                t if t == crate::battle::CommandType::ABILITY.bits() as i16 => {
                    c.ai_container.internal_ability(target_id, bc, now_ms)
                }
                t if t == crate::battle::CommandType::SPELL.bits() as i16 => {
                    c.ai_container.internal_cast(target_id, bc, now_ms)
                }
                other => {
                    tracing::debug!(
                        player = actor_id,
                        command = command.id,
                        command_type = other,
                        "hotbar press for unroutable commandType — dropped",
                    );
                    false
                }
            }
        };
        if !pushed {
            // State-push raced (stack filled between the gate read and
            // the lock) — same feedback pmeteor gives.
            self.send_command_error_result(session_id, actor_id, command.id, 32535)
                .await;
        }
        tracing::debug!(
            player = actor_id,
            command = command.id,
            command_type = command.command_type,
            target = format!("0x{target_id:08X}"),
            pushed,
            "hotbar command dispatched",
        );

        self.end_command_event(handle).await;
    }

    /// Apply a client `SetTarget` (0x00CD). Port of pmeteor
    /// `PacketProcessor.cs:173`:
    ///
    /// ```text
    /// actor.currentTarget       = packet.actorID      (body[0..4])
    /// actor.isAutoAttackEnabled = packet.attackTarget != 0xE0000000  (body[4..8])
    /// broadcast SetActorTargetAnimated(actorID)
    /// ```
    ///
    /// Garlemald previously only logged this opcode, so the player never
    /// acquired a target and never auto-attacked. Because
    /// `SimpleContent30010.lua::onUpdate` gates ally engagement on
    /// `player:IsEngaged() and player.target`, that left the entire combat
    /// tutorial inert — allies never stood in, mobs were never struck, and so
    /// (with the wolf-retaliation path) never fought back. Recording the
    /// target + pushing the player's `AttackState` is the keystone that drives
    /// the whole loop: player swings → wolf takes damage → wolf gains hate →
    /// wolf retaliates. (Garlemald-Server #28.)
    pub(crate) async fn apply_player_set_target(
        &self,
        session_id: u32,
        selected_target: u32,
        auto_attack: bool,
    ) {
        let Some(handle) = self.registry.by_session(session_id).await else {
            return;
        };
        let actor_id = handle.actor_id;

        // Round 7h: the basic attack must not accept FRIENDLY targets —
        // players could soft-target THEMSELVES or an escort Ally (Sisipu)
        // and auto-attack them down. Mirrors ValidTarget::ENEMY for the
        // basic attack (battle/target_find.rs can_target is only applied
        // to AOE fan-out, never the primary target). Resolved before the
        // player write lock to avoid holding two actor locks.
        let target_friendly = if selected_target != 0 && selected_target != Self::SET_TARGET_NONE {
            match self.registry.get(selected_target).await {
                Some(t) => matches!(
                    t.kind,
                    crate::runtime::actor_registry::ActorKindTag::Player
                        | crate::runtime::actor_registry::ActorKindTag::Ally
                ),
                None => false,
            }
        } else {
            false
        };

        {
            let mut c = handle.character.write().await;

            // Record the soft target so `player.target` resolves and
            // `Character::is_engaged()` reads correctly. INVALID_ACTORID is the
            // canonical "no target" marker (pmeteor `Actor.INVALID_ACTORID`).
            let cleared = selected_target == 0 || selected_target == Self::SET_TARGET_NONE;
            c.chara.current_target = if cleared {
                crate::actor::INVALID_ACTORID
            } else {
                selected_target
            };

            // #46 sheathed-combat leak: the basic attack must not engage
            // while the weapon is sheathed. The retail client only sets
            // attackTarget when drawn, so a sheathed 0x00CD is a silent
            // drop (pmeteor PacketProcessor.cs:266-295 trusts the client
            // and sends no error on this path) — keep the soft target and
            // the reticle broadcast below.
            let in_active_mode = c.base.current_main_state == crate::actor::MAIN_STATE_ACTIVE;
            let valid_combat_target = auto_attack
                && !cleared
                && !target_friendly
                && selected_target != actor_id
                && selected_target != crate::actor::INVALID_ACTORID
                && in_active_mode;

            if valid_combat_target {
                // Shared battle-clock anchor — same fix as
                // `apply_actor_engage` (#28 S0.2): epoch ms here armed
                // the player's swing clock past the ticker's domain.
                let now_ms = crate::runtime::clock::server_now_ms();
                let delay = c.get_attack_delay_ms();
                let cur = c
                    .ai_container
                    .current_state()
                    .map(|s| s.target_actor_id)
                    .filter(|id| *id != 0);
                // Re-engage only when switching targets — re-engaging the same
                // target restarts the swing clock (same gate as
                // `apply_actor_engage` / pmeteor `if (IsEngaged) return`).
                if cur != Some(selected_target) {
                    c.ai_container.clear_states();
                    let started = c
                        .ai_container
                        .internal_engage(selected_target, now_ms, delay);
                    tracing::debug!(
                        player = format!("0x{actor_id:08X}"),
                        target = format!("0x{selected_target:08X}"),
                        delay,
                        started,
                        "player auto-attack engage (0x00CD)",
                    );
                }
            } else if !auto_attack {
                // Auto-attack toggled off (attackTarget == 0xE0000000) — stop
                // swinging but keep the soft target so the reticle stays.
                c.ai_container.clear_states();
            } else if !in_active_mode {
                tracing::debug!(
                    player = format!("0x{actor_id:08X}"),
                    target = format!("0x{selected_target:08X}"),
                    "auto-attack engage rejected — not in active mode (0x00CD)",
                );
            }
        }

        // Broadcast the target reticle so nearby clients (and our own, for the
        // animated draw) render who the player is locked onto. pmeteor sends
        // SetActorTargetAnimated here. `0` on a clear leaves the reticle empty.
        let bytes =
            crate::packets::send::actor::build_set_actor_target_animated(actor_id, selected_target)
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
    }

    /// Port of pmeteor `Player.HandleNpcLs` (Player.cs:1975-1986): the
    /// client opened an NPC-linkshell chat (`npcLsId` = first integer
    /// LuaParam). Find the first active quest whose `npcLsFrom ==
    /// npcLsId` and fire its `onNpcLS(player, quest, from, msgStep)`
    /// hook via `fire_quest_event_hook` (which builds the LuaQuestHandle
    /// with the npc-ls scratchpad fields and bridges the script's own
    /// `player:EndEvent()` through the EventOutbox). If no quest claims
    /// the id, mirror the script's else-branch by closing the event so
    /// the client doesn't soft-lock in the LS window.
    async fn handle_npc_ls_chat(
        &self,
        handle: &ActorHandle,
        lua_params: &[common::luaparam::LuaParam],
    ) {
        use common::luaparam::LuaParam;
        // The clicked linkshell id MAY arrive as the first integer LuaParam
        // (Int32 0x0 / UInt32 0x1 / Byte 0xC / Short 0x1B / Actor 0x6), but
        // the 1.x client's NpcLinkshellChatCommand EventStart param tail is
        // UNRELIABLE: across live runs the same click produced
        // [Int32(0), …] one time and an empty tail the next (the command is
        // fired via executePlayerCommandLocal whose serialized args vary).
        // So treat the id as a best-effort HINT for disambiguation only —
        // never the sole key. (Garlemald-Server #46.)
        let npc_ls_id_hint = lua_params.iter().find_map(|p| match p {
            LuaParam::Int32(v) => u32::try_from(*v).ok(),
            LuaParam::UInt32(v) => Some(*v),
            LuaParam::Byte(v) => Some(*v as u32),
            LuaParam::Short(v) => Some(*v as u32),
            LuaParam::Actor(v) => Some(*v),
            _ => None,
        });
        // The linkshell window only lists pearls with a PENDING message, so a
        // click means "read the pending one". Find the player's quest(s) with
        // an active NpcLs (npcLsFrom != 0). Prefer one whose npcLsFrom matches
        // the client hint — the client speaks ZERO-BASED (it mirrors the
        // playerWork.npcLinkshellChatCalling[N] index that SetNpcLs stores
        // zero-based: NewNpcLsMsg(1) → Calling[0]), while the quest stores the
        // RAW 1-based value NewNpcLsMsg(from) was given (which onNpcLS needs:
        // man0l1 branches on `from == 1`). So a hint H matches stored H+1 (or
        // H raw, defensively). When the hint is absent/garbage, fall back to
        // the sole pending NpcLs quest. (Garlemald-Server #46.)
        let matched = {
            let c = handle.character.read().await;
            let pending: Vec<(u32, u32, u8)> = c
                .quest_journal
                .slots
                .iter()
                .flatten()
                .filter(|q| q.get_npc_ls_from() != 0)
                .map(|q| (q.quest_id(), q.get_npc_ls_from(), q.get_npc_ls_msg_step()))
                .collect();
            npc_ls_id_hint
                .and_then(|id| {
                    pending
                        .iter()
                        .find(|(_, from, _)| *from == id + 1 || *from == id)
                        .copied()
                })
                .or_else(|| pending.first().copied())
        };
        let Some((quest_id, from, msg_step)) = matched else {
            tracing::debug!(
                player = handle.actor_id,
                ?npc_ls_id_hint,
                "NpcLs chat: no pending quest NpcLs — closing event",
            );
            self.end_command_event(handle).await;
            return;
        };
        tracing::debug!(
            player = handle.actor_id,
            quest = quest_id,
            from,
            msg_step,
            "NpcLs chat → firing onNpcLS",
        );
        self.fire_quest_event_hook(
            handle,
            quest_id,
            "onNpcLS",
            vec![
                crate::lua::QuestHookArg::Int(from as i64),
                crate::lua::QuestHookArg::Int(msg_step as i64),
            ],
        )
        .await;
    }

    /// Mirror of pmeteor's `RequestQuestJournalCommand.lua::onEventStarted`
    /// — the client's EventStart carries `(questId, mapCode)` LuaParams:
    /// `mapCode == nil` is the journal text pane (reply `["requestedData",
    /// "qtdata", questId, sequence, …getJournalInformation()]`), a present
    /// mapCode is the journal MAP pane (reply `["requestedData", "qtmap",
    /// questId, …getJournalMapMarkerList()]` — the marker ids select rows
    /// in the client's quest_marker sheet). Both replies ride 0x0133
    /// GenericDataPacket, then a single `EndEvent`. (Garlemald-Server #46.)
    ///
    /// When the request params don't parse (no numeric questId), fall back
    /// to the previous behaviour — one default qtdata per active quest —
    /// which is the shape the man0g0 opener path was validated against.
    async fn send_quest_journal_data(
        &self,
        handle: &ActorHandle,
        session_id: u32,
        lua_params: &[common::luaparam::LuaParam],
    ) {
        use common::luaparam::LuaParam;
        let Some(client) = self.world.client(session_id).await else {
            return;
        };
        let actor_id = handle.actor_id;

        // (questId, mapCode) = the first two numeric params. The qtdata
        // request ends after questId (mapCode rides as Nil/absent), the
        // qtmap request carries the map sheet code second.
        let mut nums = lua_params.iter().filter_map(|p| match p {
            LuaParam::Int32(v) => Some(*v as i64),
            LuaParam::UInt32(v) => Some(*v as i64),
            _ => None,
        });
        let requested_quest_id = nums.next().and_then(|v| u32::try_from(v).ok());
        let map_code = nums.next();

        // Single-quest reply for a parseable request.
        let mut replied = false;
        // The quest whose journal entry was just opened (qtdata request only),
        // so we can fire its `onJournalRequest` hook exactly once — scoped
        // strictly to the requested quest, never fanned out to all actives.
        let mut journal_hook_quest: Option<u32> = None;
        if let Some(quest_id) = requested_quest_id {
            let quest_state = {
                let c = handle.character.read().await;
                c.quest_journal.get(quest_id).map(|q| {
                    (
                        q.get_sequence(),
                        crate::lua::LuaQuestHandle {
                            player_id: actor_id,
                            quest_id,
                            has_quest: true,
                            sequence: q.get_sequence(),
                            flags: q.get_flags(),
                            counters: [
                                q.get_counter(0),
                                q.get_counter(1),
                                q.get_counter(2),
                                q.get_counter(3),
                            ],
                            npc_ls_from: q.get_npc_ls_from(),
                            npc_ls_msg_step: q.get_npc_ls_msg_step(),
                            queue: crate::lua::command::CommandQueue::new(),
                        },
                    )
                })
            };
            if let Some((sequence, quest_handle)) = quest_state {
                let (tag, getter) = if map_code.is_none() {
                    ("qtdata", "getJournalInformation")
                } else {
                    ("qtmap", "getJournalMapMarkerList")
                };
                let extra = self
                    .call_journal_getter(handle, quest_id, getter, quest_handle)
                    .await;
                let mut params = vec![
                    LuaParam::String("requestedData".to_string()),
                    LuaParam::String(tag.to_string()),
                    LuaParam::Int32(quest_id as i32),
                ];
                if tag == "qtdata" {
                    params.push(LuaParam::Int32(sequence as i32));
                }
                if extra.is_empty() {
                    // pmeteor's C# pads at least one Nil when the unpacked
                    // table is empty, so the client's reader always sees a
                    // tail param.
                    params.push(LuaParam::Nil);
                } else {
                    params.extend(extra);
                }
                let mut pkt = crate::packets::send::player::build_generic_data(actor_id, &params);
                // 1.x client silently drops event-flavoured subpackets
                // where SubPacketHeader.target_id != receiving actor's
                // session id (same gotcha as `broadcast_quest_enpc_update`).
                pkt.set_target_id(actor_id);
                client.send_bytes(pkt.to_bytes()).await;
                tracing::debug!(
                    player = actor_id,
                    quest = quest_id,
                    sequence,
                    tag,
                    "RequestQuestJournalCommand → reply sent",
                );
                replied = true;
                // Only the info (`qtdata`) request drives the journal-handoff
                // hook; the map-marker (`qtmap`) request is a read-only view.
                if tag == "qtdata" {
                    journal_hook_quest = Some(quest_id);
                }
            }
        }

        if !replied && requested_quest_id.is_none() {
            // Legacy fallback: default qtdata per active quest.
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
                let params = vec![
                    LuaParam::String("requestedData".to_string()),
                    LuaParam::String("qtdata".to_string()),
                    LuaParam::Int32(quest_id as i32),
                    LuaParam::Int32(sequence as i32),
                    LuaParam::Nil,
                ];
                let mut pkt = crate::packets::send::player::build_generic_data(actor_id, &params);
                pkt.set_target_id(actor_id);
                client.send_bytes(pkt.to_bytes()).await;
                tracing::debug!(
                    player = actor_id,
                    quest = quest_id,
                    sequence = sequence,
                    "RequestQuestJournalCommand → qtdata sent (fallback)",
                );
            }
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

        // Journal-handoff hook — after the qtdata reply, fire the opened
        // quest's `onJournalRequest(player, quest)` so scripts can advance
        // flag state on a journal read (e.g. man0u0's Ul'dah mini-tutorial
        // uses it to light the next NPC marker). Scoped strictly to the
        // requested quest id; quests without the hook are a quiet no-op
        // (`call_quest_hook` returns early on a missing global function).
        if let Some(quest_id) = journal_hook_quest {
            self.fire_quest_hook(handle, quest_id, "onJournalRequest", vec![])
                .await;
        }
    }

    /// Run `getJournalInformation` / `getJournalMapMarkerList` against the
    /// quest's script and capture its returns (see
    /// [`crate::lua::LuaEngine::call_quest_getter`]). Empty on any miss —
    /// quests without the getter fall back to pmeteor's default-empty
    /// journalInfo / marker list.
    async fn call_journal_getter(
        &self,
        handle: &ActorHandle,
        quest_id: u32,
        getter_name: &'static str,
        quest_handle: crate::lua::LuaQuestHandle,
    ) -> Vec<common::luaparam::LuaParam> {
        let Some(engine) = self.lua.as_ref() else {
            return Vec::new();
        };
        let Some(script_name) = engine.catalogs().quest_script_name(quest_id) else {
            return Vec::new();
        };
        let script_path = engine.resolver().quest(&script_name);
        if !script_path.exists() {
            return Vec::new();
        }
        let snapshot = {
            let c = handle.character.read().await;
            build_player_snapshot_from_character(&c)
        };
        let engine_clone = engine.clone();
        let result = tokio::task::spawn_blocking(move || {
            engine_clone.call_quest_getter(&script_path, getter_name, snapshot, quest_handle)
        })
        .await;
        match result {
            Ok(Ok(values)) => values,
            Ok(Err(e)) => {
                tracing::warn!(
                    quest = quest_id,
                    getter = getter_name,
                    err = %e,
                    "journal getter failed",
                );
                Vec::new()
            }
            Err(join_err) => {
                tracing::warn!(
                    quest = quest_id,
                    getter = getter_name,
                    error = %join_err,
                    "journal getter dispatch panicked",
                );
                Vec::new()
            }
        }
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
        event_name: &str,
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
            // Pass the eventName as the hook's trailing arg —
            // `onEmote(player, quest, npc, eventName)` /
            // `onPush(.., eventName)` / `onCommand(.., eventName)` need it to
            // tell WHICH condition fired (e.g. "emoteDefault1" = /bow). Omitting
            // it left eventName nil, so man0l1 SEQ_040's hand-signal test never
            // matched a branch — no DoEmote animation, no counter advance.
            // Harmless for `onTalk` (3-arg), which ignores the extra value.
            // (Garlemald-Server #46.)
            self.fire_quest_event_hook(
                handle,
                quest_id,
                hook_name,
                vec![
                    crate::lua::QuestHookArg::Npc(npc_spec.clone()),
                    crate::lua::QuestHookArg::Str(event_name.to_string()),
                ],
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
            // Seed `uniqueId` (spawn-row column), mirrored onto
            // BaseActor by `Npc::new` since round 5 — scripts that
            // read `npc:GetUniqueId()` now see the real value
            // ("baderon", …) instead of the documented-empty interim.
            // (Garlemald-Server #46, round 5.)
            unique_id: c.base.unique_id.clone(),
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

        // Resume a parked director coroutine waiting on the cinematic's
        // `_WAIT_EVENT` yield (a `callClientFunction(...)` in the director's
        // `onEventStarted`). The 1.x client posts `0x012E EventUpdate` when the
        // cinematic finishes; pmeteor resumes the coroutine here (`Player.
        // UpdateEvent` → `LuaEngine.OnEventUpdate` → `coroutine.Resume`), which
        // runs the director's continuation — its own `player:EndEvent()`,
        // `kickEventContinue`, the tutorial widgets, and crucially the steps
        // that hand movement + the active-mode/F command back to the player.
        // garlemald previously only faked the EndEvent via the event-session
        // echo below and NEVER resumed the coroutine (the referenced
        // `dispatch_event_updated` never existed), so the SEQ_005 director
        // parked forever after the first cinematic → softlock (can't move, F
        // inert). When a coroutine IS resumed it emits its own EndEvent, so we
        // skip the event-session echo to avoid a double EndEvent.
        // (Garlemald-Server #28.)
        //
        // A continuation that re-parks (multi-step `delegateEvent` chains)
        // must keep its event-owner stamp for `handle_event_start`'s resume
        // gate — the open event's owner is still on the EventSession (no
        // EndEvent has cleared it yet), so scope the resume with it.
        // (Garlemald-Server #46.)
        let current_event_owner = {
            let c = handle.character.read().await;
            c.event_session.current_event_owner
        };
        let _owner_ctx = match (self.lua.as_ref(), current_event_owner) {
            (Some(lua), owner) if owner != 0 => Some(
                crate::lua::scheduler::CoroutineScheduler::event_owner_scope(
                    lua.scheduler(),
                    actor_id,
                    owner,
                ),
            ),
            _ => None,
        };
        let resumed = self
            .lua
            .as_ref()
            .and_then(|lua| lua.fire_player_event_and_drain(actor_id, &pkt.lua_params))
            .filter(|cmds| !cmds.is_empty());
        if let Some(cmds) = resumed {
            tracing::debug!(
                player = actor_id,
                commands = cmds.len(),
                "EventUpdate resumed parked director coroutine",
            );
            // A resumed QUEST-hook continuation can carry login-scoped
            // command bursts the shared event-script drain silently drops:
            //
            // 1. The content-warp burst (CreateContentArea /
            //    StartDirectorMain / SetLoginDirector / DoZoneChangeContent)
            //    — man0l0's doExitDoor parks on the NewRectAsk RPC and emits
            //    the whole burst when this EventUpdate resumes it. Only
            //    `apply_login_lua_command` creates the content area, and its
            //    KickEvent CAPTURE (emitted at the END of the content
            //    zone-in bundle, after the director's spawn packet) is the
            //    proven ordering; the shared drain's immediate KickEvent
            //    dispatch would race the director spawn and the client would
            //    silently drop the kick (decomp: KickClientOrderEventReceiver
            //    drops kicks for unspawned owners).
            //
            // 2. The quest-handoff burst (CompleteQuest / AddQuest from
            //    `player:ReplaceQuest`) — man0l0's Hob talk parks on the
            //    processEvent020_9 choice RPC and hands off to Man0l1 on
            //    resume. The runtime drain's AddQuest arm fires the new
            //    quest's `onStart` with world=None, which LOGS AND DROPS the
            //    hook's commands (Man0l1's StartSequence + the inn-warp
            //    processEvent010 RPC never reached the client — the
            //    black-screen-at-Hob softlock). The processor's
            //    apply_add_quest drains `onStart` through
            //    apply_login_lua_command, bridging the RPC to the wire and
            //    parking the onStart coroutine for the next EventUpdate.
            //
            // Gridania never hits either case because its equivalents come
            // out of hook FIRST slices (drained by fire_quest_event_hook via
            // apply_login_lua_command); the Limsa opener is the first to
            // emit them from resumed continuations (it parks on choice RPCs
            // first). Director-coroutine continuations (the #28 mid-fight
            // flows) never emit these commands and keep the shared drain,
            // whose direct KickEvent dispatch `kickEventContinue` relies on.
            // (Garlemald-Server #25.)
            // `Logout` / `QuitGame` are the terminal commands of
            // `LogoutCommand.lua`'s confirm round-trip (and the
            // dead-Return path) — they are applied ONLY by
            // `apply_login_lua_command` (processor.rs:1687-1692), never
            // the runtime drain, so a resumed Exit-game confirm would be
            // silently dropped on `apply_event_script_commands`. Route
            // them (and the content/quest bursts) through the login
            // command applier. (Garlemald-Server #46 live test.)
            // 3. The after-quest warp burst (CreateDirector /
            //    StartDirectorMain / SetLoginDirector / KickEvent /
            //    DoZoneChange) — man0l1's Baderon talk (and every other
            //    AfterQuestWarpDirector staging site: man0l0/man0g0/man0u0/
            //    man0g1, the city populace miounne/momodi, the battle-zone
            //    exit_door/exit_trigger/yda) parks on `processEvent020`
            //    (`callClientFunction` → `_WAIT_EVENT`) and emits the whole
            //    director-staging burst when this EventUpdate resumes it.
            //    `SetLoginDirector` is the load-bearing tell: it ONLY appears
            //    in director-staged-warp bursts. The runtime drain has NO arm
            //    for CreateDirector / StartDirectorMain / SetLoginDirector
            //    (all hit `apply_runtime_lua_command`'s `_ => false`), so the
            //    director is never created/registered/login-set, never spawns
            //    in the deferred zone-in bundle, and the runtime KickEvent arm
            //    (`apply_kick_event`) direct-dispatches the `noticeEvent` kick
            //    immediately — to an unspawned owner the client silently drops
            //    (decomp: KickClientOrderEventReceiver gates on actor[+0x5c]).
            //    `onEventStarted` → `quest:OnNotice` → man0l1 `onNotice(SEQ_003)`
            //    → `EndEvent` then never runs, so the post-Baderon cutscene
            //    desktop-widget mode is never cleared and the menu / linkpearl /
            //    aetheryte stay dead. Routing through `apply_login_lua_command`
            //    is the proven shape: it registers the director, refreshes the
            //    session login-director spec, CAPTURES the kick into
            //    `session.pending_kick_event` (emitted at the END of the
            //    zone-in bundle, after the director spawn), and threads
            //    DoZoneChange through the SAME `quest_apply::apply_do_zone_change`
            //    helper the runtime arm uses. Every other command in the burst
            //    (RunEventFunction / EndEvent / QuestSetNpcLsFrom / PlayerSetNpcLs /
            //    QuestStartSequence / QuestUpdateEnpcs / SendMessage) has an
            //    explicit login-applier arm, so nothing load-bearing falls into
            //    the login catch-all. (Round-3 live test — Baderon breaks menu.)
            self.apply_resumed_event_commands(&handle, cmds).await;
            return Ok(());
        }

        // No parked coroutine — fall back to the event-session echo (the prior
        // behaviour, kept for non-director EventUpdates).
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

    /// RX 0x0007 zone-in-complete — the client signals it finished
    /// loading and it's safe to receive world-spawn packets (24
    /// events/session in the retail audit; wiki: "Unknown 0x007").
    /// Today garlemald still uses `OP_RX_LANGUAGE_CODE` (0x0006) as the
    /// login trigger; 0x0007 is its per-zone-in successor. Two session
    /// latches resolve here (#46 round 4):
    ///
    /// 1. `reload_in_flight` — set by the immediate wipe+0x10 reload
    ///    emitters (`quest_apply::apply_do_zone_change` resident-
    ///    geometry branch / `apply_do_zone_change_content`); while set,
    ///    stale pre-Now-Loading 0x00CA reports are dropped. The echo
    ///    means the client is standing at the warped position, so
    ///    position streaming can resume.
    /// 2. `defer_warps_until_zone_in_ack` + `deferred_login_warp` — a
    ///    warp drained during the LOGIN window (the man0l1 rescue arm's
    ///    `WarpToPublicArea` out of PrivateAreaMasterPast/3) was parked
    ///    instead of firing a second world-load 6-15 ms behind login
    ///    bundle #1; the FIRST 0x0007 of the session applies it now, as
    ///    a normal in-session warp against a fully-loaded client. The
    ///    replayed `apply_do_zone_change` performs the position/zone
    ///    persistence the warp would have done live.
    ///
    /// Content warps no longer defer any NPC reveal to this echo:
    /// `apply_do_zone_change_content` flips `warp_complete` /
    /// `content_warp_acked` inline at warp time and ships the content
    /// NPCs IN the zone-in bundle (the 0x10 force-reload path). The old
    /// same-map escort (spawnType 0x16) held its NPCs out of the bundle
    /// and revealed them here; that path was removed when the escort
    /// moved cross-map to zone 129. (Garlemald #46.)
    ///
    /// `pub(crate)` so integration tests can simulate the client's
    /// zone-in echo directly.
    pub(crate) async fn handle_zone_in_complete(&self, session_id: u32) {
        let (deferred_warp, parked_kick) =
            if let Some(mut snap) = self.world.session(session_id).await {
                snap.reload_in_flight = false;
                snap.defer_warps_until_zone_in_ack = false;
                let deferred = snap.deferred_login_warp.take();
                // #46 escort R1 — release the content-director kick parked by
                // the KickEvent appliers (a 0x012F that would have ridden the
                // content-warp flush AFTER DeleteAllActors is dropped
                // client-side; wire-proven, session 53943). This ack means the
                // client finished loading the instance and the director actor
                // (which rode the zone-in bundle) is client-known, so the kick
                // lands and the director's onEventStarted runs. If a deferred
                // login warp is about to replay (another full reload), KEEP the
                // kick parked — it would be wiped again; the replayed warp's
                // own RX 0x0007 releases it.
                let kick = if deferred.is_none() {
                    snap.pending_content_kick_event.take()
                } else {
                    None
                };
                self.world.upsert_session(snap).await;
                (deferred, kick)
            } else {
                (None, None)
            };
        let had_deferred_warp = deferred_warp.is_some();
        tracing::debug!(
            session = session_id,
            deferred_warp = deferred_warp.is_some(),
            released_content_kick = parked_kick.is_some(),
            "RX 0x0007 zone-in-complete — reload/login latches cleared",
        );
        if let Some(kick) = parked_kick {
            if let Some(client) = self.world.client(session_id).await {
                let mut sub = crate::packets::send::events::build_kick_event(
                    kick.trigger_actor_id,
                    kick.owner_actor_id,
                    &kick.event_name,
                    5,
                    &kick.args,
                );
                sub.set_target_id(session_id);
                client.send_bytes(sub.to_bytes()).await;
                tracing::info!(
                    session = session_id,
                    trigger = format!("0x{:08X}", kick.trigger_actor_id),
                    owner = format!("0x{:08X}", kick.owner_actor_id),
                    event = %kick.event_name,
                    "parked content KickEvent released on content-warp zone-in ack",
                );
            } else {
                tracing::warn!(
                    session = session_id,
                    event = %kick.event_name,
                    "parked content KickEvent dropped — no client handle at release",
                );
            }
        }
        if let Some(w) = deferred_warp {
            tracing::info!(
                session = session_id,
                player = w.player_id,
                zone = w.zone_id,
                private_area = ?w.private_area,
                "applying login-deferred warp on zone-in ack (login-window warp deferral)",
            );
            crate::runtime::quest_apply::apply_do_zone_change(
                w.player_id,
                w.zone_id,
                w.private_area,
                w.private_area_type,
                w.spawn_type,
                w.x,
                w.y,
                w.z,
                w.rotation,
                &self.registry,
                &self.db,
                &self.world,
                self.lua.as_ref(),
            )
            .await;
        }

        // Re-establish the active quests' ENPC conditions in the
        // DESTINATION zone after an explicit (non-seamless) warp finishes.
        // The seamless path already does this (handle_update_position 3c);
        // an explicit cross-zone DoZoneChange did not, so a push trigger
        // whose `onStateChange(SetENpc)` ran while the player was still in
        // the ORIGIN zone found "no live NPC" and was never armed — e.g.
        // man0g1 SEQ_055→060: the kids' chat runs the arm, THEN the warp
        // to the White Wolf Gate zone lands, leaving GATE_TRIGGER (1090202)
        // dead so the escort could never start (Garlemald-Server #41).
        // Re-running the same idempotent re-establish (begin_sequence_swap
        // + onStateChange + diff broadcast) now that the destination zone
        // is current arms it, regardless of stream timing.
        //
        // GUARDED two ways: (1) skipped inside a content instance
        // (`active_content_script` set) — re-running the escort quest's
        // SEQ_065 onStateChange there would re-fire its one-shot
        // FLAG_ESCORT_HANDOFF rescue arm (flag already consumed) and roll
        // the duty back to SEQ_060; (2) skipped when a deferred login warp
        // just replayed — its own RX 0x0007 does the re-establish for the
        // final zone. (Garlemald-Server #41.)
        let in_content_instance = self
            .world
            .session(session_id)
            .await
            .and_then(|s| s.active_content_script)
            .is_some();
        if !had_deferred_warp
            && !in_content_instance
            && let Some(handle) = self.registry.by_session(session_id).await
        {
            let actor_id = handle.actor_id;
            let active_quest_ids: Vec<u32> = {
                let c = handle.character.read().await;
                c.quest_journal
                    .slots
                    .iter()
                    .flatten()
                    .map(|q| q.quest_id())
                    .collect()
            };
            for quest_id in active_quest_ids {
                self.apply_quest_update_enpcs(actor_id, quest_id).await;
            }
        }
    }

    /// `pub(crate)` so integration tests can drive the stale-position /
    /// reload-latch flow directly (mirrors `apply_login_lua_command`).
    pub(crate) async fn handle_update_position(&self, session_id: u32, data: &[u8]) -> Result<()> {
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

        // Hold stale position reports off the character while a
        // deferred zone-in is parked OR an immediate wipe+0x10 reload
        // is in flight (`reload_in_flight`, cleared by RX 0x0007) — the
        // client keeps reporting OLD-zone coordinates through the
        // Now-Loading gap (retail captures show the same), and writing
        // them would relocate the warp destination before the bundle
        // reads it / point `send_instance_update`'s partner-zone scan
        // at the origin (the 34-phantom-NPC stream, #46 round 4).
        if let Some(session) = self.world.session(session_id).await
            && (session.pending_zone_in.is_some() || session.reload_in_flight)
        {
            return Ok(());
        }

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
        //    a zone merge behind the scenes. SKIPPED while the player is in a
        //    content instance: the instance runs in a public field zone (the
        //    man0l1 escort uses zone 141, sea0Field01a) and the instance warp
        //    drops the player at a position that sits inside a public-world
        //    seamless boundary box (the Zephyr Gate). Without this guard the
        //    seamless check fires immediately after the cross-zone instance
        //    warp and yanks the player straight back out (141 -> 128), undoing
        //    the cross-zone load and hanging "Now Loading". A content instance
        //    is an isolated copy, not the open world, so public-zone seamless
        //    boundaries must not apply inside it. (Garlemald-Server #46.)
        let in_content_instance = match self.world.session(session_id).await {
            Some(s) => s
                .active_content_script
                .as_ref()
                .is_some_and(|a| a.parent_zone_id == s.current_zone_id),
            None => false,
        };
        let seamless = if in_content_instance {
            crate::world_manager::SeamlessResult::None
        } else {
            self.world
                .seamless_check(actor_id, session_id, Vector3::new(pkt.x, pkt.y, pkt.z))
                .await
        };
        // 3a. On a seamless zone CHANGE the registry handle's zone_id
        //     must follow (actors_in_zone / broadcast fan-out filter on
        //     it) and the new position persisted so a relog lands in the
        //     destination zone, not back at the old warp point. Mirrors
        //     the warp path (quest_apply::apply_do_zone_change). The
        //     instance list was already cleared by
        //     do_seamless_zone_change, so step 3b streams the new zone.
        let seamless_dest = if let crate::world_manager::SeamlessResult::ZoneChanged(dest) =
            seamless
        {
            self.registry.reassign_zone(actor_id, dest).await;
            {
                let mut c = handle.character.write().await;
                c.base.zone_id = dest;
            }
            let pa = self.world.session(session_id).await.and_then(|s| {
                s.current_private_area_name
                    .clone()
                    .map(|n| (n, s.current_private_area_level))
            });
            let (pa_name, pa_level) = pa.unwrap_or((String::new(), 0));
            if let Err(e) = self
                .db
                .save_player_position(
                    actor_id, dest, &pa_name, pa_level, 0, 0x10, pkt.x, pkt.y, pkt.z, pkt.rot,
                )
                .await
            {
                tracing::warn!(actor = actor_id, dest, err = %e, "seamless: position persist failed");
            }
            Some(dest)
        } else {
            None
        };

        // 3b. Continuous instance update — stream in any NPC/Ally/
        //     BattleNpc that has walked within 50y since the last update
        //     (the fix for "no NPCs at Camp Bearded Rock" — they sit far
        //     past the Zephyr Gate warp point's one-shot scan).
        self.world
            .send_instance_update(&self.registry, self.lua.as_ref(), actor_id, session_id)
            .await;

        // 3c. On a seamless zone CHANGE, re-establish the active quests' ENPC
        //     conditions for the DESTINATION zone. Without this the only thing
        //     arming a cross-zone quest push trigger (e.g. man0l1's Zephyr Gate
        //     trigger 1090004, which lives in zone 128 but is enabled by an
        //     onStateChange that ran while the player was still in town zone
        //     133) is `send_instance_update`'s per-stream-in `push_enabled`
        //     override — a single timing-fragile snapshot. Re-running the same
        //     idempotent re-establish the login path uses (apply_quest_update
        //     _enpcs → begin_sequence_swap + onStateChange + diff broadcast,
        //     processor.rs:835) now that the destination zone is current
        //     re-emits the enabling SetEventStatus + quest graphic via the
        //     session-zone-aware `broadcast_quest_enpc_update`, so the push is
        //     armed regardless of stream timing. Mirrors pmeteor re-arming
        //     quest ENPCs per stream-in with NO zone predicate
        //     (Session.UpdateInstance → GetQuestsForNpc). (Garlemald-Server #46.)
        if let Some(dest) = seamless_dest {
            let active_quest_ids: Vec<u32> = {
                let c = handle.character.read().await;
                c.quest_journal
                    .slots
                    .iter()
                    .flatten()
                    .map(|q| q.quest_id())
                    .collect()
            };
            for quest_id in active_quest_ids {
                self.apply_quest_update_enpcs(actor_id, quest_id).await;
            }
            // 3d. pmeteor parity: `DoSeamlessZoneChange` ends with
            //     `LuaEngine.CallLuaFunction(player, newZone, "onZoneIn",
            //     true)` (WorldManager.cs:947) — run the DESTINATION
            //     zone.lua's onZoneIn on every flip, same shared hook the
            //     login arm uses. (Garlemald-Server #46, round 4.)
            self.run_zone_on_zone_in_hook(&handle, session_id, dest)
                .await;
        }

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
            // The sender is the implicit target for commands whose
            // trailing `<name>` arg is omitted — Meteor's command
            // scripts receive the invoking session's player the same
            // way (`onTrigger(player, ...)`).
            let invoker = {
                let c = handle.character.read().await;
                c.base.display_name().to_string()
            };
            let feedback = if let Some(cmd) = &self.cmd {
                match cmd.run_as(&line, Some(&invoker)).await {
                    Ok(response) if !response.is_empty() => {
                        tracing::info!(%response, "command result");
                        Some((ChatKind::System, response))
                    }
                    Ok(_) => None,
                    Err(e) => {
                        tracing::warn!(error = %e, "gm command failed");
                        Some((ChatKind::SystemError, format!("command failed: {e}")))
                    }
                }
            } else {
                tracing::warn!("gm command requested via chat but CommandProcessor is not wired",);
                Some((
                    ChatKind::SystemError,
                    "GM commands are unavailable on this map-server".to_string(),
                ))
            };
            // Echo the result into the sender's chat log so failures
            // are visible in-game instead of only in the server log.
            // Meteor does the same via `player:SendMessage(MESSAGE_TYPE_
            // SYSTEM_ERROR, ...)` from each command script.
            if let Some((kind, message)) = feedback {
                let mut ob = SocialOutbox::new();
                ob.push(SocialEvent::ChatSystemToPlayer {
                    target_actor_id: handle.actor_id,
                    kind,
                    message,
                });
                for e in ob.drain() {
                    dispatch_social_event(&e, &self.registry, &self.world, &self.db).await;
                }
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
        // Real per-achievement progress from the DB. `chara_id ==
        // actor_id == session id` in this server's lobby flow, so the
        // actor id keys the read. Missing rows degrade to (0, 0).
        let (count, flags) = self
            .db
            .get_achievement_progress(handle.actor_id, pkt.achievement_id)
            .await
            .unwrap_or((0, 0));
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
/// reads: `GetPlayTime` (0 → "new player", nonzero after the first
/// `SavePlayTime` persists), `GetInitialTown`,
/// `HasQuest`, `GetZoneID`, plus the `playerWork.tribe` field read in
/// the tutorial branch.
pub(crate) fn build_player_snapshot_for_login(
    c: &Character,
) -> crate::lua::userdata::PlayerSnapshot {
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
        // Hydrated from `characters.playTime` at session-begin (see the
        // `character.chara.play_time = loaded.play_time` line in the
        // LoadedPlayer hydration) — a hardcoded 0 here re-triggered
        // `player.lua::onLogin`'s first-login branch every login.
        play_time: c.chara.play_time,
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
        // Phase C2 — login snapshot is pre-spawn; reading from the
        // live `Character` would also work but defaults are clearer
        // because there's no AI state to read yet.
        speed: c.get_speed(),
        target_actor_id: 0,
        current_event_owner: 0,
        current_event_name: String::new(),
        current_event_type: 0,
        completed_quests: Vec::new(),
        active_quests: Vec::new(),
        active_quest_states: Vec::new(),
        // Hydrated from `characters_aetherytes` at session-begin (see
        // the `load_character_aetherytes` block in the LoadedPlayer
        // hydration) — a hardcoded empty Vec here made every
        // `HasAetheryteNodeUnlocked` gate fail after a relog.
        // (Garlemald-Server #46, round 5.)
        unlocked_aetherytes: c.chara.unlocked_aetherytes.iter().copied().collect(),
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
pub(crate) fn build_player_snapshot_from_character(
    c: &Character,
) -> crate::lua::userdata::PlayerSnapshot {
    let mut snapshot = build_player_snapshot_for_login(c);
    // Overlay the live EventSession state. The Lua bindings for
    // `player:RunEventFunction(...)` and `player:EndEvent()` read
    // `current_event_owner` / `current_event_name` / `current_event_type`
    // from the snapshot to fill the wire packet's owner/name/type fields
    // — pmeteor `Player.cs::RunEventFunction` does the same with
    // `currentEventOwner` / `currentEventName` / `currentEventType` it
    // captured in `StartEvent()`. Without this overlay, every
    // `RunEventFunction` LuaCommand emitted by a quest/director hook
    // ships with `event_name=""`, which the 1.x client silently no-ops
    // — every cutscene `delegateEvent` (`processTtrNomal002`,
    // `processTtrBtl001`, …) was being dropped on the wire.
    snapshot.current_event_owner = c.event_session.current_event_owner;
    snapshot.current_event_name = c.event_session.current_event_name.clone();
    snapshot.current_event_type = c.event_session.current_event_type;
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
            counters: [
                q.get_counter(0),
                q.get_counter(1),
                q.get_counter(2),
                q.get_counter(3),
            ],
            npc_ls_from: q.get_npc_ls_from(),
            npc_ls_msg_step: q.get_npc_ls_msg_step(),
        })
        .collect();
    snapshot.completed_quests = c.quest_journal.iter_completed().collect();

    // Overlay live combat state so Lua content scripts see real engagement.
    // pmeteor's `Character.IsEngaged()` delegates to `aiContainer.IsEngaged()`
    // (an active AttackState), and `player.target` reads `currentTarget`. The
    // login snapshot hard-codes both to inert defaults; without this overlay
    // `SimpleContent30010.lua::onUpdate`'s `player:IsEngaged() and
    // player.target` gate is permanently false and the allies never engage.
    // (Garlemald-Server #28.)
    snapshot.is_engaged = c.ai_container.is_engaged();
    snapshot.target_actor_id = match c.chara.current_target {
        0 | crate::actor::INVALID_ACTORID => 0,
        id => id,
    };
    snapshot
}

#[cfg(test)]
mod login_burst_routing_tests {
    use super::*;
    use crate::lua::command::LuaCommand;

    /// Round-3 live test — a resumed after-quest-warp burst (the man0l1
    /// Baderon talk) must route through the LOGIN applier. `SetLoginDirector`
    /// is the tell; without it the AfterQuestWarpDirector is never
    /// created/spawned and its noticeEvent kick is dropped, leaving the
    /// client stuck in the post-warp cutscene desktop-widget mode (dead
    /// menu / linkpearl / aetheryte).
    #[test]
    fn set_login_director_burst_routes_to_login() {
        let burst = vec![
            LuaCommand::QuestStartSequence {
                player_id: 1,
                quest_id: 110_002,
                sequence: 3,
            },
            LuaCommand::SetLoginDirector {
                player_id: 1,
                director_actor_id: 0x6428_0002,
                class_name: "AfterQuestWarpDirector".to_string(),
                class_path: "/Director/AfterQuestWarpDirector".to_string(),
            },
            LuaCommand::KickEvent {
                player_id: 1,
                actor_id: 0x6428_0002,
                trigger: "noticeEvent".to_string(),
                args: vec![],
            },
        ];
        assert!(
            PacketProcessor::is_login_scoped_burst(&burst),
            "a burst containing SetLoginDirector must route through the login applier",
        );
    }

    /// A plain combat-tutorial continuation (no content/quest-handoff/
    /// director-staging command) stays on the runtime drain — the
    /// SetLoginDirector tell must not over-match ordinary resume bursts.
    #[test]
    fn plain_resume_burst_stays_on_runtime() {
        let burst = vec![
            LuaCommand::SendSignal {
                name: "battleComplete".to_string(),
            },
            LuaCommand::QuestStartSequence {
                player_id: 1,
                quest_id: 110_001,
                sequence: 10,
            },
        ];
        assert!(
            !PacketProcessor::is_login_scoped_burst(&burst),
            "ordinary resume bursts must NOT route through the login applier",
        );
    }
}

#[cfg(test)]
mod gather_command_routing_tests {
    use super::*;

    /// The six harvest command static actors (`0xA0F00000 | 22002..=22007`)
    /// route to `commands/DummyCommand.lua`, and nothing else does. Without
    /// this arm a Mine/Log/Fish/Quarry/Harvest/Spearfish press falls
    /// through `command_script_name` and no minigame ever opens. (Wave 3.)
    #[test]
    fn gather_command_static_actors_route_to_dummy_command() {
        for harvest_type in [
            crate::gathering::HARVEST_TYPE_MINE,
            crate::gathering::HARVEST_TYPE_LOG,
            crate::gathering::HARVEST_TYPE_FISH,
            crate::gathering::HARVEST_TYPE_QUARRY,
            crate::gathering::HARVEST_TYPE_HARVEST,
            crate::gathering::HARVEST_TYPE_SPEARFISH,
        ] {
            let owner = 0xA0F0_0000 | harvest_type;
            assert!(
                PacketProcessor::is_gather_command(owner),
                "0x{owner:08X} must be recognised as a harvest command",
            );
            assert_eq!(
                PacketProcessor::command_script_name(owner),
                Some("DummyCommand"),
                "0x{owner:08X} must route to DummyCommand",
            );
        }
    }

    /// A non-harvest low half (or the wrong high mask) is NOT a harvest
    /// command — the range check must not swallow the journal / activate /
    /// teleport command actors or an out-of-band id.
    #[test]
    fn non_gather_ids_are_not_harvest_commands() {
        // Adjacent-but-invalid harvest ids.
        assert!(!PacketProcessor::is_gather_command(0xA0F0_0000 | 22001));
        assert!(!PacketProcessor::is_gather_command(0xA0F0_0000 | 22008));
        // Other real command static actors keep their own scripts.
        assert_eq!(
            PacketProcessor::command_script_name(PacketProcessor::LOGOUT_COMMAND),
            Some("LogoutCommand"),
        );
        assert_eq!(
            PacketProcessor::command_script_name(PacketProcessor::EMOTE_STANDARD_COMMAND),
            Some("EmoteStandardCommand"),
        );
        // Right low half but wrong high mask (not a command actor at all).
        assert!(!PacketProcessor::is_gather_command(0x4000_0000 | 22002));
    }
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
