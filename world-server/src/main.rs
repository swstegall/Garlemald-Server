//! World server entry point. Phase 1 scaffold only — linkshell/party/retainer
//! managers and packet routing are ported in a later phase.

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "world-server starting (scaffold only)"
    );

    Ok(())
}
