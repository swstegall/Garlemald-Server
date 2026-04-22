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

use std::net::IpAddr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub server: ServerSection,
    pub database: DatabaseSection,
    pub scripting: ScriptingSection,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerSection {
    pub bind_ip: String,
    pub port: u16,
    pub show_timestamp: bool,
    pub world_id: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DatabaseSection {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ScriptingSection {
    /// Filesystem root of the Lua script tree (`scripts/`).
    pub script_root: PathBuf,
    /// When `false`, skip the DB loaders + `spawn_all_actors`. Used by the
    /// integration test harness.
    pub load_from_database: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerSection::default(),
            database: DatabaseSection::default(),
            scripting: ScriptingSection::default(),
        }
    }
}

impl Default for ServerSection {
    fn default() -> Self {
        Self {
            bind_ip: "127.0.0.1".to_string(),
            port: 1989,
            show_timestamp: true,
            world_id: 1,
        }
    }
}

impl Default for DatabaseSection {
    fn default() -> Self {
        Self {
            path: PathBuf::from("./data/garlemald.db"),
        }
    }
}

impl Default for ScriptingSection {
    fn default() -> Self {
        Self {
            script_root: PathBuf::from("./scripts"),
            load_from_database: true,
        }
    }
}

impl Config {
    pub fn bind_ip(&self) -> &str {
        &self.server.bind_ip
    }
    pub fn port(&self) -> u16 {
        self.server.port
    }
    pub fn db_path(&self) -> &Path {
        &self.database.path
    }
    pub fn script_root(&self) -> &Path {
        &self.scripting.script_root
    }
    pub fn load_from_database(&self) -> bool {
        self.scripting.load_from_database
    }
    #[allow(dead_code)]
    pub fn world_id(&self) -> u32 {
        self.server.world_id
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            tracing::warn!(?path, "map config not found, using defaults");
            return Ok(Self::default());
        }
        let raw =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let cfg: Config =
            toml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
        Ok(cfg)
    }

    pub fn apply_launch_args(&mut self, args: LaunchArgs) {
        if let Some(ip) = args.ip
            && ip.parse::<IpAddr>().is_ok()
        {
            self.server.bind_ip = ip;
        }
        if let Some(port) = args.port {
            self.server.port = port;
        }
        if let Some(db) = args.db_path {
            self.database.path = db;
        }
        if let Some(world_id) = args.world_id {
            self.server.world_id = world_id;
        }
    }
}

#[derive(Parser, Debug, Clone, Default)]
#[command(version, about = "FFXIV 1.23b map server", long_about = None)]
pub struct LaunchArgs {
    #[arg(long)]
    pub ip: Option<String>,
    #[arg(long)]
    pub port: Option<u16>,
    #[arg(long = "db-path")]
    pub db_path: Option<PathBuf>,
    #[arg(long = "world-id")]
    pub world_id: Option<u32>,
    #[arg(long, default_value = "./configs/map.toml")]
    pub config: String,
}
