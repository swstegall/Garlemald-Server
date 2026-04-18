use std::net::IpAddr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use serde::Deserialize;

/// Runtime configuration for the web (login/signup) server.
///
/// Mirrors the lobby / world / map config shape: TOML on disk, CLI overrides
/// via [`LaunchArgs`], localhost-safe defaults when a file is missing.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub server: ServerSection,
    pub database: DatabaseSection,
    pub session: SessionSection,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerSection {
    pub bind_ip: String,
    pub port: u16,
    pub show_timestamp: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DatabaseSection {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SessionSection {
    /// Lifetime of a session row inserted on successful login. After this
    /// expires, the lobby server rejects the token and the user has to
    /// re-authenticate through the webview.
    pub hours: u32,
}

impl Default for ServerSection {
    fn default() -> Self {
        Self {
            bind_ip: "127.0.0.1".to_string(),
            port: 54993,
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

impl Default for SessionSection {
    fn default() -> Self {
        Self { hours: 24 }
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
    pub fn session_hours(&self) -> u32 {
        self.session.hours
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            tracing::warn!(?path, "web config not found, using defaults");
            return Ok(Self::default());
        }
        let raw =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let cfg: Config =
            toml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
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
        if let Some(hours) = args.session_hours {
            self.session.hours = hours;
        }
    }
}

#[derive(Parser, Debug, Clone, Default)]
#[command(version, about = "Garlemald web (login/signup) server", long_about = None)]
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
    /// Override the session lifetime in hours
    #[arg(long)]
    pub session_hours: Option<u32>,
    /// Path to the web TOML config
    #[arg(long, default_value = "./configs/web.toml")]
    pub config: String,
}
