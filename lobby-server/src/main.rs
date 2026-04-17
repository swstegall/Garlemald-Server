//! Lobby server entry point. Phase 1 scaffold only — the full character
//! creation / selection flow will be ported in a later phase.

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "lobby-server starting (scaffold only)"
    );

    // Verify the common crate is linked end-to-end.
    let header = common::BasePacketHeader::default();
    tracing::debug!(?header, "loaded common crate");

    Ok(())
}
