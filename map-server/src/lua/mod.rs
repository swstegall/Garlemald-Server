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

pub use self::catalogs::Catalogs;
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
// `QuestHookArg` + `LuaNpcSpec` are defined at this module's top level
// as `pub`, so they're reachable as `crate::lua::{QuestHookArg, LuaNpcSpec}`
// from the processor. No additional re-export needed.

/// Extra argument passed to a quest hook (`onTalk`, `onKillBNpc`, …)
/// after the `(player, quest)` pair. Each variant is `Send` so callers
/// can pass a `Vec<QuestHookArg>` into `tokio::task::spawn_blocking`
/// and let the `LuaEngine::call_quest_hook` body build the actual
/// `Value` inside the Lua VM.
#[derive(Debug, Clone)]
pub enum QuestHookArg {
    Int(i64),
    Bool(bool),
    Nil,
    /// Materialise a fresh `LuaNpc` userdata inside the script VM.
    Npc(LuaNpcSpec),
}

/// Deferred construction params for a `LuaNpc` userdata — owned, `Send`,
/// and built on the processor side from the live NPC state before
/// crossing into the blocking pool.
#[derive(Debug, Clone)]
pub struct LuaNpcSpec {
    pub actor_id: u32,
    pub name: String,
    pub class_name: String,
    pub class_path: String,
    pub unique_id: String,
    pub zone_id: u32,
    pub zone_name: String,
    pub state: u16,
    pub pos: (f32, f32, f32),
    pub rotation: f32,
    pub actor_class_id: u32,
    pub quest_graphic: u8,
}

impl QuestHookArg {
    fn into_value(
        self,
        lua: &Lua,
        queue: Arc<Mutex<CommandQueue>>,
    ) -> mlua::Result<Value> {
        Ok(match self {
            QuestHookArg::Int(i) => Value::Integer(i as mlua::Integer),
            QuestHookArg::Bool(b) => Value::Boolean(b),
            QuestHookArg::Nil => Value::Nil,
            QuestHookArg::Npc(spec) => {
                let npc = userdata::LuaNpc {
                    base: userdata::LuaActor {
                        actor_id: spec.actor_id,
                        name: spec.name,
                        class_name: spec.class_name,
                        class_path: spec.class_path,
                        unique_id: spec.unique_id,
                        zone_id: spec.zone_id,
                        zone_name: spec.zone_name,
                        state: spec.state,
                        pos: spec.pos,
                        rotation: spec.rotation,
                        queue,
                    },
                    actor_class_id: spec.actor_class_id,
                    quest_graphic: spec.quest_graphic,
                };
                Value::UserData(lua.create_userdata(npc)?)
            }
        })
    }
}

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
    /// `extra_args` are constructed inside the function via the Send-
    /// friendly `QuestHookArg` enum: primitive variants go straight to
    /// `Value`, while `Npc(...)` materialises a fresh `LuaNpc` userdata
    /// against the same VM that will receive it. Keeping args Send
    /// lets the caller run `call_quest_hook` on the tokio blocking pool.
    ///
    /// A missing hook function is *not* an error — Meteor quest scripts
    /// only define the hooks they care about. Script-side errors keep
    /// any commands emitted up to the error point.
    pub fn call_quest_hook(
        &self,
        script_path: &Path,
        hook_name: &str,
        player_snapshot: userdata::PlayerSnapshot,
        quest_handle: userdata::LuaQuestHandle,
        extra_args: Vec<QuestHookArg>,
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
            let v = match arg.into_value(&lua, queue.clone()) {
                Ok(v) => v,
                Err(e) => {
                    return PartialLuaCallResult {
                        commands: CommandQueue::drain(&queue),
                        error: Some(anyhow::anyhow!("quest hook arg: {e}")),
                    };
                }
            };
            mv.push_back(v);
        }

        // Run inside a coroutine so the hook body (man0l0.onNotice,
        // etc.) can `callClientFunction(...)` which yields on
        // `_WAIT_EVENT`. Mirrors Meteor's `CallLuaFunction` pattern
        // (`Map Server/Lua/LuaEngine.cs:519 CreateCoroutine + Resume`).
        // If the hook yields on a known wait directive, park it in
        // the scheduler — the next `EventUpdate` / tick / signal
        // resumes it.
        let thread = match lua.create_thread(f) {
            Ok(t) => t,
            Err(e) => {
                return PartialLuaCallResult {
                    commands: CommandQueue::drain(&queue),
                    error: Some(anyhow::anyhow!("create_thread({hook_name}): {e}")),
                };
            }
        };
        let resume_result = thread.resume::<Value>(mv);
        let commands = CommandQueue::drain(&queue);
        let (value, error) = match resume_result {
            Ok(v) => (v, None),
            Err(e) => (Value::Nil, Some(anyhow::anyhow!("{hook_name}: {e}"))),
        };
        if matches!(thread.status(), mlua::ThreadStatus::Resumable) {
            let directive = scheduler::classify_yield(&value);
            let parked = ParkedCoroutine {
                lua: lua.clone(),
                thread,
                queue,
            };
            self.repark(parked, directive);
        }
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

    /// Spawn a director's `main(thisDirector)` coroutine against
    /// `directors/<class_name>.lua`. Used by
    /// `processor::apply_start_director_main` on a
    /// `LuaCommand::StartDirectorMain` drain — the script-side
    /// `director:StartDirector(true)` call is what pushes that
    /// command.
    ///
    /// Loads the script, builds a `LuaDirectorHandle` userdata bound
    /// to the script's command queue, resumes the `main` coroutine
    /// once. If it yields on `_WAIT_TIME`/`_WAIT_SIGNAL`/`_WAIT_EVENT`,
    /// parks it in the shared scheduler so `tick()` can resume it
    /// later. Returns any commands the initial slice pushed (up to
    /// the first yield or to termination) so the caller can drain
    /// them through `apply_runtime_lua_commands`.
    ///
    /// Quietly errors out on:
    /// * script not on disk (`directors/<class_name>.lua` missing),
    /// * no `main` global (some directors only define `init`),
    /// * Lua runtime error during the initial resume (commands pushed
    ///   before the error are still returned).
    pub fn spawn_director_main(
        &self,
        script_path: &Path,
        director: userdata::LuaDirectorHandle,
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
        // Re-point the handle at the freshly-installed queue so
        // coroutine-emitted commands land where the caller drains.
        let director = userdata::LuaDirectorHandle {
            queue: queue.clone(),
            ..director
        };

        let globals = lua.globals();
        let main: mlua::Function = match globals.get("main") {
            Ok(f) => f,
            Err(_) => {
                // No `main` — quiet no-op. Some directors (e.g. simple
                // content holders) only define `init`.
                return PartialLuaCallResult {
                    commands: CommandQueue::drain(&queue),
                    error: None,
                };
            }
        };

        let director_ud = match lua.create_userdata(director) {
            Ok(ud) => ud,
            Err(e) => {
                return PartialLuaCallResult {
                    commands: CommandQueue::drain(&queue),
                    error: Some(anyhow::anyhow!("create_userdata(LuaDirectorHandle): {e}")),
                };
            }
        };

        let thread = match lua.create_thread(main) {
            Ok(t) => t,
            Err(e) => {
                return PartialLuaCallResult {
                    commands: CommandQueue::drain(&queue),
                    error: Some(anyhow::anyhow!("create_thread(main): {e}")),
                };
            }
        };

        // First resume — runs until the first yield / return / error.
        let resume_result = thread.resume::<Value>(director_ud);
        let commands = CommandQueue::drain(&queue);
        let (value, error) = match resume_result {
            Ok(v) => (v, None),
            Err(e) => (Value::Nil, Some(anyhow::anyhow!("director main: {e}"))),
        };

        // Park if still alive + parked on a known wait directive.
        if matches!(
            thread.status(),
            mlua::ThreadStatus::Resumable
        ) {
            let directive = scheduler::classify_yield(&value);
            let parked = ParkedCoroutine {
                lua: lua.clone(),
                thread,
                queue,
            };
            self.repark(parked, directive);
        }

        PartialLuaCallResult { commands, error }
    }

    /// Run a director's `onEventStarted(player, director, eventName, ...)`
    /// hook inside a coroutine so scripts can `callClientFunction(...)`
    /// (which yields on `_WAIT_EVENT`). If the hook yields, park it in
    /// the shared scheduler — `EventUpdate` packets from the client
    /// resume it via [`Self::fire_player_event`]. Returns any commands
    /// emitted before the first yield / return / error.
    ///
    /// The builder closure receives the per-call mlua::Lua (lets callers
    /// construct `LuaPlayer` / `LuaDirectorHandle` userdata bound to
    /// the freshly-loaded queue) and returns the MultiValue to hand the
    /// coroutine on the initial resume.
    pub fn spawn_director_on_event_started<F>(
        &self,
        script_path: &Path,
        build_args: F,
    ) -> PartialLuaCallResult
    where
        F: FnOnce(&mlua::Lua, &Arc<Mutex<CommandQueue>>) -> Result<MultiValue, anyhow::Error>,
    {
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
        let f: mlua::Function = match globals.get("onEventStarted") {
            Ok(f) => f,
            Err(_) => {
                return PartialLuaCallResult {
                    commands: CommandQueue::drain(&queue),
                    error: None,
                };
            }
        };
        let args = match build_args(&lua, &queue) {
            Ok(mv) => mv,
            Err(e) => {
                return PartialLuaCallResult {
                    commands: CommandQueue::drain(&queue),
                    error: Some(e),
                };
            }
        };
        let thread = match lua.create_thread(f) {
            Ok(t) => t,
            Err(e) => {
                return PartialLuaCallResult {
                    commands: CommandQueue::drain(&queue),
                    error: Some(anyhow::anyhow!("create_thread(onEventStarted): {e}")),
                };
            }
        };
        let resume_result = thread.resume::<Value>(args);
        let commands = CommandQueue::drain(&queue);
        let (value, error) = match resume_result {
            Ok(v) => (v, None),
            Err(e) => (
                Value::Nil,
                Some(anyhow::anyhow!("director onEventStarted: {e}")),
            ),
        };
        if matches!(thread.status(), mlua::ThreadStatus::Resumable) {
            let directive = scheduler::classify_yield(&value);
            let parked = ParkedCoroutine {
                lua: lua.clone(),
                thread,
                queue,
            };
            self.repark(parked, directive);
        }
        PartialLuaCallResult { commands, error }
    }

    /// Drive the scheduler forward: resume any parked coroutine whose time
    /// has come. Callers invoke this once per game tick.
    ///
    /// Returns every `LuaCommand` the resumed coroutines pushed into
    /// their script-bound queues — the ticker drains the return value
    /// through `apply_runtime_lua_commands` so scheduler-driven
    /// `director:EndGuildleve(true)` / `player:AddExp(...)` /
    /// `quest:SetQuestFlag(...)` calls from inside a parked `main`
    /// coroutine actually produce the right game-state mutations.
    pub fn tick(&self) -> Vec<LuaCommand> {
        let mut all_commands = Vec::new();

        let due = self
            .scheduler
            .lock()
            .map(|mut s| s.drain_due_time())
            .unwrap_or_default();
        for parked in due {
            let queue = parked.queue.clone();
            let resume = parked.thread.resume::<Value>(());
            // Drain whatever the resumed slice pushed into the
            // script's command queue — the directive classification
            // below doesn't touch the queue, so drain before reparking.
            all_commands.extend(CommandQueue::drain(&queue));
            if let Ok(value) = resume {
                let directive = scheduler::classify_yield(&value);
                self.repark(parked, directive);
            }
        }

        all_commands
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
            vec![],
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
            vec![QuestHookArg::Bool(true)],
        );
        assert!(result.error.is_none(), "missing hook should be quiet: {:?}", result.error);
        assert!(result.commands.is_empty());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn call_quest_hook_with_npc_arg_materialises_lua_userdata() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("quests/man")).unwrap();
        // onTalk receives (player, quest, npc) — verify the npc userdata
        // exposes `GetActorClassId` correctly and that mutating the quest
        // through the handle still writes into the queue while the npc's
        // fields are independently readable.
        std::fs::write(
            root.join("quests/man/man0l0.lua"),
            r#"
                function onTalk(player, quest, npc)
                    if npc:GetActorClassId() == 1000438 then
                        quest:SetQuestFlag(5)
                    end
                end
            "#,
        )
        .unwrap();

        let engine = LuaEngine::new(&root);
        let script_path = root.join("quests/man/man0l0.lua");
        let npc_spec = LuaNpcSpec {
            actor_id: 0x12345,
            name: "Rostnsthal".to_string(),
            class_name: "PopulaceStandard".to_string(),
            class_path: "/Chara/Npc/Populace/PopulaceStandard".to_string(),
            unique_id: "rostnsthal".to_string(),
            zone_id: 193,
            zone_name: "test".to_string(),
            state: 0,
            pos: (0.0, 0.0, 0.0),
            rotation: 0.0,
            actor_class_id: 1_000_438,
            quest_graphic: 0,
        };
        let result = engine.call_quest_hook(
            &script_path,
            "onTalk",
            sample_snapshot(),
            sample_quest_handle(CommandQueue::new()),
            vec![QuestHookArg::Npc(npc_spec)],
        );
        assert!(result.error.is_none(), "onTalk errored: {:?}", result.error);
        let saw_flag = result
            .commands
            .iter()
            .any(|c| matches!(c, LuaCommand::QuestSetFlag { bit: 5, .. }));
        assert!(saw_flag, "npc:GetActorClassId matched — but SetQuestFlag didn't fire; got {:?}", result.commands);

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
            vec![QuestHookArg::Int(10)],
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
