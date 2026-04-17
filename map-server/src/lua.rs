//! Lua scripting bridge.
//!
//! The C# server uses MoonSharp with scripts laid out like
//! `scripts/zones/<zoneId>/<npcName>.lua`. Each script exposes `init`,
//! `onEventStarted`, `onEventUpdated`, `onTalk`, etc. Phase 4 replaces that
//! with `mlua` hosting Lua 5.4: we load scripts on demand, cache the
//! compiled chunks, and call a well-known function name.
//!
//! Deep integration (Actor userdata, the `npc:talk()` API surface, all the
//! game-state pass-through) is deferred. The bridge does enough to show that
//! scripts load and dispatch works.

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use mlua::{Function, Lua, Table, Value};
use tokio::sync::Mutex;

/// Thread-local Lua VM per script. The 1.23b server mutates shared state
/// inside scripts (e.g. counters), so each zone deserves its own VM.
pub struct LuaEngine {
    script_root: PathBuf,
    /// Cached VMs keyed by `"<zoneId>/<scriptPath>"`.
    vms: Mutex<HashMap<String, Arc<Lua>>>,
}

impl LuaEngine {
    pub fn new(script_root: impl Into<PathBuf>) -> Self {
        Self { script_root: script_root.into(), vms: Mutex::new(HashMap::new()) }
    }

    /// Load (or reuse) a Lua VM for `script_id` and run `fn_name` with
    /// `args`. The returned string is whatever the function returned — or an
    /// empty string for nil/error.
    pub async fn call(&self, script_id: &str, fn_name: &str, args: &[LuaArg]) -> Result<String> {
        let lua = self.get_or_load_vm(script_id).await?;
        let globals: Table = lua.globals();
        let func: Function = globals
            .get(fn_name)
            .map_err(|e| anyhow::anyhow!("lua fn {fn_name} missing in {script_id}: {e}"))?;

        let rust_args: Vec<Value> = args.iter().map(|a| a.to_lua(&lua).unwrap_or(Value::Nil)).collect();
        let result: Value = func
            .call::<Value>(mlua::MultiValue::from_vec(rust_args))
            .map_err(|e| anyhow::anyhow!("lua call failed: {e}"))?;
        Ok(match result {
            Value::String(s) => s.to_str().map(|c| c.to_string()).unwrap_or_default(),
            Value::Integer(i) => i.to_string(),
            Value::Number(n) => n.to_string(),
            Value::Boolean(b) => b.to_string(),
            Value::Nil => String::new(),
            _ => format!("{result:?}"),
        })
    }

    async fn get_or_load_vm(&self, script_id: &str) -> Result<Arc<Lua>> {
        {
            let guard = self.vms.lock().await;
            if let Some(vm) = guard.get(script_id) {
                return Ok(vm.clone());
            }
        }
        let path = self.script_root.join(script_id);
        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("reading lua script {}", path.display()))?;
        let lua = Arc::new(Lua::new());
        lua.load(&source)
            .exec()
            .map_err(|e| anyhow::anyhow!("lua parse error in {script_id}: {e}"))?;
        self.vms.lock().await.insert(script_id.to_string(), lua.clone());
        Ok(lua)
    }

    /// Check whether a script has a given function defined — used by the
    /// equivalent of `LuaEngine.CallLuaFunction` fallback chain.
    pub async fn has_fn(&self, script_id: &str, fn_name: &str) -> bool {
        if let Ok(lua) = self.get_or_load_vm(script_id).await
            && let Ok(globals) = lua.globals().get::<Value>(fn_name)
        {
            return matches!(globals, Value::Function(_));
        }
        false
    }

    pub fn script_path(&self, zone_id: u32, script_name: &str) -> PathBuf {
        self.script_root.join(zone_id.to_string()).join(script_name)
    }

    pub fn has_script_file(&self, zone_id: u32, script_name: &str) -> bool {
        Path::new(&self.script_path(zone_id, script_name)).exists()
    }
}

/// Type-erased Lua argument. Mirrors the C# `LuaUtils.CreateLuaParamList`
/// shape so callers can pass the same values they would on the wire.
#[derive(Debug, Clone)]
pub enum LuaArg {
    Int(i64),
    Uint(u64),
    Number(f64),
    String(String),
    Bool(bool),
    Nil,
    ActorId(u32),
}

impl LuaArg {
    fn to_lua(&self, lua: &Lua) -> mlua::Result<Value> {
        Ok(match self {
            LuaArg::Int(v) => Value::Integer(*v),
            LuaArg::Uint(v) => Value::Integer(*v as i64),
            LuaArg::Number(v) => Value::Number(*v),
            LuaArg::String(s) => Value::String(lua.create_string(s)?),
            LuaArg::Bool(b) => Value::Boolean(*b),
            LuaArg::Nil => Value::Nil,
            LuaArg::ActorId(id) => Value::Integer(*id as i64),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn script_lookup_is_miss_when_file_absent() {
        let engine = LuaEngine::new("/nonexistent");
        assert!(engine.call("missing.lua", "foo", &[]).await.is_err());
    }
}
