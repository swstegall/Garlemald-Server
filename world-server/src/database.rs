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

//! SQLite-backed world database queries. Ported from World Server/Database.cs.
//!
//! Several queries (linkshell member fetches, current-zone lookup) are
//! referenced by Phase-4 packet types that the Map Server will add; keep
//! them `#[allow(dead_code)]` for now so the API stays stable.
#![allow(dead_code)]

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, named_params};
use common::db::ConnCallExt;
use tokio_rusqlite::Connection;

use crate::data::DBWorld;
use crate::group::{Linkshell, LinkshellMember, RetainerGroupMember};

pub struct Database {
    conn: Connection,
}

impl Database {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = common::db::open_or_create(path).await?;
        Ok(Self { conn })
    }

    #[cfg(test)]
    pub fn conn_for_test(&self) -> &Connection {
        &self.conn
    }

    pub async fn ping(&self) -> Result<()> {
        self.conn
            .call_db(|c| {
                c.query_row("SELECT 1", [], |_| Ok(()))?;
                Ok(())
            })
            .await
            .context("ping")
    }

    pub async fn get_server(&self, server_id: u32) -> Result<Option<DBWorld>> {
        let row = self.conn
            .call_db(move |c| {
                let v = c
                    .query_row(
                        "SELECT name, address, port FROM servers WHERE id = :sid",
                        named_params! { ":sid": server_id },
                        |r| {
                            Ok(DBWorld {
                                id: server_id,
                                name: r.get::<_, String>(0)?,
                                address: r.get::<_, String>(1)?,
                                port: r.get::<_, u16>(2)?,
                                ..Default::default()
                            })
                        },
                    )
                    .optional()?;
                Ok(v)
            })
            .await?;
        Ok(row)
    }

    /// Populate `session` state from the `characters` row. Returns whether a
    /// row was found.
    pub async fn load_zone_session_info(
        &self,
        session_id: u32,
    ) -> Result<Option<SessionDbSnapshot>> {
        let row = self.conn
            .call_db(move |c| {
                let v = c
                    .query_row(
                        r"SELECT name, currentZoneId, destinationZoneId, currentActiveLinkshell
                          FROM characters WHERE id = :cid",
                        named_params! { ":cid": session_id },
                        |r| {
                            Ok(SessionDbSnapshot {
                                character_name: r.get::<_, String>(0).unwrap_or_default(),
                                current_zone_id: r.get::<_, u32>(1).unwrap_or(0),
                                destination_zone_id: r.get::<_, u32>(2).unwrap_or(0),
                                active_linkshell: r.get::<_, String>(3).unwrap_or_default(),
                            })
                        },
                    )
                    .optional()?;
                Ok(v)
            })
            .await?;
        Ok(row)
    }

    /// Read `server_zones` for zones that have a map-server endpoint
    /// configured. Returns `(zone_id, server_ip, server_port)` tuples.
    pub async fn get_server_zones(&self) -> Result<Vec<(u32, String, u16)>> {
        let rows = self.conn
            .call_db(|c| {
                let mut stmt = c.prepare(
                    r"SELECT id, serverIp, serverPort
                      FROM server_zones
                      WHERE serverIp IS NOT NULL AND serverIp <> '' AND serverPort > 0",
                )?;
                let rows: Vec<(u32, String, u16)> = stmt
                    .query_map([], |r| {
                        Ok((
                            r.get::<_, u32>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, u16>(2)?,
                        ))
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    pub async fn get_all_chara_names(&self) -> Result<Vec<(u32, String)>> {
        let rows = self.conn
            .call_db(|c| {
                let mut stmt = c.prepare("SELECT id, name FROM characters")?;
                let rows: Vec<(u32, String)> = stmt
                    .query_map([], |r| Ok((r.get::<_, u32>(0)?, r.get::<_, String>(1)?)))?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    pub async fn current_zone_for_session(&self, chara_id: u32) -> Result<u32> {
        let row = self.conn
            .call_db(move |c| {
                let v = c
                    .query_row(
                        "SELECT currentZoneId, destinationZoneId FROM characters WHERE id = :cid",
                        named_params! { ":cid": chara_id },
                        |r| Ok((r.get::<_, u32>(0)?, r.get::<_, u32>(1)?)),
                    )
                    .optional()?;
                Ok(v)
            })
            .await?;
        Ok(match row {
            Some((cur, dest)) if cur == 0 && dest != 0 => dest,
            Some((cur, dest)) if cur != 0 && dest == 0 => cur,
            _ => 0,
        })
    }

    pub async fn get_retainers(&self, chara_id: u32) -> Result<Vec<RetainerGroupMember>> {
        let rows = self.conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    r"SELECT sr.id, sr.name, sr.actorClassId, sr.cdIDOffset,
                             sr.placeName, sr.conditions, sr.level
                      FROM server_retainers sr
                      INNER JOIN characters_retainers cr ON cr.retainerId = sr.id
                      WHERE cr.characterId = :cid",
                )?;
                let rows: Vec<RetainerGroupMember> = stmt
                    .query_map(named_params! { ":cid": chara_id }, |r| {
                        let id = r.get::<_, u32>(0)? | 0xE000_0000;
                        Ok(RetainerGroupMember::new(
                            id,
                            r.get::<_, String>(1)?,
                            r.get::<_, u32>(2)?,
                            r.get::<_, u8>(3)?,
                            r.get::<_, u16>(4)?,
                            r.get::<_, u8>(5)?,
                            r.get::<_, u8>(6)?,
                        ))
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    pub async fn get_linkshell_by_name(
        &self,
        group_index: u64,
        ls_name: &str,
    ) -> Result<Option<Linkshell>> {
        let ls_name = ls_name.to_owned();
        let row = self.conn
            .call_db(move |c| {
                let v = c
                    .query_row(
                        "SELECT id, name, crestIcon, master FROM server_linkshells WHERE name = :n",
                        named_params! { ":n": ls_name },
                        |r| {
                            Ok((
                                r.get::<_, u64>(0)?,
                                r.get::<_, String>(1)?,
                                r.get::<_, u16>(2)?,
                                r.get::<_, u32>(3)?,
                            ))
                        },
                    )
                    .optional()?;
                Ok(v)
            })
            .await?;
        Ok(row.map(|(id, name, crest, master)| {
            Linkshell::new(id, group_index, name, crest, master, Linkshell::RANK_MASTER)
        }))
    }

    pub async fn get_linkshell_by_id(
        &self,
        group_index: u64,
        ls_id: u64,
    ) -> Result<Option<Linkshell>> {
        let row = self.conn
            .call_db(move |c| {
                let v = c
                    .query_row(
                        "SELECT name, crestIcon, master FROM server_linkshells WHERE id = :i",
                        named_params! { ":i": ls_id as i64 },
                        |r| {
                            Ok((
                                r.get::<_, String>(0)?,
                                r.get::<_, u16>(1)?,
                                r.get::<_, u32>(2)?,
                            ))
                        },
                    )
                    .optional()?;
                Ok(v)
            })
            .await?;
        Ok(row.map(|(name, crest, master)| {
            Linkshell::new(ls_id, group_index, name, crest, master, Linkshell::RANK_MASTER)
        }))
    }

    pub async fn get_ls_members(&self, ls_id: u64) -> Result<Vec<LinkshellMember>> {
        let rows = self.conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    "SELECT characterId, linkshellId, rank FROM characters_linkshells WHERE linkshellId = :i",
                )?;
                let mut rows: Vec<LinkshellMember> = stmt
                    .query_map(named_params! { ":i": ls_id as i64 }, |r| {
                        Ok(LinkshellMember {
                            character_id: r.get::<_, u32>(0)?,
                            linkshell_id: r.get::<_, u64>(1)?,
                            rank: r.get::<_, u8>(2)?,
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                rows.sort_by_key(|m| m.character_id);
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    pub async fn get_player_ls_membership(&self, chara_id: u32) -> Result<Vec<LinkshellMember>> {
        let rows = self.conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    "SELECT characterId, linkshellId, rank FROM characters_linkshells WHERE characterId = :cid",
                )?;
                let rows: Vec<LinkshellMember> = stmt
                    .query_map(named_params! { ":cid": chara_id }, |r| {
                        Ok(LinkshellMember {
                            character_id: r.get::<_, u32>(0)?,
                            linkshell_id: r.get::<_, u64>(1)?,
                            rank: r.get::<_, u8>(2)?,
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    pub async fn create_linkshell(&self, name: &str, crest: u16, master: u32) -> Result<u64> {
        let name = name.to_owned();
        let id = self.conn
            .call_db(move |c| {
                c.execute(
                    r"INSERT INTO server_linkshells (name, crestIcon, master, rank)
                      VALUES (:n, :c, :m, :r)",
                    named_params! {
                        ":n": name,
                        ":c": crest,
                        ":m": master,
                        ":r": Linkshell::RANK_MASTER,
                    },
                )?;
                Ok(c.last_insert_rowid() as u64)
            })
            .await?;
        Ok(id)
    }

    pub async fn linkshell_add_player(
        &self,
        ls_id: u64,
        chara_id: u32,
        rank: u8,
    ) -> Result<()> {
        self.conn
            .call_db(move |c| {
                c.execute(
                    r"INSERT INTO characters_linkshells(characterId, linkshellId, rank)
                      VALUES(:c, :l, :r)
                      ON CONFLICT(characterId, linkshellId) DO UPDATE SET rank = excluded.rank",
                    named_params! { ":c": chara_id, ":l": ls_id as i64, ":r": rank },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn linkshell_remove_player(&self, ls_id: u64, chara_id: u32) -> Result<()> {
        self.conn
            .call_db(move |c| {
                c.execute(
                    r"DELETE FROM characters_linkshells
                      WHERE characterId = :c AND linkshellId = :l",
                    named_params! { ":c": chara_id, ":l": ls_id as i64 },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn change_linkshell_crest(&self, ls_id: u64, crest: u16) -> Result<()> {
        self.conn
            .call_db(move |c| {
                c.execute(
                    "UPDATE server_linkshells SET crestIcon = :c WHERE id = :l",
                    named_params! { ":c": crest, ":l": ls_id as i64 },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn linkshell_change_rank(
        &self,
        ls_id: u64,
        chara_id: u32,
        rank: u8,
    ) -> Result<()> {
        self.conn
            .call_db(move |c| {
                c.execute(
                    r"UPDATE characters_linkshells SET rank = :r
                      WHERE characterId = :c AND linkshellId = :l",
                    named_params! { ":r": rank, ":c": chara_id, ":l": ls_id as i64 },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn character_id_by_name(&self, name: &str) -> Result<Option<u32>> {
        let name = name.to_owned();
        let id = self
            .conn
            .call_db(move |c| {
                let v: Option<u32> = c
                    .query_row(
                        "SELECT id FROM characters WHERE name = :n",
                        named_params! { ":n": name },
                        |r| r.get(0),
                    )
                    .optional()?;
                Ok(v)
            })
            .await?;
        Ok(id)
    }

    pub async fn set_active_ls(&self, chara_id: u32, name: &str) -> Result<()> {
        let name = name.to_owned();
        self.conn
            .call_db(move |c| {
                c.execute(
                    "UPDATE characters SET currentActiveLinkshell = :n WHERE id = :c",
                    named_params! { ":n": name, ":c": chara_id },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn linkshell_exists(&self, name: &str) -> Result<bool> {
        let name = name.to_owned();
        let v = self.conn
            .call_db(move |c| {
                let row: Option<i64> = c
                    .query_row(
                        "SELECT id FROM server_linkshells WHERE name = :n",
                        named_params! { ":n": name },
                        |r| r.get(0),
                    )
                    .optional()?;
                Ok(row.is_some())
            })
            .await?;
        Ok(v)
    }
}

#[derive(Debug, Clone, Default)]
pub struct SessionDbSnapshot {
    pub character_name: String,
    pub current_zone_id: u32,
    pub destination_zone_id: u32,
    pub active_linkshell: String,
}
