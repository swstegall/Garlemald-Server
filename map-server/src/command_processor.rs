//! GM console command parser. Ported from CommandProcessor.cs as a straight
//! string-dispatch table. The full command catalogue in the C# server is
//! large; Phase 4 wires a handful of the universally-useful ones and leaves
//! `unknown` as the default so unimplemented commands are visible in logs.
#![allow(dead_code)]

use std::sync::Arc;

use anyhow::Result;

use crate::world_manager::WorldManager;

pub struct CommandProcessor {
    pub world: Arc<WorldManager>,
}

impl CommandProcessor {
    pub fn new(world: Arc<WorldManager>) -> Self {
        Self { world }
    }

    /// Run a single console command. Returns the human-readable response.
    pub async fn run(&self, line: &str) -> Result<String> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(String::new());
        }
        let mut parts = trimmed.split_whitespace();
        let cmd = parts.next().unwrap_or("").to_lowercase();
        let args: Vec<&str> = parts.collect();

        match cmd.as_str() {
            "help" => Ok("commands: help, who, version, reload".into()),
            "version" => Ok(format!("map-server {}", env!("CARGO_PKG_VERSION"))),
            "who" => {
                let zones = {
                    let zones = self.world.zone(0).await;
                    zones.is_some() as usize
                };
                Ok(format!("{zones} zone(s) loaded"))
            }
            "reload" => {
                // Hook point for future `WorldManager::reload_scripts`.
                Ok("reload requested (scripts reload TODO)".into())
            }
            other => Ok(format!("unknown command: {other} (args={:?})", args)),
        }
    }
}
