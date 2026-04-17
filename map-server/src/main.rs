//! Map server entry point. Phase-4 port of project-meteor-mirror's
//! `Map Server/Program.cs`.

use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use tokio::io::{AsyncBufReadExt, BufReader};

mod actor;
mod battle;
mod command_processor;
mod config;
mod data;
mod database;
mod event;
mod gamedata;
mod inventory;
mod lua;
mod npc;
mod packets;
mod processor;
mod runtime;
mod server;
mod status;
mod world_manager;
mod zone;

use crate::command_processor::CommandProcessor;
use crate::config::{Config, LaunchArgs};
use crate::database::Database;
use crate::runtime::{ActorRegistry, GameTicker, TickerConfig};
use crate::world_manager::WorldManager;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("==================================");
    tracing::info!("Garlemald: Map Server");
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting");
    tracing::info!("==================================");

    let args = LaunchArgs::parse();
    let mut config = Config::load(&args.config)?;
    config.apply_launch_args(args);

    tracing::info!(
        host = %config.db_host, port = config.db_port, database = %config.db_name,
        "testing DB connection"
    );
    let db = Arc::new(Database::new(&config.mysql_url())?);
    match db.ping().await {
        Ok(()) => tracing::info!("DB connection ok"),
        Err(e) => {
            tracing::error!(error = %e, "DB connection failed; aborting");
            return Err(e);
        }
    }

    let world = Arc::new(WorldManager::new());
    let registry = Arc::new(ActorRegistry::new());
    let cmd = Arc::new(CommandProcessor::new(world.clone()));

    // Spawn the game-loop ticker. Walks every zone every 100ms and
    // drains the four typed outboxes (status / battle / area / inventory)
    // into real packets + DB writes + Lua calls.
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

    // Interactive console reader, mirroring the blocking `Console.ReadLine`
    // loop in the C# Program.Main.
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

    server::run(config, db, world, registry).await
}
