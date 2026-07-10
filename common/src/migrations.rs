// garlemald-server — Rust port of a FINAL FANTASY XIV v1.23b server emulator (lobby/world/map)
// Copyright (C) 2026  Samuel Stegall
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: AGPL-3.0-or-later

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
            // Every bundled file should contain a sanity-check DDL/DML
            // token — guards against accidentally shipping an empty
            // (or wholly-commented-out) migration. `ALTER TABLE`
            // migrations are a legitimate column-add path (see
            // `050_characters_quest_scenario_npc_ls.sql` for the first
            // landed example), `UPDATE` a legitimate data-fix path
            // (`052_fix_tutorial_ally_pools.sql`), and `DELETE` a
            // legitimate data-removal path
            // (`092_remove_bluebadger_closed_gate.sql`), so the check is
            // the union of all five.
            assert!(
                mig.sql.contains("CREATE TABLE")
                    || mig.sql.contains("INSERT")
                    || mig.sql.contains("ALTER TABLE")
                    || mig.sql.contains("UPDATE")
                    || mig.sql.contains("DELETE"),
                "{} has none of CREATE TABLE / INSERT / ALTER TABLE / UPDATE / DELETE",
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

    /// Seed/099 contract: the Man1l0 seafield push trigger (spawn row 2090 +
    /// the class-1090082 un-stripping) and the ASSESSOR1 gate-echo position
    /// repair, applied twice for idempotency, against the pre-099 shapes the
    /// seed guards on. The quest's SEQ_070 leg soft-locks without the spawn
    /// (Garlemald-Server #48), and the content-test walkthrough can't see
    /// seeds — this is the only automated coverage of the fix.
    #[test]
    fn man1l0_spawn_repairs_restore_trigger_and_assessor() {
        let c = rusqlite::Connection::open_in_memory().unwrap();
        c.execute_batch(
            "CREATE TABLE gamedata_actor_class (
                 id INTEGER PRIMARY KEY,
                 classPath TEXT NOT NULL,
                 displayNameId INTEGER NOT NULL DEFAULT 4294967295,
                 propertyFlags INTEGER NOT NULL DEFAULT 0,
                 eventConditions TEXT
             );
             CREATE TABLE server_spawn_locations (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 actorClassId INTEGER NOT NULL,
                 uniqueId TEXT NOT NULL DEFAULT '',
                 zoneId INTEGER NOT NULL,
                 privateAreaName TEXT NOT NULL DEFAULT '',
                 privateAreaLevel INTEGER NOT NULL DEFAULT 0,
                 positionX REAL NOT NULL DEFAULT 0,
                 positionY REAL NOT NULL DEFAULT 0,
                 positionZ REAL NOT NULL DEFAULT 0,
                 rotation REAL NOT NULL DEFAULT 0,
                 actorState INTEGER NOT NULL DEFAULT 0,
                 animationId INTEGER NOT NULL DEFAULT 0,
                 customDisplayName TEXT DEFAULT NULL
             );
             -- Pre-099 shapes: the stripped seed/003 class row and the
             -- seed/059 placeholder-at-origin ASSESSOR1 row.
             INSERT INTO gamedata_actor_class VALUES (1090082, '', 0, 0, NULL);
             INSERT INTO server_spawn_locations VALUES
                 (2107, 1000120, 'man1l0_echo3_assessor2', 230,
                  'PrivateAreaMasterPast', 7, 0, 0, 0, 0, 0, 0, NULL);",
        )
        .unwrap();

        let mig = iter()
            .find(|m| m.name == "099_man1l0_spawn_repairs.sql")
            .expect("seed/099 must be bundled");
        c.execute_batch(&mig.sql).unwrap();
        c.execute_batch(&mig.sql).unwrap(); // idempotent re-run

        let (path, flags): (String, i64) = c
            .query_row(
                "SELECT classPath, propertyFlags FROM gamedata_actor_class WHERE id = 1090082",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(path, "/Chara/Npc/Populace/PopulaceStandard");
        assert_eq!(flags, 1);

        let (zone, x, z): (i64, f64, f64) = c
            .query_row(
                "SELECT zoneId, positionX, positionZ FROM server_spawn_locations WHERE id = 2090",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(zone, 128, "seafield trigger belongs to sea0Field01");
        assert!((x - 218.58).abs() < 1e-6 && (z - 1176.56).abs() < 1e-6);

        let (ax, ay, az): (f64, f64, f64) = c
            .query_row(
                "SELECT positionX, positionY, positionZ \
                 FROM server_spawn_locations WHERE id = 2107",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert!(
            ax != 0.0 && ay != 0.0 && az != 0.0,
            "ASSESSOR1 must no longer sit at the origin placeholder"
        );
    }
}
