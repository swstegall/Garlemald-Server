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

//! `MonsterParty` — NPC-only party used for linked mob groups. Port of
//! `MonsterParty.cs`. Simpler than `Party`: no leader, no work struct,
//! just a member list.

#![allow(dead_code)]

use super::outbox::{GroupEvent, GroupOutbox};
use super::types::{GroupKind, GroupMemberRef, GroupTypeId};

#[derive(Debug, Clone)]
pub struct MonsterParty {
    pub group_id: u64,
    pub members: Vec<u32>,
}

impl MonsterParty {
    pub fn new(group_id: u64, initial_members: &[u32], outbox: &mut GroupOutbox) -> Self {
        outbox.push(GroupEvent::GroupCreated {
            group_id,
            kind: GroupKind::Monster,
            type_id: GroupTypeId::MONSTER_PARTY,
        });
        for &m in initial_members {
            outbox.push(GroupEvent::MemberAdded {
                group_id,
                kind: GroupKind::Monster,
                actor_id: m,
                is_leader: false,
            });
        }
        Self {
            group_id,
            members: initial_members.to_vec(),
        }
    }

    pub fn add_member(&mut self, actor_id: u32, outbox: &mut GroupOutbox) {
        if self.members.contains(&actor_id) {
            return;
        }
        self.members.push(actor_id);
        outbox.push(GroupEvent::MemberAdded {
            group_id: self.group_id,
            kind: GroupKind::Monster,
            actor_id,
            is_leader: false,
        });
    }

    pub fn remove_member(&mut self, actor_id: u32, outbox: &mut GroupOutbox) {
        if let Some(i) = self.members.iter().position(|id| *id == actor_id) {
            self.members.remove(i);
            outbox.push(GroupEvent::MemberRemoved {
                group_id: self.group_id,
                kind: GroupKind::Monster,
                actor_id,
            });
        }
    }

    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    pub fn contains(&self, actor_id: u32) -> bool {
        self.members.contains(&actor_id)
    }

    pub fn build_member_list(&self) -> Vec<GroupMemberRef> {
        self.members
            .iter()
            .map(|&id| GroupMemberRef::new(id, true, ""))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_and_remove() {
        let mut ob = GroupOutbox::new();
        let mut mp = MonsterParty::new(1, &[10, 11, 12], &mut ob);
        assert_eq!(mp.member_count(), 3);
        ob.drain();
        mp.remove_member(11, &mut ob);
        assert!(!mp.contains(11));
        assert_eq!(mp.member_count(), 2);
    }

    #[test]
    fn member_list_has_no_names() {
        let mut ob = GroupOutbox::new();
        let mp = MonsterParty::new(1, &[10, 11], &mut ob);
        let list = mp.build_member_list();
        assert!(list.iter().all(|m| m.name.is_empty()));
    }
}
