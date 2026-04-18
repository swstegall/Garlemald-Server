//! SQLite connection helper shared by every Garlemald server.
//!
//! `open_or_create` is the canonical entry point: it creates the parent
//! directory, opens a `tokio_rusqlite::Connection`, applies the bundled
//! schema when the file is fresh, and sets WAL + foreign-key pragmas.
//!
//! `ConnCallExt::call_db` is the shape every `database.rs` queue uses to
//! dispatch blocking rusqlite work — it pins `E = rusqlite::Error` so the
//! inline closures don't have to annotate the error type on every `Ok(..)`.

use std::future::Future;
use std::path::Path;

use anyhow::{Context, Result};
use tokio_rusqlite::Connection;

/// Embedded schema (the `CREATE TABLE IF NOT EXISTS` set needed by all three
/// servers). Applied once when the database file is first created.
pub const SCHEMA_SQL: &str = include_str!("../sql/schema.sql");

/// Open (and initialise, if fresh) the SQLite database at `path`.
///
/// Steps:
/// 1. Ensure the parent directory exists.
/// 2. Check if the file already exists.
/// 3. Open an async rusqlite connection.
/// 4. Set `journal_mode = WAL` and `foreign_keys = ON`.
/// 5. If the file was just created, execute `SCHEMA_SQL` to lay down every
///    table the server code expects.
pub async fn open_or_create(path: impl AsRef<Path>) -> Result<Connection> {
    let path = path.as_ref().to_path_buf();
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating db dir {}", parent.display()))?;
    }

    let fresh = !path.exists();
    let conn = Connection::open(path.clone())
        .await
        .with_context(|| format!("opening sqlite {}", path.display()))?;

    conn.call(|c| {
        c.pragma_update(None, "journal_mode", "WAL")?;
        c.pragma_update(None, "foreign_keys", "ON")?;
        c.pragma_update(None, "synchronous", "NORMAL")?;
        Ok::<(), rusqlite::Error>(())
    })
    .await
    .context("setting sqlite pragmas")?;

    if fresh {
        tracing::info!(path = %path.display(), "initialising fresh sqlite database");
        conn.call(|c| {
            c.execute_batch(SCHEMA_SQL)?;
            Ok::<(), rusqlite::Error>(())
        })
        .await
        .context("applying schema.sql")?;
    }

    Ok(conn)
}

/// Extension trait that pins `E = rusqlite::Error` on the async `.call()`
/// helper provided by `tokio_rusqlite::Connection`. Without it every closure
/// body needs `Ok::<_, rusqlite::Error>(..)` annotations to satisfy the
/// generic error parameter.
pub trait ConnCallExt {
    fn call_db<F, R>(&self, f: F) -> impl Future<Output = Result<R>> + Send
    where
        F: FnOnce(&mut rusqlite::Connection) -> rusqlite::Result<R> + Send + 'static,
        R: Send + 'static;
}

impl ConnCallExt for Connection {
    fn call_db<F, R>(&self, f: F) -> impl Future<Output = Result<R>> + Send
    where
        F: FnOnce(&mut rusqlite::Connection) -> rusqlite::Result<R> + Send + 'static,
        R: Send + 'static,
    {
        async move {
            self.call(f).await.map_err(anyhow::Error::from)
        }
    }
}
