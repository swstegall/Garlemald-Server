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

/// Runtime configuration for the world server.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub server: ServerSection,
    pub database: DatabaseSection,
    pub servers: ServersSection,
    /// Populated at startup from the server list config. Default `"Unknown"`
    /// if the entry is missing.
    #[serde(skip, default = "default_server_name")]
    pub server_name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerSection {
    pub bind_ip: String,
    pub port: u16,
    pub show_timestamp: bool,
    /// This world's entry id in the server list config (`servers.toml`);
    /// used to resolve the display name for the welcome MOTD.
    pub world_id: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DatabaseSection {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServersSection {
    /// Path to the server-list TOML shared with lobby-server. Falls back to
    /// a built-in localhost world if the file is missing.
    pub path: PathBuf,
}

fn default_server_name() -> String {
    "Unknown".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerSection::default(),
            database: DatabaseSection::default(),
            servers: ServersSection::default(),
            server_name: default_server_name(),
        }
    }
}

impl Default for ServerSection {
    fn default() -> Self {
        Self {
            bind_ip: "127.0.0.1".to_string(),
            port: 54992,
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

impl Default for ServersSection {
    fn default() -> Self {
        Self {
            path: PathBuf::from("./configs/servers.toml"),
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
    pub fn world_id(&self) -> u32 {
        self.server.world_id
    }
    pub fn db_path(&self) -> &Path {
        &self.database.path
    }
    pub fn servers_path(&self) -> &Path {
        &self.servers.path
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            tracing::warn!(?path, "world config not found, using defaults");
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
#[command(version, about = "FFXIV 1.23b world server", long_about = None)]
pub struct LaunchArgs {
    #[arg(long)]
    pub ip: Option<String>,
    #[arg(long)]
    pub port: Option<u16>,
    #[arg(long = "db-path")]
    pub db_path: Option<PathBuf>,
    #[arg(long = "world-id")]
    pub world_id: Option<u32>,
    /// Boot only far enough to validate config + DB, print a SMOKE_OK /
    /// SMOKE_FAIL marker, then exit (CI/dev fail-fast).
    #[arg(long)]
    pub smoke: bool,
    /// Suppress the interactive stdin console (no-op on servers without one).
    #[arg(long = "no-console")]
    pub no_console: bool,
    #[arg(long, default_value = "./configs/world.toml")]
    pub config: String,
}
