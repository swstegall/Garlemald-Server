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
            other => format!("unknown command: {other} (args={:?})", args.rest()),
        };
        Ok(response)
    }

    fn help() -> String {
        "commands: help, who, version, reload, \
         revive <name>, die <name>, givegil <qty> <name>, \
         giveexp <qty> <class_id> <name>, \
         hireretainer <retainer_id> <name>, \
         dismissretainer <retainer_id> <name>, \
         listretainers <name>, \
         setsleeping <name>, dream <id> <name>, wake <name>, \
         accruerest <minutes> <name>"
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
}
