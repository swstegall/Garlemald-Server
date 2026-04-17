//! Lobby server entry point. Loads config, pings MySQL, spawns the listener.

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
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("==================================");
    tracing::info!("Garlemald: Lobby Server");
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting");
    tracing::info!("==================================");

    let args = LaunchArgs::parse();
    let mut config = Config::load(&args.config)?;
    config.apply_launch_args(args);

    tracing::info!(host = %config.db_host, port = config.db_port, database = %config.db_name, "testing DB connection");
    let db = Database::new(&config.mysql_url())?;
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
