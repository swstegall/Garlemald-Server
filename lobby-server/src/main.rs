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

//! Lobby server entry point. Loads TOML config, opens (and auto-creates) the
//! SQLite database, pings it, and spawns the listener.

use anyhow::Result;
use clap::Parser;

mod character_creator;
mod config;
mod data;
mod database;
mod hardcoded;
mod packets;
mod processor;
mod server;

use crate::config::{Config, LaunchArgs};
use crate::database::Database;
use crate::processor::PacketProcessor;

#[tokio::main]
async fn main() -> Result<()> {
    common::logging::init("[LOBBY]");
    common::packet_log::init("[LOBBY]");

    tracing::info!("==================================");
    tracing::info!("Garlemald: Lobby Server");
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting");
    tracing::info!("==================================");

    let args = LaunchArgs::parse();
    tracing::debug!(config_path = %args.config, "loading config");
    let mut config = Config::load(&args.config)?;
    config.apply_launch_args(args);
    tracing::info!(
        bind_ip = %config.bind_ip(),
        port = config.port(),
        db_path = %config.db_path().display(),
        "config resolved"
    );

    tracing::info!(db_path = %config.db_path().display(), "opening sqlite database");
    let db = Database::open(config.db_path()).await?;
    match db.ping().await {
        Ok(()) => tracing::info!("DB connection ok"),
        Err(e) => {
            tracing::error!(error = %e, "DB connection failed; aborting");
            return Err(e);
        }
    }

    let processor = PacketProcessor::new(db);
    server::run(config, processor).await
}
