use std::net::IpAddr;
use std::path::Path;

use anyhow::{Context, Result};
use clap::Parser;
use common::ini::IniFile;

/// Runtime configuration for the lobby server. Mirrors the C# `ConfigConstants`
/// plus the `ApplyLaunchArgs` overrides.
#[derive(Debug, Clone)]
pub struct Config {
    pub bind_ip: String,
    pub port: u16,
    /// Preserved from the C# config so round-trip reads/writes don't drop the
    /// key, even though our tracing subscriber controls timestamp formatting.
    #[allow(dead_code)]
    pub show_timestamp: bool,

    pub db_host: String,
    pub db_port: u16,
    pub db_name: String,
    pub db_user: String,
    pub db_password: String,
}

impl Config {
    pub fn mysql_url(&self) -> String {
        format!(
            "mysql://{user}:{pass}@{host}:{port}/{db}",
            user = self.db_user,
            pass = self.db_password,
            host = self.db_host,
            port = self.db_port,
            db = self.db_name,
        )
    }

    pub fn load(ini_path: impl AsRef<Path>) -> Result<Self> {
        let path = ini_path.as_ref();
        if !path.exists() {
            tracing::warn!(?path, "lobby_config.ini not found, loading defaults");
        }
        let ini = IniFile::open_with(path, true, false)
            .with_context(|| format!("reading {}", path.display()))?;

        let port: u32 = ini.get_value("General", "server_port", 54994u32);
        let show_timestamp = ini
            .get_value("General", "showtimestamp", "true".to_string())
            .eq_ignore_ascii_case("true");
        let db_port: u32 = ini.get_value("Database", "port", 3306u32);

        Ok(Config {
            bind_ip: ini.get_value("General", "server_ip", "127.0.0.1".to_string()),
            port: port as u16,
            show_timestamp,
            db_host: ini.get_value("Database", "host", String::new()),
            db_port: db_port as u16,
            db_name: ini.get_value("Database", "database", String::new()),
            db_user: ini.get_value("Database", "username", String::new()),
            db_password: ini.get_value("Database", "password", String::new()),
        })
    }

    pub fn apply_launch_args(&mut self, args: LaunchArgs) {
        if let Some(ip) = args.ip {
            if ip.parse::<IpAddr>().is_ok() {
                self.bind_ip = ip;
            } else {
                tracing::warn!("invalid --ip ignored");
            }
        }
        if let Some(port) = args.port {
            self.port = port;
        }
        if let Some(user) = args.user {
            self.db_user = user;
        }
        if let Some(password) = args.password {
            self.db_password = password;
        }
        if let Some(db) = args.db {
            self.db_name = db;
        }
        if let Some(host) = args.host {
            self.db_host = host;
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
    /// MySQL host override
    #[arg(long)]
    pub host: Option<String>,
    /// MySQL database name override
    #[arg(long)]
    pub db: Option<String>,
    /// MySQL username override
    #[arg(long)]
    pub user: Option<String>,
    /// MySQL password override (-p matches the C# CLI)
    #[arg(long = "password", short = 'p')]
    pub password: Option<String>,
    /// Path to lobby_config.ini
    #[arg(long, default_value = "./lobby_config.ini")]
    pub config: String,
}
