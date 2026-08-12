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

//! The world/server list served at character select — `configs/servers.toml`.
//!
//! Deployment configuration (world name, advertised address/port, list
//! position, active flag) used to live in the `servers` SQLite table, which
//! forced operators to hand-edit the database. It is now an editable TOML
//! file, consistent with the per-service configs (issue #11):
//!
//! ```toml
//! [[servers]]
//! id            = 1
//! name          = "Fernehalwes"
//! address       = "127.0.0.1"
//! port          = 54992
//! list_position = 1
//! max_chars     = 5000
//! is_active     = true
//! ```
//!
//! Lobby-server builds the character-select world list from this, and
//! world-server resolves its own display name (`[server].world_id` →
//! [`ServerEntry::id`]) for the welcome MOTD. Live population is runtime
//! state, not configuration, and deliberately has no field here.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

/// The parsed server list.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerList {
    pub servers: Vec<ServerEntry>,
}

/// One world as advertised at character select.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerEntry {
    /// Stable world id — `characters.serverId` rows and world-server's
    /// `[server].world_id` key on this.
    pub id: u32,
    pub name: String,
    /// Address handed to the client for the lobby → world handoff.
    pub address: String,
    pub port: u16,
    #[serde(default = "default_list_position")]
    pub list_position: u16,
    /// Advertised capacity. Not yet consulted at runtime (live population is
    /// runtime state); kept so operator configs round-trip losslessly.
    #[serde(default = "default_max_chars")]
    pub max_chars: u32,
    #[serde(default = "default_is_active")]
    pub is_active: bool,
}

fn default_list_position() -> u16 {
    1
}
fn default_max_chars() -> u32 {
    5000
}
fn default_is_active() -> bool {
    true
}

impl Default for ServerList {
    /// The one-box localhost world every fresh checkout boots with —
    /// mirrors the row the old schema seeded.
    fn default() -> Self {
        Self {
            servers: vec![ServerEntry {
                id: 1,
                name: "Fernehalwes".to_string(),
                address: "127.0.0.1".to_string(),
                port: 54992,
                list_position: 1,
                max_chars: 5000,
                is_active: true,
            }],
        }
    }
}

impl ServerList {
    /// Load the server list from `path`.
    ///
    /// A missing file falls back to the built-in localhost default (with a
    /// warning) so a fresh checkout boots with zero config edits; a file
    /// that exists but fails to parse is a hard error — silently masking a
    /// typo'd config with defaults would be worse than failing the boot.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            tracing::warn!(
                ?path,
                "server list config not found, using localhost default"
            );
            return Ok(Self::default());
        }
        let raw =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let list: ServerList =
            toml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
        Ok(list)
    }

    /// Worlds shown at character select, in config-file order.
    /// (The old SQL read was `WHERE isActive = 1`.)
    pub fn active(&self) -> impl Iterator<Item = &ServerEntry> {
        self.servers.iter().filter(|s| s.is_active)
    }

    /// Look up a world by id, active or not — matches the old
    /// `WHERE id = :sid` read, which did not filter on the active flag.
    pub fn by_id(&self, id: u32) -> Option<&ServerEntry> {
        self.servers.iter().find(|s| s.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_shipped_shape() {
        let list: ServerList = toml::from_str(
            r#"
            [[servers]]
            id            = 1
            name          = "Fernehalwes"
            address       = "127.0.0.1"
            port          = 54992
            list_position = 1
            max_chars     = 5000
            is_active     = true

            [[servers]]
            id            = 2
            name          = "Cornelia"
            address       = "10.0.0.2"
            port          = 54992
            list_position = 2
            is_active     = false
            "#,
        )
        .unwrap();
        assert_eq!(list.servers.len(), 2);
        let w = &list.servers[0];
        assert_eq!((w.id, w.port, w.list_position), (1, 54992, 1));
        assert_eq!(w.name, "Fernehalwes");
        // max_chars omitted on the second entry → default
        assert_eq!(list.servers[1].max_chars, 5000);
    }

    #[test]
    fn active_filters_and_by_id_does_not() {
        let list: ServerList = toml::from_str(
            r#"
            [[servers]]
            id = 1
            name = "A"
            address = "127.0.0.1"
            port = 54992
            [[servers]]
            id = 2
            name = "B"
            address = "127.0.0.1"
            port = 54993
            is_active = false
            "#,
        )
        .unwrap();
        assert_eq!(list.active().count(), 1);
        assert_eq!(list.active().next().unwrap().id, 1);
        // by_id still resolves the inactive world, like the old SQL did.
        assert_eq!(list.by_id(2).unwrap().name, "B");
        assert!(list.by_id(3).is_none());
    }

    #[test]
    fn unknown_keys_are_rejected() {
        // deny_unknown_fields: a typo'd key must fail loudly, not vanish.
        let err = toml::from_str::<ServerList>(
            r#"
            [[servers]]
            id = 1
            name = "A"
            address = "127.0.0.1"
            port = 54992
            adress_typo = "oops"
            "#,
        );
        assert!(err.is_err());
    }

    #[test]
    fn missing_file_falls_back_to_localhost_default() {
        let list = ServerList::load("/nonexistent/servers.toml").unwrap();
        assert_eq!(list.servers.len(), 1);
        let w = &list.servers[0];
        assert_eq!((w.id, w.port, w.is_active), (1, 54992, true));
        assert_eq!(w.name, "Fernehalwes");
    }

    #[test]
    fn empty_document_and_explicit_empty_list_differ() {
        // `#[serde(default)]` on the container: a document that omits the
        // `servers` key entirely behaves like a missing file — the built-in
        // localhost world — keeping "no config given" consistent at both
        // granularities.
        let list: ServerList = toml::from_str("").unwrap();
        assert_eq!(list.servers.len(), 1);
        assert_eq!(list.servers[0].name, "Fernehalwes");

        // An operator who explicitly writes an empty list gets an empty
        // list — that's a stated choice, not an omission.
        let list: ServerList = toml::from_str("servers = []").unwrap();
        assert!(list.servers.is_empty());
    }
}
