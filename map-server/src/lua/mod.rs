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
    /// An owned string — e.g. the `eventName` passed to
    /// `onEmote`/`onPush`/`onCommand` (`"emoteDefault1"`, `"pushDefault"`, …).
    Str(String),
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
    fn into_value(self, lua: &Lua, queue: Arc<Mutex<CommandQueue>>) -> mlua::Result<Value> {
        Ok(match self {
            QuestHookArg::Int(i) => Value::Integer(i as mlua::Integer),
            QuestHookArg::Bool(b) => Value::Boolean(b),
            QuestHookArg::Nil => Value::Nil,
            QuestHookArg::Str(s) => Value::String(lua.create_string(&s)?),
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
                        // Quest-hook NPCs are dialogue / quest givers,
                        // not combat — engagement defaults are correct.
                        is_engaged: false,
                        speed: 5.0,
                        target_actor_id: 0,
                        hp: 0,
                        max_hp: 0,
                    },
                    actor_class_id: spec.actor_class_id,
                    quest_graphic: spec.quest_graphic,
                };
                Value::UserData(lua.create_userdata(npc)?)
            }
        })
    }
}

/// Convert a wire-format [`common::luaparam::LuaParam`] into the matching
/// `mlua::Value` for handoff to a Lua coroutine. Used by
/// [`LuaEngine::call_director_on_event_started`] to forward the original
/// `EventStartPacket` lua_params tail through to the script — pmeteor's
/// `LuaUtils.CreateLuaParamObjectList` does the equivalent on the C# side.
fn lua_param_to_value(lua: &Lua, param: common::luaparam::LuaParam) -> mlua::Result<Value> {
    use common::luaparam::LuaParam;
    Ok(match param {
        LuaParam::Int32(v) => Value::Integer(v as mlua::Integer),
        LuaParam::UInt32(v) => Value::Integer(v as mlua::Integer),
        LuaParam::String(s) => Value::String(lua.create_string(&s)?),
        LuaParam::True => Value::Boolean(true),
        LuaParam::False => Value::Boolean(false),
        LuaParam::Nil => Value::Nil,
        LuaParam::Actor(id) => Value::Integer(id as mlua::Integer),
        LuaParam::Byte(b) => Value::Integer(b as mlua::Integer),
        LuaParam::Short(s) => Value::Integer(s as mlua::Integer),
        // Type7/Type9 are exotic — director scripts that need them can read
        // the raw u32/u64 fallbacks; for now we'd never hit this on the
        // EventStart path, so log and forward Nil.
        LuaParam::Type7 { actor_id, .. } => Value::Integer(actor_id as mlua::Integer),
        LuaParam::Type9 { item1, .. } => Value::Integer(item1 as mlua::Integer),
    })
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

        let owner_player_id = player_snapshot.actor_id;
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
        let resume_result = thread.resume::<MultiValue>(mv);
        let commands = CommandQueue::drain(&queue);
        let (value, error) = match resume_result {
            Ok(v) => (v, None),
            Err(e) => (MultiValue::new(), Some(anyhow::anyhow!("{hook_name}: {e}"))),
        };
        if matches!(thread.status(), mlua::ThreadStatus::Resumable) {
            let directive = scheduler::classify_yield_mv(&value);
            let parked = ParkedCoroutine {
                lua: lua.clone(),
                thread,
                queue,
                owner_player_id,
            };
            self.repark(parked, directive);
        }
        PartialLuaCallResult { commands, error }
    }

    /// Call a quest-script *getter* — `getJournalInformation` /
    /// `getJournalMapMarkerList` — and capture its return values as wire
    /// LuaParams. Direct call, no coroutine: pmeteor routes these through
    /// `LuaEngine.CallLuaFunctionForReturn` and the getters are pure
    /// `(player, quest) → values` lookups (no `callClientFunction`
    /// yields). A missing getter returns an empty list (most quests don't
    /// override them); conversion stops at the first nil, matching Lua's
    /// `unpack` semantics in pmeteor's
    /// `RequestQuestJournalCommand.lua::onEventStarted`. Commands the
    /// getter happens to enqueue are dropped — the journal pane is a
    /// read-only view. (Garlemald-Server #46.)
    pub fn call_quest_getter(
        &self,
        script_path: &Path,
        getter_name: &str,
        player_snapshot: userdata::PlayerSnapshot,
        quest_handle: userdata::LuaQuestHandle,
    ) -> anyhow::Result<Vec<common::luaparam::LuaParam>> {
        use common::luaparam::LuaParam;
        let (lua, queue) = self.load_script(script_path)?;
        let quest_handle = userdata::LuaQuestHandle {
            queue: queue.clone(),
            ..quest_handle
        };
        let globals = lua.globals();
        let f: Function = match globals.get(getter_name) {
            Ok(f) => f,
            Err(_) => return Ok(Vec::new()),
        };
        let player_ud = lua
            .create_userdata(userdata::LuaPlayer {
                snapshot: player_snapshot,
                queue: queue.clone(),
            })
            .map_err(|e| anyhow::anyhow!("create_userdata(LuaPlayer): {e}"))?;
        let quest_ud = lua
            .create_userdata(quest_handle)
            .map_err(|e| anyhow::anyhow!("create_userdata(LuaQuestHandle): {e}"))?;
        let call = f.call::<MultiValue>((player_ud, quest_ud));
        let dropped = CommandQueue::drain(&queue);
        if !dropped.is_empty() {
            tracing::debug!(
                getter = getter_name,
                dropped = dropped.len(),
                "quest getter enqueued commands — dropped (read-only view)",
            );
        }
        let values = call.map_err(|e| anyhow::anyhow!("{getter_name}: {e}"))?;
        let mut out = Vec::new();
        for v in values {
            match v {
                Value::Integer(i) => out.push(LuaParam::Int32(i as i32)),
                Value::Number(n) => out.push(LuaParam::Int32(n as i32)),
                Value::Boolean(true) => out.push(LuaParam::True),
                Value::Boolean(false) => out.push(LuaParam::False),
                Value::String(s) => out.push(LuaParam::String(s.to_string_lossy().to_string())),
                _ => break,
            }
        }
        Ok(out)
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
        let resume_result = thread.resume::<MultiValue>(director_ud);
        let commands = CommandQueue::drain(&queue);
        let (value, error) = match resume_result {
            Ok(v) => (v, None),
            Err(e) => (
                MultiValue::new(),
                Some(anyhow::anyhow!("director main: {e}")),
            ),
        };

        // Park if still alive + parked on a known wait directive.
        if matches!(thread.status(), mlua::ThreadStatus::Resumable) {
            let directive = scheduler::classify_yield_mv(&value);
            let parked = ParkedCoroutine {
                lua: lua.clone(),
                thread,
                queue,
                // No player in scope for a director `main`.
                owner_player_id: 0,
            };
            self.repark(parked, directive);
        }

        PartialLuaCallResult { commands, error }
    }

    /// High-level wrapper around [`Self::spawn_director_on_event_started`]
    /// — builds the canonical pmeteor argument shape `(player, director,
    /// eventType, eventName, ...lua_params)` from owned, `Send`-safe inputs
    /// and dispatches `onEventStarted`. Used by `handle_event_start` when
    /// an inbound `EventStartPacket` targets a content director.
    ///
    /// The pmeteor reference is `LuaEngine.cs::EventStarted` (lines 651-682):
    /// it prepends `eventType` then `eventName` as `LuaParam`s before calling
    /// `Director.OnEventStart(player, args)`, which then prepends `player +
    /// director`. So the final coroutine receives `(player, director,
    /// eventType, eventName, ...originalLuaParams)`. Garlemald's 1.x director
    /// scripts (e.g. `QuestDirectorMan0g001.lua::onEventStarted(player, actor,
    /// triggerName)`) declare three parameters; Lua silently ignores extras.
    pub fn call_director_on_event_started(
        &self,
        script_path: &Path,
        player_snapshot: userdata::PlayerSnapshot,
        director_handle: userdata::LuaDirectorHandle,
        event_name: String,
        event_type: u8,
        lua_params: Vec<common::luaparam::LuaParam>,
    ) -> PartialLuaCallResult {
        let owner_player_id = player_snapshot.actor_id;
        self.spawn_director_on_event_started(script_path, owner_player_id, |lua, queue| {
            let player = userdata::LuaPlayer {
                snapshot: player_snapshot,
                queue: queue.clone(),
            };
            let director = userdata::LuaDirectorHandle {
                queue: queue.clone(),
                ..director_handle
            };
            let player_ud = lua
                .create_userdata(player)
                .map_err(|e| anyhow::anyhow!("create_userdata(LuaPlayer): {e}"))?;
            let director_ud = lua
                .create_userdata(director)
                .map_err(|e| anyhow::anyhow!("create_userdata(LuaDirectorHandle): {e}"))?;
            let mut mv = MultiValue::new();
            mv.push_back(Value::UserData(player_ud));
            mv.push_back(Value::UserData(director_ud));
            mv.push_back(Value::Integer(event_type as mlua::Integer));
            let event_name_lua = lua
                .create_string(&event_name)
                .map_err(|e| anyhow::anyhow!("create_string(event_name): {e}"))?;
            mv.push_back(Value::String(event_name_lua));
            for p in lua_params {
                let v = lua_param_to_value(lua, p)
                    .map_err(|e| anyhow::anyhow!("lua_param_to_value: {e}"))?;
                mv.push_back(v);
            }
            Ok(mv)
        })
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
    /// Run a `commands/<Name>.lua::onEventStarted(player, command, eventType,
    /// eventName, ...luaParams)` script — the runtime dispatch for a client
    /// command static actor (e.g. ActivateCommand, the active/passive toggle).
    /// Mirrors pmeteor `LuaEngine.cs::EventStarted` for a Command target. The
    /// `command` arg is a stub `LuaActor` carrying the command actor id (the
    /// scripts that need it only read its id; ActivateCommand.lua ignores it).
    /// (Garlemald-Server #28.)
    #[allow(clippy::too_many_arguments)]
    pub fn call_command_on_event_started(
        &self,
        script_path: &Path,
        player_snapshot: userdata::PlayerSnapshot,
        command_actor_id: u32,
        event_name: String,
        // Wire field kept for signature symmetry with the director path, but
        // NOT forwarded to command scripts (they take triggerName, not
        // eventType — see the arg-push block below). (Garlemald-Server #46.)
        _event_type: u8,
        lua_params: Vec<common::luaparam::LuaParam>,
        // Identity of the physical actor the command was invoked on, when
        // the caller resolved it (harvest commands: the clicked gather
        // node's `(zone_id, unique_id)`). The command static actor's own
        // id (`command_actor_id`) is the same for every node, so the
        // struck node's identity rides on the `commandActor` userdata's
        // `zone_id` / `unique_id` fields — `DummyCommand.lua` reads them
        // back via `commandActor:GetZoneID()` / `:GetUniqueId()` to route
        // to the right template. `None` leaves the fields empty (every
        // non-harvest command). (Wave 3 gather partial.)
        command_actor_identity: Option<(u32, String)>,
    ) -> PartialLuaCallResult {
        let owner_player_id = player_snapshot.actor_id;
        let (command_zone_id, command_unique_id) = match command_actor_identity {
            Some((zone_id, unique_id)) => (zone_id, unique_id),
            None => (0, String::new()),
        };
        self.spawn_director_on_event_started(script_path, owner_player_id, |lua, queue| {
            let player = userdata::LuaPlayer {
                snapshot: player_snapshot,
                queue: queue.clone(),
            };
            let command = userdata::LuaActor {
                actor_id: command_actor_id,
                name: String::new(),
                class_name: String::new(),
                class_path: String::new(),
                unique_id: command_unique_id,
                zone_id: command_zone_id,
                zone_name: String::new(),
                state: 0,
                pos: (0.0, 0.0, 0.0),
                rotation: 0.0,
                queue: queue.clone(),
                is_engaged: false,
                speed: 0.0,
                target_actor_id: 0,
                hp: 0,
                max_hp: 0,
            };
            let player_ud = lua
                .create_userdata(player)
                .map_err(|e| anyhow::anyhow!("create_userdata(LuaPlayer): {e}"))?;
            let command_ud = lua
                .create_userdata(command)
                .map_err(|e| anyhow::anyhow!("create_userdata(LuaActor command): {e}"))?;
            let mut mv = MultiValue::new();
            mv.push_back(Value::UserData(player_ud));
            mv.push_back(Value::UserData(command_ud));
            // Command scripts take `onEventStarted(player, command, triggerName,
            // ...params)` — triggerName IS the event_name string (e.g.
            // "commandRequest"), and the real LuaParams (emoteId, showText, …)
            // follow. Unlike pmeteor (whose command scripts carry an extra
            // leading `eventType`), garlemald's ported command scripts in
            // scripts/lua/commands/*.lua drop it, so we must NOT push
            // `event_type` here: doing so shifted triggerName→eventType(0) and
            // emoteId→event_name("commandRequest"), which made
            // EmoteStandardCommand.lua's `string.format("%d", emoteId)` crash on
            // a string and every free emote silently no-op. The director path
            // (call_director_on_event_started) keeps eventType — directors DO
            // declare it. (Garlemald-Server #46.)
            let event_name_lua = lua
                .create_string(&event_name)
                .map_err(|e| anyhow::anyhow!("create_string(event_name): {e}"))?;
            mv.push_back(Value::String(event_name_lua));
            for p in lua_params {
                let v = lua_param_to_value(lua, p)
                    .map_err(|e| anyhow::anyhow!("lua_param_to_value: {e}"))?;
                mv.push_back(v);
            }
            Ok(mv)
        })
    }

    /// Run a plain (non-quest, non-command) NPC/object's own
    /// `onEventStarted(player, npc, eventType, eventName, ...params)` —
    /// the path that opens the aetheryte's teleport/homepoint/leve menu
    /// (`AetheryteParent.lua`) and any base populace dialogue. Mirror of
    /// pmeteor `LuaEngine.EventStarted` -> `CallLuaFunction(player,
    /// target, "onEventStarted", ...)` on the TARGET ACTOR's script.
    /// Builds a `LuaNpc` (not a bare `LuaActor`) so the script's
    /// `npc:GetActorClassId()` returns the real class id (AetheryteParent
    /// keys `aetheryteParentLinks` on it). Yielding menus park on
    /// `_WAIT_EVENT` and resume on the client's 0x012E EventUpdate, exactly
    /// like command / quest delegate round-trips. (Garlemald-Server #46
    /// live test round 2.)
    pub fn call_npc_on_event_started(
        &self,
        script_path: &Path,
        player_snapshot: userdata::PlayerSnapshot,
        npc_spec: LuaNpcSpec,
        event_name: String,
        // Like the command path: NPC/object scripts take
        // `onEventStarted(player, npc, triggerName, …)` (triggerName = the
        // event_name string), NOT eventType. Kept for signature symmetry only.
        // (Garlemald-Server #46.)
        _event_type: u8,
        lua_params: Vec<common::luaparam::LuaParam>,
    ) -> PartialLuaCallResult {
        let owner_player_id = player_snapshot.actor_id;
        self.spawn_director_on_event_started(script_path, owner_player_id, |lua, queue| {
            let player = userdata::LuaPlayer {
                snapshot: player_snapshot,
                queue: queue.clone(),
            };
            let npc = userdata::LuaNpc {
                base: userdata::LuaActor {
                    actor_id: npc_spec.actor_id,
                    name: npc_spec.name,
                    class_name: npc_spec.class_name,
                    class_path: npc_spec.class_path,
                    unique_id: npc_spec.unique_id,
                    zone_id: npc_spec.zone_id,
                    zone_name: npc_spec.zone_name,
                    state: npc_spec.state,
                    pos: npc_spec.pos,
                    rotation: npc_spec.rotation,
                    queue: queue.clone(),
                    is_engaged: false,
                    speed: 5.0,
                    target_actor_id: 0,
                    hp: 0,
                    max_hp: 0,
                },
                actor_class_id: npc_spec.actor_class_id,
                quest_graphic: npc_spec.quest_graphic,
            };
            let player_ud = lua
                .create_userdata(player)
                .map_err(|e| anyhow::anyhow!("create_userdata(LuaPlayer): {e}"))?;
            let npc_ud = lua
                .create_userdata(npc)
                .map_err(|e| anyhow::anyhow!("create_userdata(LuaNpc): {e}"))?;
            let mut mv = MultiValue::new();
            mv.push_back(Value::UserData(player_ud));
            mv.push_back(Value::UserData(npc_ud));
            // No eventType push — base NPC/object scripts compare their 3rd arg
            // (triggerName) to STRING literals like "talkDefault" / "pushDefault"
            // / "caution" / "exit". Pushing the eventType integer here made
            // those comparisons silently always-false (number != string).
            // (Garlemald-Server #46.)
            let event_name_lua = lua
                .create_string(&event_name)
                .map_err(|e| anyhow::anyhow!("create_string(event_name): {e}"))?;
            mv.push_back(Value::String(event_name_lua));
            for p in lua_params {
                let v = lua_param_to_value(lua, p)
                    .map_err(|e| anyhow::anyhow!("lua_param_to_value: {e}"))?;
                mv.push_back(v);
            }
            Ok(mv)
        })
    }

    pub fn spawn_director_on_event_started<F>(
        &self,
        script_path: &Path,
        owner_player_id: u32,
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
        let resume_result = thread.resume::<MultiValue>(args);
        let commands = CommandQueue::drain(&queue);
        let (value, error) = match resume_result {
            Ok(v) => (v, None),
            Err(e) => (
                MultiValue::new(),
                Some(anyhow::anyhow!("director onEventStarted: {e}")),
            ),
        };
        if matches!(thread.status(), mlua::ThreadStatus::Resumable) {
            let directive = scheduler::classify_yield_mv(&value);
            let parked = ParkedCoroutine {
                lua: lua.clone(),
                thread,
                queue,
                owner_player_id,
            };
            self.repark(parked, directive);
        }
        PartialLuaCallResult { commands, error }
    }

    /// Run a content script's hook (typically `onCreate`) inside a
    /// coroutine. Hook signature: `hook(player, contentArea, director)`
    /// matching `SimpleContent30010.lua::onCreate(starterPlayer,
    /// contentArea, director)` from the man0g0 SEQ_005 combat tutorial.
    ///
    /// Phase-A wiring: the script will likely call several Lua bindings
    /// that aren't fully implemented (`SpawnBattleNpcById`, `SetMod`,
    /// `currentParty:AddMember`, etc.) — those are no-op-with-logging
    /// stubs in `userdata.rs`. This helper still drives the script
    /// through to completion (or first error) so the calls are visible
    /// in tracing, and any commands the script emits before/after the
    /// stub log lines are drained back to the caller.
    ///
    /// Mirrors C# `LuaEngine.CallLuaFunction(player, contentArea,
    /// "onCreate", true)` semantics, sized for the content/onCreate
    /// path specifically.
    pub fn call_content_hook(
        &self,
        script_path: &Path,
        hook_name: &str,
        player_snapshot: userdata::PlayerSnapshot,
        content_area: userdata::LuaContentArea,
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
        // Re-point all three handles at the freshly-installed queue
        // so commands the hook emits land where the caller drains.
        let player = userdata::LuaPlayer {
            snapshot: player_snapshot,
            queue: queue.clone(),
        };
        // Phase C2b/C3 — also re-point every roster snapshot's queue
        // so `allies[i].Engage(target)` / `players[i]:IsEngaged()` /
        // etc. push into the script's queue, not the queue the
        // caller stamped at content-area-build time.
        let mut content_area = content_area;
        content_area.queue = queue.clone();
        for actor in content_area.allies.iter_mut() {
            actor.queue = queue.clone();
        }
        for actor in content_area.monsters.iter_mut() {
            actor.queue = queue.clone();
        }
        let director = userdata::LuaDirectorHandle {
            queue: queue.clone(),
            ..director
        };

        let globals = lua.globals();
        let f: Function = match globals.get(hook_name) {
            Ok(f) => f,
            Err(_) => {
                // Missing hook — quiet no-op, drain script-top-level
                // commands so anything the require() chain pushed
                // still reaches the caller.
                return PartialLuaCallResult {
                    commands: CommandQueue::drain(&queue),
                    error: None,
                };
            }
        };

        let owner_player_id = player.snapshot.actor_id;
        let player_ud = match lua.create_userdata(player) {
            Ok(ud) => ud,
            Err(e) => {
                return PartialLuaCallResult {
                    commands: CommandQueue::drain(&queue),
                    error: Some(anyhow::anyhow!("create_userdata(LuaPlayer): {e}")),
                };
            }
        };
        let area_ud = match lua.create_userdata(content_area) {
            Ok(ud) => ud,
            Err(e) => {
                return PartialLuaCallResult {
                    commands: CommandQueue::drain(&queue),
                    error: Some(anyhow::anyhow!("create_userdata(LuaContentArea): {e}")),
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

        let mut mv = MultiValue::new();
        mv.push_back(Value::UserData(player_ud));
        mv.push_back(Value::UserData(area_ud));
        mv.push_back(Value::UserData(director_ud));

        let thread = match lua.create_thread(f) {
            Ok(t) => t,
            Err(e) => {
                return PartialLuaCallResult {
                    commands: CommandQueue::drain(&queue),
                    error: Some(anyhow::anyhow!("create_thread({hook_name}): {e}")),
                };
            }
        };
        let resume_result = thread.resume::<MultiValue>(mv);
        let commands = CommandQueue::drain(&queue);
        let (value, error) = match resume_result {
            Ok(v) => (v, None),
            Err(e) => (MultiValue::new(), Some(anyhow::anyhow!("{hook_name}: {e}"))),
        };
        if matches!(thread.status(), mlua::ThreadStatus::Resumable) {
            let directive = scheduler::classify_yield_mv(&value);
            let parked = ParkedCoroutine {
                lua: lua.clone(),
                thread,
                queue,
                owner_player_id,
            };
            self.repark(parked, directive);
        }
        PartialLuaCallResult { commands, error }
    }

    /// pmeteor `Npc.CreateScriptBindPacket` parity: call the NPC base
    /// class's `init(npc)` and return its results as wire LuaParams —
    /// they become the ActorInstantiate params after the standard
    /// 7-param prefix `[classPath, false×5, actorClassId]`. Per-class
    /// shapes matter: `DoorStandard.init` returns `(false, false, 0,
    /// 0, 0, 0)` — the trailing four ints feed the client's
    /// `_setReactionTriggerBox_cpp` — while `PopulaceStandard.init`
    /// returns `(false, false, 0, 0)`. The previous universal
    /// populace-shaped tail crashed the client's `DoorStandard:
    /// initForEvent` ("invalid argument: 2, integer" → error 40000 →
    /// kicked to login) whenever a door's event condition was enabled.
    /// `None` when the script / `init` is missing or errors — callers
    /// fall back to the populace-shaped tail.
    pub fn call_npc_init(&self, class_path_lower: &str) -> Option<Vec<common::luaparam::LuaParam>> {
        use common::luaparam::LuaParam;
        let path = self.resolver().base_class(class_path_lower);
        if !path.exists() {
            return None;
        }
        let (lua, _queue) = self.load_script(&path).ok()?;
        let init: mlua::Function = lua.globals().get("init").ok()?;
        // Base-class `init` implementations ignore the npc argument
        // (they return constants); pass Nil rather than building a
        // throwaway userdata. A script that does dereference it errors
        // here and falls back to the default tail.
        let ret: MultiValue = init.call(mlua::Value::Nil).ok()?;
        let mut out = Vec::with_capacity(ret.len());
        for v in ret.iter() {
            out.push(match v {
                mlua::Value::Nil => LuaParam::Nil,
                mlua::Value::Boolean(true) => LuaParam::True,
                mlua::Value::Boolean(false) => LuaParam::False,
                mlua::Value::Integer(i) => LuaParam::Int32(*i as i32),
                mlua::Value::Number(n) => LuaParam::Int32(*n as i32),
                mlua::Value::String(s) => LuaParam::String(s.to_str().ok()?.to_string()),
                _ => LuaParam::Nil,
            });
        }
        Some(out)
    }

    /// B6 of the SEQ_005 unblock plan — fire a content script's
    /// `onUpdate(tick, area)` hook. Sibling to
    /// [`call_content_hook`] that takes the simpler 2-arg shape
    /// `SimpleContent30010.lua::onUpdate` expects.
    ///
    /// Phase B6 simplification: drains commands the hook emits but
    /// doesn't park yields (onUpdate isn't expected to yield —
    /// it's a per-tick poll, not a multi-frame coroutine). If
    /// scripts grow long-running update logic later, the parking
    /// machinery from `call_content_hook` can drop in.
    pub fn call_content_on_update(
        &self,
        script_path: &Path,
        tick: u64,
        content_area: userdata::LuaContentArea,
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
        // Phase C2b/C3 — re-point every roster snapshot's queue to
        // the script-bound queue so commands emitted by the iterator
        // bodies land in the right drain.
        let mut content_area = userdata::LuaContentArea {
            queue: queue.clone(),
            ..content_area
        };
        for actor in content_area.allies.iter_mut() {
            actor.queue = queue.clone();
        }
        for actor in content_area.monsters.iter_mut() {
            actor.queue = queue.clone();
        }

        let globals = lua.globals();
        let f: Function = match globals.get("onUpdate") {
            Ok(f) => f,
            Err(_) => {
                // No onUpdate — quiet no-op. Some content scripts
                // only define onCreate.
                return PartialLuaCallResult {
                    commands: CommandQueue::drain(&queue),
                    error: None,
                };
            }
        };

        let area_ud = match lua.create_userdata(content_area) {
            Ok(ud) => ud,
            Err(e) => {
                return PartialLuaCallResult {
                    commands: CommandQueue::drain(&queue),
                    error: Some(anyhow::anyhow!("create_userdata(LuaContentArea): {e}")),
                };
            }
        };

        // Direct call (no coroutine). onUpdate is a per-tick poll
        // and shouldn't yield.
        let result: mlua::Result<Value> = f.call((tick, mlua::Value::UserData(area_ud)));
        let commands = CommandQueue::drain(&queue);
        let error = result.err().map(|e| anyhow::anyhow!("onUpdate: {e}"));
        PartialLuaCallResult { commands, error }
    }

    /// Re-point the VM's queue-capturing globals (`GetWorldManager`,
    /// `GetStaticActor`, …) at a parked coroutine's OWN queue before
    /// resuming it. `load_script` installs a fresh queue on every call
    /// into a cached VM, so any call that interleaves between a park and
    /// its resume leaves the globals pushing into a queue nobody drains
    /// — live proof: Man0g1's `onStart` parked on the opening cutscene,
    /// `onStateChange` re-pointed the VM, and the resumed slice's
    /// `GetWorldManager():DoZoneChange(...)` vanished while the
    /// userdata-held `player:EndEvent()` (bound to the parked queue)
    /// arrived — the endless post-Canopy "Now Loading". (#28.)
    fn repoint_globals_for_resume(&self, parked: &ParkedCoroutine) {
        if let Err(e) = install_globals(&parked.lua, parked.queue.clone(), self.catalogs.clone()) {
            tracing::warn!(
                owner = parked.owner_player_id,
                error = %e,
                "repoint_globals_for_resume failed — global-pushed commands may be lost",
            );
        }
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
    /// Returns `(owner_player_id, commands)` batches — one per resumed
    /// coroutine — so the ticker can route each batch through the EVENT
    /// bridge for that player (event-flavoured commands like
    /// `KickEvent`/`RunEventFunction`/`EndEvent` are dropped by the
    /// plain runtime drain). `owner_player_id == 0` means no player
    /// context (apply via the runtime drain as before).
    pub fn tick(&self) -> Vec<(u32, Vec<LuaCommand>)> {
        let mut all_commands = Vec::new();

        let due = self
            .scheduler
            .lock()
            .map(|mut s| s.drain_due_time())
            .unwrap_or_default();
        for parked in due {
            let queue = parked.queue.clone();
            let owner = parked.owner_player_id;
            self.repoint_globals_for_resume(&parked);
            let resume = parked.thread.resume::<MultiValue>(());
            // Drain whatever the resumed slice pushed into the
            // script's command queue — the directive classification
            // below doesn't touch the queue, so drain before reparking.
            let cmds = CommandQueue::drain(&queue);
            if cmds.is_empty() {
                // A resumed slice that queued nothing is legal (e.g.
                // straight into another wait) but worth a breadcrumb —
                // it is also the signature of a queue-repoint bug
                // (commands landing in a reinstalled queue).
                tracing::debug!(owner, "tick: resumed coroutine produced no commands");
            } else {
                all_commands.push((owner, cmds));
            }
            match resume {
                Ok(value) => {
                    let directive = scheduler::classify_yield_mv(&value);
                    self.repark(parked, directive);
                }
                Err(e) => {
                    tracing::warn!(
                        owner,
                        error = %e,
                        "tick: parked coroutine resume errored — coroutine dropped",
                    );
                }
            }
        }

        all_commands
    }

    /// Notify the scheduler that `signal` fired, resuming every coroutine
    /// parked on it via [`Self::fire_signal_and_drain`] (commands discarded).
    pub fn fire_signal(&self, signal: &str) {
        let _ = self.fire_signal_and_drain(signal);
    }

    /// Like [`Self::fire_signal`] but drains the commands each resumed
    /// coroutine queued before it yielded/finished, and re-parks any
    /// coroutine that yielded again — mirroring [`Self::tick`] /
    /// [`Self::fire_player_event_and_drain`]. Returns the collected commands
    /// for the caller to apply (the old `fire_signal` resumed coroutines but
    /// silently dropped their commands and lost re-yielded handles, so a
    /// director woken on "playerActive" never ran its post-`waitForSignal`
    /// body). (Garlemald-Server #28.)
    pub fn fire_signal_and_drain(&self, signal: &str) -> Vec<LuaCommand> {
        let due = self
            .scheduler
            .lock()
            .map(|mut s| s.drain_signal(signal))
            .unwrap_or_default();
        tracing::debug!(
            signal,
            resumed = due.len(),
            "fire_signal: draining parked coroutines",
        );
        let mut all_commands = Vec::new();
        for parked in due {
            let queue = parked.queue.clone();
            self.repoint_globals_for_resume(&parked);
            let resume = parked.thread.resume::<MultiValue>(());
            all_commands.extend(CommandQueue::drain(&queue));
            match resume {
                Ok(value) => {
                    if matches!(parked.thread.status(), mlua::ThreadStatus::Resumable) {
                        let directive = scheduler::classify_yield_mv(&value);
                        self.repark(parked, directive);
                    } else {
                        // Previously silent — the live SEQ_005 `wait(1)`-
                        // never-resumes break leaves NO trace if the
                        // coroutine comes back non-Resumable here (it is
                        // dropped, nothing reparks, and subsequent signals
                        // find nothing). Log the terminal state so the
                        // disappearance is attributable. (#28 S1.4.)
                        tracing::debug!(
                            signal,
                            owner = parked.owner_player_id,
                            status = ?parked.thread.status(),
                            "fire_signal: coroutine completed (non-resumable) — dropped from scheduler",
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        signal,
                        owner = parked.owner_player_id,
                        error = %e,
                        "fire_signal: coroutine resume errored — coroutine dropped",
                    );
                }
            }
        }
        all_commands
    }

    /// Notify the scheduler that `player_id` just received an event update.
    pub fn fire_player_event(&self, player_id: u32, params: &[common::luaparam::LuaParam]) -> bool {
        self.fire_player_event_and_drain(player_id, params)
            .is_some()
    }

    /// Like [`Self::fire_player_event`] but returns the commands the
    /// resumed coroutine queued before yielding/finishing, and re-parks
    /// the coroutine if it yielded again on a known directive. `None`
    /// means no coroutine was waiting; otherwise the (possibly empty)
    /// command list is returned.
    ///
    /// `params` is the wire-format LuaParams tail of the client's
    /// `0x012E EventUpdate`. It becomes the resume values of the parked
    /// `coroutine.yield("_WAIT_EVENT", ...)` — i.e. the RETURN VALUES of
    /// the script's `callClientFunction(...)`. pmeteor does the same in
    /// `LuaEngine.OnEventUpdate` (`coroutine.Resume(LuaUtils.
    /// CreateLuaParamObjectList(args))`). Dropping them breaks every
    /// script that reads a client choice — e.g. man0l0's exit door
    /// (`processEventNewRectAsk` → `choice == 1`) silently took the
    /// "no" branch because `choice` resumed as nil (Garlemald-Server
    /// issue 25 follow-up). Conversion happens HERE, after the take,
    /// because mlua Values must be created from the parked coroutine's
    /// own VM (one VM per script path).
    ///
    /// Falls back to `player_id=0` if no coroutine is parked under the
    /// specific id — `coroutine.yield("_WAIT_EVENT", player)` from
    /// `global.lua` passes the `LuaPlayer` userdata as the second yield
    /// arg, but mlua's `Thread::resume::<Value>` discards yield args
    /// past the first, so the scheduler ends up parking everything on
    /// the bare-string variant's player_id=0 fallback.
    pub fn fire_player_event_and_drain(
        &self,
        player_id: u32,
        params: &[common::luaparam::LuaParam],
    ) -> Option<Vec<LuaCommand>> {
        let parked = self.scheduler.lock().ok().and_then(|mut s| {
            s.take_event(player_id).or_else(|| {
                if player_id != 0 {
                    s.take_event(0)
                } else {
                    None
                }
            })
        })?;
        self.repoint_globals_for_resume(&parked);
        let mut args = MultiValue::new();
        for p in params {
            match lua_param_to_value(&parked.lua, p.clone()) {
                Ok(v) => args.push_back(v),
                Err(e) => {
                    tracing::warn!(
                        player = player_id,
                        error = %e,
                        param = ?p,
                        "fire_player_event: lua_param conversion failed — resuming with nil"
                    );
                    args.push_back(Value::Nil);
                }
            }
        }
        let resume_result = parked.thread.resume::<MultiValue>(args);
        let parked_queue_ptr = format!("{:p}", std::sync::Arc::as_ptr(&parked.queue));
        let commands = CommandQueue::drain(&parked.queue);
        // DIAGNOSTIC (Garlemald-Server #46): show exactly which commands a
        // resumed coroutine drained + the queue ptr it drained from, so a live
        // Isandorel-talk repro reveals whether `QuestIncCounter` is present
        // (queue/apply bug) or absent because the binding pushed to a DIFFERENT
        // queue (compare against the IncCounter binding's logged queue_ptr).
        // Temporary triage.
        if !commands.is_empty() {
            tracing::debug!(
                player = player_id,
                count = commands.len(),
                drained_queue_ptr = %parked_queue_ptr,
                commands = ?commands,
                "fire_player_event_and_drain: resumed coroutine drained these commands",
            );
        }
        match resume_result {
            Ok(value) => {
                if matches!(parked.thread.status(), mlua::ThreadStatus::Resumable) {
                    // Coroutine yielded again — re-park on whatever
                    // directive it returned. Without this, a multi-step
                    // script that yields more than once would lose its
                    // handle.
                    let directive = scheduler::classify_yield_mv(&value);
                    self.repark(parked, directive);
                }
            }
            Err(e) => {
                tracing::warn!(
                    player = player_id,
                    error = %e,
                    "fire_player_event: coroutine resume errored — coroutine dropped",
                );
            }
        }
        Some(commands)
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
    use crate::lua::command::LuaCommandArg;

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
                counters: [0; 4],
                npc_ls_from: 0,
                npc_ls_msg_step: 0,
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
            counters: [0; 4],
            npc_ls_from: 0,
            npc_ls_msg_step: 0,
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

        assert!(
            result.error.is_none(),
            "onStart errored: {:?}",
            result.error
        );
        let has_set_flag = result.commands.iter().any(|c| {
            matches!(
                c,
                LuaCommand::QuestSetFlag {
                    bit: 2,
                    quest_id: 110_001,
                    ..
                }
            )
        });
        let has_start_seq = result.commands.iter().any(|c| {
            matches!(
                c,
                LuaCommand::QuestStartSequence {
                    sequence: 5,
                    quest_id: 110_001,
                    ..
                }
            )
        });
        assert!(
            has_set_flag,
            "missing QuestSetFlag; got {:?}",
            result.commands
        );
        assert!(
            has_start_seq,
            "missing QuestStartSequence; got {:?}",
            result.commands
        );

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
        assert!(
            result.error.is_none(),
            "missing hook should be quiet: {:?}",
            result.error
        );
        assert!(result.commands.is_empty());

        let _ = std::fs::remove_dir_all(root);
    }

    /// Garlemald-Server #46 — `call_quest_getter` captures the journal
    /// getters' return values: integers convert to Int32, conversion
    /// stops at the first nil (Lua `unpack` semantics), a missing getter
    /// is an empty list, and commands the getter enqueues are dropped.
    #[test]
    fn call_quest_getter_captures_returns_and_stops_at_nil() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("quests/man")).unwrap();
        std::fs::write(
            root.join("quests/man/man0l0.lua"),
            r#"
                function getJournalMapMarkerList(player, quest)
                    quest:SetQuestFlag(9) -- read-only view: must be dropped
                    return 11000101, 11000102, nil, 11000103
                end
            "#,
        )
        .unwrap();

        let engine = LuaEngine::new(&root);
        let script_path = root.join("quests/man/man0l0.lua");
        let values = engine
            .call_quest_getter(
                &script_path,
                "getJournalMapMarkerList",
                sample_snapshot(),
                sample_quest_handle(CommandQueue::new()),
            )
            .expect("getter should run");
        assert_eq!(
            values,
            vec![
                common::luaparam::LuaParam::Int32(11_000_101),
                common::luaparam::LuaParam::Int32(11_000_102),
            ],
            "two ids before the nil, nothing after",
        );

        let missing = engine
            .call_quest_getter(
                &script_path,
                "getJournalInformation",
                sample_snapshot(),
                sample_quest_handle(CommandQueue::new()),
            )
            .expect("missing getter is a quiet empty");
        assert!(missing.is_empty());

        let _ = std::fs::remove_dir_all(root);
    }

    /// Garlemald-Server #46 — the man0l1 SEQ_003 `onNotice` hook (fired
    /// server-driven by the AfterQuestWarpDirector kick after the Baderon
    /// talk) must drive the client-side `processEventTu_001` (the NPC-linkshell
    /// tutorial) to lift the desktopWidgetMode-16 mask processEvent020 leaves
    /// the client in. Only `processEventTu_001` calls
    /// `cancelDesktopWidgetMode(16)` (→ mode 120, which the menu gate allows);
    /// `endTutorialMode` = SendDataPacket(7) cancels mode 120, NOT 16.
    ///
    /// CRUCIAL ordering invariant (the round-5 softlock regression): the hook
    /// must emit it with the RAW non-parking `player:RunEventFunction`, NOT
    /// `callClientFunction`. `callClientFunction` parks on
    /// `coroutine.yield("_WAIT_EVENT")` waiting for a client EventUpdate that
    /// `processEventTu_001` (a synchronous UI mask) never sends — so the
    /// trailing `player:EndEvent()` would never run, the noticeEvent would
    /// stay open, and the client would be event-locked (softlock). This test
    /// asserts BOTH the RunEventFunction AND the EndEvent are emitted in the
    /// same (non-parking) drain — if a future edit reintroduces
    /// `callClientFunction` here, the hook parks and the EndEvent vanishes,
    /// failing this test.
    #[test]
    fn real_man0l1_on_notice_seq003_unmasks_menu() {
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/lua"));
        let script_path = root.join("quests/man/man0l1.lua");
        let engine = LuaEngine::new(root);

        let quest_handle = userdata::LuaQuestHandle {
            player_id: 42,
            quest_id: 110_002,
            has_quest: true,
            sequence: 3, // SEQ_003
            flags: 0,
            counters: [0; 4],
            npc_ls_from: 1,
            npc_ls_msg_step: 1,
            queue: CommandQueue::new(),
        };
        let result = engine.call_quest_hook(
            &script_path,
            "onNotice",
            sample_snapshot(),
            quest_handle,
            vec![QuestHookArg::Nil],
        );
        assert!(
            result.error.is_none(),
            "onNotice errored: {:?}",
            result.error
        );
        assert!(
            result.commands.iter().any(|c| matches!(
                c,
                LuaCommand::RunEventFunction { function_name, args, .. }
                    if function_name == "delegateEvent"
                        && args.iter().any(|a| matches!(
                            a,
                            LuaCommandArg::String(s) if s == "processEventTu_001"
                        ))
            )),
            "man0l1 onNotice(SEQ_003) must drive processEventTu_001 (RunEventFunction) \
             to cancel desktopWidgetMode 16; got {:?}",
            result.commands,
        );
        // Non-parking invariant: the EndEvent MUST be emitted in the same
        // drain. If onNotice parks on `callClientFunction` (the round-5
        // softlock), the hook yields before reaching EndEvent and this fails.
        assert!(
            result
                .commands
                .iter()
                .any(|c| matches!(c, LuaCommand::EndEvent { .. })),
            "man0l1 onNotice(SEQ_003) must also emit EndEvent in the same drain \
             (proves it did NOT park on callClientFunction); got {:?}",
            result.commands,
        );
    }

    /// P4 (Garlemald-Server) — the REAL `man0u0.lua` (Ul'dah opener)
    /// `onJournalRequest` hook is the journal-handoff that advances the
    /// Merchant-Strip mini-tutorial chain. Once Ascilia's first talk is
    /// done (`FLAG_SEQ000_MINITUT0` = bit 0) but her follow-up talk isn't
    /// (`FLAG_SEQ000_MINITUT1` = bit 1), opening the journal must set bit 1
    /// and re-run the ENPC swap so the next NPC marker (Fretful Farmhand)
    /// lights up. It must NOT touch flags once bit 1 is already set, and
    /// must no-op before bit 0 is set (chain hasn't started).
    #[test]
    fn real_man0u0_on_journal_request_advances_minitut1() {
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/lua"));
        let script_path = root.join("quests/man/man0u0.lua");
        assert!(script_path.exists());
        let engine = LuaEngine::new(root);

        let handle_with_flags = |flags: u32| userdata::LuaQuestHandle {
            player_id: 42,
            quest_id: 110_009, // Man0u0
            has_quest: true,
            sequence: 0, // SEQ_000
            flags,
            counters: [0; 4],
            npc_ls_from: 0,
            npc_ls_msg_step: 0,
            queue: CommandQueue::new(),
        };

        // MINITUT0 set, MINITUT1 not → advance to MINITUT1 + UpdateENPCs.
        let advanced = engine.call_quest_hook(
            &script_path,
            "onJournalRequest",
            sample_snapshot(),
            handle_with_flags(0b0001),
            vec![],
        );
        assert!(
            advanced.error.is_none(),
            "onJournalRequest errored: {:?}",
            advanced.error
        );
        assert!(
            advanced.commands.iter().any(|c| matches!(
                c,
                LuaCommand::QuestSetFlag {
                    bit: 1,
                    quest_id: 110_009,
                    ..
                }
            )),
            "journal handoff must SetFlag(MINITUT1); got {:?}",
            advanced.commands,
        );
        assert!(
            advanced.commands.iter().any(|c| matches!(
                c,
                LuaCommand::QuestUpdateEnpcs {
                    quest_id: 110_009,
                    ..
                }
            )),
            "journal handoff must re-run UpdateENPCs; got {:?}",
            advanced.commands,
        );

        // MINITUT0 and MINITUT1 both set → no further advance.
        let already = engine.call_quest_hook(
            &script_path,
            "onJournalRequest",
            sample_snapshot(),
            handle_with_flags(0b0011),
            vec![],
        );
        assert!(
            already.error.is_none(),
            "onJournalRequest errored: {:?}",
            already.error
        );
        assert!(
            !already
                .commands
                .iter()
                .any(|c| matches!(c, LuaCommand::QuestSetFlag { bit: 1, .. })),
            "must not re-set MINITUT1 once set; got {:?}",
            already.commands,
        );

        // MINITUT0 not yet set → no-op (the chain hasn't started).
        let not_started = engine.call_quest_hook(
            &script_path,
            "onJournalRequest",
            sample_snapshot(),
            handle_with_flags(0b0000),
            vec![],
        );
        assert!(
            not_started.error.is_none(),
            "onJournalRequest errored: {:?}",
            not_started.error
        );
        assert!(
            not_started.commands.is_empty(),
            "must no-op before MINITUT0 is set; got {:?}",
            not_started.commands,
        );

        // On a later sequence (SEQ_010) the guard must fire even with
        // MINITUT0 set → no flag mutation, no ENPC refresh.
        let wrong_sequence = engine.call_quest_hook(
            &script_path,
            "onJournalRequest",
            sample_snapshot(),
            userdata::LuaQuestHandle {
                sequence: 10, // SEQ_010, not SEQ_000
                ..handle_with_flags(0b0001)
            },
            vec![],
        );
        assert!(
            wrong_sequence.error.is_none(),
            "onJournalRequest errored: {:?}",
            wrong_sequence.error
        );
        assert!(
            wrong_sequence.commands.is_empty(),
            "journal handoff must no-op outside SEQ_000; got {:?}",
            wrong_sequence.commands,
        );
    }

    /// Garlemald-Server #46 live test — drive the REAL `man0l1.lua`
    /// `onNpcLS` hook at SEQ_003 (from=1, msgStep=1), the Path-Companion
    /// linkshell beat that ends tutorial mode. Asserts the hook (a)
    /// emits the localized-display-name narration line (msg 339 from
    /// pack 1, sender display 1000015) via the new 0x0161 path, and (b)
    /// delivers `endTutorialMode` = SendDataPacket(7) — the belt-and-
    /// braces menu unlock. Before the fix `onNpcLS` was unreachable (no
    /// HandleNpcLs dispatch) and SendDataPacket was dropped on the
    /// quest-hook drain.
    #[test]
    fn real_man0l1_on_npc_ls_seq003_ends_tutorial() {
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/lua"));
        let script_path = root.join("quests/man/man0l1.lua");
        assert!(script_path.exists());
        let engine = LuaEngine::new(root);

        let quest_handle = userdata::LuaQuestHandle {
            player_id: 42,
            quest_id: 110_002,
            has_quest: true,
            sequence: 3, // SEQ_003
            flags: 0,
            counters: [0; 4],
            npc_ls_from: 1,
            npc_ls_msg_step: 1,
            queue: CommandQueue::new(),
        };
        let result = engine.call_quest_hook(
            &script_path,
            "onNpcLS",
            sample_snapshot(),
            quest_handle,
            vec![QuestHookArg::Int(1), QuestHookArg::Int(1)],
        );
        assert!(
            result.error.is_none(),
            "onNpcLS errored: {:?}",
            result.error
        );
        assert!(
            result.commands.iter().any(|c| matches!(
                c,
                LuaCommand::SendGameMessageLocalizedDisplayName {
                    text_id: 339,
                    display_id: 1_000_015,
                    ..
                }
            )),
            "expected the Path-Companion narration line (msg 339, sender 1000015); got {:?}",
            result.commands,
        );
        assert!(
            result
                .commands
                .iter()
                .any(|c| matches!(c, LuaCommand::SendDataPacket { .. })),
            "expected endTutorialMode → SendDataPacket(7); got {:?}",
            result.commands,
        );
    }

    /// Garlemald-Server #46 — drive the REAL `man0l1.lua` journal
    /// getters. Pins the marker beat-map (`getJournalMapMarkerList`) and
    /// the counter-derived journal info (`getJournalInformation`) against
    /// the production script, which also syntax-checks the whole file.
    #[test]
    fn real_man0l1_journal_getters() {
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/lua"));
        let script_path = root.join("quests/man/man0l1.lua");
        assert!(script_path.exists());
        let engine = LuaEngine::new(root);

        let handle_at = |sequence: u32, counters: [u16; 4]| userdata::LuaQuestHandle {
            player_id: 42,
            quest_id: 110_002,
            has_quest: true,
            sequence,
            flags: 0,
            counters,
            npc_ls_from: 0,
            npc_ls_msg_step: 0,
            queue: CommandQueue::new(),
        };
        let markers = |sequence: u32, counters: [u16; 4]| {
            engine
                .call_quest_getter(
                    &script_path,
                    "getJournalMapMarkerList",
                    sample_snapshot(),
                    handle_at(sequence, counters),
                )
                .expect("marker getter should run")
        };
        use common::luaparam::LuaParam::Int32;

        // SEQ_003: Camp Bearded Rock attunement → base+2.
        assert_eq!(markers(3, [0; 4]), vec![Int32(11_000_102)]);
        // SEQ_007 fresh: both guild errands outstanding → CUL + MSK.
        assert_eq!(
            markers(7, [0; 4]),
            vec![Int32(11_000_103), Int32(11_000_104)],
        );
        // SEQ_007 after the Balloonfish sale (CUL counter=1, slot 1) and
        // the MSK Echo exit (MSK counter=4, slot 2): no markers left.
        assert_eq!(markers(7, [0, 1, 4, 0]), vec![]);
        // SEQ_048: Zephyr Gate muster → base+6.
        assert_eq!(markers(48, [0; 4]), vec![Int32(11_000_106)]);
        // SEQ_050 (escort instance) and SEQ_070 (linkshell beat): none.
        assert_eq!(markers(50, [0; 4]), vec![]);
        assert_eq!(markers(70, [0; 4]), vec![]);
        // SEQ_092: back to Baderon → base+1.
        assert_eq!(markers(92, [0; 4]), vec![Int32(11_000_101)]);

        // getJournalInformation: (0, CUL*5, MSK*5) from the SEQ_007
        // counters.
        let info = engine
            .call_quest_getter(
                &script_path,
                "getJournalInformation",
                sample_snapshot(),
                handle_at(7, [0, 1, 4, 0]),
            )
            .expect("info getter should run");
        assert_eq!(info, vec![Int32(0), Int32(5), Int32(20)]);
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
        assert!(
            saw_flag,
            "npc:GetActorClassId matched — but SetQuestFlag didn't fire; got {:?}",
            result.commands
        );

        let _ = std::fs::remove_dir_all(root);
    }

    /// `npc.Id` must expose the actor INSTANCE id (distinct from the class id
    /// via `GetActorClassId`). onEmote passes `npc.Id` to `player:DoEmote` as
    /// the target — a nil there errors the binding (`target_actor_id: u32`)
    /// and unwinds the hook. (Garlemald-Server #46.)
    #[test]
    fn lua_npc_exposes_instance_id_field() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("quests/man")).unwrap();
        std::fs::write(
            root.join("quests/man/man0l0.lua"),
            r#"
                function onTalk(player, quest, npc)
                    if npc.Id == 0x12345 and npc:GetActorClassId() == 1000155 then
                        quest:SetQuestFlag(6)
                    end
                end
            "#,
        )
        .unwrap();

        let engine = LuaEngine::new(&root);
        let script_path = root.join("quests/man/man0l0.lua");
        let npc_spec = LuaNpcSpec {
            actor_id: 0x12345,
            name: "Sisipu".to_string(),
            class_name: "PopulaceStandard".to_string(),
            class_path: "/Chara/Npc/Populace/PopulaceStandard".to_string(),
            unique_id: "sisipu".to_string(),
            zone_id: 230,
            zone_name: "test".to_string(),
            state: 0,
            pos: (0.0, 0.0, 0.0),
            rotation: 0.0,
            actor_class_id: 1_000_155,
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
        assert!(
            result
                .commands
                .iter()
                .any(|c| matches!(c, LuaCommand::QuestSetFlag { bit: 6, .. })),
            "npc.Id (instance) / GetActorClassId (class) mismatch; got {:?}",
            result.commands
        );

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
        assert!(
            seen,
            "script didn't see sequence=10; got {:?}",
            result.commands
        );

        let _ = std::fs::remove_dir_all(root);
    }

    /// Regression: a `QuestHookArg::Str` reaches the hook as its positional
    /// arg. `onEmote(player, quest, npc, eventName)` needs the eventName
    /// string ("emoteDefault1" = /bow) to pick the right branch — the
    /// dispatcher previously passed only the npc, leaving eventName nil so
    /// man0l1 SEQ_040's hand-signal test never matched. (Garlemald-Server #46.)
    #[test]
    fn call_quest_hook_passes_string_event_name_arg() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("quests/man")).unwrap();
        std::fs::write(
            root.join("quests/man/man0l1.lua"),
            r#"
                function onEmote(player, quest, npc, eventName)
                    if eventName == "emoteDefault1" then
                        quest:StartSequence(77)
                    end
                end
            "#,
        )
        .unwrap();

        let engine = LuaEngine::new(&root);
        let script_path = root.join("quests/man/man0l1.lua");
        let result = engine.call_quest_hook(
            &script_path,
            "onEmote",
            sample_snapshot(),
            sample_quest_handle(CommandQueue::new()),
            // [npc, eventName] — npc is unused by this hook so Nil stands in.
            vec![
                QuestHookArg::Nil,
                QuestHookArg::Str("emoteDefault1".to_string()),
            ],
        );
        assert!(result.error.is_none(), "hook errored: {:?}", result.error);
        assert!(
            result
                .commands
                .iter()
                .any(|c| matches!(c, LuaCommand::QuestStartSequence { sequence: 77, .. })),
            "onEmote didn't receive the eventName string; got {:?}",
            result.commands
        );

        let _ = std::fs::remove_dir_all(root);
    }

    fn sample_content_area(queue: Arc<Mutex<CommandQueue>>) -> userdata::LuaContentArea {
        userdata::LuaContentArea {
            parent_zone_id: 133,
            area_name: "man0g01".to_string(),
            area_class_path: "/Area/PrivateArea/Content/PrivateAreaMasterSimpleContent".to_string(),
            director_name: "Quest/QuestDirectorMan0g001".to_string(),
            director_actor_id: 0x6608_0001,
            queue,
            players: Vec::new(),
            allies: Vec::new(),
            monsters: Vec::new(),
        }
    }

    fn sample_director(queue: Arc<Mutex<CommandQueue>>) -> userdata::LuaDirectorHandle {
        userdata::LuaDirectorHandle {
            name: "Quest/QuestDirectorMan0g001".to_string(),
            actor_id: 0x6608_0001,
            class_path: "/Director/Quest/QuestDirectorMan0g001".to_string(),
            queue,
        }
    }

    /// End-to-end smoke test for the SEQ_005 binding chain. A
    /// synthetic content script's `onCreate` exercises the
    /// 3 user-bindings the man0g0 combat tutorial drives — `SetMod`
    /// (B3), `currentParty:AddMember` (B2), `positionX` write — and
    /// the test asserts every binding produces the expected
    /// `LuaCommand` variant in the drain. Replaces the Phase-A
    /// "no-op stubs" assertion now that B2/B3 are live.
    #[test]
    fn call_content_hook_runs_oncreate_and_drains_commands() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("content")).unwrap();
        std::fs::write(
            root.join("content/SimpleContent30010.lua"),
            r#"
            function onCreate(starterPlayer, contentArea, director)
                -- B-phase bindings exercised in turn:
                starterPlayer.positionX = 362.0       -- SetPos
                starterPlayer:SetMod(114, 1)          -- B3 SetActorMod (MinimumHpLock)
                local p = starterPlayer.currentParty
                if p ~= nil then
                    p:AddMember(0x12345678)            -- B2 PartyAddMember
                end
                director:AddMember(starterPlayer)      -- B4 DirectorAddMember (player)
                return true
            end
            "#,
        )
        .unwrap();

        let engine = LuaEngine::new(&root);
        let script_path = root.join("content/SimpleContent30010.lua");
        let dummy_queue = CommandQueue::new();
        let result = engine.call_content_hook(
            &script_path,
            "onCreate",
            sample_snapshot(),
            sample_content_area(dummy_queue.clone()),
            sample_director(dummy_queue),
        );
        assert!(
            result.error.is_none(),
            "onCreate errored: {:?}",
            result.error
        );
        // Position assignment funnels through SetPos.
        let saw_setpos = result
            .commands
            .iter()
            .any(|c| matches!(c, LuaCommand::SetPos { .. }));
        assert!(
            saw_setpos,
            "expected SetPos from positionX assignment; got {:?}",
            result.commands
        );
        // B3 — SetMod(114, 1) emits SetActorMod.
        let saw_setmod = result.commands.iter().any(|c| {
            matches!(
                c,
                LuaCommand::SetActorMod {
                    modifier_key: 114,
                    value: 1,
                    ..
                }
            )
        });
        assert!(
            saw_setmod,
            "expected SetActorMod{{modifier_key:114,value:1}}; got {:?}",
            result.commands
        );
        // B2 — currentParty:AddMember(0x12345678) emits PartyAddMember.
        let saw_party_add = result.commands.iter().any(|c| {
            matches!(
                c,
                LuaCommand::PartyAddMember {
                    member_actor_id: 0x1234_5678,
                    ..
                }
            )
        });
        assert!(
            saw_party_add,
            "expected PartyAddMember{{member:0x12345678}}; got {:?}",
            result.commands
        );
        // B4 — director:AddMember(starterPlayer) emits DirectorAddMember
        // with the LuaPlayer's actor_id extracted from the userdata.
        let saw_director_add = result
            .commands
            .iter()
            .any(|c| matches!(c, LuaCommand::DirectorAddMember { .. }));
        assert!(
            saw_director_add,
            "expected DirectorAddMember from director:AddMember(player); got {:?}",
            result.commands
        );
        let _ = std::fs::remove_dir_all(root);
    }

    /// B6: `call_content_on_update` runs the script's `onUpdate(tick,
    /// area)` body and drains any commands it emits. Uses a
    /// synthetic content script that calls `area:GetPlayers()` /
    /// `:GetMonsters()` / `:GetAllies()` — exercises the empty-
    /// iterator stubs added in this phase.
    #[test]
    fn call_content_on_update_runs_and_drains_commands() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("content")).unwrap();
        std::fs::write(
            root.join("content/SimpleContent30010.lua"),
            r#"
            function onUpdate(tick, area)
                -- Exercise the B6 binding stubs. Empty iterators /
                -- empty tables — the loops just terminate without
                -- aborting. The test verifies the body completes.
                local players = area:GetPlayers()
                local mobs = area:GetMonsters()
                local allies = area:GetAllies()
                for _ in players do
                    -- never reached
                end
                if mobs and #mobs > 0 then
                    -- never reached
                end
                if allies and #allies > 0 then
                    -- never reached
                end
                return tick
            end
            "#,
        )
        .unwrap();

        let engine = LuaEngine::new(&root);
        let script_path = root.join("content/SimpleContent30010.lua");
        let dummy_queue = CommandQueue::new();
        let result =
            engine.call_content_on_update(&script_path, 7, sample_content_area(dummy_queue));
        assert!(
            result.error.is_none(),
            "onUpdate errored: {:?}",
            result.error
        );
        // No commands emitted by the empty-iterator path — the test
        // success criterion is "didn't abort".
        assert!(result.commands.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn call_content_on_update_missing_hook_is_quiet() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("content")).unwrap();
        std::fs::write(
            root.join("content/SimpleContent30010.lua"),
            "function onCreate() end\n",
        )
        .unwrap();
        let engine = LuaEngine::new(&root);
        let script_path = root.join("content/SimpleContent30010.lua");
        let dummy_queue = CommandQueue::new();
        let result =
            engine.call_content_on_update(&script_path, 0, sample_content_area(dummy_queue));
        assert!(result.error.is_none());
        assert!(result.commands.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    /// Garlemald-Server #46 live test — drive the REAL main-menu
    /// command scripts (`LogoutCommand.lua`, `TeleportCommand.lua`)
    /// through `call_command_on_event_started`. Both must load without
    /// error and emit the `delegateCommand` confirm round-trip
    /// (`RunEventFunction` with function_name "delegateCommand"), which
    /// parks the coroutine on `_WAIT_EVENT` — the proof that the new
    /// dispatch arms (Exit / Teleport buttons) reach a working script.
    /// Before the fix these EventStarts fell through `command_script_name`
    /// and the buttons did nothing.
    #[test]
    fn real_main_menu_command_scripts_park_on_delegate() {
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/lua"));
        let engine = LuaEngine::new(root);
        for script in ["LogoutCommand", "TeleportCommand"] {
            let script_path = root.join(format!("commands/{script}.lua"));
            assert!(script_path.exists(), "{script} must be on disk");
            let result = engine.call_command_on_event_started(
                &script_path,
                sample_snapshot(),
                0xA0F0_5E9B, // command static actor (id is cosmetic here)
                "commandRequest".to_string(),
                0,
                Vec::new(),
                None,
            );
            assert!(
                result.error.is_none(),
                "{script} onEventStarted errored: {:?}",
                result.error,
            );
            assert!(
                result.commands.iter().any(|c| matches!(
                    c,
                    LuaCommand::RunEventFunction { function_name, .. }
                        if function_name == "delegateCommand"
                )),
                "{script} must fire a delegateCommand round-trip; got {:?}",
                result.commands,
            );
        }
    }

    /// Garlemald-Server #46 regression — a FREE emote (the emote menu /
    /// `/bow`) fires an EventStart against the EmoteStandardCommand static
    /// actor carrying `event_name="commandRequest"` + LuaParams
    /// `[emoteId, showText]`. The command script signature is
    /// `onEventStarted(player, actor, triggerName, emoteId, showText, …)` —
    /// NO eventType. Before the fix, `call_command_on_event_started` pushed a
    /// spurious `event_type` integer, so `emoteId` received the STRING
    /// "commandRequest" → `emoteTable[emoteId]` was nil → the script hit
    /// `string.format("%d", emoteId)` and CRASHED, and no emote ever played
    /// outside a scripted quest interaction. Assert the real numeric emoteId
    /// reaches the script: Bow (105) → `player:doEmote(0, animId 5, descId
    /// 21041)` → a `LuaCommand::DoEmote { emote_id: 5, message_id: 21041 }`.
    #[test]
    fn real_emote_standard_command_plays_emote() {
        use common::luaparam::LuaParam;
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/lua"));
        let engine = LuaEngine::new(root);
        let script_path = root.join("commands/EmoteStandardCommand.lua");
        assert!(script_path.exists(), "EmoteStandardCommand must be on disk");
        let result = engine.call_command_on_event_started(
            &script_path,
            sample_snapshot(),
            0xA0F0_5E26, // EmoteStandardCommand static actor
            "commandRequest".to_string(),
            0, // event_type — now ignored on the command path
            vec![LuaParam::Int32(105), LuaParam::Int32(1)], // emoteId=Bow, showText=1
            None,
        );
        assert!(
            result.error.is_none(),
            "EmoteStandardCommand onEventStarted errored (arg shift?): {:?}",
            result.error,
        );
        assert!(
            result.commands.iter().any(|c| matches!(
                c,
                LuaCommand::DoEmote { emote_id, message_id, .. }
                    if *emote_id == 5 && *message_id == 21041
            )),
            "Bow must produce DoEmote(animId 5, descId 21041); got {:?}",
            result.commands,
        );
    }

    /// #28 S0.4 — drive the REAL `SimpleContent30010.lua::onUpdate`
    /// with an engaged player + an unengaged ally and assert the
    /// `SetMod(modifiersGlobal.MovementSpeed, 8)` line reaches the
    /// command stream as `SetActorMod{modifier_key: 61}`. Pins two
    /// script bugs at once: the script previously read
    /// `modifiersGlobal.Speed` (nil — modifiers.lua only defines
    /// `MovementSpeed = 61`) so the SetMod silently no-opped, and it
    /// never `require`d "ally" so `allyGlobal.EngageTarget` raised on
    /// every tick (swallowed by the ticker's onUpdate drain).
    #[test]
    fn real_simple_content_30010_on_update_sets_movement_speed_mod() {
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/lua"));
        let script_path = root.join("content/SimpleContent30010.lua");
        assert!(script_path.exists());
        let engine = LuaEngine::new(root);

        let dummy_queue = CommandQueue::new();
        let mut area = sample_content_area(dummy_queue.clone());
        let mut player = sample_snapshot();
        player.is_engaged = true;
        player.target_actor_id = 0x4000_2099; // the wolf
        area.players.push(player);
        // Two allies, like production (Yda + Papalymo): the script's
        // `for i = 0, #allies - 1` loop over the 1-indexed roster table
        // skips the nil `allies[0]`, so the first REAL entry it
        // processes is `allies[1]` — the first ally pushed here.
        for (id, name) in [(0x4000_2001u32, "yda"), (0x4000_2002u32, "papalymo")] {
            area.allies.push(userdata::LuaActor {
                actor_id: id,
                name: name.to_string(),
                class_name: String::new(),
                class_path: String::new(),
                unique_id: String::new(),
                zone_id: 166,
                zone_name: String::new(),
                state: 2,
                pos: (365.266, 4.122, -700.73),
                rotation: 0.0,
                queue: dummy_queue.clone(),
                is_engaged: false,
                speed: 5.0,
                target_actor_id: 0,
                hp: 0,
                max_hp: 0,
            });
        }
        area.monsters.push(userdata::LuaActor {
            actor_id: 0x4000_2099,
            name: "bloodthirsty_wolf".to_string(),
            class_name: String::new(),
            class_path: String::new(),
            unique_id: String::new(),
            zone_id: 166,
            zone_name: String::new(),
            state: 2,
            pos: (374.427, 4.4, -698.711),
            rotation: 0.0,
            queue: dummy_queue,
            is_engaged: false,
            speed: 5.0,
            target_actor_id: 0,
            hp: 0,
            max_hp: 0,
        });

        let result = engine.call_content_on_update(&script_path, 1, area);
        assert!(
            result.error.is_none(),
            "real onUpdate errored: {:?}",
            result.error,
        );
        assert!(
            result.commands.iter().any(|c| matches!(
                c,
                LuaCommand::SetActorMod {
                    actor_id: 0x4000_2001,
                    modifier_key: 61, // Modifier::MovementSpeed
                    value: 8,
                }
            )),
            "ally SetMod(MovementSpeed, 8) must reach the command stream; got {:?}",
            result.commands,
        );
        // The engage pair rides the same drain (allyGlobal.EngageTarget).
        assert!(
            result.commands.iter().any(|c| matches!(
                c,
                LuaCommand::ActorEngage {
                    actor_id: 0x4000_2001,
                    ..
                }
            )),
            "ally engage must follow the speed mod; got {:?}",
            result.commands,
        );
    }

    /// Garlemald-Server #46 — drive the REAL
    /// `SimpleContentMan0l101.lua` escort through its lifecycle:
    /// onCreate spawns Sisipu (bnpc 25) + the 8 ankle biters (26-33,
    /// one per on-trail ambush point — seed/072) + the minimap
    /// duty-halo ring object (class 1290003 — seed/073), then onUpdate
    /// (1) emits the sheet-decoded duty-entry banner set (51005
    /// protect + 50011 bound + 25018 timer, worldMaster-owned) and
    /// Sisipu's send-off bark on her first live sighting, has her LEAD
    /// toward the first ROUTE waypoint while the player is close and
    /// the road is clear, and re-homes the halo ring onto her, (2)
    /// holds the walk + pulls mobs when an ankle biter is close, and
    /// (3) fires `sendSignal("escortComplete")` + the arrival bark
    /// once BOTH Sisipu and the player are inside the
    /// lighthouse-approach arrival radius. Also syntax-checks the
    /// script.
    #[test]
    fn real_man0l1_escort_content_walks_holds_and_signals() {
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/lua"));
        let script_path = root.join("content/SimpleContentMan0l101.lua");
        assert!(script_path.exists());
        let engine = LuaEngine::new(root);

        // onCreate: spawns + roster + locks. The escortState entry for
        // player 42 (sample_snapshot) arms the onUpdate machine in the
        // process-cached VM.
        let dummy_queue = CommandQueue::new();
        let result = engine.call_content_hook(
            &script_path,
            "onCreate",
            sample_snapshot(),
            sample_content_area(dummy_queue.clone()),
            sample_director(dummy_queue.clone()),
        );
        assert!(
            result.error.is_none(),
            "onCreate errored: {:?}",
            result.error
        );
        let spawn_ids: Vec<u32> = result
            .commands
            .iter()
            .filter_map(|c| match c {
                LuaCommand::SpawnBattleNpcById { bnpc_id, .. } => Some(*bnpc_id),
                _ => None,
            })
            .collect();
        assert_eq!(
            spawn_ids,
            (25..=33).collect::<Vec<u32>>(),
            "onCreate must spawn Sisipu (25) + 8 ankle biters (26-33, one per ambush point — seed/072)",
        );
        // The minimap duty-halo ring object (invisible
        // ContentPrivateAreaRange — seed/073) spawns at Sisipu's seed
        // position; its deterministic id is what onUpdate re-acquires
        // from the roster to MoveTo it alongside her.
        let ring_id = result
            .commands
            .iter()
            .find_map(|c| match c {
                LuaCommand::SpawnActor {
                    actor_class_id: 1_290_003,
                    unique_id,
                    expected_actor_id,
                    ..
                } if unique_id == "escortAreaRange" => Some(*expected_actor_id),
                _ => None,
            })
            .expect("onCreate must spawn the minimap duty-halo ring (class 1290003 — seed/073)");

        let escort_actor =
            |pos: (f32, f32, f32), queue: Arc<Mutex<CommandQueue>>| userdata::LuaActor {
                actor_id: 0x4008_0001,
                name: "sisipu".to_string(),
                class_name: String::new(),
                class_path: String::new(),
                unique_id: String::new(),
                zone_id: 128,
                zone_name: String::new(),
                state: 2,
                pos,
                rotation: 0.0,
                queue,
                is_engaged: false,
                speed: 5.0,
                target_actor_id: 0,
                hp: 0,
                max_hp: 0,
            };
        let mob_actor =
            |pos: (f32, f32, f32), queue: Arc<Mutex<CommandQueue>>| userdata::LuaActor {
                actor_id: 0x4008_0011,
                name: "ankle_biter".to_string(),
                class_name: String::new(),
                class_path: String::new(),
                unique_id: String::new(),
                zone_id: 128,
                zone_name: String::new(),
                state: 2,
                pos,
                rotation: 0.0,
                queue,
                is_engaged: false,
                speed: 5.0,
                target_actor_id: 0,
                hp: 0,
                max_hp: 0,
            };

        // Player standing right next to Sisipu (inside PLAYER_LEASH) —
        // she only advances while the player keeps up.
        let near_player = |pos: (f32, f32, f32)| {
            let mut snap = sample_snapshot();
            snap.pos = pos;
            snap
        };

        // Round 7f — Sisipu TRACES the player's recorded footsteps. From
        // her seed spawn (-49, 36.43, 162) the nearest TRAIL breadcrumb
        // is (-45.02, 36.92, 155.90) (the second recorded sample of the
        // full-walk trail), so with the player in leash range and the
        // road clear she must step toward it — x increasing above -49 —
        // at the BREADCRUMB'S recorded ground Y (36.92), never an
        // interpolated one.
        let q = CommandQueue::new();
        let mut area = sample_content_area(q.clone());
        area.players.push(near_player((-52.0, 36.0, 158.0)));
        area.allies
            .push(escort_actor((-49.0, 36.43, 162.0), q.clone()));
        area.monsters
            .push(mob_actor((124.43, 46.40, 1205.66), q.clone()));
        // The halo ring rides the same roster (director member of kind
        // Npc → monsters — ticker.rs build_content_rosters).
        let mut ring_actor = mob_actor((-49.0, 36.43, 162.0), q.clone());
        ring_actor.actor_id = ring_id;
        area.monsters.push(ring_actor);
        let result = engine.call_content_on_update(&script_path, 1, area);
        assert!(result.error.is_none(), "tick1: {:?}", result.error);
        let step = result.commands.iter().find_map(|c| match c {
            LuaCommand::MoveActorToPosition {
                actor_id: 0x4008_0001,
                x,
                y,
                ..
            } => Some((*x, *y)),
            _ => None,
        });
        assert!(
            step.is_some_and(|(x, y)| x > -49.0 && (y - 36.92).abs() < 0.01),
            "clear road + close player must step Sisipu toward the nearest trail breadcrumb at its recorded Y; got {:?}",
            result.commands,
        );
        // First live sighting of Sisipu → the duty-entry banner set
        // (sheet-decoded worldMaster ids — man0l1.lua TEXT_*): 51005
        // "Protect <displayName> from harm." carries her actor id as
        // the param, 50011 "You are now bound by duty.", 25018 "There
        // are <N> minutes remaining." carries the 30-minute limit.
        assert!(
            result.commands.iter().any(|c| matches!(
                c,
                LuaCommand::SendGameMessage {
                    text_owner_id: 0x5FF8_0001,
                    text_id: 51005,
                    params,
                    ..
                } if params.as_slice() == [crate::lua::command::GameMessageParam::Actor(0x4008_0001)]
            )),
            "duty start must announce 51005 Protect <Sisipu> with her actor id as param; got {:?}",
            result.commands,
        );
        assert!(
            result.commands.iter().any(|c| matches!(
                c,
                LuaCommand::SendGameMessage {
                    text_owner_id: 0x5FF8_0001,
                    text_id: 50011,
                    ..
                }
            )),
            "duty start must announce 50011 'You are now bound by duty.'; got {:?}",
            result.commands,
        );
        assert!(
            result.commands.iter().any(|c| matches!(
                c,
                LuaCommand::SendGameMessage {
                    text_owner_id: 0x5FF8_0001,
                    text_id: 25018,
                    params,
                    ..
                } if params.as_slice() == [crate::lua::command::GameMessageParam::Int(30)]
            )),
            "duty start must announce 25018 with the 30-minute param; got {:?}",
            result.commands,
        );
        // ...followed by Sisipu's send-off bark (man0l1 QUEST-sheet
        // row 105, quest actor 0xA0F1ADB2 as text owner).
        assert!(
            result.commands.iter().any(|c| matches!(
                c,
                LuaCommand::SendGameMessage {
                    text_owner_id: 0xA0F1_ADB2,
                    text_id: 105,
                    ..
                }
            )),
            "duty start must emit the say-105 send-off bark; got {:?}",
            result.commands,
        );
        // The halo ring is re-homed onto Sisipu's position this tick
        // (walking + throttle window open).
        assert!(
            result.commands.iter().any(|c| matches!(
                c,
                LuaCommand::MoveActorToPosition { actor_id, .. } if *actor_id == ring_id
            )),
            "the minimap halo ring must follow Sisipu; got {:?}",
            result.commands,
        );

        // Tick 2 — ambush at the third single ambush point (bnpc 28,
        // seed/072: 69.96, 41.22, 449.76): the lone live ankle biter
        // inside the engage radius freezes the walk and pulls onto the
        // escort party by itself.
        let q = CommandQueue::new();
        let mut area = sample_content_area(q.clone());
        area.players.push(near_player((66.0, 41.0, 446.0)));
        area.allies
            .push(escort_actor((68.0, 41.2, 448.0), q.clone()));
        area.monsters
            .push(mob_actor((69.96, 41.22, 449.76), q.clone()));
        let result = engine.call_content_on_update(&script_path, 2, area);
        assert!(result.error.is_none(), "tick2: {:?}", result.error);
        assert!(
            !result
                .commands
                .iter()
                .any(|c| matches!(c, LuaCommand::MoveActorToPosition { .. })),
            "ambush must hold the walk; got {:?}",
            result.commands,
        );
        assert!(
            result
                .commands
                .iter()
                .any(|c| matches!(c, LuaCommand::ActorEngage { .. })),
            "ambush must pull the mob/escort into combat; got {:?}",
            result.commands,
        );

        // Tick 3 — ARRIVAL: the PLAYER inside the lighthouse-approach
        // radius of ARRIVAL_GOAL (137.44, 60.33, 1322), Sisipu at their
        // side, the last ambush point (bnpc 33) far behind:
        // escortComplete must fire (goal arrival, NOT kill count — the
        // far mob is still alive).
        let q = CommandQueue::new();
        let mut area = sample_content_area(q.clone());
        area.players.push(near_player((135.0, 60.0, 1318.0)));
        area.allies
            .push(escort_actor((133.0, 60.2, 1320.0), q.clone()));
        area.monsters
            .push(mob_actor((124.43, 46.40, 1205.66), q.clone()));
        let result = engine.call_content_on_update(&script_path, 3, area);
        assert!(result.error.is_none(), "arrival tick: {:?}", result.error);
        assert!(
            result
                .commands
                .iter()
                .any(|c| matches!(c, LuaCommand::SendSignal { name } if name == "escortComplete")),
            "party inside the arrival radius must fire escortComplete; got {:?}",
            result.commands,
        );
        // Arrival bark: "We've finally arrived...and in one piece!"
        // (man0l1 QUEST-sheet row 290) rides the same drain.
        assert!(
            result.commands.iter().any(|c| matches!(
                c,
                LuaCommand::SendGameMessage {
                    text_owner_id: 0xA0F1_ADB2,
                    text_id: 290,
                    ..
                }
            )),
            "arrival must emit the say-290 bark; got {:?}",
            result.commands,
        );
    }

    /// Garlemald-Server #46 — the escort FAIL flow: Sisipu takes real
    /// damage now (no MinimumHpLock), and her death — she vanishes from
    /// the live-only ally roster — must roll the quest back to SEQ_048
    /// (re-arming the ZEPHYR_TRIGGER for a retry), tear the content
    /// down, eject the player to the gate, and drain the parked
    /// director coroutine.
    #[test]
    fn real_man0l1_escort_sisipu_death_fails_and_ejects() {
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/lua"));
        let script_path = root.join("content/SimpleContentMan0l101.lua");
        assert!(script_path.exists());
        let engine = LuaEngine::new(root);

        // onCreate arms the per-player escort state (and must NOT lock
        // Sisipu's HP any more — only the player keeps the tutorial
        // floor).
        let dummy_queue = CommandQueue::new();
        let result = engine.call_content_hook(
            &script_path,
            "onCreate",
            sample_snapshot(),
            sample_content_area(dummy_queue.clone()),
            sample_director(dummy_queue.clone()),
        );
        assert!(
            result.error.is_none(),
            "onCreate errored: {:?}",
            result.error
        );
        let hp_locks = result
            .commands
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    LuaCommand::SetActorMod {
                        modifier_key: 114, // Modifier::MinimumHpLock (modifiers.lua)
                        ..
                    }
                )
            })
            .count();
        assert_eq!(
            hp_locks, 1,
            "only the PLAYER keeps the MinimumHpLock floor — Sisipu takes real damage",
        );

        let escort_actor =
            |pos: (f32, f32, f32), queue: Arc<Mutex<CommandQueue>>| userdata::LuaActor {
                actor_id: 0x4008_0001,
                name: "sisipu".to_string(),
                class_name: String::new(),
                class_path: String::new(),
                unique_id: String::new(),
                zone_id: 128,
                zone_name: String::new(),
                state: 2,
                pos,
                rotation: 0.0,
                queue,
                is_engaged: false,
                speed: 5.0,
                target_actor_id: 0,
                hp: 0,
                max_hp: 0,
            };
        let near_player = |pos: (f32, f32, f32)| {
            let mut snap = sample_snapshot();
            snap.pos = pos;
            snap
        };

        // Tick 10 — Sisipu alive: latches the "seen" flag.
        let q = CommandQueue::new();
        let mut area = sample_content_area(q.clone());
        area.players.push(near_player((-52.0, 36.0, 158.0)));
        area.allies
            .push(escort_actor((-49.0, 36.43, 162.0), q.clone()));
        let result = engine.call_content_on_update(&script_path, 10, area);
        assert!(result.error.is_none(), "tick10: {:?}", result.error);

        // Tick 11 — Sisipu is gone from the live-only roster (dead):
        // the FAIL flow must fire.
        let q = CommandQueue::new();
        let mut area = sample_content_area(q.clone());
        area.players.push(near_player((-52.0, 36.0, 158.0)));
        let result = engine.call_content_on_update(&script_path, 11, area);
        assert!(result.error.is_none(), "tick11: {:?}", result.error);
        assert!(
            result.commands.iter().any(|c| matches!(
                c,
                LuaCommand::QuestStartSequence {
                    quest_id: 110_002,
                    sequence: 48,
                    ..
                }
            )),
            "fail must roll man0l1 back to SEQ_048; got {:?}",
            result.commands,
        );
        assert!(
            result
                .commands
                .iter()
                .any(|c| matches!(c, LuaCommand::ContentFinished { .. })),
            "fail must tear the content instance down; got {:?}",
            result.commands,
        );
        assert!(
            result
                .commands
                .iter()
                .any(|c| matches!(c, LuaCommand::WarpToPublicArea { .. })),
            "fail must eject the player to the public gate; got {:?}",
            result.commands,
        );
        assert!(
            result
                .commands
                .iter()
                .any(|c| matches!(c, LuaCommand::SendSignal { name } if name == "escortComplete")),
            "fail must drain the parked director coroutine; got {:?}",
            result.commands,
        );
    }

    /// Garlemald-Server #46 — the rewritten QuestDirectorMan0l101
    /// loads (syntax check incl. its `require("quests/man/man0l1")`)
    /// and `init()` returns the director class path.
    #[test]
    fn real_quest_director_man0l101_loads() {
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/lua"));
        let script_path = root.join("directors/Quest/QuestDirectorMan0l101.lua");
        assert!(script_path.exists());
        let engine = LuaEngine::new(root);
        let (lua, _q) = engine.load_script(&script_path).expect("director loads");
        let init: Function = lua.globals().get("init").expect("init defined");
        let path: String = init.call(()).expect("init runs");
        assert_eq!(path, "/Director/Quest/QuestDirectorMan0l101");
    }

    /// B7: `call_content_hook(.., "onZoneIn", ..)` runs a script's
    /// `onZoneIn(player, contentArea, director)` hook and drains
    /// commands. Mirrors the
    /// `apply_do_zone_change_content` post-warp callback.
    #[test]
    fn call_content_hook_runs_onzonein_and_drains_commands() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("content")).unwrap();
        std::fs::write(
            root.join("content/SimpleContent30010.lua"),
            r#"
            function onZoneIn(player, contentArea, director)
                -- Just exercise that the hook ran by mutating the
                -- player's position; the SetPos command in the
                -- drain proves it.
                player.positionX = 1000.0
                return true
            end
            "#,
        )
        .unwrap();
        let engine = LuaEngine::new(&root);
        let script_path = root.join("content/SimpleContent30010.lua");
        let dummy_queue = CommandQueue::new();
        let result = engine.call_content_hook(
            &script_path,
            "onZoneIn",
            sample_snapshot(),
            sample_content_area(dummy_queue.clone()),
            sample_director(dummy_queue),
        );
        assert!(
            result.error.is_none(),
            "onZoneIn errored: {:?}",
            result.error
        );
        let saw_setpos = result
            .commands
            .iter()
            .any(|c| matches!(c, LuaCommand::SetPos { .. }));
        assert!(
            saw_setpos,
            "expected SetPos from onZoneIn body; got {:?}",
            result.commands
        );
        let _ = std::fs::remove_dir_all(root);
    }

    /// Phase C2 — guard against the duplicate-`IsEngaged`-stub
    /// regression. Before this fix, `LuaPlayer` had two
    /// `add_method("IsEngaged", …)` calls; mlua is last-write-wins
    /// per `feedback_meteor_decomp_authoritative_for_engine_bindings.md`,
    /// so the later `(|_, _, _| Ok(false))` stub silently overrode
    /// the real implementation that reads `snapshot.is_engaged`. The
    /// player was *always* reported as not-engaged regardless of
    /// actual combat state, breaking the SEQ_005 ally-engagement
    /// gating in `SimpleContent30010.lua::onUpdate`.
    #[test]
    fn c2_lua_player_is_engaged_reads_snapshot_not_stub() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("content")).unwrap();
        std::fs::write(
            root.join("content/Probe.lua"),
            r#"
            function probe(player)
                if player:IsEngaged() then
                    player.positionX = 9999.0
                else
                    player.positionX = -1.0
                end
                return true
            end
            "#,
        )
        .unwrap();
        let engine = LuaEngine::new(&root);
        let script_path = root.join("content/Probe.lua");
        let dummy_queue = CommandQueue::new();
        let mut snap = sample_snapshot();
        snap.is_engaged = true;
        snap.speed = 8.0;
        snap.target_actor_id = 0x1234_5678;
        let result = engine.call_content_hook(
            &script_path,
            "probe",
            snap,
            sample_content_area(dummy_queue.clone()),
            sample_director(dummy_queue),
        );
        assert!(result.error.is_none(), "probe errored: {:?}", result.error);
        // The 9999.0 SetPos proves the IsEngaged branch ran true —
        // i.e. snapshot.is_engaged actually reached the binding.
        let saw_engaged_setpos = result
            .commands
            .iter()
            .any(|c| matches!(c, LuaCommand::SetPos { x, .. } if (*x - 9999.0).abs() < 0.01));
        assert!(
            saw_engaged_setpos,
            "player:IsEngaged() should reflect snapshot.is_engaged=true; got commands: {:?}",
            result.commands,
        );
    }

    /// Phase C2 — `player:GetSpeed()` reads from `snapshot.speed`
    /// (not the previous stubbed `0`). Combined with `IsEngaged`
    /// this is the snapshot-plumbing-correct guard.
    #[test]
    fn c2_lua_player_get_speed_reads_snapshot() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("content")).unwrap();
        std::fs::write(
            root.join("content/Speed.lua"),
            r#"
            function probe(player)
                player.positionX = player:GetSpeed() * 1.0
                return true
            end
            "#,
        )
        .unwrap();
        let engine = LuaEngine::new(&root);
        let script_path = root.join("content/Speed.lua");
        let dummy_queue = CommandQueue::new();
        let mut snap = sample_snapshot();
        snap.speed = 8.0;
        let result = engine.call_content_hook(
            &script_path,
            "probe",
            snap,
            sample_content_area(dummy_queue.clone()),
            sample_director(dummy_queue),
        );
        assert!(result.error.is_none(), "probe errored: {:?}", result.error);
        let saw_8 = result
            .commands
            .iter()
            .any(|c| matches!(c, LuaCommand::SetPos { x, .. } if (*x - 8.0).abs() < 0.01));
        assert!(
            saw_8,
            "GetSpeed should return 8 from snapshot.speed=8.0; got: {:?}",
            result.commands
        );
    }

    /// Phase C2 — `player.target` returns a userdata with the right
    /// `.actorId` when `snapshot.target_actor_id` is set. Nil when
    /// it's zero.
    #[test]
    fn c2_lua_player_target_returns_userdata_or_nil() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("content")).unwrap();
        std::fs::write(
            root.join("content/Target.lua"),
            r#"
            -- f32 can't represent every u32 actor id losslessly, so
            -- compare in Lua (i64-backed) and signal pass/fail
            -- through a small magnitude that survives f32 rounding.
            function probe(player)
                if player.target then
                    if player.target.actorId == 0x40001234 then
                        player.positionX = 7.0   -- target with right id
                    else
                        player.positionX = 8.0   -- target with wrong id (bug)
                    end
                else
                    player.positionX = -1.0      -- nil target
                end
                return true
            end
            "#,
        )
        .unwrap();
        let engine = LuaEngine::new(&root);
        let script_path = root.join("content/Target.lua");

        // With a target — SetPos x carries the target id.
        {
            let dummy_queue = CommandQueue::new();
            let mut snap = sample_snapshot();
            snap.target_actor_id = 0x4000_1234;
            let result = engine.call_content_hook(
                &script_path,
                "probe",
                snap,
                sample_content_area(dummy_queue.clone()),
                sample_director(dummy_queue),
            );
            assert!(result.error.is_none());
            let saw_match = result
                .commands
                .iter()
                .any(|c| matches!(c, LuaCommand::SetPos { x, .. } if (*x - 7.0).abs() < 0.01));
            assert!(
                saw_match,
                "player.target.actorId should match 0x40001234; got: {:?}",
                result.commands,
            );
        }

        // Without a target — branch hits the -1.0 fallback.
        {
            let dummy_queue = CommandQueue::new();
            let snap = sample_snapshot(); // target_actor_id = 0
            let result = engine.call_content_hook(
                &script_path,
                "probe",
                snap,
                sample_content_area(dummy_queue.clone()),
                sample_director(dummy_queue),
            );
            assert!(result.error.is_none());
            let saw_neg1 = result
                .commands
                .iter()
                .any(|c| matches!(c, LuaCommand::SetPos { x, .. } if (*x + 1.0).abs() < 0.01));
            assert!(
                saw_neg1,
                "player.target should be nil when target_actor_id=0; got: {:?}",
                result.commands,
            );
        }
    }

    /// Phase C2b — `area:GetAllies()` returns an indexed Lua table
    /// of LuaActor userdata when the LuaContentArea was built with
    /// real allies. `#allies` reports the right count; `allies[i]`
    /// has the right `IsEngaged()` / `actorId` for the i-th ally.
    #[test]
    fn c2b_content_area_get_allies_yields_real_actors() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("content")).unwrap();
        std::fs::write(
            root.join("content/Allies.lua"),
            r#"
            function probe(_player, area, _director)
                local allies = area:GetAllies()
                local count = #allies
                if count == 2
                   and allies[1]:IsEngaged() == true
                   and allies[2]:IsEngaged() == false
                then
                    _player.positionX = 42.0  -- pass signal
                else
                    _player.positionX = -1.0  -- fail signal
                end
                return true
            end
            "#,
        )
        .unwrap();
        let engine = LuaEngine::new(&root);
        let script_path = root.join("content/Allies.lua");
        let dummy_queue = CommandQueue::new();

        // Build a content area with 2 allies — one engaged, one not.
        let mut area = sample_content_area(dummy_queue.clone());
        area.allies.push(userdata::LuaActor {
            actor_id: 0x4000_0001,
            name: "yda".to_string(),
            class_name: String::new(),
            class_path: String::new(),
            unique_id: String::new(),
            zone_id: 166,
            zone_name: String::new(),
            state: 0,
            pos: (0.0, 0.0, 0.0),
            rotation: 0.0,
            queue: dummy_queue.clone(),
            is_engaged: true,
            speed: 8.0,
            target_actor_id: 0,
            hp: 0,
            max_hp: 0,
        });
        area.allies.push(userdata::LuaActor {
            actor_id: 0x4000_0002,
            name: "papalymo".to_string(),
            class_name: String::new(),
            class_path: String::new(),
            unique_id: String::new(),
            zone_id: 166,
            zone_name: String::new(),
            state: 0,
            pos: (0.0, 0.0, 0.0),
            rotation: 0.0,
            queue: dummy_queue.clone(),
            is_engaged: false,
            speed: 5.0,
            target_actor_id: 0,
            hp: 0,
            max_hp: 0,
        });

        let result = engine.call_content_hook(
            &script_path,
            "probe",
            sample_snapshot(),
            area,
            sample_director(dummy_queue),
        );
        assert!(result.error.is_none(), "probe errored: {:?}", result.error);
        let saw_pass = result
            .commands
            .iter()
            .any(|c| matches!(c, LuaCommand::SetPos { x, .. } if (*x - 42.0).abs() < 0.01));
        assert!(
            saw_pass,
            "Expected GetAllies() to return 2 LuaActors with right IsEngaged; got: {:?}",
            result.commands,
        );
    }

    /// Phase C2b — `area:GetPlayers()` returns a stateful iterator
    /// the script consumes via `for player in players do`. Each
    /// yield is a LuaPlayer userdata.
    #[test]
    fn c2b_content_area_get_players_yields_iterator() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("content")).unwrap();
        std::fs::write(
            root.join("content/Players.lua"),
            r#"
            function probe(_player, area, _director)
                local players = area:GetPlayers()
                local count = 0
                local any_engaged = false
                for p in players do
                    count = count + 1
                    if p:IsEngaged() then
                        any_engaged = true
                    end
                end
                if count == 2 and any_engaged then
                    _player.positionX = 77.0  -- pass
                else
                    _player.positionX = -1.0  -- fail
                end
                return true
            end
            "#,
        )
        .unwrap();
        let engine = LuaEngine::new(&root);
        let script_path = root.join("content/Players.lua");
        let dummy_queue = CommandQueue::new();

        let mut area = sample_content_area(dummy_queue.clone());
        let p1 = userdata::PlayerSnapshot {
            actor_id: 0x0001,
            is_engaged: true,
            ..Default::default()
        };
        let p2 = userdata::PlayerSnapshot {
            actor_id: 0x0002,
            is_engaged: false,
            ..Default::default()
        };
        area.players.push(p1);
        area.players.push(p2);

        let result = engine.call_content_hook(
            &script_path,
            "probe",
            sample_snapshot(),
            area,
            sample_director(dummy_queue),
        );
        assert!(result.error.is_none(), "probe errored: {:?}", result.error);
        let saw_pass = result
            .commands
            .iter()
            .any(|c| matches!(c, LuaCommand::SetPos { x, .. } if (*x - 77.0).abs() < 0.01));
        assert!(
            saw_pass,
            "Expected GetPlayers() iterator to yield 2 players (one engaged); got: {:?}",
            result.commands,
        );
    }

    /// Phase C3 — the `ally.lua::allyGlobal.EngageTarget` body calls
    /// `ally.Engage(target)` (dot syntax, not colon) which mlua's
    /// `add_method` doesn't natively support. The fix: an Index
    /// meta-method handler that returns a closure-bound function.
    /// This test verifies the dot-syntax call lands the expected
    /// `LuaCommand::ActorEngage` with the right ids.
    #[test]
    fn c3_lua_actor_dot_engage_pushes_command() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("content")).unwrap();
        std::fs::write(
            root.join("content/Engage.lua"),
            r#"
            function probe(_player, area, _director)
                local allies = area:GetAllies()
                local monsters = area:GetMonsters()
                if #allies > 0 and #monsters > 0 then
                    allies[1].Engage(monsters[1])
                end
                return true
            end
            "#,
        )
        .unwrap();
        let engine = LuaEngine::new(&root);
        let script_path = root.join("content/Engage.lua");
        let dummy_queue = CommandQueue::new();

        let mut area = sample_content_area(dummy_queue.clone());
        area.allies.push(userdata::LuaActor {
            actor_id: 0x4000_0001,
            name: "yda".to_string(),
            class_name: String::new(),
            class_path: String::new(),
            unique_id: String::new(),
            zone_id: 166,
            zone_name: String::new(),
            state: 0,
            pos: (0.0, 0.0, 0.0),
            rotation: 0.0,
            queue: dummy_queue.clone(),
            is_engaged: false,
            speed: 5.0,
            target_actor_id: 0,
            hp: 0,
            max_hp: 0,
        });
        area.monsters.push(userdata::LuaActor {
            actor_id: 0x4000_0099,
            name: "wolf".to_string(),
            class_name: String::new(),
            class_path: String::new(),
            unique_id: String::new(),
            zone_id: 166,
            zone_name: String::new(),
            state: 0,
            pos: (0.0, 0.0, 0.0),
            rotation: 0.0,
            queue: dummy_queue.clone(),
            is_engaged: false,
            speed: 5.0,
            target_actor_id: 0,
            hp: 0,
            max_hp: 0,
        });

        let result = engine.call_content_hook(
            &script_path,
            "probe",
            sample_snapshot(),
            area,
            sample_director(dummy_queue),
        );
        assert!(result.error.is_none(), "probe errored: {:?}", result.error);
        let saw_engage = result.commands.iter().any(|c| {
            matches!(
                c,
                LuaCommand::ActorEngage {
                    actor_id: 0x4000_0001,
                    target_actor_id: 0x4000_0099,
                }
            )
        });
        assert!(
            saw_engage,
            "Expected ActorEngage{{actor:0x40000001, target:0x40000099}}; got: {:?}",
            result.commands,
        );
    }

    /// Phase C3 — `ally.hateContainer.AddBaseHate(target)` is a
    /// double-dot dispatch: `hateContainer` returns a
    /// LuaHateContainer userdata via Index, then `AddBaseHate(target)`
    /// is a dot-syntax call on that userdata. Both legs need
    /// closure-bound Index handlers to work without mlua's implicit
    /// self.
    #[test]
    fn c3_lua_actor_hate_container_dot_add_base_hate_pushes_command() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("content")).unwrap();
        std::fs::write(
            root.join("content/Hate.lua"),
            r#"
            function probe(_player, area, _director)
                local allies = area:GetAllies()
                local monsters = area:GetMonsters()
                if #allies > 0 and #monsters > 0 then
                    allies[1].hateContainer.AddBaseHate(monsters[1])
                end
                return true
            end
            "#,
        )
        .unwrap();
        let engine = LuaEngine::new(&root);
        let script_path = root.join("content/Hate.lua");
        let dummy_queue = CommandQueue::new();

        let mut area = sample_content_area(dummy_queue.clone());
        area.allies.push(userdata::LuaActor {
            actor_id: 0x4000_0001,
            name: "yda".to_string(),
            class_name: String::new(),
            class_path: String::new(),
            unique_id: String::new(),
            zone_id: 166,
            zone_name: String::new(),
            state: 0,
            pos: (0.0, 0.0, 0.0),
            rotation: 0.0,
            queue: dummy_queue.clone(),
            is_engaged: false,
            speed: 5.0,
            target_actor_id: 0,
            hp: 0,
            max_hp: 0,
        });
        area.monsters.push(userdata::LuaActor {
            actor_id: 0x4000_0099,
            name: "wolf".to_string(),
            class_name: String::new(),
            class_path: String::new(),
            unique_id: String::new(),
            zone_id: 166,
            zone_name: String::new(),
            state: 0,
            pos: (0.0, 0.0, 0.0),
            rotation: 0.0,
            queue: dummy_queue.clone(),
            is_engaged: false,
            speed: 5.0,
            target_actor_id: 0,
            hp: 0,
            max_hp: 0,
        });

        let result = engine.call_content_hook(
            &script_path,
            "probe",
            sample_snapshot(),
            area,
            sample_director(dummy_queue),
        );
        assert!(result.error.is_none(), "probe errored: {:?}", result.error);
        let saw_hate = result.commands.iter().any(|c| {
            matches!(
                c,
                LuaCommand::HateContainerAddBaseHate {
                    actor_id: 0x4000_0001,
                    target_actor_id: 0x4000_0099,
                }
            )
        });
        assert!(
            saw_hate,
            "Expected HateContainerAddBaseHate{{actor:0x40000001, target:0x40000099}}; got: {:?}",
            result.commands,
        );
    }

    /// Phase C2b — empty roster vectors keep the existing
    /// "iterator-yields-nothing" / "#table == 0" behavior so onUpdate
    /// scripts skip their loop bodies when no content is active.
    #[test]
    fn c2b_content_area_empty_rosters_skip_loop() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("content")).unwrap();
        std::fs::write(
            root.join("content/Empty.lua"),
            r#"
            function probe(_player, area, _director)
                local allies = area:GetAllies()
                local monsters = area:GetMonsters()
                local players = area:GetPlayers()
                local player_count = 0
                for _ in players do
                    player_count = player_count + 1
                end
                if #allies == 0 and #monsters == 0 and player_count == 0 then
                    _player.positionX = 1.0  -- pass
                else
                    _player.positionX = -1.0  -- fail
                end
                return true
            end
            "#,
        )
        .unwrap();
        let engine = LuaEngine::new(&root);
        let script_path = root.join("content/Empty.lua");
        let dummy_queue = CommandQueue::new();
        let area = sample_content_area(dummy_queue.clone());

        let result = engine.call_content_hook(
            &script_path,
            "probe",
            sample_snapshot(),
            area,
            sample_director(dummy_queue),
        );
        assert!(result.error.is_none(), "probe errored: {:?}", result.error);
        let saw_pass = result
            .commands
            .iter()
            .any(|c| matches!(c, LuaCommand::SetPos { x, .. } if (*x - 1.0).abs() < 0.01));
        assert!(
            saw_pass,
            "Empty rosters should keep #t==0; got: {:?}",
            result.commands
        );
    }

    #[test]
    fn call_content_hook_missing_oncreate_is_quiet() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("content")).unwrap();
        std::fs::write(
            root.join("content/SimpleContent30010.lua"),
            "function onUpdate() end\n",
        )
        .unwrap();

        let engine = LuaEngine::new(&root);
        let script_path = root.join("content/SimpleContent30010.lua");
        let dummy_queue = CommandQueue::new();
        let result = engine.call_content_hook(
            &script_path,
            "onCreate",
            sample_snapshot(),
            sample_content_area(dummy_queue.clone()),
            sample_director(dummy_queue),
        );
        assert!(
            result.error.is_none(),
            "missing onCreate should be a quiet no-op; got {:?}",
            result.error,
        );
        assert!(result.commands.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    /// `call_director_on_event_started` should run the director's
    /// `onEventStarted(player, director, eventType, eventName, ...)` hook
    /// in a coroutine, drain commands the hook emits up to the first
    /// yield, and park the coroutine on `_WAIT_EVENT` when it yields via
    /// `callClientFunction`. Mirrors pmeteor `LuaEngine.cs::EventStarted`
    /// → `Director.OnEventStart` end-to-end.
    #[test]
    fn call_director_on_event_started_runs_until_first_callclientfunction_yield() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("directors/Quest")).unwrap();
        // Stand-in for QuestDirectorMan0g001.lua's onEventStarted body —
        // emits one SendMessage, then a callClientFunction (RunEventFunction
        // + yield on _WAIT_EVENT). The test asserts both commands surface
        // in the returned PartialLuaCallResult.
        std::fs::write(
            root.join("global.lua"),
            r#"
                function callClientFunction(player, fn_name, ...)
                    player:RunEventFunction(fn_name, ...)
                    return coroutine.yield("_WAIT_EVENT", player)
                end
            "#,
        )
        .unwrap();
        std::fs::write(
            root.join("directors/Quest/QuestDirectorTestEv.lua"),
            r#"
                require ("global")
                function onEventStarted(player, director, eventType, eventName)
                    player:SendMessage(0x20, "", "Starting")
                    callClientFunction(player, "delegateEvent", player, "processTtrBtl001")
                    -- never reached on the first slice
                    player:EndEvent()
                end
            "#,
        )
        .unwrap();

        let engine = LuaEngine::new(&root);
        let script_path = root.join("directors/Quest/QuestDirectorTestEv.lua");
        let dummy_queue = CommandQueue::new();
        let result = engine.call_director_on_event_started(
            &script_path,
            sample_snapshot(),
            sample_director(dummy_queue),
            "noticeEvent".to_string(),
            5,
            vec![],
        );

        assert!(
            result.error.is_none(),
            "onEventStarted errored: {:?}",
            result.error,
        );
        let has_send_msg = result
            .commands
            .iter()
            .any(|c| matches!(c, LuaCommand::SendMessage { .. }));
        let has_run_event = result
            .commands
            .iter()
            .any(|c| matches!(c, LuaCommand::RunEventFunction { .. }));
        assert!(
            has_send_msg,
            "missing SendGameMessage; got {:?}",
            result.commands,
        );
        assert!(
            has_run_event,
            "missing RunEventFunction (callClientFunction emit); got {:?}",
            result.commands,
        );
        // EndEvent shouldn't fire — the coroutine yielded on _WAIT_EVENT
        // before reaching it.
        assert!(
            !result
                .commands
                .iter()
                .any(|c| matches!(c, LuaCommand::EndEvent { .. })),
            "EndEvent should be deferred until coroutine resumes; got {:?}",
            result.commands,
        );
        let _ = std::fs::remove_dir_all(root);
    }

    /// Missing `onEventStarted` global is a quiet no-op — many directors
    /// only define `init` / `main`.
    #[test]
    fn call_director_on_event_started_missing_hook_is_a_quiet_no_op() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("directors/Quest")).unwrap();
        std::fs::write(
            root.join("directors/Quest/QuestDirectorTestNoHook.lua"),
            "function init() return \"/Director/Quest/QuestDirectorTestNoHook\" end",
        )
        .unwrap();

        let engine = LuaEngine::new(&root);
        let script_path = root.join("directors/Quest/QuestDirectorTestNoHook.lua");
        let result = engine.call_director_on_event_started(
            &script_path,
            sample_snapshot(),
            sample_director(CommandQueue::new()),
            "noticeEvent".to_string(),
            5,
            vec![],
        );
        assert!(
            result.error.is_none(),
            "missing onEventStarted should be quiet: {:?}",
            result.error,
        );
        assert!(result.commands.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    /// Cinematic NPC choreography — `actor:MoveTo(x, y, z, rot, moveState)`
    /// pushes a `LuaCommand::MoveActorToPosition` carrying the actor id +
    /// target coords. The dispatcher then updates position + broadcasts
    /// 0x00CF MoveActorToPositionPacket. Verified end-to-end via
    /// integration test below; this just asserts the binding shape.
    #[test]
    fn lua_actor_move_to_pushes_move_actor_to_position() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("content")).unwrap();
        std::fs::write(
            root.join("content/Cinematic.lua"),
            r#"
            function probe(_player, area, _director)
                local allies = area:GetAllies()
                if #allies > 0 then
                    -- Move with explicit moveState (1 = run)
                    allies[1]:MoveTo(365.5, 4.1, -700.0, 1.5, 1)
                    -- And one with the default moveState (0 = walk).
                    allies[1]:MoveTo(366.0, 4.1, -699.0, 0.0)
                end
                return true
            end
            "#,
        )
        .unwrap();
        let engine = LuaEngine::new(&root);
        let script_path = root.join("content/Cinematic.lua");
        let dummy_queue = CommandQueue::new();
        let mut area = sample_content_area(dummy_queue.clone());
        area.allies.push(userdata::LuaActor {
            actor_id: 0x4000_2001,
            name: "yda".to_string(),
            class_name: String::new(),
            class_path: String::new(),
            unique_id: String::new(),
            zone_id: 166,
            zone_name: String::new(),
            state: 0,
            pos: (0.0, 0.0, 0.0),
            rotation: 0.0,
            queue: dummy_queue.clone(),
            is_engaged: false,
            speed: 5.0,
            target_actor_id: 0,
            hp: 0,
            max_hp: 0,
        });
        let result = engine.call_content_hook(
            &script_path,
            "probe",
            sample_snapshot(),
            area,
            sample_director(dummy_queue),
        );
        assert!(result.error.is_none(), "probe errored: {:?}", result.error);
        let moves: Vec<_> = result
            .commands
            .iter()
            .filter_map(|c| match c {
                LuaCommand::MoveActorToPosition {
                    actor_id,
                    x,
                    y,
                    z,
                    rotation,
                    move_state,
                } => Some((*actor_id, *x, *y, *z, *rotation, *move_state)),
                _ => None,
            })
            .collect();
        assert_eq!(
            moves.len(),
            2,
            "expected 2 MoveActorToPosition cmds; got {:?}",
            result.commands
        );
        assert_eq!(moves[0], (0x4000_2001, 365.5, 4.1, -700.0, 1.5, 1));
        // Second call omitted move_state — should default to 0.
        assert_eq!(moves[1], (0x4000_2001, 366.0, 4.1, -699.0, 0.0, 0));
    }

    /// `actor:LookAt(target)` — accepts both raw u32 actor ids and
    /// LuaActor / LuaPlayer userdata. Pushes
    /// `LuaCommand::SetActorTargetAnimated`.
    #[test]
    fn lua_actor_look_at_pushes_set_actor_target_animated() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("content")).unwrap();
        std::fs::write(
            root.join("content/LookAt.lua"),
            r#"
            function probe(_player, area, _director)
                local allies = area:GetAllies()
                local monsters = area:GetMonsters()
                if #allies > 0 and #monsters > 0 then
                    -- LookAt with a userdata target (pmeteor canonical
                    -- form: the cinematic passes the other actor object).
                    allies[1]:LookAt(monsters[1])
                    -- LookAt with a raw integer id (rare; some scripts
                    -- pass `target.id` instead of the object).
                    allies[1]:LookAt(0x40003333)
                end
                return true
            end
            "#,
        )
        .unwrap();
        let engine = LuaEngine::new(&root);
        let script_path = root.join("content/LookAt.lua");
        let dummy_queue = CommandQueue::new();
        let mut area = sample_content_area(dummy_queue.clone());
        area.allies.push(userdata::LuaActor {
            actor_id: 0x4000_2001,
            name: "yda".to_string(),
            class_name: String::new(),
            class_path: String::new(),
            unique_id: String::new(),
            zone_id: 166,
            zone_name: String::new(),
            state: 0,
            pos: (0.0, 0.0, 0.0),
            rotation: 0.0,
            queue: dummy_queue.clone(),
            is_engaged: false,
            speed: 5.0,
            target_actor_id: 0,
            hp: 0,
            max_hp: 0,
        });
        area.monsters.push(userdata::LuaActor {
            actor_id: 0x4000_2099,
            name: "wolf".to_string(),
            class_name: String::new(),
            class_path: String::new(),
            unique_id: String::new(),
            zone_id: 166,
            zone_name: String::new(),
            state: 0,
            pos: (0.0, 0.0, 0.0),
            rotation: 0.0,
            queue: dummy_queue.clone(),
            is_engaged: false,
            speed: 5.0,
            target_actor_id: 0,
            hp: 0,
            max_hp: 0,
        });
        let result = engine.call_content_hook(
            &script_path,
            "probe",
            sample_snapshot(),
            area,
            sample_director(dummy_queue),
        );
        assert!(result.error.is_none(), "probe errored: {:?}", result.error);
        let look_ats: Vec<_> = result
            .commands
            .iter()
            .filter_map(|c| match c {
                LuaCommand::SetActorTargetAnimated {
                    source_actor_id,
                    target_actor_id,
                } => Some((*source_actor_id, *target_actor_id)),
                _ => None,
            })
            .collect();
        assert_eq!(
            look_ats.len(),
            2,
            "expected 2 LookAt cmds; got {:?}",
            result.commands
        );
        assert_eq!(look_ats[0], (0x4000_2001, 0x4000_2099));
        assert_eq!(look_ats[1], (0x4000_2001, 0x4000_3333));
    }

    /// LuaPlayer mirrors of MoveTo / LookAt — same wire effect (0x00CF /
    /// 0x00D3), routed through the player's snapshot.actor_id rather
    /// than a sibling actor's id. Verified via the `_player` arg
    /// passed into the content-hook probe (which is the LuaPlayer
    /// userdata).
    #[test]
    fn lua_player_move_to_and_look_at_mirror_actor_bindings() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("content")).unwrap();
        std::fs::write(
            root.join("content/PlayerCinematic.lua"),
            r#"
            function probe(player, area, _director)
                local monsters = area:GetMonsters()
                player:MoveTo(100.0, 0.0, 200.0, 1.5, 1)
                player:MoveTo(101.0, 0.0, 201.0, 0.0)  -- default moveState
                if #monsters > 0 then
                    player:LookAt(monsters[1])
                end
                player:LookAt(0x40004444)
                return true
            end
            "#,
        )
        .unwrap();
        let engine = LuaEngine::new(&root);
        let script_path = root.join("content/PlayerCinematic.lua");
        let dummy_queue = CommandQueue::new();
        let mut area = sample_content_area(dummy_queue.clone());
        area.monsters.push(userdata::LuaActor {
            actor_id: 0x4000_5099,
            name: "wolf".to_string(),
            class_name: String::new(),
            class_path: String::new(),
            unique_id: String::new(),
            zone_id: 166,
            zone_name: String::new(),
            state: 0,
            pos: (0.0, 0.0, 0.0),
            rotation: 0.0,
            queue: dummy_queue.clone(),
            is_engaged: false,
            speed: 5.0,
            target_actor_id: 0,
            hp: 0,
            max_hp: 0,
        });
        let snap = sample_snapshot();
        let player_actor_id = snap.actor_id;
        let result = engine.call_content_hook(
            &script_path,
            "probe",
            snap,
            area,
            sample_director(dummy_queue),
        );
        assert!(result.error.is_none(), "probe errored: {:?}", result.error);
        let moves: Vec<_> = result
            .commands
            .iter()
            .filter_map(|c| match c {
                LuaCommand::MoveActorToPosition {
                    actor_id,
                    x,
                    y,
                    z,
                    rotation,
                    move_state,
                } => Some((*actor_id, *x, *y, *z, *rotation, *move_state)),
                _ => None,
            })
            .collect();
        assert_eq!(
            moves.len(),
            2,
            "expected 2 MoveActorToPosition cmds; got {:?}",
            result.commands
        );
        assert_eq!(moves[0], (player_actor_id, 100.0, 0.0, 200.0, 1.5, 1));
        assert_eq!(moves[1], (player_actor_id, 101.0, 0.0, 201.0, 0.0, 0));
        let look_ats: Vec<_> = result
            .commands
            .iter()
            .filter_map(|c| match c {
                LuaCommand::SetActorTargetAnimated {
                    source_actor_id,
                    target_actor_id,
                } => Some((*source_actor_id, *target_actor_id)),
                _ => None,
            })
            .collect();
        assert_eq!(
            look_ats.len(),
            2,
            "expected 2 LookAt cmds; got {:?}",
            result.commands
        );
        assert_eq!(look_ats[0], (player_actor_id, 0x4000_5099));
        assert_eq!(look_ats[1], (player_actor_id, 0x4000_4444));
    }

    /// End-to-end repro of the SEQ_005 F-press chain using the REAL
    /// `global.lua` yield shapes — BARE pairs, not tables. The director
    /// parks on `waitForSignal("playerActive")`, the F press's
    /// `fire_signal_and_drain` resumes it into `wait(0.05)` (which must
    /// RE-PARK on time, not be dropped), and the ticker's `tick()` then
    /// drains the post-wait continuation as a batch owned by the
    /// trigger player. Before the MultiValue capture fix, the bare
    /// `("_WAIT_SIGNAL", name)` yield lost its name and the coroutine
    /// was dropped at re-park (the live softlock); the older
    /// integration test missed it because its synthetic `wait()`
    /// yielded the TABLE form. (Garlemald-Server #28.)
    #[test]
    fn bare_yield_signal_then_wait_then_tick_round_trip() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("directors/Quest")).unwrap();
        // Mirror scripts/lua/global.lua EXACTLY: bare-pair yields.
        std::fs::write(
            root.join("global.lua"),
            r#"
                function wait(seconds)
                    return coroutine.yield("_WAIT_TIME", seconds);
                end
                function waitForSignal(signal)
                    return coroutine.yield("_WAIT_SIGNAL", signal);
                end
            "#,
        )
        .unwrap();
        std::fs::write(
            root.join("directors/Quest/QuestDirectorSignalWait.lua"),
            r#"
                require ("global")
                function onEventStarted(player, director, eventType, eventName)
                    waitForSignal("playerActive")
                    wait(0.05)
                    player:SendMessage(0x20, "", "post-wait continuation")
                end
            "#,
        )
        .unwrap();

        let engine = LuaEngine::new(&root);
        let script_path = root.join("directors/Quest/QuestDirectorSignalWait.lua");
        let dummy_queue = CommandQueue::new();
        let result = engine.call_director_on_event_started(
            &script_path,
            sample_snapshot(),
            sample_director(dummy_queue),
            "noticeEvent".to_string(),
            5,
            vec![],
        );
        assert!(result.error.is_none(), "spawn errored: {:?}", result.error);
        {
            let sched = engine.scheduler().lock().unwrap();
            assert_eq!(
                sched.pending_signal_count(),
                1,
                "bare _WAIT_SIGNAL yield must park on the signal",
            );
        }

        // F press: resume off the signal; the coroutine runs into
        // `wait(0.05)` and must RE-PARK on time.
        let resumed = engine.fire_signal_and_drain("playerActive");
        assert!(
            resumed.is_empty(),
            "no commands queued between the signal and the wait; got {resumed:?}",
        );
        {
            let sched = engine.scheduler().lock().unwrap();
            assert_eq!(sched.pending_signal_count(), 0);
            assert_eq!(
                sched.pending_time_count(),
                1,
                "bare _WAIT_TIME yield after the signal must re-park on time",
            );
        }

        // Ticker: after the deadline, the continuation drains as a
        // batch owned by the trigger player (snapshot actor 42).
        std::thread::sleep(std::time::Duration::from_millis(120));
        let batches = engine.tick();
        assert_eq!(
            batches.len(),
            1,
            "one coroutine batch should drain; got {batches:?}",
        );
        let (owner, cmds) = &batches[0];
        assert_eq!(*owner, 42, "batch must carry the trigger player as owner");
        assert!(
            cmds.iter()
                .any(|c| matches!(c, LuaCommand::SendMessage { .. })),
            "post-wait continuation commands must drain; got {cmds:?}",
        );
    }

    /// FULL live-sequence repro of the SEQ_005 chain: cinematic
    /// `_WAIT_EVENT` park -> EventUpdate resume -> `_WAIT_SIGNAL`
    /// re-park -> F-press signal resume -> `_WAIT_TIME` re-park ->
    /// ticker drain. All with global.lua's BARE yield pairs.
    /// (Garlemald-Server #28.)
    #[test]
    fn full_cinematic_signal_wait_chain_round_trip() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("directors/Quest")).unwrap();
        std::fs::write(
            root.join("global.lua"),
            r#"
                function callClientFunction(player, fn_name, ...)
                    player:RunEventFunction(fn_name, ...)
                    return coroutine.yield("_WAIT_EVENT", player);
                end
                function wait(seconds)
                    return coroutine.yield("_WAIT_TIME", seconds);
                end
                function waitForSignal(signal)
                    return coroutine.yield("_WAIT_SIGNAL", signal);
                end
                function kickEventContinue(player, actor, trigger, ...)
                    player:KickEvent(actor, trigger, ...);
                    return coroutine.yield("_WAIT_EVENT", player);
                end
            "#,
        )
        .unwrap();
        std::fs::write(
            root.join("directors/Quest/QuestDirectorFullChain.lua"),
            r#"
                require ("global")
                function onEventStarted(player, director, eventType, eventName)
                    callClientFunction(player, "delegateEvent", player, "processTtrBtl001")
                    player:EndEvent()
                    waitForSignal("playerActive")
                    wait(0.05)
                    kickEventContinue(player, director, "noticeEvent", "noticeEvent")
                    callClientFunction(player, "delegateEvent", player, "processTtrBtl002")
                end
            "#,
        )
        .unwrap();

        let engine = LuaEngine::new(&root);
        let script_path = root.join("directors/Quest/QuestDirectorFullChain.lua");
        let dummy_queue = CommandQueue::new();
        let result = engine.call_director_on_event_started(
            &script_path,
            sample_snapshot(),
            sample_director(dummy_queue),
            "noticeEvent".to_string(),
            5,
            vec![],
        );
        assert!(result.error.is_none(), "spawn errored: {:?}", result.error);
        // First slice: RunEventFunction queued, parked on the event.
        assert!(
            result
                .commands
                .iter()
                .any(|c| matches!(c, LuaCommand::RunEventFunction { .. })),
            "cinematic RunEventFunction should drain; got {:?}",
            result.commands,
        );
        assert_eq!(engine.scheduler().lock().unwrap().pending_event_count(), 1);

        // Client finishes the cinematic -> EventUpdate resume. The
        // coroutine emits EndEvent then parks on the signal.
        let cmds = engine
            .fire_player_event_and_drain(42, &[])
            .expect("a coroutine should be parked on the player event");
        assert!(
            cmds.iter()
                .any(|c| matches!(c, LuaCommand::EndEvent { .. })),
            "EndEvent should drain off the EventUpdate resume; got {cmds:?}",
        );
        {
            let sched = engine.scheduler().lock().unwrap();
            assert_eq!(sched.pending_event_count(), 0);
            assert_eq!(
                sched.pending_signal_count(),
                1,
                "director must be parked on playerActive after the cinematic",
            );
        }

        // F press.
        let resumed = engine.fire_signal_and_drain("playerActive");
        assert!(resumed.is_empty(), "straight into wait(); got {resumed:?}");
        assert_eq!(
            engine.scheduler().lock().unwrap().pending_time_count(),
            1,
            "wait(0.05) must re-park on time after the signal resume",
        );

        // Ticker: post-wait slice queues the kick (kickEventContinue)
        // and parks on the next client event.
        std::thread::sleep(std::time::Duration::from_millis(120));
        let batches = engine.tick();
        assert_eq!(batches.len(), 1, "got {batches:?}");
        let (owner, cmds) = &batches[0];
        assert_eq!(*owner, 42);
        assert!(
            cmds.iter()
                .any(|c| matches!(c, LuaCommand::KickEvent { .. })),
            "kickEventContinue's KickEvent must drain from the tick; got {cmds:?}",
        );
        assert_eq!(engine.scheduler().lock().unwrap().pending_event_count(), 1);

        // Client replies to the kick -> next resume runs
        // processTtrBtl002's RunEventFunction.
        let cmds = engine
            .fire_player_event_and_drain(42, &[])
            .expect("parked on the kick reply");
        assert!(
            cmds.iter()
                .any(|c| matches!(c, LuaCommand::RunEventFunction { .. })),
            "processTtrBtl002 must drain off the kick reply; got {cmds:?}",
        );
    }

    /// Same chain as `full_cinematic_signal_wait_chain_round_trip` but
    /// against the REAL production scripts (`scripts/lua/` — the full
    /// `QuestDirectorMan0g001.lua` require chain, global.lua's real
    /// helpers, and the real `wait(1)`), so script-side regressions in
    /// the SEQ_005 F-press flow fail in CI instead of live.
    /// (Garlemald-Server #28.)
    #[test]
    fn real_man0g001_director_f_press_chain() {
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/lua"));
        assert!(
            root.join("directors/Quest/QuestDirectorMan0g001.lua")
                .exists()
        );
        let engine = LuaEngine::new(root);

        let mut snapshot = sample_snapshot();
        snapshot.current_class = 2; // gladiator -> IsDiscipleOfWar
        snapshot.active_quests = vec![110_005]; // Man0g0
        snapshot.active_quest_states = vec![userdata::QuestStateSnapshot {
            quest_id: 110_005,
            sequence: 5,
            flags: 0,
            counters: [0; 4],
            npc_ls_from: 0,
            npc_ls_msg_step: 0,
        }];

        let script_path = root.join("directors/Quest/QuestDirectorMan0g001.lua");
        let dummy_queue = CommandQueue::new();
        let result = engine.call_director_on_event_started(
            &script_path,
            snapshot,
            sample_director(dummy_queue),
            "noticeEvent".to_string(),
            5,
            vec![],
        );
        assert!(
            result.error.is_none(),
            "director spawn errored: {:?}",
            result.error,
        );
        // First slice: startTutorialMode's SendDataPacket + the
        // processTtrBtl001 RunEventFunction, parked on the cinematic.
        assert!(
            result
                .commands
                .iter()
                .any(|c| matches!(c, LuaCommand::RunEventFunction { .. })),
            "processTtrBtl001 should drain; got {:?}",
            result.commands,
        );
        assert_eq!(
            engine.scheduler().lock().unwrap().pending_event_count(),
            1,
            "parked on the cinematic EventUpdate",
        );

        // Cinematic done.
        let cmds = engine
            .fire_player_event_and_drain(42, &[])
            .expect("parked on cinematic");
        assert!(
            cmds.iter()
                .any(|c| matches!(c, LuaCommand::EndEvent { .. })),
            "EndEvent after the cinematic; got {cmds:?}",
        );
        assert_eq!(
            engine.scheduler().lock().unwrap().pending_signal_count(),
            1,
            "parked on playerActive",
        );

        // F press.
        let resumed = engine.fire_signal_and_drain("playerActive");
        assert!(resumed.is_empty(), "straight into wait(1); got {resumed:?}",);
        assert_eq!(
            engine.scheduler().lock().unwrap().pending_time_count(),
            1,
            "wait(1) must re-park on time",
        );

        // The REAL wait(1): sleep past it, then tick.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let batches = engine.tick();
        assert_eq!(
            batches.len(),
            1,
            "the post-wait continuation must drain; got {batches:?}",
        );
        let (owner, cmds) = &batches[0];
        assert_eq!(*owner, 42);
        assert!(
            cmds.iter()
                .any(|c| matches!(c, LuaCommand::KickEvent { .. })),
            "kickEventContinue's KickEvent must drain; got {cmds:?}",
        );
        assert_eq!(
            engine.scheduler().lock().unwrap().pending_event_count(),
            1,
            "parked on the kick reply",
        );

        // Client EventStart reply to the kick — resumes into
        // processTtrBtl002's RunEventFunction, parked on the targeting
        // tutorial's return.
        let cmds = engine
            .fire_player_event_and_drain(42, &[])
            .expect("parked on the kick reply");
        assert!(
            cmds.iter()
                .any(|c| matches!(c, LuaCommand::RunEventFunction { .. })),
            "processTtrBtl002 must drain off the kick reply; got {cmds:?}",
        );
        assert_eq!(engine.scheduler().lock().unwrap().pending_event_count(), 1);

        // Btl002 returns (EventUpdate) — EndEvent drains and the
        // director parks on the S4.1 kill gate's signal: the fight is
        // free-running from here (full sequence covered end-to-end by
        // `s4_2_real_director_full_sequence_kill_gate_to_warp`).
        let cmds = engine
            .fire_player_event_and_drain(42, &[])
            .expect("parked on Btl002");
        assert!(
            cmds.iter()
                .any(|c| matches!(c, LuaCommand::EndEvent { .. })),
            "EndEvent after Btl002; got {cmds:?}",
        );
        {
            let sched = engine.scheduler().lock().unwrap();
            assert_eq!(sched.pending_event_count(), 0);
            assert_eq!(
                sched.pending_signal_count(),
                1,
                "director must be parked on battleComplete during the fight",
            );
        }
    }

    /// Tutorial kill EXP (opening quests): the three REAL SEQ_005
    /// content scripts award retail's per-kill EXP from `onUpdate` by
    /// watching the live-hostile count — 1000/kill for the Gridania
    /// wolves and Limsa aurelias, 3000 for the Ul'dah goobbue
    /// (FFXIVenturer era guides + open-beta footage OCR; 3000 total per
    /// city). Per script: arming ticks never grant; a hostile dropping
    /// off the live roster lands exactly one `AddExp(per_kill, current
    /// class)` per kill; an empty-ally roster (pre-onCreate window)
    /// never touches the counter; and a SECOND session interleaved on
    /// the same cached VM with a different live count neither mints
    /// phantom EXP nor disturbs the first session's counter.
    #[test]
    fn real_seq005_content_scripts_grant_tutorial_kill_exp() {
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/lua"));
        let actor = |id: u32, name: &str| userdata::LuaActor {
            actor_id: id,
            name: name.to_string(),
            class_name: String::new(),
            class_path: String::new(),
            unique_id: String::new(),
            zone_id: 166,
            zone_name: String::new(),
            state: 2,
            pos: (0.0, 0.0, 0.0),
            rotation: 0.0,
            queue: CommandQueue::new(),
            is_engaged: false,
            speed: 0.0,
            target_actor_id: 0,
            hp: 0,
            max_hp: 0,
        };
        let grants_in = |cmds: &[LuaCommand]| {
            cmds.iter()
                .filter_map(|c| match c {
                    LuaCommand::AddExp {
                        actor_id,
                        class_id,
                        exp,
                    } => Some((*actor_id, *class_id, *exp)),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        for (script, hostiles, per_kill) in [
            ("content/SimpleContent30010.lua", 3u32, 1000i32), // Gridania wolves
            ("content/SimpleContent30002.lua", 3, 1000),       // Limsa aurelias
            ("content/SimpleContent30079.lua", 1, 3000),       // Ul'dah goobbue
        ] {
            let script_path = root.join(script);
            assert!(script_path.exists(), "{script} missing");
            // Fresh engine per script = fresh VM, like a server boot;
            // every tick below shares it, like the live ticker.
            let engine = LuaEngine::new(root);
            // Session A (actor 42) and session B (actor 77) — each tick
            // carries its own per-session roster, mirroring
            // build_content_rosters.
            let area_for = |player_id: u32, live: u32, with_ally: bool| {
                let mut snap = sample_snapshot();
                snap.actor_id = player_id;
                snap.current_class = 2; // state_mainSkill[0] → AddExp target
                let mut area = sample_content_area(CommandQueue::new());
                area.players = vec![snap];
                if with_ally {
                    area.allies = vec![actor(0x4500_0000 + player_id, "ally")];
                }
                area.monsters = (0..live)
                    .map(|i| actor(0x4600_0000 + player_id * 16 + i, "mob"))
                    .collect();
                area
            };

            // Pre-onCreate window: no roster at all (no allies) — the
            // counter must not arm, or a stale value could later pay out
            // against an unspawned fight.
            let r0 = engine.call_content_on_update(&script_path, 1, area_for(42, 0, false));
            assert!(r0.error.is_none(), "{script} tick0 errored: {:?}", r0.error);

            // Session A arms: every hostile live — no grant.
            let r1 = engine.call_content_on_update(&script_path, 2, area_for(42, hostiles, true));
            assert!(r1.error.is_none(), "{script} tick1 errored: {:?}", r1.error);
            assert_eq!(
                grants_in(&r1.commands),
                vec![],
                "{script}: no grant while everything is alive",
            );

            // Session B arms on the SAME cached VM with its own roster.
            let r2 = engine.call_content_on_update(&script_path, 3, area_for(77, hostiles, true));
            assert!(r2.error.is_none(), "{script} tick2 errored: {:?}", r2.error);
            assert_eq!(
                grants_in(&r2.commands),
                vec![],
                "{script}: session B arming must not read session A's count",
            );

            // Session A: one hostile gone from the live-only roster
            // (killed — S0.5 filters dead members out before the script
            // runs) → exactly one per-kill grant to A.
            let r3 =
                engine.call_content_on_update(&script_path, 4, area_for(42, hostiles - 1, true));
            assert!(r3.error.is_none(), "{script} tick3 errored: {:?}", r3.error);
            assert_eq!(
                grants_in(&r3.commands),
                vec![(42, 2, per_kill)],
                "{script}: exactly one per-kill grant to the player's class",
            );

            // Session B still at full strength on the next interleaved
            // tick: the scalar-global bug would see B's count (3) vs A's
            // stored 2 and mint phantom EXP here.
            let r4 = engine.call_content_on_update(&script_path, 5, area_for(77, hostiles, true));
            assert!(r4.error.is_none(), "{script} tick4 errored: {:?}", r4.error);
            assert_eq!(
                grants_in(&r4.commands),
                vec![],
                "{script}: session B at full strength must not be paid for A's kill",
            );

            // Session A again, unchanged count — no repeat grant.
            let r5 =
                engine.call_content_on_update(&script_path, 6, area_for(42, hostiles - 1, true));
            assert!(r5.error.is_none(), "{script} tick5 errored: {:?}", r5.error);
            assert_eq!(
                grants_in(&r5.commands),
                vec![],
                "{script}: unchanged count must not re-grant",
            );
        }
    }

    /// The Limsa Hob handoff (man0l0 SEQ_010 → Man0l1), slice by slice
    /// against the REAL scripts — the 2026-06-12 live run crashed the
    /// client here because the final slice warped to the inn with the
    /// talkDefault event still open: pmeteor's closer lives in hob.lua
    /// (a PrivateArea populace script garlemald's talk dispatch never
    /// reaches), man0l0's HOB arm early-returns at ReplaceQuest, and
    /// man0l1.onStart carried no closer. The fix mirrors man0g1: the
    /// onStart final slice must end with EndEvent.
    #[test]
    fn real_limsa_hob_handoff_final_slice_closes_event() {
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/lua"));
        let engine = LuaEngine::new(root);

        let mut snapshot = sample_snapshot();
        snapshot.actor_id = 1;
        snapshot.current_class = 3;
        snapshot.active_quests = vec![110_001];
        snapshot.active_quest_states = vec![userdata::QuestStateSnapshot {
            quest_id: 110_001,
            sequence: 10, // SEQ_010 — Limsa port, talking to Hob
            flags: 0,
            counters: [0; 4],
            npc_ls_from: 0,
            npc_ls_msg_step: 0,
        }];

        // Slice 1: the talk → man0l0.onTalk parks on the choice RPC.
        let hob = LuaNpcSpec {
            actor_id: 0x4730_00DC, // live Hob actor id (log 06:13:57)
            name: "hob".into(),
            class_name: "PopulaceStandard".into(),
            class_path: "/Chara/Npc/Populace/PopulaceStandard".into(),
            unique_id: "hob".into(),
            zone_id: 230,
            zone_name: "sea0Town01a".into(),
            state: 0,
            pos: (0.0, 0.0, 0.0),
            rotation: 0.0,
            actor_class_id: 1_000_151, // HOB in man0l0.lua
            quest_graphic: 0,
        };
        let r1 = engine.call_quest_hook(
            &root.join("quests/man/man0l0.lua"),
            "onTalk",
            snapshot.clone(),
            userdata::LuaQuestHandle {
                player_id: 1,
                quest_id: 110_001,
                has_quest: true,
                sequence: 10,
                flags: 0,
                counters: [0; 4],
                npc_ls_from: 0,
                npc_ls_msg_step: 0,
                queue: CommandQueue::new(),
            },
            vec![QuestHookArg::Npc(hob)],
        );
        assert!(r1.error.is_none(), "onTalk errored: {:?}", r1.error);
        assert_eq!(engine.scheduler().lock().unwrap().pending_event_count(), 1);

        // Slice 2: EventUpdate carries choice=1 → ReplaceQuest hands off
        // to Man0l1 (CompleteQuest + AddQuest) and the onTalk coroutine
        // finishes (its EndEvent is intentionally skipped — the closer
        // belongs to man0l1.onStart, after processEvent010).
        let s2 = engine
            .fire_player_event_and_drain(1, &[common::luaparam::LuaParam::Int32(1)])
            .expect("onTalk parked on the choice RPC");
        assert!(
            s2.iter().any(|c| matches!(
                c,
                LuaCommand::CompleteQuest {
                    quest_id: 110_001,
                    ..
                }
            )) && s2.iter().any(|c| matches!(
                c,
                LuaCommand::AddQuest {
                    quest_id: 110_002,
                    ..
                }
            )),
            "ReplaceQuest must push CompleteQuest(110001)+AddQuest(110002); got {s2:?}",
        );
        assert_eq!(engine.scheduler().lock().unwrap().pending_event_count(), 0);

        // Slice 3: apply_add_quest fires man0l1.onStart → parks on the
        // processEvent010 RPC.
        let mut snapshot2 = snapshot.clone();
        snapshot2.active_quests = vec![110_002];
        snapshot2.active_quest_states = vec![userdata::QuestStateSnapshot {
            quest_id: 110_002,
            sequence: 0,
            flags: 0,
            counters: [0; 4],
            npc_ls_from: 0,
            npc_ls_msg_step: 0,
        }];
        let r3 = engine.call_quest_hook(
            &root.join("quests/man/man0l1.lua"),
            "onStart",
            snapshot2,
            userdata::LuaQuestHandle {
                player_id: 1,
                quest_id: 110_002,
                has_quest: true,
                sequence: 0,
                flags: 0,
                counters: [0; 4],
                npc_ls_from: 0,
                npc_ls_msg_step: 0,
                queue: CommandQueue::new(),
            },
            vec![],
        );
        assert!(r3.error.is_none(), "onStart errored: {:?}", r3.error);
        assert_eq!(engine.scheduler().lock().unwrap().pending_event_count(), 1);

        // Slice 4: the inn warp — and the load-bearing EndEvent closer,
        // which must come BEFORE the warp: garlemald's DoZoneChange
        // ships the full zone-in replay inline (deleting Hob, the
        // event's owner), and both an absent AND a trailing EndEvent
        // crashed the live client (2026-06-12, two runs).
        let s4 = engine
            .fire_player_event_and_drain(1, &[])
            .expect("onStart parked on processEvent010");
        let end_pos = s4
            .iter()
            .position(|c| matches!(c, LuaCommand::EndEvent { .. }));
        let warp_pos = s4
            .iter()
            .position(|c| matches!(c, LuaCommand::DoZoneChange { zone_id: 133, .. }));
        assert!(
            warp_pos.is_some(),
            "final slice must warp to the inn; got {s4:?}"
        );
        assert!(
            end_pos.is_some() && end_pos < warp_pos,
            "final slice must CLOSE the talk event BEFORE the warp; got {s4:?}",
        );
        assert_eq!(engine.scheduler().lock().unwrap().pending_event_count(), 0);
    }

    /// All three follow-up quests' `onStart` (the opener → MSQ-2
    /// handoffs) warp the player to the adventurers' guild from inside
    /// the still-open handoff talk event — each final slice must close
    /// the event BEFORE the DoZoneChange (the warp's inline zone-in
    /// replay deletes the event-owner NPC; a trailing EndEvent arrives
    /// after the client has already crashed — 2026-06-12 live runs).
    #[test]
    fn real_follow_up_quest_onstart_final_slice_ends_event() {
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/lua"));
        for (script, quest_id, dest_zone) in [
            ("quests/man/man0l1.lua", 110_002u32, 133u32), // Limsa → the inn
            ("quests/man/man0g1.lua", 110_006, 155),       // Gridania → Carline Canopy
            ("quests/man/man0u1.lua", 110_010, 175),       // Ul'dah → the Quicksand
        ] {
            let engine = LuaEngine::new(root);
            let mut snapshot = sample_snapshot();
            snapshot.actor_id = 1;
            snapshot.active_quests = vec![quest_id];
            snapshot.active_quest_states = vec![userdata::QuestStateSnapshot {
                quest_id,
                sequence: 0,
                flags: 0,
                counters: [0; 4],
                npc_ls_from: 0,
                npc_ls_msg_step: 0,
            }];
            let r = engine.call_quest_hook(
                &root.join(script),
                "onStart",
                snapshot,
                userdata::LuaQuestHandle {
                    player_id: 1,
                    quest_id,
                    has_quest: true,
                    sequence: 0,
                    flags: 0,
                    counters: [0; 4],
                    npc_ls_from: 0,
                    npc_ls_msg_step: 0,
                    queue: CommandQueue::new(),
                },
                vec![],
            );
            assert!(r.error.is_none(), "{script} onStart errored: {:?}", r.error);
            assert_eq!(
                engine.scheduler().lock().unwrap().pending_event_count(),
                1,
                "{script}: onStart must park on its intro RPC",
            );
            let last = engine
                .fire_player_event_and_drain(1, &[])
                .expect("onStart parked");
            let end_pos = last
                .iter()
                .position(|c| matches!(c, LuaCommand::EndEvent { .. }));
            let warp_pos = last.iter().position(
                |c| matches!(c, LuaCommand::DoZoneChange { zone_id, .. } if *zone_id == dest_zone),
            );
            assert!(
                warp_pos.is_some(),
                "{script}: final slice must warp to zone {dest_zone}; got {last:?}",
            );
            assert!(
                end_pos.is_some() && end_pos < warp_pos,
                "{script}: final slice must close the event BEFORE the warp; got {last:?}",
            );
        }
    }

    /// Cross-thread repro of the LIVE ticker topology: the signal resume
    /// (and its `wait()` re-park) happens on one thread while a ticker
    /// thread hammers `engine.tick()` concurrently — mirroring the
    /// processor async task vs the ticker's `spawn_blocking`. The live
    /// SEQ_005 run parks on time after the F-press signal resume but the
    /// production ticker never drains it; if that reproduces here, it is
    /// an engine bug — if not, it is environmental. (Garlemald-Server #28.)
    #[test]
    fn ticker_thread_drains_park_made_on_another_thread() {
        let root = tmpdir();
        std::fs::create_dir_all(root.join("directors/Quest")).unwrap();
        std::fs::write(
            root.join("global.lua"),
            r#"
                function wait(seconds)
                    return coroutine.yield("_WAIT_TIME", seconds);
                end
                function waitForSignal(signal)
                    return coroutine.yield("_WAIT_SIGNAL", signal);
                end
            "#,
        )
        .unwrap();
        std::fs::write(
            root.join("directors/Quest/QuestDirectorThreaded.lua"),
            r#"
                require ("global")
                function onEventStarted(player, director, eventType, eventName)
                    waitForSignal("playerActive")
                    wait(0.2)
                    player:SendMessage(0x20, "", "post-wait")
                end
            "#,
        )
        .unwrap();

        let engine = Arc::new(LuaEngine::new(&root));
        let script_path = root.join("directors/Quest/QuestDirectorThreaded.lua");
        let dummy_queue = CommandQueue::new();
        let result = engine.call_director_on_event_started(
            &script_path,
            sample_snapshot(),
            sample_director(dummy_queue),
            "noticeEvent".to_string(),
            5,
            vec![],
        );
        assert!(result.error.is_none(), "{:?}", result.error);

        // Ticker thread: hammer tick() every 10ms for up to 3s, collect
        // any drained batches.
        let tick_engine = engine.clone();
        let ticker = std::thread::spawn(move || {
            let mut got: Vec<(u32, Vec<LuaCommand>)> = Vec::new();
            for _ in 0..300 {
                got.extend(tick_engine.tick());
                if !got.is_empty() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            got
        });

        // Resume off the signal from THIS thread (the "processor").
        std::thread::sleep(std::time::Duration::from_millis(50));
        let resumed = engine.fire_signal_and_drain("playerActive");
        assert!(resumed.is_empty());
        assert_eq!(
            engine.scheduler().lock().unwrap().pending_time_count(),
            1,
            "wait(0.2) parked on time from the processor thread",
        );

        let got = ticker.join().unwrap();
        assert_eq!(
            got.len(),
            1,
            "the ticker thread must drain the cross-thread park; got {got:?}",
        );
        assert!(
            got[0]
                .1
                .iter()
                .any(|c| matches!(c, LuaCommand::SendMessage { .. })),
            "post-wait continuation must drain; got {got:?}",
        );
    }
}
