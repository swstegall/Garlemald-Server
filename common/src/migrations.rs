//! Bundled SQL migrations — the Meteor-ported seed data applied on fresh
//! or upgrading Garlemald databases.
//!
//! `build.rs` enumerates `common/sql/seed/*.sql`, gzip-compresses each
//! file, and writes a manifest listing every bundled blob. This module
//! `include!`s that manifest and provides [`iter`] for the db layer.
//!
//! Migrations are identified by filename (e.g. `001_gamedata_items.sql`).
//! The runner (`common::db::apply_migrations`) records applied names in a
//! `schema_migrations` tracking table so existing databases only pick up
//! *new* migrations on upgrade.

use std::io::Read;

use flate2::read::GzDecoder;

include!(concat!(env!("OUT_DIR"), "/seed_manifest.rs"));

/// One bundled migration, ready to execute.
pub struct Migration {
    pub name: &'static str,
    pub sql: String,
}

/// Iterate every migration the binary was built with, in filename order.
/// Each call decompresses on-demand — there is no long-lived cache.
pub fn iter() -> impl Iterator<Item = Migration> {
    SEED_MIGRATIONS.iter().map(|(name, gz)| {
        let mut dec = GzDecoder::new(*gz);
        let mut sql = String::new();
        dec.read_to_string(&mut sql).unwrap_or_else(|e| {
            panic!("decompressing migration {name}: {e}");
        });
        Migration { name, sql }
    })
}

/// Count of bundled migrations (useful for startup logs).
pub fn count() -> usize {
    SEED_MIGRATIONS.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_non_empty() {
        assert!(count() > 0);
    }

    #[test]
    fn every_migration_decompresses_to_sqlite_sql() {
        for mig in iter() {
            assert!(!mig.sql.is_empty(), "{} decompressed empty", mig.name);
            // Every bundled file should contain a sanity-check token.
            assert!(
                mig.sql.contains("CREATE TABLE") || mig.sql.contains("INSERT"),
                "{} has neither CREATE TABLE nor INSERT",
                mig.name,
            );
        }
    }

    #[test]
    fn migration_names_are_sorted() {
        let names: Vec<&str> = iter().map(|m| m.name).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted, "migrations are expected in filename order");
    }
}
