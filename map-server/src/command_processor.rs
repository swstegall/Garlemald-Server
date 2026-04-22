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
            other => format!("unknown command: {other} (args={:?})", args.rest()),
        };
        Ok(response)
    }

    fn help() -> String {
        "commands: help, who, version, reload, \
         revive <name>, die <name>, givegil <qty> <name>, \
         giveexp <qty> <class_id> <name>"
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
}
