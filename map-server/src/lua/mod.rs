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

//! Full-fat Lua host API for the Map Server.
//!
//! Replaces the Phase-4 thin mlua wrapper. Provides:
//!   - Script path resolver mirroring `FILEPATH_*` from `LuaEngine.cs`
//!   - Per-script VM cache, each VM pre-loaded with globals (`GetWorldManager`,
//!     `GetStaticActor`, `GetItemGamedata`, …)
//!   - UserData impls for Actor/Player/Npc/Zone/WorldManager/ItemData so
//!     scripts can call the same methods they expect in the C# server
//!   - Command queue: side effects flow Lua → Rust as typed `LuaCommand`s
//!   - Coroutine scheduler: `_WAIT_TIME` / `_WAIT_SIGNAL` / `_WAIT_EVENT` yields
//!   - NPC parent/child script resolution (`scripts/base/X.lua` + unique
//!     override), matching the C# `CallLuaFunctionNpc`
//!   - GM command runner with `properties` table introspection
//!
//! Script evaluation is synchronous: the map-server game loop invokes a
//! script, drains the resulting command queue, then applies the commands.
//! No awaiting inside Lua.

#![allow(dead_code)]

pub mod catalogs;
pub mod command;
pub mod globals;
pub mod gm_command;
pub mod paths;
pub mod scheduler;
pub mod userdata;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use mlua::{Function, Lua, MultiValue, Value};

use self::catalogs::Catalogs;
use self::command::{CommandQueue, LuaCommand};
use self::globals::install_globals;
use self::paths::PathResolver;
use self::scheduler::CoroutineScheduler;

// Forward-looking re-exports so the processor and game loop can
// `use map_server::lua::LuaPlayer` without drilling into submodules.
#[allow(unused_imports)]
pub use command::{CommandQueue as LuaCommandQueue, LuaCommand as LuaCommandKind};
#[allow(unused_imports)]
pub use scheduler::{CoroutineScheduler as LuaScheduler, ParkedCoroutine, YieldDirective};
#[allow(unused_imports)]
pub use userdata::{
    LuaActor, LuaNpc, LuaPlayer, LuaQuestDataHandle, LuaQuestHandle, LuaZone, PlayerSnapshot,
    QuestStateSnapshot, ZoneSnapshot,
};

pub struct LuaEngine {
    resolver: PathResolver,
    catalogs: Arc<Catalogs>,
    scheduler: Arc<Mutex<CoroutineScheduler>>,
    /// Cached one-VM-per-script-path. Every VM has globals pre-installed.
    vm_cache: Mutex<HashMap<String, Arc<Lua>>>,
}

impl LuaEngine {
    pub fn new(script_root: impl Into<std::path::PathBuf>) -> Self {
        Self {
            resolver: PathResolver::new(script_root),
            catalogs: Arc::new(Catalogs::new()),
            scheduler: CoroutineScheduler::shared(),
            vm_cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn resolver(&self) -> &PathResolver {
        &self.resolver
    }

    /// Drop every cached VM so the next `load_script` call re-reads the
    /// file from disk and re-evaluates. Used by the `!reload` console
    /// command to pick up script edits without restarting the server.
    /// Returns the number of scripts evicted.
    pub fn reload_scripts(&self) -> usize {
        let Ok(mut cache) = self.vm_cache.lock() else {
            return 0;
        };
        let n = cache.len();
        cache.clear();
        n
    }

    pub fn catalogs(&self) -> &Arc<Catalogs> {
        &self.catalogs
    }

    pub fn scheduler(&self) -> &Arc<Mutex<CoroutineScheduler>> {
        &self.scheduler
    }

    /// Load a script (from cache if already loaded). Each call gets its own
    /// command queue so commands can be inspected after the script returns.
    pub fn load_script(&self, path: &Path) -> Result<(Arc<Lua>, Arc<Mutex<CommandQueue>>)> {
        let key = path.display().to_string();
        if let Some(lua) = self
            .vm_cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(&key).cloned())
        {
            let queue = CommandQueue::new();
            self.reinstall_queue_globals(&lua, queue.clone())?;
            return Ok((lua, queue));
        }

        let source = std::fs::read_to_string(path)
            .with_context(|| format!("reading lua script {}", path.display()))?;
        let lua = Arc::new(Lua::new());
        // Point `require` at the script root so `require("global")` resolves
        // to `scripts/lua/global.lua`. Without this mlua searches only the
        // default Lua paths (/usr/local/share/lua/..., ./global.lua) and
        // `player.lua`'s very first line (`require("global");`) aborts the
        // script before any function body runs. The `?.lua` / `?/init.lua`
        // patterns mirror the default lua loader; we just prefix them with
        // our resolver root.
        let root = self.resolver.root.display().to_string();
        let path_patterns = format!("{root}/?.lua;{root}/?/init.lua");
        {
            let package: mlua::Table = lua.globals().get("package")?;
            package.set("path", path_patterns)?;
        }
        let queue = CommandQueue::new();
        install_globals(&lua, queue.clone(), self.catalogs.clone())?;
        lua.load(&source)
            .exec()
            .map_err(|e| anyhow::anyhow!("parse {key}: {e}"))?;

        self.vm_cache
            .lock()
            .ok()
            .map(|mut cache| cache.insert(key, lua.clone()));
        Ok((lua, queue))
    }

    fn reinstall_queue_globals(&self, lua: &Lua, queue: Arc<Mutex<CommandQueue>>) -> Result<()> {
        // Re-register functions whose closures capture the queue, so every
        // call has a fresh command bucket. Read-only catalogs already in the
        // VM are preserved (their closures capture `catalogs` by Arc clone).
        install_globals(lua, queue, self.catalogs.clone())
            .map_err(|e| anyhow::anyhow!("install_globals: {e}"))
    }

    /// Execute an arbitrary top-level function on a script. If the function
    /// returns a Lua thread (i.e. the script wraps the body in a coroutine),
    /// this calls the thread and classifies the resulting yield.
    ///
    /// Returns the raw Lua return values plus any drained commands.
    pub fn call(
        &self,
        script_path: &Path,
        function_name: &str,
        args: MultiValue,
    ) -> Result<LuaCallResult> {
        let (lua, queue) = self.load_script(script_path)?;
        let globals = lua.globals();
        let f: Function = globals.get(function_name).map_err(|e| {
            anyhow::anyhow!("{function_name} not in {}: {e}", script_path.display())
        })?;
        let result: Value = f
            .call::<Value>(args)
            .map_err(|e| anyhow::anyhow!("{function_name}: {e}"))?;
        let commands = CommandQueue::drain(&queue);
        Ok(LuaCallResult {
            value: result,
            commands,
        })
    }

    /// Player-hook helper: load `player.lua` (or whichever script path was
    /// passed), build a `LuaPlayer` userdata with the caller's snapshot
    /// wrapping the freshly-created command queue, and invoke `function_name`
    /// with the player as the sole argument. Mirrors the shape of C#
    /// `LuaEngine.CallLuaFunction(player, target=player, ...)`. Returns the
    /// commands drained from the queue so the caller can apply them.
    pub fn call_player_hook(
        &self,
        script_path: &Path,
        function_name: &str,
        snapshot: userdata::PlayerSnapshot,
    ) -> Result<LuaCallResult> {
        let (lua, queue) = self.load_script(script_path)?;
        let globals = lua.globals();
        let f: Function = globals.get(function_name).map_err(|e| {
            anyhow::anyhow!("{function_name} not in {}: {e}", script_path.display())
        })?;
        let player = userdata::LuaPlayer {
            snapshot,
            queue: queue.clone(),
        };
        let user_data = lua
            .create_userdata(player)
            .map_err(|e| anyhow::anyhow!("create_userdata(LuaPlayer): {e}"))?;
        let result: Value = f
            .call::<Value>(user_data)
            .map_err(|e| anyhow::anyhow!("{function_name}: {e}"))?;
        let commands = CommandQueue::drain(&queue);
        Ok(LuaCallResult {
            value: result,
            commands,
        })
    }

    /// Variant of [`call_player_hook`] that keeps any commands the script
    /// emitted *before* it errored out. Side effects queued up to the point
    /// of the error are still valid — for `onLogin` in particular the
    /// opening `player:SendMessage(...)` and any `AddItems` that ran
    /// before an unsupported `charaWork` index aborted the frame should
    /// still reach `apply_login_lua_command`. Discarding them on error was
    /// the bug that made partial progress look like a total no-op.
    pub fn call_player_hook_best_effort(
        &self,
        script_path: &Path,
        function_name: &str,
        snapshot: userdata::PlayerSnapshot,
    ) -> PartialLuaCallResult {
        let (lua, queue) = match self.load_script(script_path) {
            Ok(pair) => pair,
            Err(e) => {
                return PartialLuaCallResult {
                    commands: Vec::new(),
                    error: Some(e),
                };
            }
        };
        let globals = lua.globals();
        let f: Function = match globals.get(function_name) {
            Ok(f) => f,
            Err(e) => {
                return PartialLuaCallResult {
                    commands: Vec::new(),
                    error: Some(anyhow::anyhow!(
                        "{function_name} not in {}: {e}",
                        script_path.display()
                    )),
                };
            }
        };
        let player = userdata::LuaPlayer {
            snapshot,
            queue: queue.clone(),
        };
        let user_data = match lua.create_userdata(player) {
            Ok(ud) => ud,
            Err(e) => {
                return PartialLuaCallResult {
                    commands: CommandQueue::drain(&queue),
                    error: Some(anyhow::anyhow!("create_userdata(LuaPlayer): {e}")),
                };
            }
        };
        let call = f.call::<Value>(user_data);
        let commands = CommandQueue::drain(&queue);
        let error = call.err().map(|e| anyhow::anyhow!("{function_name}: {e}"));
        PartialLuaCallResult { commands, error }
    }

    /// Fire one of the five quest hooks — `onStart`, `onFinish`,
    /// `onStateChange`, `onTalk`, `onKillBNpc` — with the Meteor calling
    /// convention: `(player, quest, …extra_args)`.
    ///
    /// Builds a `LuaPlayer` from `snapshot` and a `LuaQuestHandle` keyed
    /// to the same live quest-state snapshot, then passes any `extra_args`
    /// after them. Returns the drained command queue so the processor
    /// can apply the mutations the hook emitted.
    ///
    /// A missing hook function is *not* an error — Meteor quest scripts
    /// only define the hooks they care about, so we treat "function not
    /// found" as a quiet success with no commands. Script-side errors
    /// (parse failures, nil dereferences) keep any commands emitted up
    /// to the error point, matching `call_player_hook_best_effort`.
    pub fn call_quest_hook(
        &self,
        script_path: &Path,
        hook_name: &str,
        player_snapshot: userdata::PlayerSnapshot,
        quest_handle: userdata::LuaQuestHandle,
        extra_args: Vec<Value>,
    ) -> PartialLuaCallResult {
        let (lua, queue) = match self.load_script(script_path) {
            Ok(pair) => pair,
            Err(e) => {
                return PartialLuaCallResult {
                    commands: Vec::new(),
                    error: Some(e),
                };
            }
        };
        // Re-point the handle's queue at the freshly-installed one so
        // mutations the hook enqueues land in the right bucket.
        let quest_handle = userdata::LuaQuestHandle {
            queue: queue.clone(),
            ..quest_handle
        };

        let globals = lua.globals();
        let f: Function = match globals.get(hook_name) {
            Ok(f) => f,
            Err(_) => {
                // Missing hook → quiet no-op. Drain anything the script
                // top-level produced (rare; normally nothing).
                return PartialLuaCallResult {
                    commands: CommandQueue::drain(&queue),
                    error: None,
                };
            }
        };

        let player = userdata::LuaPlayer {
            snapshot: player_snapshot,
            queue: queue.clone(),
        };
        let player_ud = match lua.create_userdata(player) {
            Ok(ud) => ud,
            Err(e) => {
                return PartialLuaCallResult {
                    commands: CommandQueue::drain(&queue),
                    error: Some(anyhow::anyhow!("create_userdata(LuaPlayer): {e}")),
                };
            }
        };
        let quest_ud = match lua.create_userdata(quest_handle) {
            Ok(ud) => ud,
            Err(e) => {
                return PartialLuaCallResult {
                    commands: CommandQueue::drain(&queue),
                    error: Some(anyhow::anyhow!("create_userdata(LuaQuestHandle): {e}")),
                };
            }
        };

        let mut mv = MultiValue::new();
        mv.push_back(Value::UserData(player_ud));
        mv.push_back(Value::UserData(quest_ud));
        for arg in extra_args {
            mv.push_back(arg);
        }

        let call = f.call::<Value>(mv);
        let commands = CommandQueue::drain(&queue);
        let error = call.err().map(|e| anyhow::anyhow!("{hook_name}: {e}"));
        PartialLuaCallResult { commands, error }
    }

    /// NPC-specific helper: try the unique-override script first, then fall
    /// back to the base-class script. Mirrors the C# `CallLuaFunctionNpc`.
    pub fn call_npc(
        &self,
        base_class_path: Option<&str>,
        zone_name: &str,
        class_name: &str,
        unique_id: &str,
        function_name: &str,
        args: MultiValue,
    ) -> Result<LuaCallResult> {
        let child = self.resolver.npc(zone_name, class_name, unique_id);
        if child.exists() {
            return self.call(&child, function_name, args);
        }
        if let Some(base) = base_class_path {
            let parent = self.resolver.base_class(base);
            if parent.exists() {
                return self.call(&parent, function_name, args);
            }
        }
        anyhow::bail!(
            "no script for {zone_name}/{class_name}/{unique_id} (base: {base_class_path:?})"
        );
    }

    /// Drive the scheduler forward: resume any parked coroutine whose time
    /// has come. Callers invoke this once per game tick.
    pub fn tick(&self) -> Vec<LuaCommand> {
        let mut all_commands = Vec::new();

        let due = self
            .scheduler
            .lock()
            .map(|mut s| s.drain_due_time())
            .unwrap_or_default();
        for parked in due {
            if let Ok(value) = parked.thread.resume::<Value>(()) {
                // If the coroutine yielded again, re-park it; otherwise its
                // accumulated commands are already in the shared queue.
                let directive = scheduler::classify_yield(&value);
                self.repark(parked, directive);
            }
        }

        // Draining per-script queues is the caller's job (via `call`); what
        // we return here are the side effects produced by *scheduler-driven*
        // resumes, which write into the cache-shared queues. The cleanest
        // thing is to return an empty vec and require callers to keep the
        // queue handles they were given by `call()`. For now, return empty.
        std::mem::take(&mut all_commands)
    }

    /// Notify the scheduler that `signal` fired. Any coroutine parked on it
    /// is resumed once.
    pub fn fire_signal(&self, signal: &str) {
        let due = self
            .scheduler
            .lock()
            .map(|mut s| s.drain_signal(signal))
            .unwrap_or_default();
        for parked in due {
            let _ = parked.thread.resume::<Value>(());
        }
    }

    /// Notify the scheduler that `player_id` just received an event update.
    pub fn fire_player_event(&self, player_id: u32, args: MultiValue) -> bool {
        let Some(parked) = self
            .scheduler
            .lock()
            .ok()
            .and_then(|mut s| s.take_event(player_id))
        else {
            return false;
        };
        let _ = parked.thread.resume::<Value>(args);
        true
    }

    fn repark(&self, parked: ParkedCoroutine, directive: YieldDirective) {
        let Ok(mut scheduler) = self.scheduler.lock() else {
            return;
        };
        match directive {
            YieldDirective::WaitTime(s) => scheduler.park_time(s, parked),
            YieldDirective::WaitSignal(name) => scheduler.park_signal(name, parked),
            YieldDirective::WaitEvent(pid) => scheduler.park_event(pid, parked),
            YieldDirective::Finished | YieldDirective::Other => { /* drop */ }
        }
    }
}

#[derive(Debug)]
pub struct LuaCallResult {
    pub value: Value,
    pub commands: Vec<LuaCommand>,
}

/// Partial result from [`LuaEngine::call_player_hook_best_effort`]. Carries
/// any commands queued before the frame errored so the caller can still
/// apply them. `error = None` means the hook ran cleanly to completion.
#[derive(Debug)]
pub struct PartialLuaCallResult {
    pub commands: Vec<LuaCommand>,
    pub error: Option<anyhow::Error>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir() -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("garlemald-lua-engine-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_script_errors() {
        let engine = LuaEngine::new("/nonexistent");
        let result = engine.call(
            std::path::Path::new("/nonexistent/not_a_script.lua"),
            "onTalk",
            mlua::MultiValue::new(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn simple_script_runs_and_returns() {
        let root = tmpdir();
        std::fs::write(
            root.join("simple.lua"),
            "function add(a, b) return a + b end",
        )
        .unwrap();
        let engine = LuaEngine::new(&root);
        let lua_val = {
            let (lua, _q) = engine.load_script(&root.join("simple.lua")).unwrap();
            let add: Function = lua.globals().get("add").unwrap();
            add.call::<i64>((2i64, 3i64)).unwrap()
        };
        assert_eq!(lua_val, 5);
        let _ = std::fs::remove_dir_all(root);
    }

    fn sample_snapshot() -> userdata::PlayerSnapshot {
        userdata::PlayerSnapshot {
            actor_id: 42,
            active_quests: vec![110_001],
            active_quest_states: vec![userdata::QuestStateSnapshot {
                quest_id: 110_001,
                sequence: 0,
                flags: 0,
                counters: [0; 3],
            }],
            ..Default::default()
        }
    }

    fn sample_quest_handle(queue: Arc<Mutex<CommandQueue>>) -> userdata::LuaQuestHandle {
        userdata::LuaQuestHandle {
            player_id: 42,
            quest_id: 110_001,
            has_quest: true,
            sequence: 0,
            flags: 0,
            counters: [0; 3],
            queue,
        }
    }

    #[test]
    fn call_quest_hook_passes_player_and_quest_and_drains_commands() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("quests/man")).unwrap();
        // onStart mutates the quest via the handle; the call should emit
        // a QuestSetFlag command.
        std::fs::write(
            root.join("quests/man/man0l0.lua"),
            r#"
                function onStart(player, quest)
                    quest:SetQuestFlag(2)
                    quest:StartSequence(5)
                end
            "#,
        )
        .unwrap();

        let engine = LuaEngine::new(&root);
        let script_path = root.join("quests/man/man0l0.lua");
        let dummy_queue = CommandQueue::new();
        let result = engine.call_quest_hook(
            &script_path,
            "onStart",
            sample_snapshot(),
            sample_quest_handle(dummy_queue),
            Vec::new(),
        );

        assert!(result.error.is_none(), "onStart errored: {:?}", result.error);
        let has_set_flag = result
            .commands
            .iter()
            .any(|c| matches!(c, LuaCommand::QuestSetFlag { bit: 2, quest_id: 110_001, .. }));
        let has_start_seq = result
            .commands
            .iter()
            .any(|c| matches!(c, LuaCommand::QuestStartSequence { sequence: 5, quest_id: 110_001, .. }));
        assert!(has_set_flag, "missing QuestSetFlag; got {:?}", result.commands);
        assert!(has_start_seq, "missing QuestStartSequence; got {:?}", result.commands);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn call_quest_hook_missing_function_is_a_quiet_no_op() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("quests/man")).unwrap();
        // Script defines only onStart — calling onFinish should silently
        // no-op (no error) because Meteor quest scripts only implement
        // the hooks they care about.
        std::fs::write(
            root.join("quests/man/man0l0.lua"),
            "function onStart(player, quest) end",
        )
        .unwrap();

        let engine = LuaEngine::new(&root);
        let script_path = root.join("quests/man/man0l0.lua");
        let result = engine.call_quest_hook(
            &script_path,
            "onFinish",
            sample_snapshot(),
            sample_quest_handle(CommandQueue::new()),
            vec![Value::Boolean(true)],
        );
        assert!(result.error.is_none(), "missing hook should be quiet: {:?}", result.error);
        assert!(result.commands.is_empty());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn call_quest_hook_receives_extra_args_after_player_and_quest() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("quests/man")).unwrap();
        // onStateChange(player, quest, sequence) — if the script gets
        // the right sequence it enqueues a command carrying it back via
        // StartSequence so the test can inspect.
        std::fs::write(
            root.join("quests/man/man0l0.lua"),
            r#"
                function onStateChange(player, quest, sequence)
                    if sequence == 10 then
                        quest:StartSequence(99)
                    end
                end
            "#,
        )
        .unwrap();

        let engine = LuaEngine::new(&root);
        let script_path = root.join("quests/man/man0l0.lua");
        let result = engine.call_quest_hook(
            &script_path,
            "onStateChange",
            sample_snapshot(),
            sample_quest_handle(CommandQueue::new()),
            vec![Value::Integer(10)],
        );
        assert!(result.error.is_none(), "hook errored: {:?}", result.error);
        let seen = result
            .commands
            .iter()
            .any(|c| matches!(c, LuaCommand::QuestStartSequence { sequence: 99, .. }));
        assert!(seen, "script didn't see sequence=10; got {:?}", result.commands);

        let _ = std::fs::remove_dir_all(root);
    }
}
