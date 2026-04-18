//! `ContentGroup` + `GLContentGroup` — roster for a Director's
//! instance. Port of `ContentGroup.cs` + `GLContentGroup.cs` +
//! `ContentGroupWork.cs`.

#![allow(dead_code)]

use super::outbox::{GroupEvent, GroupOutbox};
use super::types::{GroupKind, GroupMemberRef, GroupTypeId};

/// `ContentGroupWork._globalTemp.director` — the upper 32 bits hold the
/// Director's composite actor id, the low 32 are zero in retail.
pub fn pack_director(director_actor_id: u32) -> u64 {
    (director_actor_id as u64) << 32
}

pub fn unpack_director(packed: u64) -> u32 {
    ((packed >> 32) & 0xFFFF_FFFF) as u32
}

#[derive(Debug, Clone, Default)]
pub struct ContentGroupWork {
    pub director_packed: u64,
    pub property: [bool; 32],
}

/// Instance roster. Tied to a `Director` via `director_actor_id`.
#[derive(Debug, Clone)]
pub struct ContentGroup {
    pub group_id: u64,
    pub director_actor_id: u32,
    pub members: Vec<u32>,
    pub work: ContentGroupWork,
    pub is_started: bool,
    /// Defaults to `CONTENT_SIMPLE_24B`, matching the C# base. Subtypes
    /// (Guildleve, PublicPop, etc.) set their own.
    pub type_id: GroupTypeId,
}

impl ContentGroup {
    /// `ContentGroup(groupIndex, director, initialMembers)`.
    pub fn new(
        group_id: u64,
        director_actor_id: u32,
        initial_members: &[u32],
        outbox: &mut GroupOutbox,
    ) -> Self {
        let work = ContentGroupWork {
            director_packed: pack_director(director_actor_id),
            property: [false; 32],
        };
        let me = Self {
            group_id,
            director_actor_id,
            members: initial_members.to_vec(),
            work,
            is_started: false,
            type_id: GroupTypeId::CONTENT_SIMPLE_24B,
        };
        outbox.push(GroupEvent::GroupCreated {
            group_id,
            kind: GroupKind::Content,
            type_id: me.type_id,
        });
        for &m in initial_members {
            outbox.push(GroupEvent::MemberAdded {
                group_id,
                kind: GroupKind::Content,
                actor_id: m,
                is_leader: false,
            });
        }
        me
    }

    pub fn set_type_id(&mut self, id: GroupTypeId) {
        self.type_id = id;
    }

    /// `Start()` — unlocks broadcast. Before this, member mutations
    /// don't fan out to clients.
    pub fn start(&mut self, outbox: &mut GroupOutbox) {
        self.is_started = true;
        outbox.push(GroupEvent::SynchWorkValues {
            group_id: self.group_id,
            kind: GroupKind::Content,
        });
    }

    pub fn is_started(&self) -> bool {
        self.is_started
    }

    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    pub fn contains(&self, actor_id: u32) -> bool {
        self.members.contains(&actor_id)
    }

    /// `AddMember(actor)` — idempotent. Once `started`, the dispatcher
    /// re-broadcasts the full roster to all members.
    pub fn add_member(&mut self, actor_id: u32, outbox: &mut GroupOutbox) {
        if self.members.contains(&actor_id) {
            return;
        }
        self.members.push(actor_id);
        outbox.push(GroupEvent::MemberAdded {
            group_id: self.group_id,
            kind: GroupKind::Content,
            actor_id,
            is_leader: false,
        });
    }

    /// `RemoveMember(memberId)`.
    pub fn remove_member(&mut self, actor_id: u32, outbox: &mut GroupOutbox) {
        if let Some(idx) = self.members.iter().position(|id| *id == actor_id) {
            self.members.remove(idx);
            outbox.push(GroupEvent::MemberRemoved {
                group_id: self.group_id,
                kind: GroupKind::Content,
                actor_id,
            });
        }
    }

    /// `CheckDestroy()` — caller passes a closure that reports whether
    /// each member id has a live session. If none do, emit an auto-
    /// delete event.
    pub fn check_destroy(&self, is_online: impl Fn(u32) -> bool, outbox: &mut GroupOutbox) {
        if self.members.iter().copied().any(is_online) {
            return;
        }
        outbox.push(GroupEvent::ContentGroupAutoDelete {
            group_id: self.group_id,
        });
    }

    /// `DeleteGroup()` — explicit teardown. Emits a `GroupDeleted`
    /// event + clears the member list.
    pub fn delete(&mut self, outbox: &mut GroupOutbox) {
        outbox.push(GroupEvent::GroupDeleted {
            group_id: self.group_id,
            kind: GroupKind::Content,
            former_members: std::mem::take(&mut self.members),
        });
    }

    /// `BuildMemberList(id)` — the content variant omits names (the
    /// client is already showing them via other packets).
    pub fn build_member_list(&self, requester_actor_id: u32) -> Vec<GroupMemberRef> {
        let mut out = Vec::with_capacity(self.members.len());
        out.push(GroupMemberRef::new(requester_actor_id, true, ""));
        for &id in &self.members {
            if id != requester_actor_id {
                out.push(GroupMemberRef::new(id, true, ""));
            }
        }
        out
    }
}

/// Thin marker — `GLContentGroup` is a ContentGroup whose `type_id`
/// is `CONTENT_GUILDLEVE`. Retail uses it to tell the client "this
/// instance is a guildleve."
pub fn new_guildleve_content_group(
    group_id: u64,
    director_actor_id: u32,
    initial_members: &[u32],
    outbox: &mut GroupOutbox,
) -> ContentGroup {
    let mut cg = ContentGroup::new(group_id, director_actor_id, initial_members, outbox);
    cg.set_type_id(GroupTypeId::CONTENT_GUILDLEVE);
    cg
}

/// Re-export under the old C# name so call sites read naturally.
pub type GLContentGroup = ContentGroup;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn director_pack_round_trip() {
        let p = pack_director(0x6000_0001);
        assert_eq!(unpack_director(p), 0x6000_0001);
        assert_eq!(p & 0xFFFF_FFFF, 0);
    }

    #[test]
    fn new_content_group_includes_initial_members() {
        let mut ob = GroupOutbox::new();
        let cg = ContentGroup::new(100, 0x6000_0001, &[0xA000_0001, 0xA000_0002], &mut ob);
        assert_eq!(cg.member_count(), 2);
        assert!(cg.contains(0xA000_0001));
        let added = ob
            .events
            .iter()
            .filter(|e| matches!(e, GroupEvent::MemberAdded { .. }))
            .count();
        assert_eq!(added, 2);
    }

    #[test]
    fn add_member_idempotent() {
        let mut ob = GroupOutbox::new();
        let mut cg = ContentGroup::new(1, 100, &[], &mut ob);
        ob.drain();
        cg.add_member(200, &mut ob);
        cg.add_member(200, &mut ob);
        assert_eq!(cg.member_count(), 1);
        let added = ob
            .events
            .iter()
            .filter(|e| matches!(e, GroupEvent::MemberAdded { .. }))
            .count();
        assert_eq!(added, 1);
    }

    #[test]
    fn check_destroy_fires_autodelete_when_empty_of_sessions() {
        let mut ob = GroupOutbox::new();
        let cg = ContentGroup::new(1, 100, &[200, 201], &mut ob);
        ob.drain();
        cg.check_destroy(|_| false, &mut ob);
        assert!(
            ob.events
                .iter()
                .any(|e| matches!(e, GroupEvent::ContentGroupAutoDelete { .. }))
        );
    }

    #[test]
    fn check_destroy_noop_when_any_online() {
        let mut ob = GroupOutbox::new();
        let cg = ContentGroup::new(1, 100, &[200, 201], &mut ob);
        ob.drain();
        cg.check_destroy(|id| id == 201, &mut ob);
        assert!(ob.is_empty());
    }

    #[test]
    fn delete_emits_group_deleted_and_clears_members() {
        let mut ob = GroupOutbox::new();
        let mut cg = ContentGroup::new(1, 100, &[200, 201], &mut ob);
        ob.drain();
        cg.delete(&mut ob);
        assert!(cg.members.is_empty());
        let evt = ob
            .events
            .iter()
            .find(|e| matches!(e, GroupEvent::GroupDeleted { .. }))
            .unwrap();
        match evt {
            GroupEvent::GroupDeleted { former_members, .. } => {
                assert_eq!(former_members.len(), 2);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn guildleve_content_group_sets_type_id() {
        let mut ob = GroupOutbox::new();
        let cg = new_guildleve_content_group(1, 100, &[200], &mut ob);
        assert_eq!(cg.type_id, GroupTypeId::CONTENT_GUILDLEVE);
    }
}
