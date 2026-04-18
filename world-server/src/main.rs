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
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("==================================");
    tracing::info!("Garlemald: World Server");
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting");
    tracing::info!("==================================");

    let args = LaunchArgs::parse();
    let mut config = Config::load(&args.config)?;
    config.apply_launch_args(args);

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
