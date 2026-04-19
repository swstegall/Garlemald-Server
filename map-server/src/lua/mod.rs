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
pub use userdata::{LuaActor, LuaNpc, LuaPlayer, LuaZone, PlayerSnapshot, ZoneSnapshot};

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
}
