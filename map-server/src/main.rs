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

//! Map server entry point.

use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use tokio::io::{AsyncBufReadExt, BufReader};

mod achievement;
mod actor;
mod battle;
mod command_processor;
mod config;
mod data;
mod database;
mod director;
mod event;
mod gamedata;
mod group;
mod inventory;
mod lua;
mod npc;
mod packets;
mod processor;
mod runtime;
mod server;
mod social;
mod status;
mod world_manager;
mod zone;

use crate::command_processor::CommandProcessor;
use crate::config::{Config, LaunchArgs};
use crate::database::Database;
use crate::lua::LuaEngine;
use crate::runtime::{ActorRegistry, GameTicker, TickerConfig};
use crate::world_manager::WorldManager;

#[tokio::main]
async fn main() -> Result<()> {
    common::logging::init("[MAP]  ");
    common::packet_log::init("[MAP]  ");

    tracing::info!("==================================");
    tracing::info!("Garlemald: Map Server");
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
        script_root = %config.script_root().display(),
        load_from_database = config.load_from_database(),
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

    let world = Arc::new(WorldManager::new());
    let registry = Arc::new(ActorRegistry::new());
    let lua = Arc::new(LuaEngine::new(config.script_root().to_path_buf()));
    let cmd = Arc::new(CommandProcessor::new(
        world.clone(),
        registry.clone(),
        db.clone(),
        lua.clone(),
    ));
    tracing::info!(path = ?config.script_root(), "lua engine initialised");

    // Phase-2 loaders — zones, private areas, entrances, seamless
    // boundaries. Skipped when the test harness flips
    // `load_from_database = false`.
    if config.load_from_database() {
        let bind_ip = config.bind_ip().to_string();
        let port = config.port();
        match world.load_from_database(&db, &bind_ip, port).await {
            Ok(()) => tracing::info!(zones = world.zone_count().await, "zones loaded"),
            Err(e) => {
                tracing::error!(error = %e, "world load failed; continuing with empty zones");
            }
        }
        // Phase-3 spawn pass — materialise NPCs from the seed rows the
        // zone loaders just populated. Requires `ActorClass` metadata
        // loaded from DB, so do that first.
        match db.load_actor_classes().await {
            Ok(classes) => {
                let battle_ids = std::collections::HashSet::<u32>::new();
                let npc_appearances = db.load_npc_appearances().await.unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "npc appearances load failed; NPCs will ship with model_id=0");
                    std::collections::HashMap::new()
                });
                tracing::info!(count = npc_appearances.len(), "npc appearances loaded");
                let ctx = crate::npc::SpawnContext {
                    world: &world,
                    registry: &registry,
                    actor_classes: &classes,
                    battle_class_ids: &battle_ids,
                    npc_appearances: &npc_appearances,
                };
                let spawned = ctx.spawn_all_actors().await;
                tracing::info!(count = spawned.len(), "npc spawn pass complete");
            }
            Err(e) => {
                tracing::warn!(error = %e, "actor classes load failed; skipping spawn pass");
            }
        }
    }

    // Spawn the game-loop ticker.
    tokio::spawn({
        let ticker = GameTicker::new(
            TickerConfig::default(),
            world.clone(),
            registry.clone(),
            db.clone(),
        );
        async move {
            ticker.run().await;
        }
    });

    // Interactive console reader.
    tokio::spawn({
        let cmd = cmd.clone();
        async move {
            let stdin = BufReader::new(tokio::io::stdin());
            let mut lines = stdin.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::info!(%line, "[Console Input]");
                if let Ok(response) = cmd.run(&line).await
                    && !response.is_empty()
                {
                    tracing::info!(%response, "command result");
                }
            }
        }
    });

    server::run(config, db, world, registry, lua).await
}
