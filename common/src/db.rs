//! SQLite connection helper shared by every Garlemald server.
//!
//! `open_or_create` is the canonical entry point: it creates the parent
//! directory, opens a `tokio_rusqlite::Connection`, applies the bundled
//! schema when the file is fresh, applies any new seed migrations, and
//! sets WAL + foreign-key pragmas.
//!
//! `ConnCallExt::call_db` is the shape every `database.rs` queue uses to
//! dispatch blocking rusqlite work — it pins `E = rusqlite::Error` so the
//! inline closures don't have to annotate the error type on every `Ok(..)`.

use std::future::Future;
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use rusqlite::named_params;
use tokio_rusqlite::Connection;

use crate::migrations;

/// Embedded schema (the `CREATE TABLE IF NOT EXISTS` set needed by all
/// servers). Applied once when the database file is first created.
pub const SCHEMA_SQL: &str = include_str!("../sql/schema.sql");

/// Ported MySQL -> SQLite data dumps from `project-meteor-mirror` are
/// tracked here. Each migration is keyed by filename; the runner records
/// applied names in `schema_migrations` so upgrading existing databases
/// picks up only the new files on next boot.
const SCHEMA_MIGRATIONS_DDL: &str = r#"
    CREATE TABLE IF NOT EXISTS schema_migrations (
        name       TEXT PRIMARY KEY,
        applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );
"#;

/// Open (and initialise, if fresh) the SQLite database at `path`.
///
/// Steps:
/// 1. Ensure the parent directory exists.
/// 2. Open an async rusqlite connection and set WAL + foreign keys.
/// 3. If the file was just created, execute `SCHEMA_SQL` to lay down
///    every table the server code expects.
/// 4. Apply any bundled seed migrations that haven't run yet (tracked
///    in `schema_migrations`).
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

    apply_migrations(&conn).await?;

    Ok(conn)
}

/// Apply every bundled migration that isn't already recorded in
/// `schema_migrations`. Each migration runs inside its own transaction
/// and is recorded on success; a failure rolls back and surfaces the
/// error so the server process aborts rather than limping on with a
/// half-seeded DB.
pub async fn apply_migrations(conn: &Connection) -> Result<()> {
    conn.call(|c| {
        c.execute_batch(SCHEMA_MIGRATIONS_DDL)?;
        Ok::<(), rusqlite::Error>(())
    })
    .await
    .context("creating schema_migrations table")?;

    let applied: std::collections::HashSet<String> = conn
        .call(|c| {
            let mut stmt = c.prepare("SELECT name FROM schema_migrations")?;
            let rows: Vec<String> = stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<_>>()?;
            Ok::<_, rusqlite::Error>(rows.into_iter().collect())
        })
        .await
        .context("reading schema_migrations")?;

    let total = migrations::count();
    let mut applied_now = 0usize;
    for migration in migrations::iter() {
        if applied.contains(migration.name) {
            continue;
        }
        let started = Instant::now();
        let name = migration.name.to_string();
        let sql = migration.sql;
        let moved_name = name.clone();
        conn.call(move |c| {
            let tx = c.transaction()?;
            tx.execute_batch(&sql)?;
            tx.execute(
                "INSERT INTO schema_migrations(name) VALUES(:n)",
                named_params! { ":n": moved_name },
            )?;
            tx.commit()?;
            Ok::<(), rusqlite::Error>(())
        })
        .await
        .with_context(|| format!("applying migration {name}"))?;
        tracing::info!(
            migration = %name,
            took_ms = started.elapsed().as_millis() as u64,
            "migration applied",
        );
        applied_now += 1;
    }
    tracing::info!(
        total_bundled = total,
        newly_applied = applied_now,
        "migration pass complete",
    );
    Ok(())
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
        async move { self.call(f).await.map_err(anyhow::Error::from) }
    }
}
