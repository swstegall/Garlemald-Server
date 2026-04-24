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

//! GM console command parser. Ported from `CommandProcessor.cs` as a
//! string-dispatch table. The full retail catalogue is large; this wires
//! the commands that exercise the in-progress subsystems (revive / die /
//! givegil / giveexp) plus `reload` for live Lua hot-reload.
//!
//! Target-resolution convention: when a command expects a player target,
//! pass the character name as the last positional argument. We resolve
//! name → character id via the DB, then look up the live `ActorHandle`
//! through `ActorRegistry::by_session` (safe because for Players
//! `character_id == session_id == actor_id`, see `processor.rs:133`).
#![allow(dead_code)]

use std::sync::Arc;

use anyhow::Result;

use crate::database::Database;
use crate::lua::LuaEngine;
use crate::runtime::actor_registry::ActorRegistry;
use crate::world_manager::WorldManager;

pub struct CommandProcessor {
    pub world: Arc<WorldManager>,
    pub registry: Arc<ActorRegistry>,
    pub db: Arc<Database>,
    pub lua: Arc<LuaEngine>,
}

impl CommandProcessor {
    pub fn new(
        world: Arc<WorldManager>,
        registry: Arc<ActorRegistry>,
        db: Arc<Database>,
        lua: Arc<LuaEngine>,
    ) -> Self {
        Self {
            world,
            registry,
            db,
            lua,
        }
    }

    /// Run a single console command. Returns the human-readable response
    /// that `main.rs` forwards to the log. Unknown commands echo back so
    /// the operator notices typos.
    pub async fn run(&self, line: &str) -> Result<String> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(String::new());
        }
        let mut tokens = trimmed.split_whitespace();
        let cmd = tokens.next().unwrap_or("").to_lowercase();
        let args = Args::new(tokens.collect::<Vec<&str>>());

        let response = match cmd.as_str() {
            "help" => Self::help(),
            "version" => format!("map-server {}", env!("CARGO_PKG_VERSION")),
            "who" => format!("{} zone(s) loaded", self.world.zone_count().await),
            "reload" => {
                let n = self.lua.reload_scripts();
                format!("reloaded: {n} lua script(s) dropped from cache")
            }
            "revive" => self.handle_revive(&args).await,
            "home" => self.handle_home(&args).await,
            "sethome" => self.handle_set_home(&args).await,
            "die" => self.handle_die(&args).await,
            "givegil" => self.handle_givegil(&args).await,
            "giveexp" => self.handle_giveexp(&args).await,
            "hireretainer" => self.handle_hire_retainer(&args).await,
            "dismissretainer" => self.handle_dismiss_retainer(&args).await,
            "listretainers" => self.handle_list_retainers(&args).await,
            "setsleeping" => self.handle_set_sleeping(&args).await,
            "dream" => self.handle_dream(&args).await,
            "wake" => self.handle_wake(&args).await,
            "accruerest" => self.handle_accrue_rest(&args).await,
            "issuechocobo" => self.handle_issue_chocobo(&args).await,
            "rentchocobo" => self.handle_rent_chocobo(&args).await,
            "dismount" => self.handle_dismount(&args).await,
            "joingc" => self.handle_join_gc(&args).await,
            "setgcrank" => self.handle_set_gc_rank(&args).await,
            "addgcseals" => self.handle_add_gc_seals(&args).await,
            "warp" => self.handle_warp(&args).await,
            "talkto" => self.handle_talkto(&args).await,
            other => format!("unknown command: {other} (args={:?})", args.rest()),
        };
        Ok(response)
    }

    fn help() -> String {
        "commands: help, who, version, reload, \
         revive <name>, home <name>, sethome <aetheryte_id> <name>, \
         die <name>, givegil <qty> <name>, \
         giveexp <qty> <class_id> <name>, \
         hireretainer <retainer_id> <name>, \
         dismissretainer <retainer_id> <name>, \
         listretainers <name>, \
         setsleeping <name>, dream <id> <name>, wake <name>, \
         accruerest <minutes> <name>, \
         issuechocobo <appearance> <chocobo_name> <player_name>, \
         rentchocobo <minutes> <name>, dismount <name>, \
         joingc <gc> <name>, setgcrank <gc> <rank> <name>, \
         addgcseals <gc> <amount> <name>, \
         warp <zone> <x> <y> <z> <name>, \
         talkto <actor_class_id> <name>"
            .into()
    }

    async fn handle_revive(&self, args: &Args<'_>) -> String {
        let Some(name) = args.rest_joined(0) else {
            return "usage: revive <name>".into();
        };
        match self.resolve_live_target(&name).await {
            TargetLookup::Ok { actor_id, zone } => {
                crate::runtime::dispatcher::apply_revive(
                    actor_id,
                    &self.registry,
                    &self.world,
                    &zone,
                )
                .await;
                format!("revived {name} (actor {actor_id})")
            }
            TargetLookup::Err(e) => e,
        }
    }

    /// `home <name>` — server-driven home-point revive. Restores HP/MP
    /// + state via `apply_revive`, then warps the player to their
    /// stored homepoint aetheryte. Mirrors the "bandaid fix for
    /// returning while dead" branch in `TeleportCommand.lua` so we
    /// can verify the death/revive flow without standing up the
    /// client's death-overlay menu network.
    async fn handle_home(&self, args: &Args<'_>) -> String {
        let Some(name) = args.rest_joined(0) else {
            return "usage: home <name>".into();
        };
        match self.resolve_live_target(&name).await {
            TargetLookup::Ok { actor_id, zone } => {
                let outcome = crate::runtime::dispatcher::apply_home_point_revive(
                    actor_id,
                    &self.registry,
                    &self.world,
                    &zone,
                )
                .await;
                match outcome {
                    crate::runtime::dispatcher::HomePointReviveOutcome::Warped {
                        homepoint,
                        zone_id,
                        x,
                        y,
                        z,
                    } => format!(
                        "home-point revived {name} (actor {actor_id}) to aetheryte {homepoint} \
                         → zone {zone_id} ({x:.2}, {y:.2}, {z:.2})"
                    ),
                    crate::runtime::dispatcher::HomePointReviveOutcome::InPlace => format!(
                        "revived {name} in place — no usable homepoint set (was 0 or unknown id)"
                    ),
                    crate::runtime::dispatcher::HomePointReviveOutcome::UnknownPlayer => {
                        format!("{name} dropped from registry mid-call")
                    }
                }
            }
            TargetLookup::Err(e) => e,
        }
    }

    /// `sethome <aetheryte_id> <name>` — admin shortcut to set a
    /// player's homepoint without walking them through the
    /// `AetheryteChild.lua` menu. Persists via the same DB path the
    /// in-game flow uses (`Database::save_player_home_points`) and
    /// mirrors into CharaState if the character is online.
    async fn handle_set_home(&self, args: &Args<'_>) -> String {
        let homepoint = match args.parse_u32(0) {
            Ok(id) => id,
            Err(e) => return format!("usage: sethome <aetheryte_id> <name> — {e}"),
        };
        let Some(name) = args.rest_joined(1) else {
            return "usage: sethome <aetheryte_id> <name>".into();
        };
        let Some(chara_id) = self.lookup_character_id(&name).await else {
            return format!("unknown character: {name}");
        };
        // Read the existing inn id so we don't clobber it on the
        // homepoint write (the two share a DB row but represent
        // independent player choices).
        let inn = self
            .db
            .load_player_character(chara_id)
            .await
            .ok()
            .flatten()
            .map(|p| p.homepoint_inn)
            .unwrap_or(0);
        if let Err(e) = self
            .db
            .save_player_home_points(chara_id, homepoint, inn)
            .await
        {
            return format!("sethome failed: {e}");
        }
        if let Some(handle) = self.registry.get(chara_id).await {
            let mut c = handle.character.write().await;
            c.chara.homepoint = homepoint;
        }
        let resolved = crate::actor::aetheryte::lookup(homepoint)
            .map(|s| format!("zone {} at ({:.2}, {:.2}, {:.2})", s.zone_id, s.x, s.y, s.z))
            .unwrap_or_else(|| "(unknown aetheryte id; warp will in-place revive)".into());
        format!("set {name}'s homepoint to {homepoint} — {resolved}")
    }

    async fn handle_die(&self, args: &Args<'_>) -> String {
        let Some(name) = args.rest_joined(0) else {
            return "usage: die <name>".into();
        };
        match self.resolve_live_target(&name).await {
            TargetLookup::Ok { actor_id, zone } => {
                crate::runtime::dispatcher::apply_die(
                    actor_id,
                    &self.registry,
                    &self.world,
                    &zone,
                )
                .await;
                format!("killed {name} (actor {actor_id})")
            }
            TargetLookup::Err(e) => e,
        }
    }

    async fn handle_givegil(&self, args: &Args<'_>) -> String {
        let qty = match args.parse_i32(0) {
            Ok(q) => q,
            Err(e) => return format!("usage: givegil <qty> <name> — {e}"),
        };
        let Some(name) = args.rest_joined(1) else {
            return "usage: givegil <qty> <name>".into();
        };
        let Some(chara_id) = self.lookup_character_id(&name).await else {
            return format!("unknown character: {name}");
        };
        match self.db.add_gil(chara_id, qty).await {
            Ok(total) => format!("gave {qty} gil to {name} (total now {total})"),
            Err(e) => format!("givegil failed: {e}"),
        }
    }

    async fn handle_giveexp(&self, args: &Args<'_>) -> String {
        let qty = match args.parse_i32(0) {
            Ok(q) => q,
            Err(e) => return format!("usage: giveexp <qty> <class_id> <name> — {e}"),
        };
        let class_id = match args.parse_u8(1) {
            Ok(c) => c,
            Err(e) => return format!("usage: giveexp <qty> <class_id> <name> — {e}"),
        };
        let Some(name) = args.rest_joined(2) else {
            return "usage: giveexp <qty> <class_id> <name>".into();
        };
        let Some(chara_id) = self.lookup_character_id(&name).await else {
            return format!("unknown character: {name}");
        };
        // Read current exp + delta, then persist. The per-class column
        // is derived inside `Database::set_exp`; we don't expose the
        // column mapping here.
        let current = self
            .db
            .load_class_levels_and_exp(chara_id)
            .await
            .ok()
            .and_then(|row| row.skill_point.get(class_id as usize).copied())
            .unwrap_or(0);
        let new_exp = current.saturating_add(qty).max(0);
        match self.db.set_exp(chara_id, class_id, new_exp).await {
            Ok(()) => format!(
                "gave {qty} exp to {name} (class {class_id}; total now {new_exp})"
            ),
            Err(e) => format!("giveexp failed: {e}"),
        }
    }

    async fn handle_hire_retainer(&self, args: &Args<'_>) -> String {
        let retainer_id = match args.parse_i32(0) {
            Ok(id) => id,
            Err(e) => return format!("usage: hireretainer <retainer_id> <name> — {e}"),
        };
        let Some(name) = args.rest_joined(1) else {
            return "usage: hireretainer <retainer_id> <name>".into();
        };
        let Some(chara_id) = self.lookup_character_id(&name).await else {
            return format!("unknown character: {name}");
        };
        match self.db.hire_retainer(chara_id, retainer_id as u32).await {
            Ok(true) => format!("hired retainer {retainer_id} for {name}"),
            Ok(false) => {
                format!("{name} already owns retainer {retainer_id} (no-op)")
            }
            Err(e) => format!("hireretainer failed: {e}"),
        }
    }

    async fn handle_dismiss_retainer(&self, args: &Args<'_>) -> String {
        let retainer_id = match args.parse_i32(0) {
            Ok(id) => id,
            Err(e) => return format!("usage: dismissretainer <retainer_id> <name> — {e}"),
        };
        let Some(name) = args.rest_joined(1) else {
            return "usage: dismissretainer <retainer_id> <name>".into();
        };
        let Some(chara_id) = self.lookup_character_id(&name).await else {
            return format!("unknown character: {name}");
        };
        match self.db.dismiss_retainer(chara_id, retainer_id as u32).await {
            Ok(true) => format!("dismissed retainer {retainer_id} from {name}"),
            Ok(false) => {
                format!("{name} does not own retainer {retainer_id} (no-op)")
            }
            Err(e) => format!("dismissretainer failed: {e}"),
        }
    }

    async fn handle_list_retainers(&self, args: &Args<'_>) -> String {
        let Some(name) = args.rest_joined(0) else {
            return "usage: listretainers <name>".into();
        };
        let Some(chara_id) = self.lookup_character_id(&name).await else {
            return format!("unknown character: {name}");
        };
        match self.db.list_character_retainers(chara_id).await {
            Ok(list) if list.is_empty() => format!("{name} owns no retainers"),
            Ok(list) => {
                let mut out = format!("{name} owns {} retainer(s):\n", list.len());
                for (i, r) in list.iter().enumerate() {
                    out.push_str(&format!(
                        "  #{idx}: id={id} name={name} actorClass={ac} level={lv}\n",
                        idx = i + 1,
                        id = r.id,
                        name = r.name,
                        ac = r.actor_class_id,
                        lv = r.level,
                    ));
                }
                out
            }
            Err(e) => format!("listretainers failed: {e}"),
        }
    }

    async fn handle_set_sleeping(&self, args: &Args<'_>) -> String {
        let Some(name) = args.rest_joined(0) else {
            return "usage: setsleeping <name>".into();
        };
        let Some(chara_id) = self.lookup_character_id(&name).await else {
            return format!("unknown character: {name}");
        };
        let Some(handle) = self.registry.get(chara_id).await else {
            return format!("{name} is not online");
        };
        let Some(zone_arc) = self.world.zone(handle.zone_id).await else {
            return format!("{name}'s zone {} not loaded", handle.zone_id);
        };
        let is_inn = { zone_arc.read().await.core.is_inn };
        if !is_inn {
            return format!("{name} is not in an inn zone (zone {})", handle.zone_id);
        }
        let (x, y, z) = {
            let c = handle.character.read().await;
            (c.base.position_x, c.base.position_y, c.base.position_z)
        };
        let inn_code = crate::actor::inn::inn_code_from_position((x, y, z), true);
        let Some(bed) = crate::actor::inn::sleeping_position_for_inn(inn_code) else {
            return format!("{name} is in zone {} but not in any known inn room", handle.zone_id);
        };
        {
            let mut c = handle.character.write().await;
            c.base.position_x = bed.0;
            c.base.position_y = bed.1;
            c.base.position_z = bed.2;
            c.base.rotation = bed.3;
        }
        format!("snapped {name} to inn-room {inn_code} bed ({:.2}, {:.2}, {:.2})", bed.0, bed.1, bed.2)
    }

    async fn handle_dream(&self, args: &Args<'_>) -> String {
        let dream_id = match args.parse_u8(0) {
            Ok(d) => d,
            Err(e) => return format!("usage: dream <id> <name> — {e}"),
        };
        let Some(name) = args.rest_joined(1) else {
            return "usage: dream <id> <name>".into();
        };
        let Some(chara_id) = self.lookup_character_id(&name).await else {
            return format!("unknown character: {name}");
        };
        let Some(handle) = self.registry.by_session(chara_id).await else {
            return format!("{name} is not online");
        };
        let session_id = handle.session_id;
        if session_id == 0 {
            return format!("{name} has no session");
        }
        if let Some(mut s) = self.world.session(session_id).await {
            s.current_dream_id = Some(dream_id);
            self.world.upsert_session(s).await;
        }
        format!("set dream id {dream_id} on {name} (actor {})", handle.actor_id)
    }

    async fn handle_wake(&self, args: &Args<'_>) -> String {
        let Some(name) = args.rest_joined(0) else {
            return "usage: wake <name>".into();
        };
        let Some(chara_id) = self.lookup_character_id(&name).await else {
            return format!("unknown character: {name}");
        };
        let Some(handle) = self.registry.by_session(chara_id).await else {
            return format!("{name} is not online");
        };
        let session_id = handle.session_id;
        if session_id == 0 {
            return format!("{name} has no session");
        }
        if let Some(mut s) = self.world.session(session_id).await {
            s.current_dream_id = None;
            s.is_sleeping = false;
            self.world.upsert_session(s).await;
        }
        format!("cleared dream state on {name}")
    }

    async fn handle_issue_chocobo(&self, args: &Args<'_>) -> String {
        // issuechocobo <appearance> <chocobo_name> <player_name>
        // `chocobo_name` is a single token to keep parsing simple;
        // multi-word player names can still have spaces in the tail.
        let appearance = match args.parse_u8(0) {
            Ok(a) => a,
            Err(e) => {
                return format!(
                    "usage: issuechocobo <appearance> <chocobo_name> <player_name> — {e}"
                );
            }
        };
        let Some(chocobo_name) = args.token(1) else {
            return "usage: issuechocobo <appearance> <chocobo_name> <player_name>".into();
        };
        let Some(player_name) = args.rest_joined(2) else {
            return "usage: issuechocobo <appearance> <chocobo_name> <player_name>".into();
        };
        let Some(chara_id) = self.lookup_character_id(&player_name).await else {
            return format!("unknown character: {player_name}");
        };
        if let Err(e) = self
            .db
            .issue_player_chocobo(chara_id, appearance, chocobo_name)
            .await
        {
            return format!("issuechocobo failed: {e}");
        }
        // If the character is online, mirror into CharaState so the
        // next snapshot reads are right without a re-login.
        if let Some(handle) = self.registry.get(chara_id).await {
            let mut c = handle.character.write().await;
            c.chara.has_chocobo = true;
            c.chara.chocobo_appearance = appearance;
            c.chara.chocobo_name = chocobo_name.to_string();
        }
        format!(
            "issued chocobo (appearance={appearance}, name={chocobo_name}) to {player_name}"
        )
    }

    async fn handle_rent_chocobo(&self, args: &Args<'_>) -> String {
        let minutes = match args.parse_u8(0) {
            Ok(m) => m,
            Err(e) => return format!("usage: rentchocobo <minutes> <name> — {e}"),
        };
        let Some(name) = args.rest_joined(1) else {
            return "usage: rentchocobo <minutes> <name>".into();
        };
        let Some(chara_id) = self.lookup_character_id(&name).await else {
            return format!("unknown character: {name}");
        };
        let Some(handle) = self.registry.get(chara_id).await else {
            return format!("{name} is not online");
        };
        let now = common::utils::unix_timestamp() as u32;
        {
            let mut c = handle.character.write().await;
            c.chara.rental_expire_time = now + (minutes as u32 * 60);
            c.chara.rental_min_left = minutes;
            c.chara.mount_state = 1;
            c.base.current_main_state = crate::actor::MAIN_STATE_MOUNTED;
            c.chara.new_main_state = crate::actor::MAIN_STATE_MOUNTED;
        }
        format!(
            "rented chocobo for {name} ({minutes}m; expires at unix {})",
            now + (minutes as u32 * 60)
        )
    }

    async fn handle_dismount(&self, args: &Args<'_>) -> String {
        let Some(name) = args.rest_joined(0) else {
            return "usage: dismount <name>".into();
        };
        let Some(chara_id) = self.lookup_character_id(&name).await else {
            return format!("unknown character: {name}");
        };
        let Some(handle) = self.registry.get(chara_id).await else {
            return format!("{name} is not online");
        };
        {
            let mut c = handle.character.write().await;
            c.chara.mount_state = 0;
            c.chara.rental_expire_time = 0;
            c.chara.rental_min_left = 0;
            c.base.current_main_state = crate::actor::MAIN_STATE_PASSIVE;
            c.chara.new_main_state = crate::actor::MAIN_STATE_PASSIVE;
        }
        format!("dismounted {name}")
    }

    /// `warp <zone> <x> <y> <z> <name>` — instant zone-change for
    /// headless testing. Mirrors Meteor's `!warp` GM command
    /// (`Data/scripts/commands/gm/warp.lua`) but without a director
    /// chain: we just mutate the session's destination + current
    /// zone and ship a `DoZoneChange` LuaCommand-style payload. For
    /// the `AfterQuestWarpDirector` flow this works as a standalone
    /// warp without the director piggyback (quest scripts can still
    /// create the director separately).
    async fn handle_warp(&self, args: &Args<'_>) -> String {
        let zone_id = match args.parse_u32(0) {
            Ok(z) => z,
            Err(e) => return format!("usage: warp <zone> <x> <y> <z> <name> — {e}"),
        };
        let x = match args.parse_f32(1) {
            Ok(v) => v,
            Err(e) => return format!("usage: warp <zone> <x> <y> <z> <name> — {e}"),
        };
        let y = match args.parse_f32(2) {
            Ok(v) => v,
            Err(e) => return format!("usage: warp <zone> <x> <y> <z> <name> — {e}"),
        };
        let z = match args.parse_f32(3) {
            Ok(v) => v,
            Err(e) => return format!("usage: warp <zone> <x> <y> <z> <name> — {e}"),
        };
        let Some(name) = args.rest_joined(4) else {
            return "usage: warp <zone> <x> <y> <z> <name>".into();
        };
        let Some(chara_id) = self.lookup_character_id(&name).await else {
            return format!("unknown character: {name}");
        };
        let Some(handle) = self.registry.get(chara_id).await else {
            return format!("{name} is not online");
        };
        // Persist destination + base position on CharaState so the
        // next zone-in bundle reads them.
        let rotation;
        let current_zone_id;
        let actor_id = handle.actor_id;
        let session_id = handle.session_id;
        {
            let mut c = handle.character.write().await;
            current_zone_id = c.base.zone_id;
            rotation = c.base.rotation;
            c.base.zone_id = zone_id;
            c.base.position_x = x;
            c.base.position_y = y;
            c.base.position_z = z;
        }
        if let Some(mut session) = self.world.session(session_id).await {
            session.destination_zone_id = zone_id;
            session.destination_x = x;
            session.destination_y = y;
            session.destination_z = z;
            session.destination_spawn_type = 2; // retail "warp by gm" code
            self.world.upsert_session(session).await;
        }
        // Same-zone warp: emit a SetActorPosition packet so the client
        // actually moves. Cross-zone warp needs a full DoZoneChange
        // flow the caller can drive by logging out + logging back in.
        // `spawn_type=2` matches the retail "warp-by-GM" spawn code;
        // `is_zoning_player=false` because we're not going through the
        // loading screen. (Cross-zone variant left as a follow-up.)
        if current_zone_id == zone_id {
            if let Some(client) = self.world.client(session_id).await {
                let pkt = crate::packets::send::build_set_actor_position(
                    actor_id,
                    actor_id as i32,
                    x,
                    y,
                    z,
                    rotation,
                    2,
                    false,
                );
                client.send_bytes(pkt.to_bytes()).await;
            }
        }
        format!(
            "warped {name} to zone {zone_id} at ({x:.2}, {y:.2}, {z:.2})"
        )
    }

    async fn handle_join_gc(&self, args: &Args<'_>) -> String {
        let gc = match args.parse_u8(0) {
            Ok(g) => g,
            Err(e) => return format!("usage: joingc <gc> <name> — {e}"),
        };
        let Some(name) = args.rest_joined(1) else {
            return "usage: joingc <gc> <name>".into();
        };
        if !crate::actor::gc::is_valid_gc(gc) {
            return format!("invalid gc id {gc} (expected 1/2/3 = Maelstrom/TwinAdder/Flames)");
        }
        let Some(chara_id) = self.lookup_character_id(&name).await else {
            return format!("unknown character: {name}");
        };
        if let Err(e) = self.db.set_gc_current(chara_id, gc).await {
            return format!("joingc failed: {e}");
        }
        // Start at Recruit if they've never been promoted in this GC.
        // `load_player_character` returns `Result<Option<LoadedPlayer>>`
        // — flatten the Result → Option layer first.
        let existing = self
            .db
            .load_player_character(chara_id)
            .await
            .ok()
            .flatten()
            .map(|p| match gc {
                1 => p.gc_limsa_rank,
                2 => p.gc_gridania_rank,
                3 => p.gc_uldah_rank,
                _ => 0,
            })
            .unwrap_or(0);
        let rank = if existing == 0 { crate::actor::gc::RANK_RECRUIT } else { existing };
        if let Err(e) = self.db.set_gc_rank(chara_id, gc, rank).await {
            return format!("joingc: set_gc_rank failed: {e}");
        }
        if let Some(handle) = self.registry.get(chara_id).await {
            let mut c = handle.character.write().await;
            c.chara.gc_current = gc;
            match gc {
                1 => c.chara.gc_rank_limsa = rank,
                2 => c.chara.gc_rank_gridania = rank,
                3 => c.chara.gc_rank_uldah = rank,
                _ => {}
            }
        }
        format!("{name} joined GC {gc} at rank {rank}")
    }

    async fn handle_set_gc_rank(&self, args: &Args<'_>) -> String {
        let gc = match args.parse_u8(0) {
            Ok(g) => g,
            Err(e) => return format!("usage: setgcrank <gc> <rank> <name> — {e}"),
        };
        let rank = match args.parse_u8(1) {
            Ok(r) => r,
            Err(e) => return format!("usage: setgcrank <gc> <rank> <name> — {e}"),
        };
        let Some(name) = args.rest_joined(2) else {
            return "usage: setgcrank <gc> <rank> <name>".into();
        };
        if !crate::actor::gc::is_valid_gc(gc) {
            return format!("invalid gc id {gc} (expected 1/2/3)");
        }
        let Some(chara_id) = self.lookup_character_id(&name).await else {
            return format!("unknown character: {name}");
        };
        match self.db.set_gc_rank(chara_id, gc, rank).await {
            Ok(()) => {
                if let Some(handle) = self.registry.get(chara_id).await {
                    let mut c = handle.character.write().await;
                    match gc {
                        1 => c.chara.gc_rank_limsa = rank,
                        2 => c.chara.gc_rank_gridania = rank,
                        3 => c.chara.gc_rank_uldah = rank,
                        _ => {}
                    }
                }
                format!("set {name}'s GC {gc} rank to {rank}")
            }
            Err(e) => format!("setgcrank failed: {e}"),
        }
    }

    async fn handle_add_gc_seals(&self, args: &Args<'_>) -> String {
        let gc = match args.parse_u8(0) {
            Ok(g) => g,
            Err(e) => return format!("usage: addgcseals <gc> <amount> <name> — {e}"),
        };
        let amount = match args.parse_i32(1) {
            Ok(a) => a,
            Err(e) => return format!("usage: addgcseals <gc> <amount> <name> — {e}"),
        };
        let Some(name) = args.rest_joined(2) else {
            return "usage: addgcseals <gc> <amount> <name>".into();
        };
        if !crate::actor::gc::is_valid_gc(gc) {
            return format!("invalid gc id {gc} (expected 1/2/3)");
        }
        let Some(chara_id) = self.lookup_character_id(&name).await else {
            return format!("unknown character: {name}");
        };
        match self.db.add_seals(chara_id, gc, amount).await {
            Ok(total) => format!(
                "granted {amount} GC {gc} seals to {name} (total now {total})"
            ),
            Err(e) => format!("addgcseals failed: {e}"),
        }
    }

    async fn handle_accrue_rest(&self, args: &Args<'_>) -> String {
        let minutes = match args.parse_i32(0) {
            Ok(m) => m,
            Err(e) => return format!("usage: accruerest <minutes> <name> — {e}"),
        };
        let Some(name) = args.rest_joined(1) else {
            return "usage: accruerest <minutes> <name>".into();
        };
        let Some(chara_id) = self.lookup_character_id(&name).await else {
            return format!("unknown character: {name}");
        };
        let current = self
            .db
            .get_rest_bonus_exp_rate(chara_id)
            .await
            .unwrap_or(0);
        // 1.x rest bonus: +1% per minute at an inn, capped at +100%.
        // The cap is Meteor's observed max (`Player.restBonus = restBonus`
        // assignments never exceed 100 in the commit history). A
        // negative `minutes` argument decays the bonus.
        let new_total = (current.saturating_add(minutes)).clamp(0, 100);
        match self.db.set_rest_bonus_exp_rate(chara_id, new_total).await {
            Ok(()) => {
                format!(
                    "rest bonus for {name}: {current}% → {new_total}% ({:+} min)",
                    minutes
                )
            }
            Err(e) => format!("accruerest failed: {e}"),
        }
    }

    async fn lookup_character_id(&self, name: &str) -> Option<u32> {
        self.db.character_id_by_name(name).await.ok().flatten()
    }

    async fn resolve_live_target(&self, name: &str) -> TargetLookup {
        let Some(chara_id) = self.lookup_character_id(name).await else {
            return TargetLookup::Err(format!("unknown character: {name}"));
        };
        let Some(handle) = self.registry.by_session(chara_id).await else {
            return TargetLookup::Err(format!("{name} is not online"));
        };
        let Some(zone) = self.world.zone(handle.zone_id).await else {
            return TargetLookup::Err(format!("{name}'s zone {} not loaded", handle.zone_id));
        };
        TargetLookup::Ok {
            actor_id: handle.actor_id,
            zone,
        }
    }

    /// `talkto <actor_class_id> <name>` — synthesise an EventStart from
    /// the server side. Resolves the named player + an NPC with the
    /// given `actor_class_id` in the player's zone, then drives the
    /// same `chara.event_session.start_event(...)` → `EventOutbox` →
    /// `dispatch_event_event` pipeline that `handle_event_start` runs
    /// when the client sends a real EventStart packet. Exercises the
    /// full quest `onTalk` dispatch + NPC-script `onEventStarted` flow
    /// without the client needing to physically walk to the NPC +
    /// press the interact key — which has proven unreliable to drive
    /// through ffxiv-actor's synthesised CGEvents.
    async fn handle_talkto(&self, args: &Args<'_>) -> String {
        let actor_class_id = match args.parse_u32(0) {
            Ok(v) => v,
            Err(e) => return format!("usage: talkto <actor_class_id> <name> — {e}"),
        };
        let Some(name) = args.rest_joined(1) else {
            return "usage: talkto <actor_class_id> <name>".into();
        };
        let Some(chara_id) = self.lookup_character_id(&name).await else {
            return format!("unknown character: {name}");
        };
        let Some(player_handle) = self.registry.get(chara_id).await else {
            return format!("{name} is not online");
        };
        let zone_id = player_handle.zone_id;

        // Find the NPC by its actor_class_id in the same zone.
        let mut npc_handle = None;
        let actors = self.registry.actors_in_zone(zone_id).await;
        for h in actors {
            let matches = {
                let c = h.character.read().await;
                c.chara.actor_class_id == actor_class_id
            };
            if matches {
                npc_handle = Some(h);
                break;
            }
        }
        let Some(npc_handle) = npc_handle else {
            return format!(
                "no NPC with actor_class_id={actor_class_id} in zone {zone_id}",
            );
        };
        let owner_actor_id = npc_handle.actor_id;
        let npc_name = {
            let c = npc_handle.character.read().await;
            c.base.actor_name.clone()
        };

        // Start the event + drain the outbox. Same shape as
        // `handle_event_start` in processor.rs.
        let mut outbox = crate::event::outbox::EventOutbox::new();
        {
            let mut chara = player_handle.character.write().await;
            chara.event_session.start_event(
                player_handle.actor_id,
                owner_actor_id,
                "talkDefault".to_string(),
                0, // event_type
                Vec::new(),
                &mut outbox,
            );
        }
        for e in outbox.drain() {
            crate::event::dispatcher::dispatch_event_event(
                &e,
                &self.registry,
                &self.world,
                &self.db,
                Some(&self.lua),
            )
            .await;
        }

        let active_quests: Vec<u32> = {
            let c = player_handle.character.read().await;
            c.quest_journal
                .slots
                .iter()
                .flatten()
                .map(|q| q.quest_id())
                .collect()
        };

        format!(
            "talkto fired event on player {chara_id} → NPC {owner_actor_id} (class {actor_class_id}, \"{npc_name}\"), {} active quest(s) in journal",
            active_quests.len()
        )
    }
}

enum TargetLookup {
    Ok {
        actor_id: u32,
        zone: Arc<tokio::sync::RwLock<crate::zone::zone::Zone>>,
    },
    Err(String),
}

/// Tiny typed-arg shim. The C# `CommandProcessor` punts arg parsing to
/// each command's own handler — we keep the same per-command contract
/// but centralise the "index + parse + error message" plumbing so each
/// handler is a couple of lines.
struct Args<'a> {
    tokens: Vec<&'a str>,
}

impl<'a> Args<'a> {
    fn new(tokens: Vec<&'a str>) -> Self {
        Self { tokens }
    }

    fn rest(&self) -> &[&'a str] {
        &self.tokens
    }

    fn parse_i32(&self, idx: usize) -> std::result::Result<i32, String> {
        let Some(raw) = self.tokens.get(idx) else {
            return Err(format!("missing arg {idx}"));
        };
        raw.parse::<i32>()
            .map_err(|_| format!("arg {idx} '{raw}' is not an integer"))
    }

    fn parse_u8(&self, idx: usize) -> std::result::Result<u8, String> {
        let Some(raw) = self.tokens.get(idx) else {
            return Err(format!("missing arg {idx}"));
        };
        raw.parse::<u8>()
            .map_err(|_| format!("arg {idx} '{raw}' is not a byte"))
    }

    fn parse_f32(&self, idx: usize) -> std::result::Result<f32, String> {
        let Some(raw) = self.tokens.get(idx) else {
            return Err(format!("missing arg {idx}"));
        };
        raw.parse::<f32>()
            .map_err(|_| format!("arg {idx} '{raw}' is not a float"))
    }

    fn parse_u32(&self, idx: usize) -> std::result::Result<u32, String> {
        let Some(raw) = self.tokens.get(idx) else {
            return Err(format!("missing arg {idx}"));
        };
        raw.parse::<u32>()
            .map_err(|_| format!("arg {idx} '{raw}' is not an unsigned int"))
    }

    /// Single token at position `idx`, or `None` if out of range.
    fn token(&self, idx: usize) -> Option<&'a str> {
        self.tokens.get(idx).copied()
    }

    /// Concatenate tokens `[from..]` into a single space-separated string
    /// for name-like trailing args ("First Last"). Returns `None` if no
    /// tokens exist at or after `from`.
    fn rest_joined(&self, from: usize) -> Option<String> {
        if from >= self.tokens.len() {
            return None;
        }
        Some(self.tokens[from..].join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::db::ConnCallExt;
    use rusqlite::named_params;

    fn tempdb() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("garlemald-cmd-{nanos}-{seq}.db"))
    }

    async fn fixture() -> (CommandProcessor, Arc<Database>) {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        let db = Arc::new(Database::open(tempdb()).await.unwrap());
        let lua = Arc::new(LuaEngine::new("/nonexistent"));
        let cmd = CommandProcessor::new(world, registry, db.clone(), lua);
        (cmd, db)
    }

    #[tokio::test]
    async fn help_lists_commands() {
        let (cmd, _db) = fixture().await;
        let out = cmd.run("help").await.unwrap();
        assert!(out.contains("revive"));
        assert!(out.contains("givegil"));
        assert!(out.contains("reload"));
    }

    #[tokio::test]
    async fn reload_returns_cache_count() {
        let (cmd, _db) = fixture().await;
        let out = cmd.run("reload").await.unwrap();
        assert!(out.starts_with("reloaded: "));
    }

    #[tokio::test]
    async fn unknown_command_is_reported() {
        let (cmd, _db) = fixture().await;
        let out = cmd.run("frobnicate 1 2 3").await.unwrap();
        assert!(out.starts_with("unknown command: frobnicate"));
        assert!(out.contains("\"1\""));
    }

    #[tokio::test]
    async fn revive_without_name_reports_usage() {
        let (cmd, _db) = fixture().await;
        let out = cmd.run("revive").await.unwrap();
        assert_eq!(out, "usage: revive <name>");
    }

    #[tokio::test]
    async fn givegil_parses_and_writes_db() {
        let (cmd, db) = fixture().await;
        db.conn_for_test()
            .call_db(|c| {
                c.execute(
                    r"INSERT INTO characters (id, userId, slot, serverId, name)
                      VALUES (42, 0, 0, 0, 'Rich Frog')",
                    [],
                )?;
                Ok(())
            })
            .await
            .unwrap();

        let out = cmd.run("givegil 5000 Rich Frog").await.unwrap();
        assert!(out.contains("gave 5000 gil"), "got {out}");
        let total: i32 = db
            .conn_for_test()
            .call_db(|c| {
                c.query_row(
                    r"SELECT si.quantity
                      FROM characters_inventory ci
                      INNER JOIN server_items si ON ci.serverItemId = si.id
                      WHERE ci.characterId = 42 AND si.itemId = 1000001",
                    [],
                    |r| r.get(0),
                )
                .map_err(Into::into)
            })
            .await
            .unwrap();
        assert_eq!(total, 5000);
        let _ = named_params! { ":x": 0 };
    }

    #[tokio::test]
    async fn givegil_non_numeric_quantity_errors() {
        let (cmd, _db) = fixture().await;
        let out = cmd.run("givegil hello Somebody").await.unwrap();
        assert!(out.contains("not an integer"), "got {out}");
    }

    #[tokio::test]
    async fn revive_unknown_name_reports_unknown() {
        let (cmd, _db) = fixture().await;
        let out = cmd.run("revive Nobody").await.unwrap();
        assert_eq!(out, "unknown character: Nobody");
    }

    #[tokio::test]
    async fn joingc_and_setgcrank_and_addgcseals_round_trip() {
        let (cmd, db) = fixture().await;
        db.conn_for_test()
            .call_db(|c| {
                c.execute(
                    r"INSERT INTO characters (id, userId, slot, serverId, name)
                      VALUES (300, 0, 0, 0, 'Company Hopeful')",
                    [],
                )?;
                Ok(())
            })
            .await
            .unwrap();

        // joingc 1 — enlists in Maelstrom at Recruit (127).
        let out = cmd.run("joingc 1 Company Hopeful").await.unwrap();
        assert!(out.contains("joined GC 1"), "got {out}");
        let (gc, l): (i64, i64) = db
            .conn_for_test()
            .call_db(|c| {
                Ok(c.query_row(
                    r"SELECT gcCurrent, gcLimsaRank FROM characters WHERE id = 300",
                    [],
                    |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
                )?)
            })
            .await
            .unwrap();
        assert_eq!((gc, l), (1, 127));

        // setgcrank 1 15 — promote to Private First Class.
        let out2 = cmd.run("setgcrank 1 15 Company Hopeful").await.unwrap();
        assert!(out2.contains("rank to 15"), "got {out2}");
        let l2: i64 = db
            .conn_for_test()
            .call_db(|c| {
                Ok(c.query_row(
                    r"SELECT gcLimsaRank FROM characters WHERE id = 300",
                    [],
                    |r| r.get::<_, i64>(0),
                )?)
            })
            .await
            .unwrap();
        assert_eq!(l2, 15);

        // addgcseals 1 2500 — seal upsert.
        let out3 = cmd.run("addgcseals 1 2500 Company Hopeful").await.unwrap();
        assert!(out3.contains("total now 2500"), "got {out3}");
        // Second deposit merges.
        let out4 = cmd.run("addgcseals 1 500 Company Hopeful").await.unwrap();
        assert!(out4.contains("total now 3000"), "got {out4}");

        // Invalid gc id reports.
        let out5 = cmd.run("joingc 9 Company Hopeful").await.unwrap();
        assert!(out5.contains("invalid gc id 9"), "got {out5}");
    }

    #[tokio::test]
    async fn issuechocobo_persists_and_is_idempotent() {
        let (cmd, db) = fixture().await;
        db.conn_for_test()
            .call_db(|c| {
                c.execute(
                    r"INSERT INTO characters (id, userId, slot, serverId, name)
                      VALUES (200, 0, 0, 0, 'Chocobo Get')",
                    [],
                )?;
                Ok(())
            })
            .await
            .unwrap();

        // issuechocobo <appearance> <chocobo_name> <player_name>
        let out = cmd
            .run("issuechocobo 5 Boco Chocobo Get")
            .await
            .unwrap();
        assert!(out.contains("issued chocobo"), "got {out}");
        // DB persistence:
        let (has, app, name): (i64, i64, String) = db
            .conn_for_test()
            .call_db(|c| {
                Ok(c.query_row(
                    r"SELECT hasChocobo, chocoboAppearance, chocoboName
                      FROM characters_chocobo WHERE characterId = 200",
                    [],
                    |r| {
                        Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, i64>(1)?,
                            r.get::<_, String>(2)?,
                        ))
                    },
                )?)
            })
            .await
            .unwrap();
        assert_eq!(has, 1);
        assert_eq!(app, 5);
        assert_eq!(name, "Boco");

        // Re-issue overwrites (upsert semantics — Meteor's C# uses
        // ON CONFLICT DO UPDATE).
        let out2 = cmd
            .run("issuechocobo 9 Pecopeco Chocobo Get")
            .await
            .unwrap();
        assert!(out2.contains("issued chocobo"), "got {out2}");
        let (app2, name2): (i64, String) = db
            .conn_for_test()
            .call_db(|c| {
                Ok(c.query_row(
                    r"SELECT chocoboAppearance, chocoboName
                      FROM characters_chocobo WHERE characterId = 200",
                    [],
                    |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
                )?)
            })
            .await
            .unwrap();
        assert_eq!(app2, 9);
        assert_eq!(name2, "Pecopeco");
    }

    #[tokio::test]
    async fn accruerest_rolls_up_and_caps_at_hundred() {
        let (cmd, db) = fixture().await;
        db.conn_for_test()
            .call_db(|c| {
                c.execute(
                    r"INSERT INTO characters (id, userId, slot, serverId, name)
                      VALUES (99, 0, 0, 0, 'Inn Sleeper')",
                    [],
                )?;
                Ok(())
            })
            .await
            .unwrap();

        let out = cmd.run("accruerest 30 Inn Sleeper").await.unwrap();
        assert!(out.contains("0% → 30%"), "got {out}");
        let out2 = cmd.run("accruerest 90 Inn Sleeper").await.unwrap();
        assert!(out2.contains("30% → 100%"), "got {out2}");
        // Decay below zero clamps to 0.
        let out3 = cmd.run("accruerest -500 Inn Sleeper").await.unwrap();
        assert!(out3.contains("100% → 0%"), "got {out3}");
        // Final DB value is 0.
        assert_eq!(db.get_rest_bonus_exp_rate(99).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn hire_and_dismiss_retainer_round_trip() {
        let (cmd, db) = fixture().await;
        db.conn_for_test()
            .call_db(|c| {
                c.execute(
                    r"INSERT INTO characters (id, userId, slot, serverId, name)
                      VALUES (88, 0, 0, 0, 'Retainer Hirer')",
                    [],
                )?;
                Ok(())
            })
            .await
            .unwrap();

        // Before hiring: listretainers shows empty.
        let out = cmd.run("listretainers Retainer Hirer").await.unwrap();
        assert!(out.contains("owns no retainers"), "got {out}");

        // Hire Wienta (seed id 1001).
        let hired = cmd.run("hireretainer 1001 Retainer Hirer").await.unwrap();
        assert!(hired.contains("hired retainer 1001"), "got {hired}");

        // Idempotent: second hire is a no-op.
        let again = cmd.run("hireretainer 1001 Retainer Hirer").await.unwrap();
        assert!(again.contains("already owns"), "got {again}");

        // List now shows one row.
        let listed = cmd.run("listretainers Retainer Hirer").await.unwrap();
        assert!(listed.contains("id=1001"), "got {listed}");
        assert!(listed.contains("name=Wienta"), "got {listed}");

        // Dismiss succeeds, second dismiss is a no-op.
        let dismissed = cmd.run("dismissretainer 1001 Retainer Hirer").await.unwrap();
        assert!(dismissed.contains("dismissed"), "got {dismissed}");
        let again = cmd.run("dismissretainer 1001 Retainer Hirer").await.unwrap();
        assert!(again.contains("does not own"), "got {again}");
    }

    /// `sethome` persists to DB even when the player is offline. The
    /// follow-up `home` command short-circuits at "not online" — but
    /// that's all we can prove without standing up a Zone + ActorHandle
    /// in a CommandProcessor fixture (the rest of the dispatcher is
    /// covered by `home_point_revive_tests` directly against
    /// `apply_home_point_revive`).
    #[tokio::test]
    async fn sethome_persists_aetheryte_id_for_offline_character() {
        let (cmd, db) = fixture().await;
        db.conn_for_test()
            .call_db(|c| {
                c.execute(
                    r"INSERT INTO characters (id, userId, slot, serverId, name)
                      VALUES (12, 0, 0, 0, 'Limsa Resident')",
                    [],
                )?;
                Ok(())
            })
            .await
            .unwrap();

        // Set homepoint to Limsa CAP (1280001).
        let out = cmd.run("sethome 1280001 Limsa Resident").await.unwrap();
        assert!(out.contains("homepoint to 1280001"), "got {out}");
        // Coords resolved from the Rust-side aetheryte table.
        assert!(out.contains("zone 230"), "got {out}");

        // DB row updated.
        let stored: u32 = db
            .conn_for_test()
            .call_db(|c| {
                Ok(c.query_row(
                    "SELECT homepoint FROM characters WHERE id = 12",
                    [],
                    |r| r.get(0),
                )?)
            })
            .await
            .unwrap();
        assert_eq!(stored, 1_280_001);

        // Unknown aetheryte id still persists but the response notes it.
        let out2 = cmd.run("sethome 999999 Limsa Resident").await.unwrap();
        assert!(out2.contains("homepoint to 999999"), "got {out2}");
        assert!(out2.contains("unknown aetheryte"), "got {out2}");
    }

    #[tokio::test]
    async fn home_without_name_reports_usage() {
        let (cmd, _db) = fixture().await;
        let out = cmd.run("home").await.unwrap();
        assert_eq!(out, "usage: home <name>");
    }

    #[tokio::test]
    async fn home_unknown_player_reports_unknown() {
        let (cmd, _db) = fixture().await;
        let out = cmd.run("home Phantom").await.unwrap();
        assert!(out.contains("unknown character"), "got {out}");
    }
}
