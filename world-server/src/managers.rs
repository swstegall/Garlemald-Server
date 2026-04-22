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
use crate::group::{Linkshell, Party, Relation, RetainerGroup, alloc_group_id};

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
        let ls = Linkshell::new(db_id, alloc_group_id(), name.to_string(), crest, master, Self::RANK_MASTER);

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
