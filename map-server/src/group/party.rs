//! `Party` — player group of up to 8. Port of `Party.cs` + `PartyWork.cs`.

#![allow(dead_code)]

use super::outbox::{GroupEvent, GroupOutbox};
use super::types::{GroupKind, GroupMemberRef, GroupTypeId, PARTY_MAX_MEMBERS};

/// Magic marker the C# stamps into the low bits of `owner` so the
/// client knows it's reading a party leader field. Ported verbatim.
pub const PARTY_OWNER_MARKER: u64 = 0x00B3_6F92;

/// `PartyWork._globalTemp.owner` packed layout:
/// `(leader_id << 32) | PARTY_OWNER_MARKER`.
pub fn pack_owner(leader_actor_id: u32) -> u64 {
    ((leader_actor_id as u64) << 32) | PARTY_OWNER_MARKER
}

pub fn unpack_leader(owner: u64) -> u32 {
    ((owner >> 32) & 0xFFFF_FFFF) as u32
}

/// Transient party state — matches `PartyWork.cs`. Only `owner` is
/// meaningful in 1.23b; the surrounding struct is here so the wire
/// property paths (`partyGroupWork._globalTemp.owner`) stay valid.
#[derive(Debug, Clone, Default)]
pub struct PartyWork {
    pub owner: u64,
}

#[derive(Debug, Clone)]
pub struct Party {
    pub group_id: u64,
    pub members: Vec<u32>,
    pub work: PartyWork,
}

impl Party {
    /// `Party(groupId, leaderCharaId)` — seeds the party with one member
    /// (the leader) and packs the owner u64.
    pub fn new(group_id: u64, leader_actor_id: u32, outbox: &mut GroupOutbox) -> Self {
        let me = Self {
            group_id,
            members: vec![leader_actor_id],
            work: PartyWork {
                owner: pack_owner(leader_actor_id),
            },
        };
        outbox.push(GroupEvent::GroupCreated {
            group_id,
            kind: GroupKind::Party,
            type_id: GroupTypeId::PARTY,
        });
        outbox.push(GroupEvent::MemberAdded {
            group_id,
            kind: GroupKind::Party,
            actor_id: leader_actor_id,
            is_leader: true,
        });
        me
    }

    pub fn leader(&self) -> u32 {
        unpack_leader(self.work.owner)
    }

    pub fn set_leader(&mut self, actor_id: u32, outbox: &mut GroupOutbox) {
        self.work.owner = pack_owner(actor_id);
        outbox.push(GroupEvent::LeaderChanged {
            group_id: self.group_id,
            new_leader_actor_id: actor_id,
        });
    }

    pub fn is_in_party(&self, actor_id: u32) -> bool {
        self.members.contains(&actor_id)
    }

    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    pub fn is_full(&self) -> bool {
        self.members.len() >= PARTY_MAX_MEMBERS
    }

    /// `AddMember(memberId)`. Rejects duplicates + full parties — the
    /// C# trusts its call sites; we're stricter.
    pub fn add_member(&mut self, actor_id: u32, outbox: &mut GroupOutbox) -> bool {
        if self.is_full() || self.members.contains(&actor_id) {
            return false;
        }
        self.members.push(actor_id);
        outbox.push(GroupEvent::MemberAdded {
            group_id: self.group_id,
            kind: GroupKind::Party,
            actor_id,
            is_leader: false,
        });
        true
    }

    /// `RemoveMember(memberId)`. If it empties the party, fires a
    /// `PartyEmptied` event so the world manager can disband.
    pub fn remove_member(&mut self, actor_id: u32, outbox: &mut GroupOutbox) -> bool {
        let Some(idx) = self.members.iter().position(|id| *id == actor_id) else {
            return false;
        };
        self.members.remove(idx);
        outbox.push(GroupEvent::MemberRemoved {
            group_id: self.group_id,
            kind: GroupKind::Party,
            actor_id,
        });
        // If the leader left, transfer to the next member.
        if self.leader() == actor_id && !self.members.is_empty() {
            self.set_leader(self.members[0], outbox);
        }
        if self.members.is_empty() {
            outbox.push(GroupEvent::PartyEmptied {
                group_id: self.group_id,
            });
        }
        true
    }

    /// Build the member-list for a specific requester. Matches the C#
    /// `BuildMemberList` (requester first, then everyone else).
    pub fn build_member_list(
        &self,
        requester_actor_id: u32,
        name_lookup: impl Fn(u32) -> String,
    ) -> Vec<GroupMemberRef> {
        let mut out = Vec::with_capacity(self.members.len());
        out.push(GroupMemberRef::new(
            requester_actor_id,
            true,
            name_lookup(requester_actor_id),
        ));
        for &id in &self.members {
            if id != requester_actor_id {
                out.push(GroupMemberRef::new(id, true, name_lookup(id)));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(_id: u32) -> String {
        "Foo".to_string()
    }

    #[test]
    fn owner_pack_round_trip() {
        let packed = pack_owner(0xA000_0001);
        assert_eq!(unpack_leader(packed), 0xA000_0001);
        assert_eq!(packed & 0xFFFF_FFFF, PARTY_OWNER_MARKER);
    }

    #[test]
    fn new_party_has_one_member_and_leader() {
        let mut ob = GroupOutbox::new();
        let p = Party::new(1, 0xA000_0001, &mut ob);
        assert_eq!(p.leader(), 0xA000_0001);
        assert_eq!(p.member_count(), 1);
        assert!(
            ob.events
                .iter()
                .any(|e| matches!(e, GroupEvent::GroupCreated { .. }))
        );
    }

    #[test]
    fn add_and_remove_member() {
        let mut ob = GroupOutbox::new();
        let mut p = Party::new(1, 100, &mut ob);
        ob.drain();
        assert!(p.add_member(200, &mut ob));
        assert_eq!(p.member_count(), 2);
        assert!(p.is_in_party(200));
        assert!(!p.add_member(200, &mut ob), "dupe rejected");
        assert!(p.remove_member(100, &mut ob));
        assert_eq!(p.leader(), 200, "leader auto-transferred");
    }

    #[test]
    fn removing_last_member_fires_empty_event() {
        let mut ob = GroupOutbox::new();
        let mut p = Party::new(1, 100, &mut ob);
        ob.drain();
        p.remove_member(100, &mut ob);
        assert!(
            ob.events
                .iter()
                .any(|e| matches!(e, GroupEvent::PartyEmptied { .. }))
        );
    }

    #[test]
    fn party_cap_enforced() {
        let mut ob = GroupOutbox::new();
        let mut p = Party::new(1, 1, &mut ob);
        for i in 2u32..=8 {
            assert!(p.add_member(i, &mut ob));
        }
        assert!(p.is_full());
        assert!(!p.add_member(9, &mut ob));
    }

    #[test]
    fn build_member_list_puts_requester_first() {
        let mut ob = GroupOutbox::new();
        let mut p = Party::new(1, 100, &mut ob);
        p.add_member(200, &mut ob);
        p.add_member(300, &mut ob);
        let list = p.build_member_list(200, name);
        assert_eq!(list[0].actor_id, 200);
        let ids: Vec<u32> = list.iter().map(|m| m.actor_id).collect();
        assert_eq!(ids.len(), 3);
    }
}
