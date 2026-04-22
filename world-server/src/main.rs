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

//! World server entry point.

use std::sync::Arc;

use anyhow::Result;
use clap::Parser;

mod config;
mod data;
mod database;
mod group;
mod managers;
mod packets;
mod processor;
mod server;
mod world_master;

use crate::config::{Config, LaunchArgs};
use crate::database::Database;
use crate::world_master::WorldMaster;

#[tokio::main]
async fn main() -> Result<()> {
    common::logging::init("[WORLD]");
    common::packet_log::init("[WORLD]");

    tracing::info!("==================================");
    tracing::info!("Garlemald: World Server");
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting");
    tracing::info!("==================================");

    let args = LaunchArgs::parse();
    tracing::debug!(config_path = %args.config, "loading config");
    let mut config = Config::load(&args.config)?;
    config.apply_launch_args(args);
    tracing::info!(
        bind_ip = %config.bind_ip(),
        port = config.port(),
        world_id = config.world_id(),
        db_path = %config.db_path().display(),
        "config resolved"
    );

    tracing::info!(db_path = %config.db_path().display(), "opening sqlite database");
    let db = Arc::new(Database::open(config.db_path()).await?);
    match db.ping().await {
        Ok(()) => tracing::info!("DB connection ok"),
        Err(e) => {
            tracing::error!(error = %e, "DB connection failed; aborting");
            return Err(e);
        }
    }

    // Pull this world's metadata from the DB (falls back to "Unknown" if the
    // row is missing, matching the C# `Program.cs` welcome message logic).
    match db.get_server(config.world_id()).await {
        Ok(Some(world)) => {
            tracing::info!(name = %world.name, "loaded world info from DB");
            config.server_name = world.name;
        }
        Ok(None) => {
            tracing::warn!("world row missing; MOTD disabled");
        }
        Err(e) => {
            tracing::warn!(error = %e, "world lookup failed; MOTD disabled");
        }
    }

    let world = Arc::new(WorldMaster::new());
    server::run(config, db, world).await
}
