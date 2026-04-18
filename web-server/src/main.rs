//! Web server entry point. Serves the login + signup HTML forms and, on
//! success, redirects the caller to `ffxiv://login_success?sessionId=…`,
//! which the `garlemald-client` webview intercepts (see
//! `../../garlemald-client/src/login/webview.rs`).
//!
//! The web server owns two tables: `users` (created by schema.sql) and
//! `sessions` (shared with the lobby server). It never touches the rest of
//! the schema — character creation still lives in the lobby flow.

use anyhow::Result;
use clap::Parser;

mod config;
mod database;
mod handlers;
mod server;
mod session;
mod templates;

use crate::config::{Config, LaunchArgs};
use crate::database::Database;

#[tokio::main]
async fn main() -> Result<()> {
    common::logging::init("[WEB]  ");

    tracing::info!("==================================");
    tracing::info!("Garlemald: Web Server");
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
        session_hours = config.session_hours(),
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

    server::run(config, db).await
}
