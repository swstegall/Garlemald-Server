//! Map server entry point. Phase 1 scaffold only — zone/actor/Lua ports land
//! in later phases.

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "map-server starting (scaffold only)"
    );

    // Spin up Lua to verify mlua links. `mlua::Error` isn't Send+Sync on 0.11,
    // so bridge through to_string() rather than using `?` straight.
    let lua = mlua::Lua::new();
    let v: i64 = lua
        .load("return 1 + 1")
        .eval()
        .map_err(|e| anyhow::anyhow!("lua eval: {e}"))?;
    tracing::debug!(lua_result = v, "lua runtime healthy");

    Ok(())
}
