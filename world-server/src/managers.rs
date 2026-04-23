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

//! Group/party/linkshell/retainer/relation managers.
//!
//! Ported from LinkshellManager.cs, PartyManager.cs, RelationGroupManager.cs,
//! RetainerGroupManager.cs. State is kept in-memory, behind a single async
//! `Mutex` per manager (cheap since these are low-traffic operations).
//!
//! Persistence happens through the `database` module when mutating state
//! that also lives in the DB (linkshell creation, rank changes, …).

#![allow(dead_code)]

use std::collections::HashMap;

use anyhow::Result;
use tokio::sync::Mutex;

use crate::database::Database;
use crate::group::{Linkshell, LinkshellMember, Party, Relation, RetainerGroup, alloc_group_id};

// ---------------------------------------------------------------------------
// PartyManager
// ---------------------------------------------------------------------------

pub struct PartyManager {
    /// Map session id → party. Every player is in at most one party.
    parties: Mutex<HashMap<u32, Party>>,
}

impl PartyManager {
    pub fn new() -> Self {
        Self { parties: Mutex::new(HashMap::new()) }
    }

    pub async fn get_party(&self, session_id: u32) -> Option<Party> {
        self.parties.lock().await.get(&session_id).cloned()
    }

    pub async fn ensure_party(&self, owner: u32) -> Party {
        let mut map = self.parties.lock().await;
        map.entry(owner).or_insert_with(|| Party::new(owner)).clone()
    }

    pub async fn add_member(&self, owner: u32, new_member: u32) {
        let mut map = self.parties.lock().await;
        if let Some(p) = map.get_mut(&owner) {
            p.add_member(new_member);
        }
    }

    pub async fn remove_member(&self, owner: u32, removed: u32) -> bool {
        let mut map = self.parties.lock().await;
        if let Some(p) = map.get_mut(&owner) {
            p.remove_member(removed)
        } else {
            false
        }
    }

    pub async fn disband(&self, owner: u32) {
        self.parties.lock().await.remove(&owner);
    }
}

impl Default for PartyManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// LinkshellManager
// ---------------------------------------------------------------------------

pub struct LinkshellManager {
    cache: Mutex<HashMap<u64, Linkshell>>,
    by_name: Mutex<HashMap<String, u64>>,
}

impl LinkshellManager {
    pub const RANK_MASTER: u8 = 0x0A;
    pub const RANK_LEADER: u8 = 0x02;
    pub const RANK_MEMBER: u8 = 0x01;

    pub fn new() -> Self {
        Self { cache: Mutex::new(HashMap::new()), by_name: Mutex::new(HashMap::new()) }
    }

    /// Verify a linkshell name can be created. Returns 0 on success; matching
    /// the C# error codes: 1 = name taken, 2 = banned, 3 = DB failure.
    pub async fn can_create_linkshell(&self, db: &Database, name: &str) -> i32 {
        if name.trim().is_empty() {
            return 2;
        }
        match db.linkshell_exists(name).await {
            Ok(true) => 1,
            Ok(false) => 0,
            Err(_) => 3,
        }
    }

    pub async fn create_linkshell(
        &self,
        db: &Database,
        name: &str,
        crest: u16,
        master: u32,
    ) -> Result<Option<Linkshell>> {
        let db_id = db.create_linkshell(name, crest, master).await?;
        if db_id == 0 {
            return Ok(None);
        }
        db.linkshell_add_player(db_id, master, Self::RANK_MASTER).await?;
        let mut ls = Linkshell::new(
            db_id,
            alloc_group_id(),
            name.to_string(),
            crest,
            master,
            Self::RANK_MASTER,
        );
        ls.members.push(LinkshellMember {
            character_id: master,
            linkshell_id: db_id,
            rank: Self::RANK_MASTER,
        });

        let mut by_name = self.by_name.lock().await;
        by_name.insert(name.to_string(), db_id);
        self.cache.lock().await.insert(db_id, ls.clone());

        Ok(Some(ls))
    }

    pub async fn delete_linkshell(&self, _name: &str) -> Result<()> {
        // TODO: map-server coordination needed before enabling destructive
        // delete; C# raised `NotImplementedException` here.
        Ok(())
    }

    pub async fn change_linkshell_crest(
        &self,
        db: &Database,
        ls_name: &str,
        crest: u16,
    ) -> Result<()> {
        let ls_id = {
            let by_name = self.by_name.lock().await;
            by_name.get(ls_name).copied()
        };
        if let Some(id) = ls_id {
            db.change_linkshell_crest(id, crest).await?;
            if let Some(ls) = self.cache.lock().await.get_mut(&id) {
                ls.crest = crest;
            }
        }
        Ok(())
    }

    pub async fn change_linkshell_master(
        &self,
        _db: &Database,
        _ls_name: &str,
        _master: u32,
    ) -> Result<()> {
        // The C# writes the new master ID directly on the cached object; DB
        // update is queued for a later patch. Mirror that shape.
        Ok(())
    }

    pub async fn get_linkshell(&self, name: &str) -> Option<Linkshell> {
        let id = self.by_name.lock().await.get(name).copied()?;
        self.cache.lock().await.get(&id).cloned()
    }

    /// Cache-through lookup: if `name` isn't in the in-memory map, hydrate
    /// from DB (including the member roster) so ops that landed via
    /// another process become visible.
    pub async fn get_or_load_linkshell(&self, db: &Database, name: &str) -> Result<Option<Linkshell>> {
        if let Some(ls) = self.get_linkshell(name).await {
            return Ok(Some(ls));
        }
        let Some(mut ls) = db.get_linkshell_by_name(alloc_group_id(), name).await? else {
            return Ok(None);
        };
        ls.members = db.get_ls_members(ls.db_id).await.unwrap_or_default();
        self.by_name.lock().await.insert(ls.name.clone(), ls.db_id);
        self.cache.lock().await.insert(ls.db_id, ls.clone());
        Ok(Some(ls))
    }

    /// Cheap clone of the cached `Linkshell` by id, for callers that
    /// need the member list (notification fan-out) without hitting
    /// the DB. Returns `None` if the id isn't in the cache —
    /// `get_or_load_linkshell(name)` is the warm-up path.
    pub async fn get_cached_by_id(&self, ls_id: u64) -> Option<Linkshell> {
        self.cache.lock().await.get(&ls_id).cloned()
    }

    pub async fn add_member(
        &self,
        db: &Database,
        ls_id: u64,
        chara_id: u32,
        rank: u8,
    ) -> Result<()> {
        db.linkshell_add_player(ls_id, chara_id, rank).await?;
        if let Some(ls) = self.cache.lock().await.get_mut(&ls_id) {
            ls.members.retain(|m| m.character_id != chara_id);
            ls.members.push(LinkshellMember {
                character_id: chara_id,
                linkshell_id: ls_id,
                rank,
            });
        }
        Ok(())
    }

    pub async fn remove_member(&self, db: &Database, ls_id: u64, chara_id: u32) -> Result<()> {
        db.linkshell_remove_player(ls_id, chara_id).await?;
        if let Some(ls) = self.cache.lock().await.get_mut(&ls_id) {
            ls.members.retain(|m| m.character_id != chara_id);
        }
        Ok(())
    }

    pub async fn change_rank(
        &self,
        db: &Database,
        ls_id: u64,
        chara_id: u32,
        rank: u8,
    ) -> Result<()> {
        db.linkshell_change_rank(ls_id, chara_id, rank).await?;
        if let Some(ls) = self.cache.lock().await.get_mut(&ls_id) {
            if let Some(m) = ls.members.iter_mut().find(|m| m.character_id == chara_id) {
                m.rank = rank;
            }
        }
        Ok(())
    }
}

impl Default for LinkshellManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// RelationGroupManager
// ---------------------------------------------------------------------------

pub struct RelationGroupManager {
    relations: Mutex<HashMap<u32, Relation>>,
}

impl RelationGroupManager {
    pub fn new() -> Self {
        Self { relations: Mutex::new(HashMap::new()) }
    }

    pub async fn create(&self, host: u32, guest: u32) -> Relation {
        let rel = Relation::new(host, guest);
        self.relations.lock().await.insert(host, rel.clone());
        rel
    }

    pub async fn remove(&self, host: u32) {
        self.relations.lock().await.remove(&host);
    }

    pub async fn get(&self, host: u32) -> Option<Relation> {
        self.relations.lock().await.get(&host).cloned()
    }
}

impl Default for RelationGroupManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// RetainerGroupManager
// ---------------------------------------------------------------------------

pub struct RetainerGroupManager {
    groups: Mutex<HashMap<u32, RetainerGroup>>,
}

impl RetainerGroupManager {
    pub fn new() -> Self {
        Self { groups: Mutex::new(HashMap::new()) }
    }

    pub async fn load_for_player(
        &self,
        db: &Database,
        chara_id: u32,
    ) -> Result<RetainerGroup> {
        let members = db.get_retainers(chara_id).await.unwrap_or_default();
        let group = RetainerGroup::new(chara_id, members);
        self.groups.lock().await.insert(chara_id, group.clone());
        Ok(group)
    }

    pub async fn get(&self, chara_id: u32) -> Option<RetainerGroup> {
        self.groups.lock().await.get(&chara_id).cloned()
    }
}

impl Default for RetainerGroupManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::named_params;

    fn tempdb() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("garlemald-ls-{nanos}-{seq}.db"))
    }

    async fn seed_character(db: &Database, id: u32, name: &str) {
        use common::db::ConnCallExt;
        let name = name.to_owned();
        db.conn_for_test()
            .call_db(move |c| {
                c.execute(
                    r"INSERT INTO characters (id, userId, slot, serverId, name)
                      VALUES (:i, 0, 0, 0, :n)",
                    named_params! { ":i": id, ":n": name },
                )?;
                Ok(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn create_seeds_master_into_cache_and_db() {
        let db = Database::open(tempdb()).await.unwrap();
        seed_character(&db, 1, "Master").await;
        let mgr = LinkshellManager::new();

        let ls = mgr
            .create_linkshell(&db, "ShellA", 0x1234, 1)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ls.members.len(), 1);
        assert_eq!(ls.members[0].character_id, 1);
        assert_eq!(ls.members[0].rank, LinkshellManager::RANK_MASTER);

        let from_cache = mgr.get_linkshell("ShellA").await.unwrap();
        assert_eq!(from_cache.members.len(), 1);

        let db_members = db.get_ls_members(ls.db_id).await.unwrap();
        assert_eq!(db_members.len(), 1);
    }

    #[tokio::test]
    async fn invite_leave_and_rank_round_trip() {
        let db = Database::open(tempdb()).await.unwrap();
        seed_character(&db, 1, "Master").await;
        seed_character(&db, 2, "Recruit").await;
        let mgr = LinkshellManager::new();

        let ls = mgr
            .create_linkshell(&db, "ShellB", 0x9999, 1)
            .await
            .unwrap()
            .unwrap();

        // Invite.
        mgr.add_member(&db, ls.db_id, 2, LinkshellManager::RANK_MEMBER)
            .await
            .unwrap();
        let after_invite = mgr.get_linkshell("ShellB").await.unwrap();
        assert_eq!(after_invite.members.len(), 2);

        // Promote.
        mgr.change_rank(&db, ls.db_id, 2, LinkshellManager::RANK_LEADER)
            .await
            .unwrap();
        let after_rank = mgr.get_linkshell("ShellB").await.unwrap();
        assert_eq!(
            after_rank
                .members
                .iter()
                .find(|m| m.character_id == 2)
                .unwrap()
                .rank,
            LinkshellManager::RANK_LEADER,
        );
        let db_rank = db.get_ls_members(ls.db_id).await.unwrap();
        assert_eq!(
            db_rank.iter().find(|m| m.character_id == 2).unwrap().rank,
            LinkshellManager::RANK_LEADER,
        );
        // Sanity: master's rank was not touched by the scoped UPDATE.
        assert_eq!(
            db_rank.iter().find(|m| m.character_id == 1).unwrap().rank,
            LinkshellManager::RANK_MASTER,
        );

        // Leave.
        mgr.remove_member(&db, ls.db_id, 2).await.unwrap();
        let after_leave = mgr.get_linkshell("ShellB").await.unwrap();
        assert_eq!(after_leave.members.len(), 1);
        assert!(
            db.get_ls_members(ls.db_id)
                .await
                .unwrap()
                .iter()
                .all(|m| m.character_id != 2),
        );
    }

    /// `get_cached_by_id` mirrors the by-name cache lookup. Returns
    /// the same `Linkshell` clone the by-name lookup yields; returns
    /// `None` for a never-loaded id without falling through to the DB.
    #[tokio::test]
    async fn get_cached_by_id_round_trips_after_create() {
        let db = Database::open(tempdb()).await.unwrap();
        seed_character(&db, 1, "Master").await;
        seed_character(&db, 2, "Recruit").await;
        let mgr = LinkshellManager::new();

        // Cold cache → None.
        assert!(mgr.get_cached_by_id(0xDEADBEEF).await.is_none());

        let ls = mgr
            .create_linkshell(&db, "ShellById", 0x4242, 1)
            .await
            .unwrap()
            .unwrap();
        mgr.add_member(&db, ls.db_id, 2, LinkshellManager::RANK_MEMBER)
            .await
            .unwrap();

        let by_id = mgr
            .get_cached_by_id(ls.db_id)
            .await
            .expect("just-created LS should be cached by id");
        assert_eq!(by_id.name, "ShellById");
        assert_eq!(by_id.members.len(), 2);

        let by_name = mgr.get_linkshell("ShellById").await.unwrap();
        assert_eq!(by_id.db_id, by_name.db_id);
        assert_eq!(by_id.members.len(), by_name.members.len());
    }

    #[tokio::test]
    async fn get_or_load_hydrates_missing_cache() {
        let db = Database::open(tempdb()).await.unwrap();
        seed_character(&db, 1, "Master").await;
        seed_character(&db, 2, "Recruit").await;

        // Populate via one manager, then read through a freshly-constructed
        // one — simulates the processor restarting with the DB intact.
        {
            let mgr = LinkshellManager::new();
            let ls = mgr
                .create_linkshell(&db, "ShellC", 0, 1)
                .await
                .unwrap()
                .unwrap();
            mgr.add_member(&db, ls.db_id, 2, LinkshellManager::RANK_MEMBER)
                .await
                .unwrap();
        }

        let mgr2 = LinkshellManager::new();
        assert!(mgr2.get_linkshell("ShellC").await.is_none());
        let ls = mgr2
            .get_or_load_linkshell(&db, "ShellC")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ls.members.len(), 2);
        assert!(mgr2.get_linkshell("ShellC").await.is_some());
    }
}
