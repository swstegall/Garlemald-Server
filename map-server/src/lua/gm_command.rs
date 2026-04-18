//! `!<cmd>` GM command runner. Ported from `LuaEngine.RunGMCommand`.
//!
//! The C# original reads a `properties` table out of the script to determine
//! (a) the permission level, (b) the parameter-type string (`"sssssss"`,
//! `"iif"`, etc.), and (c) the description. We replicate that introspection
//! and run `onTrigger` with the coerced parameters.

#![allow(dead_code)]

use std::path::Path;

use anyhow::Result;
use mlua::{Function, Lua, Table, Value};

use super::command::{CommandQueue, LuaCommand};
use super::paths::PathResolver;

#[derive(Debug, Clone, Default)]
pub struct CommandProperties {
    pub permissions: u8,
    pub parameters: String,
    pub description: String,
}

pub struct GmCommandRunner<'a> {
    pub resolver: &'a PathResolver,
    pub queue: std::sync::Arc<std::sync::Mutex<CommandQueue>>,
    pub lua: mlua::Lua,
}

impl<'a> GmCommandRunner<'a> {
    pub fn new(
        resolver: &'a PathResolver,
        queue: std::sync::Arc<std::sync::Mutex<CommandQueue>>,
    ) -> Self {
        Self {
            resolver,
            queue,
            lua: mlua::Lua::new(),
        }
    }

    /// Look up the command's `properties` table without executing `onTrigger`.
    /// Used by `!help <cmd>` to show the description.
    pub fn describe(&self, cmd: &str) -> Result<Option<CommandProperties>> {
        let path = self.resolver.gm_command(cmd);
        if !Path::new(&path).exists() {
            return Ok(None);
        }
        let source = std::fs::read_to_string(&path)?;
        self.lua
            .load(&source)
            .exec()
            .map_err(|e| anyhow::anyhow!("parse {path:?}: {e}"))?;

        let props: Value = self.lua.globals().get("properties").unwrap_or(Value::Nil);
        if let Value::Table(t) = props {
            Ok(Some(read_properties(&t)))
        } else {
            Ok(Some(CommandProperties::default()))
        }
    }

    /// Invoke `onTrigger(player, argCount, …)` with `args` coerced per the
    /// command's `parameters` string. `is_gm` gates commands with
    /// `permissions > 0`.
    pub fn run(
        &self,
        cmd: &str,
        player_id: Option<u32>,
        is_gm: bool,
        args: &[String],
    ) -> Result<Option<CommandProperties>> {
        let path = self.resolver.gm_command(cmd);
        if !Path::new(&path).exists() {
            CommandQueue::push(
                &self.queue,
                LuaCommand::LogError(format!("GM command script missing: {}", path.display())),
            );
            return Ok(None);
        }
        let source = std::fs::read_to_string(&path)?;
        self.lua
            .load(&source)
            .exec()
            .map_err(|e| anyhow::anyhow!("parse {path:?}: {e}"))?;

        let props: CommandProperties = match self.lua.globals().get::<Value>("properties")? {
            Value::Table(t) => read_properties(&t),
            _ => CommandProperties::default(),
        };

        if props.permissions > 0 && !is_gm {
            CommandQueue::push(
                &self.queue,
                LuaCommand::LogError(format!("{cmd}: insufficient permissions")),
            );
            return Ok(Some(props));
        }

        let on_trigger: Value = self.lua.globals().get("onTrigger")?;
        let Value::Function(on_trigger) = on_trigger else {
            CommandQueue::push(
                &self.queue,
                LuaCommand::LogError(format!("{cmd}: onTrigger missing")),
            );
            return Ok(Some(props));
        };

        let coerced = coerce_args(&self.lua, &props.parameters, args)?;
        let mut multi = mlua::MultiValue::new();
        multi.push_back(
            player_id
                .map(|id| Value::Integer(id as i64))
                .unwrap_or(Value::Nil),
        );
        multi.push_back(Value::Integer(coerced.len() as i64));
        for v in coerced {
            multi.push_back(v);
        }
        self.invoke(&on_trigger, multi)?;
        Ok(Some(props))
    }

    fn invoke(&self, on_trigger: &Function, args: mlua::MultiValue) -> Result<()> {
        on_trigger
            .call::<Value>(args)
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!("onTrigger: {e}"))
    }
}

fn read_properties(table: &Table) -> CommandProperties {
    let permissions: i64 = table.get("permissions").unwrap_or(0);
    let parameters: String = table.get("parameters").unwrap_or_default();
    let description: String = table.get("description").unwrap_or_default();
    CommandProperties {
        permissions: permissions.clamp(0, 255) as u8,
        parameters,
        description,
    }
}

fn coerce_args(lua: &Lua, parameters: &str, args: &[String]) -> Result<Vec<Value>> {
    let mut out = Vec::with_capacity(parameters.len());
    for (i, ty) in parameters.chars().enumerate() {
        let Some(raw) = args.get(i) else { break };
        let value: Value = match ty {
            'i' => Value::Integer(raw.parse::<i64>().unwrap_or(0)),
            'd' | 'f' => Value::Number(raw.parse::<f64>().unwrap_or(0.0)),
            's' => Value::String(lua.create_string(raw)?),
            _ => Value::String(lua.create_string(raw)?),
        };
        out.push(value);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_script(name: &str, body: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("garlemald-lua-{}", uuid_like()));
        std::fs::create_dir_all(dir.join("commands/gm")).unwrap();
        let path = dir.join(format!("commands/gm/{name}.lua"));
        std::fs::write(&path, body).unwrap();
        dir
    }

    fn uuid_like() -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("{nanos:x}")
    }

    #[test]
    fn describe_returns_properties() {
        let root = temp_script(
            "ping",
            r#"
            properties = {
                permissions = 0,
                parameters = "s",
                description = "ping pong"
            }
            function onTrigger(player, argc, target)
                return target
            end
            "#,
        );
        let resolver = PathResolver::new(&root);
        let queue = CommandQueue::new();
        let runner = GmCommandRunner::new(&resolver, queue);
        let props = runner.describe("ping").unwrap().expect("found");
        assert_eq!(props.parameters, "s");
        assert_eq!(props.description, "ping pong");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gm_denied_for_non_gm_when_permissions_set() {
        let root = temp_script(
            "secret",
            r#"
            properties = { permissions = 1, parameters = "", description = "" }
            function onTrigger() end
            "#,
        );
        let resolver = PathResolver::new(&root);
        let queue = CommandQueue::new();
        let runner = GmCommandRunner::new(&resolver, queue.clone());
        runner.run("secret", Some(1), false, &[]).unwrap();
        let drained = CommandQueue::drain(&queue);
        assert!(matches!(drained.first(), Some(LuaCommand::LogError(_))));
        let _ = std::fs::remove_dir_all(root);
    }
}
