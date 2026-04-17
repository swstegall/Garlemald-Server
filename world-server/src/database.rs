//! MySQL-backed world database queries. Ported from World Server/Database.cs.
//!
//! Several queries (linkshell member fetches, current-zone lookup) are
//! referenced by Phase-4 packet types that the Map Server will add; keep
//! them `#[allow(dead_code)]` for now so the API stays stable.
#![allow(dead_code)]

use anyhow::{Context, Result};
use mysql_async::{Pool, Row, prelude::*};

use crate::data::DBWorld;
use crate::group::{Linkshell, LinkshellMember, RetainerGroupMember};

pub struct Database {
    pool: Pool,
}

impl Database {
    pub fn new(url: &str) -> Result<Self> {
        let pool = Pool::from_url(url).context("parsing mysql url")?;
        Ok(Self { pool })
    }

    pub async fn ping(&self) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        "SELECT 1".ignore(&mut conn).await?;
        Ok(())
    }

    pub async fn get_server(&self, server_id: u32) -> Result<Option<DBWorld>> {
        let mut conn = self.pool.get_conn().await?;
        let row: Option<(String, String, u16)> =
            "SELECT name, address, port FROM servers WHERE id = :sid"
                .with(params! { "sid" => server_id })
                .first(&mut conn)
                .await?;
        Ok(row.map(|(name, address, port)| DBWorld {
            id: server_id,
            name,
            address,
            port,
            ..Default::default()
        }))
    }

    /// Populate `session` state from the `characters` row. Returns whether a
    /// row was found.
    pub async fn load_zone_session_info(
        &self,
        session_id: u32,
    ) -> Result<Option<SessionDbSnapshot>> {
        let mut conn = self.pool.get_conn().await?;
        let row: Option<Row> = r"SELECT name, currentZoneId, destinationZoneId,
                                         currentActiveLinkshell
                                  FROM characters WHERE id = :cid"
            .with(params! { "cid" => session_id })
            .first(&mut conn)
            .await?;

        Ok(row.map(|mut r| SessionDbSnapshot {
            character_name: r.take::<String, _>("name").unwrap_or_default(),
            current_zone_id: r.take::<u32, _>("currentZoneId").unwrap_or(0),
            destination_zone_id: r.take::<u32, _>("destinationZoneId").unwrap_or(0),
            active_linkshell: r.take::<String, _>("currentActiveLinkshell").unwrap_or_default(),
        }))
    }

    pub async fn get_all_chara_names(&self) -> Result<Vec<(u32, String)>> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<(u32, String)> = "SELECT id, name FROM characters"
            .with(())
            .map(&mut conn, |(id, name): (u32, String)| (id, name))
            .await?;
        Ok(rows)
    }

    pub async fn current_zone_for_session(&self, chara_id: u32) -> Result<u32> {
        let mut conn = self.pool.get_conn().await?;
        let row: Option<(u32, u32)> =
            "SELECT currentZoneId, destinationZoneId FROM characters WHERE id = :cid"
                .with(params! { "cid" => chara_id })
                .first(&mut conn)
                .await?;
        Ok(match row {
            Some((cur, dest)) if cur == 0 && dest != 0 => dest,
            Some((cur, dest)) if cur != 0 && dest == 0 => cur,
            _ => 0,
        })
    }

    pub async fn get_retainers(&self, chara_id: u32) -> Result<Vec<RetainerGroupMember>> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<Row> = r"SELECT id, name, actorClassId, cdIDOffset, placeName, conditions, level
            FROM server_retainers
            INNER JOIN characters_retainers ON retainerId = server_retainers.id
            WHERE characterId = :cid"
            .with(params! { "cid" => chara_id })
            .fetch(&mut conn)
            .await?;

        Ok(rows
            .into_iter()
            .map(|mut r| {
                let id = r.take::<u32, _>("id").unwrap_or(0) | 0xE000_0000;
                RetainerGroupMember::new(
                    id,
                    r.take::<String, _>("name").unwrap_or_default(),
                    r.take::<u32, _>("actorClassId").unwrap_or(0),
                    r.take::<u8, _>("cdIDOffset").unwrap_or(0),
                    r.take::<u16, _>("placeName").unwrap_or(0),
                    r.take::<u8, _>("conditions").unwrap_or(0),
                    r.take::<u8, _>("level").unwrap_or(0),
                )
            })
            .collect())
    }

    pub async fn get_linkshell_by_name(
        &self,
        group_index: u64,
        ls_name: &str,
    ) -> Result<Option<Linkshell>> {
        let mut conn = self.pool.get_conn().await?;
        let row: Option<(u64, String, u16, u32)> =
            "SELECT id, name, crestIcon, master FROM server_linkshells WHERE name = :n"
                .with(params! { "n" => ls_name })
                .first(&mut conn)
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
        let mut conn = self.pool.get_conn().await?;
        let row: Option<(String, u16, u32)> =
            "SELECT name, crestIcon, master FROM server_linkshells WHERE id = :i"
                .with(params! { "i" => ls_id })
                .first(&mut conn)
                .await?;
        Ok(row.map(|(name, crest, master)| {
            Linkshell::new(ls_id, group_index, name, crest, master, Linkshell::RANK_MASTER)
        }))
    }

    pub async fn get_ls_members(&self, ls_id: u64) -> Result<Vec<LinkshellMember>> {
        let mut conn = self.pool.get_conn().await?;
        let mut rows: Vec<LinkshellMember> =
            "SELECT characterId, linkshellId, rank FROM characters_linkshells WHERE linkshellId = :i"
                .with(params! { "i" => ls_id })
                .map(&mut conn, |(character_id, linkshell_id, rank): (u32, u64, u8)| {
                    LinkshellMember { character_id, linkshell_id, rank }
                })
                .await?;
        rows.sort_by_key(|m| m.character_id);
        Ok(rows)
    }

    pub async fn get_player_ls_membership(&self, chara_id: u32) -> Result<Vec<LinkshellMember>> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<LinkshellMember> =
            "SELECT characterId, linkshellId, rank FROM characters_linkshells WHERE characterId = :cid"
                .with(params! { "cid" => chara_id })
                .map(&mut conn, |(character_id, linkshell_id, rank): (u32, u64, u8)| {
                    LinkshellMember { character_id, linkshell_id, rank }
                })
                .await?;
        Ok(rows)
    }

    pub async fn create_linkshell(&self, name: &str, crest: u16, master: u32) -> Result<u64> {
        let mut conn = self.pool.get_conn().await?;
        r"INSERT INTO server_linkshells (name, crestIcon, master, rank)
          VALUES (:n, :c, :m, :r)"
            .with(params! {
                "n" => name,
                "c" => crest,
                "m" => master,
                "r" => Linkshell::RANK_MASTER,
            })
            .ignore(&mut conn)
            .await?;
        Ok(conn.last_insert_id().unwrap_or(0))
    }

    pub async fn linkshell_add_player(
        &self,
        ls_id: u64,
        chara_id: u32,
        rank: u8,
    ) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        r"INSERT INTO characters_linkshells(characterId, linkshellId, rank)
          VALUES(:c, :l, :r)"
            .with(params! { "c" => chara_id, "l" => ls_id, "r" => rank })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    pub async fn linkshell_remove_player(&self, ls_id: u64, chara_id: u32) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        r"DELETE FROM characters_linkshells
          WHERE characterId = :c AND linkshellId = :l"
            .with(params! { "c" => chara_id, "l" => ls_id })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    pub async fn change_linkshell_crest(&self, ls_id: u64, crest: u16) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        "UPDATE server_linkshells SET crestIcon = :c WHERE id = :l"
            .with(params! { "c" => crest, "l" => ls_id })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    pub async fn linkshell_change_rank(&self, chara_id: u32, rank: u8) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        "UPDATE characters_linkshells SET rank = :r WHERE characterId = :c"
            .with(params! { "r" => rank, "c" => chara_id })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    pub async fn set_active_ls(&self, chara_id: u32, name: &str) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        "UPDATE characters SET currentActiveLinkshell = :n WHERE id = :c"
            .with(params! { "n" => name, "c" => chara_id })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    pub async fn linkshell_exists(&self, name: &str) -> Result<bool> {
        let mut conn = self.pool.get_conn().await?;
        let row: Option<u64> = "SELECT id FROM server_linkshells WHERE name = :n"
            .with(params! { "n" => name })
            .first(&mut conn)
            .await?;
        Ok(row.is_some())
    }
}

#[derive(Debug, Clone, Default)]
pub struct SessionDbSnapshot {
    pub character_name: String,
    pub current_zone_id: u32,
    pub destination_zone_id: u32,
    pub active_linkshell: String,
}
