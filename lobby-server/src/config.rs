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

/// Runtime configuration for the lobby server.
///
/// Loaded from a TOML file via `Config::load`; missing sections/fields fall
/// back to the `Default` impl below so a fresh checkout boots against
/// localhost without any config file at all.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub server: ServerSection,
    pub database: DatabaseSection,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerSection {
    pub bind_ip: String,
    pub port: u16,
    /// Preserved for round-trip config writes — the tracing subscriber owns
    /// timestamp formatting so this value is not consulted directly.
    pub show_timestamp: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DatabaseSection {
    /// Path to the SQLite file, created on first run if missing.
    pub path: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerSection::default(),
            database: DatabaseSection::default(),
        }
    }
}

impl Default for ServerSection {
    fn default() -> Self {
        Self {
            bind_ip: "127.0.0.1".to_string(),
            port: 54994,
            show_timestamp: true,
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

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            tracing::warn!(?path, "lobby config not found, using defaults");
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;
        let cfg: Config = toml::from_str(&raw)
            .with_context(|| format!("parsing {}", path.display()))?;
        Ok(cfg)
    }

    pub fn apply_launch_args(&mut self, args: LaunchArgs) {
        if let Some(ip) = args.ip {
            if ip.parse::<IpAddr>().is_ok() {
                self.server.bind_ip = ip;
            } else {
                tracing::warn!("invalid --ip ignored");
            }
        }
        if let Some(port) = args.port {
            self.server.port = port;
        }
        if let Some(db) = args.db_path {
            self.database.path = db;
        }
    }
}

#[derive(Parser, Debug, Clone, Default)]
#[command(version, about = "FFXIV 1.23b lobby server", long_about = None)]
pub struct LaunchArgs {
    /// Override bind IP (e.g. --ip 0.0.0.0)
    #[arg(long)]
    pub ip: Option<String>,
    /// Override bind port
    #[arg(long)]
    pub port: Option<u16>,
    /// Override SQLite file path
    #[arg(long = "db-path")]
    pub db_path: Option<PathBuf>,
    /// Path to the lobby TOML config
    #[arg(long, default_value = "./configs/lobby.toml")]
    pub config: String,
}
