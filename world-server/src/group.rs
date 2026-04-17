//! Group hierarchy ported from World Server/DataObjects/Group.
//!
//! The original C# has a class-inheritance hierarchy (Group → Party/Linkshell/…).
//! Rust doesn't have inheritance, so we model the shared data on every group
//! kind (id, members) as a `struct Group` that specialized kinds compose.

#![allow(dead_code)]

use std::sync::atomic::{AtomicU64, Ordering};

pub const PARTY_TYPE: u16 = 0x2711;
pub const LINKSHELL_TYPE: u16 = 0x2712;
pub const RETAINER_TYPE: u16 = 0x2713;
pub const RELATION_TYPE: u16 = 0x2714;

/// Monotonic group-id generator. Each live group instance gets a unique id in
/// the `0xA000_0000` space (bit 63/62 flags match the C# allocator shape).
static NEXT_GROUP_ID: AtomicU64 = AtomicU64::new(0xA000_0000);

pub fn alloc_group_id() -> u64 {
    NEXT_GROUP_ID.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug, Clone)]
pub struct GroupHeader {
    pub group_id: u64,
    pub group_type: u16,
    pub max_members: u16,
}

// ---------------------------------------------------------------------------
// Party
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Party {
    pub header: GroupHeader,
    pub owner: u32,
    pub leader: u32,
    pub members: Vec<u32>,
}

impl Party {
    pub const MAX_MEMBERS: u16 = 8;

    pub fn new(owner: u32) -> Self {
        Self {
            header: GroupHeader {
                group_id: alloc_group_id(),
                group_type: PARTY_TYPE,
                max_members: Self::MAX_MEMBERS,
            },
            owner,
            leader: owner,
            members: vec![owner],
        }
    }

    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    pub fn add_member(&mut self, session_id: u32) {
        if !self.members.contains(&session_id) {
            self.members.push(session_id);
        }
    }

    pub fn remove_member(&mut self, session_id: u32) -> bool {
        if let Some(idx) = self.members.iter().position(|&id| id == session_id) {
            self.members.remove(idx);
            if self.leader == session_id {
                self.leader = self.members.first().copied().unwrap_or(0);
            }
            true
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Linkshell
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LinkshellMember {
    pub character_id: u32,
    pub linkshell_id: u64,
    pub rank: u8,
}

#[derive(Debug, Clone)]
pub struct Linkshell {
    pub header: GroupHeader,
    /// DB primary key of the linkshell row (distinct from `group_id`, which
    /// is the session-local index).
    pub db_id: u64,
    pub name: String,
    pub crest: u16,
    pub master: u32,
    pub rank: u8,
    pub members: Vec<LinkshellMember>,
}

impl Linkshell {
    pub const MAX_MEMBERS: u16 = 128;
    pub const RANK_MASTER: u8 = 0x0A;
    pub const RANK_LEADER: u8 = 0x02;
    pub const RANK_MEMBER: u8 = 0x01;

    pub fn new(db_id: u64, group_id: u64, name: String, crest: u16, master: u32, rank: u8) -> Self {
        Self {
            header: GroupHeader {
                group_id,
                group_type: LINKSHELL_TYPE,
                max_members: Self::MAX_MEMBERS,
            },
            db_id,
            name,
            crest,
            master,
            rank,
            members: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Retainer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RetainerGroupMember {
    pub id: u32,
    pub name: String,
    pub actor_class_id: u32,
    pub cd_id_offset: u8,
    pub place_name: u16,
    pub conditions: u8,
    pub level: u8,
}

impl RetainerGroupMember {
    pub fn new(
        id: u32,
        name: String,
        actor_class_id: u32,
        cd_id_offset: u8,
        place_name: u16,
        conditions: u8,
        level: u8,
    ) -> Self {
        Self { id, name, actor_class_id, cd_id_offset, place_name, conditions, level }
    }
}

#[derive(Debug, Clone)]
pub struct RetainerGroup {
    pub header: GroupHeader,
    pub owner: u32,
    pub members: Vec<RetainerGroupMember>,
}

impl RetainerGroup {
    pub const MAX_MEMBERS: u16 = 12;

    pub fn new(owner: u32, members: Vec<RetainerGroupMember>) -> Self {
        Self {
            header: GroupHeader {
                group_id: alloc_group_id(),
                group_type: RETAINER_TYPE,
                max_members: Self::MAX_MEMBERS,
            },
            owner,
            members,
        }
    }
}

// ---------------------------------------------------------------------------
// Relation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Relation {
    pub header: GroupHeader,
    pub owner: u32,
    pub partner: u32,
}

impl Relation {
    pub fn new(owner: u32, partner: u32) -> Self {
        Self {
            header: GroupHeader {
                group_id: alloc_group_id(),
                group_type: RELATION_TYPE,
                max_members: 2,
            },
            owner,
            partner,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn party_add_and_remove() {
        let mut p = Party::new(1);
        assert_eq!(p.member_count(), 1);
        p.add_member(2);
        p.add_member(3);
        assert_eq!(p.member_count(), 3);
        assert!(p.remove_member(1));
        assert_eq!(p.leader, 2);
        assert_eq!(p.member_count(), 2);
    }

    #[test]
    fn group_ids_are_unique() {
        let a = alloc_group_id();
        let b = alloc_group_id();
        assert!(a < b);
    }
}
