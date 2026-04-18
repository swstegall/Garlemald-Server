//! Axum router + listener wiring. Kept thin on purpose — all request
//! handling lives in `handlers.rs`.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::Router;
use axum::routing::get;
use tower_http::trace::TraceLayer;

use crate::config::Config;
use crate::database::Database;
use crate::handlers::{self, AppState};

pub async fn run(config: Config, db: Database) -> Result<()> {
    let bind_addr: SocketAddr = format!("{}:{}", config.bind_ip(), config.port())
        .parse()
        .with_context(|| {
            format!(
                "parsing bind address {}:{}",
                config.bind_ip(),
                config.port()
            )
        })?;

    let state = AppState {
        db: Arc::new(db),
        session_hours: config.session_hours(),
    };

    let app = Router::new()
        .route("/", get(handlers::root))
        .route(
            "/login",
            get(handlers::login_form).post(handlers::login_submit),
        )
        .route(
            "/signup",
            get(handlers::signup_form).post(handlers::signup_submit),
        )
        .route("/healthz", get(handlers::healthz))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    tracing::info!(%bind_addr, "web server listening");
    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .with_context(|| format!("binding {bind_addr}"))?;
    axum::serve(listener, app.into_make_service())
        .await
        .context("axum::serve")?;
    Ok(())
}
