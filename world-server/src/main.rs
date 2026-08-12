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
    let smoke = args.smoke;
    let _no_console = args.no_console; // no interactive console here; accepted for CLI parity
    tracing::debug!(config_path = %args.config, "loading config");
    let mut config = match Config::load(&args.config) {
        Ok(cfg) => cfg,
        Err(e) => {
            if smoke {
                std::process::exit(common::smoke::smoke_fail(
                    "World",
                    "config",
                    &e.to_string(),
                    common::smoke::EXIT_CONFIG,
                ));
            }
            return Err(e);
        }
    };
    config.apply_launch_args(args);
    tracing::info!(
        bind_ip = %config.bind_ip(),
        port = config.port(),
        world_id = config.world_id(),
        db_path = %config.db_path().display(),
        "config resolved"
    );

    tracing::info!(db_path = %config.db_path().display(), "opening sqlite database");
    let db = match Database::open(config.db_path()).await {
        Ok(db) => Arc::new(db),
        Err(e) => {
            if smoke {
                std::process::exit(common::smoke::smoke_fail(
                    "World",
                    "database",
                    &e.to_string(),
                    common::smoke::EXIT_DATABASE,
                ));
            }
            return Err(e);
        }
    };
    match db.ping().await {
        Ok(()) => tracing::info!("DB connection ok"),
        Err(e) => {
            if smoke {
                std::process::exit(common::smoke::smoke_fail(
                    "World",
                    "database",
                    &e.to_string(),
                    common::smoke::EXIT_DATABASE,
                ));
            }
            tracing::error!(error = %e, "DB connection failed; aborting");
            return Err(e);
        }
    }

    // Resolve this world's display name from the server list config (falls
    // back to "Unknown" if the entry is missing, matching the C# `Program.cs`
    // welcome message logic). Issue #11: the list lives in servers.toml now,
    // not the DB.
    match common::server_list::ServerList::load(config.servers_path()) {
        Ok(list) => match list.by_id(config.world_id()) {
            Some(world) => {
                tracing::info!(name = %world.name, "loaded world info from server list");
                config.server_name = world.name.clone();
            }
            None => {
                tracing::warn!(
                    world_id = config.world_id(),
                    path = %config.servers_path().display(),
                    "world entry missing from server list; MOTD disabled"
                );
            }
        },
        Err(e) => {
            if smoke {
                std::process::exit(common::smoke::smoke_fail(
                    "World",
                    "config",
                    &e.to_string(),
                    common::smoke::EXIT_CONFIG,
                ));
            }
            // A present-but-malformed server list is a deployment error;
            // fail fast like lobby-server does rather than booting with a
            // config the operator thinks is in effect.
            tracing::error!(error = %e, "server list load failed; aborting");
            return Err(e);
        }
    }

    let world = Arc::new(WorldMaster::new());
    server::run(config, db, world, smoke).await
}
