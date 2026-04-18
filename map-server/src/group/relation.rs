//! `RelationGroup`, `TradeGroup`, `RetainerMeetingRelationGroup` —
//! two-member bindings. Port of `RelationGroup.cs`, `TradeGroup.cs`,
//! `RetainerMeetingRelationGroup.cs`.
//!
//! All three share the same wire-state layout: a host actor id (packed
//! with a magic marker) + an "other" actor id + a `variableCommand`
//! selector + a `topicGroup` cross-reference.

#![allow(dead_code)]

use super::outbox::{GroupEvent, GroupOutbox};
use super::types::{GroupKind, GroupMemberRef, GroupTypeId, RELATION_COMMAND_TRADE};

/// Marker the C# stamps into the low bits of `host` (`0xC17909`).
pub const RELATION_HOST_MARKER: u64 = 0x00C1_7909;

pub fn pack_host(actor_id: u32) -> u64 {
    ((actor_id as u64) << 32) | RELATION_HOST_MARKER
}

pub fn unpack_host(packed: u64) -> u32 {
    ((packed >> 32) & 0xFFFF_FFFF) as u32
}

#[derive(Debug, Clone, Default)]
pub struct RelationWork {
    pub host_packed: u64,
    pub variable_command: u32,
}

/// Shared body for the three relation-like types.
#[derive(Debug, Clone)]
struct RelationBody {
    group_id: u64,
    other_actor_id: u32,
    topic_group_id: u64,
    work: RelationWork,
}

impl RelationBody {
    fn new(group_id: u64, host: u32, other: u32, command: u32, topic_group_id: u64) -> Self {
        Self {
            group_id,
            other_actor_id: other,
            topic_group_id,
            work: RelationWork {
                host_packed: pack_host(host),
                variable_command: command,
            },
        }
    }

    fn host(&self) -> u32 {
        unpack_host(self.work.host_packed)
    }
}

// ---------------------------------------------------------------------------
// RelationGroup — party-invitation + similar bindings.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RelationGroup {
    body: RelationBody,
}

impl RelationGroup {
    /// `RelationGroup(groupIndex, host, other, command, topicGroup)`.
    pub fn new(
        group_id: u64,
        host: u32,
        other: u32,
        command: u32,
        topic_group_id: u64,
        outbox: &mut GroupOutbox,
    ) -> Self {
        let body = RelationBody::new(group_id, host, other, command, topic_group_id);
        outbox.push(GroupEvent::GroupCreated {
            group_id,
            kind: GroupKind::Relation,
            type_id: GroupTypeId::GROUP_INVITATION,
        });
        Self { body }
    }

    pub fn group_id(&self) -> u64 {
        self.body.group_id
    }

    pub fn host(&self) -> u32 {
        self.body.host()
    }

    pub fn other(&self) -> u32 {
        self.body.other_actor_id
    }

    pub fn variable_command(&self) -> u32 {
        self.body.work.variable_command
    }

    pub fn topic_group(&self) -> u64 {
        self.body.topic_group_id
    }

    pub fn build_member_list(
        &self,
        name_lookup: impl Fn(u32) -> (String, bool),
    ) -> Vec<GroupMemberRef> {
        build_two_member_list(self.host(), self.body.other_actor_id, name_lookup)
    }

    pub fn delete(&mut self, outbox: &mut GroupOutbox) {
        outbox.push(GroupEvent::GroupDeleted {
            group_id: self.body.group_id,
            kind: GroupKind::Relation,
            former_members: vec![self.host(), self.body.other_actor_id],
        });
    }
}

// ---------------------------------------------------------------------------
// TradeGroup — relation variant with type_id = Trade and command = 30001.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TradeGroup {
    body: RelationBody,
}

impl TradeGroup {
    pub fn new(group_id: u64, host: u32, other: u32, outbox: &mut GroupOutbox) -> Self {
        // Trade groups hard-code `variableCommand = 30001` + no
        // `topicGroup` (the client doesn't read it in trade flow).
        let body = RelationBody::new(group_id, host, other, RELATION_COMMAND_TRADE, 0);
        outbox.push(GroupEvent::GroupCreated {
            group_id,
            kind: GroupKind::Trade,
            type_id: GroupTypeId::TRADE_RELATION,
        });
        Self { body }
    }

    pub fn group_id(&self) -> u64 {
        self.body.group_id
    }
    pub fn host(&self) -> u32 {
        self.body.host()
    }
    pub fn other(&self) -> u32 {
        self.body.other_actor_id
    }

    pub fn build_member_list(
        &self,
        name_lookup: impl Fn(u32) -> (String, bool),
    ) -> Vec<GroupMemberRef> {
        build_two_member_list(self.host(), self.body.other_actor_id, name_lookup)
    }

    pub fn delete(&mut self, outbox: &mut GroupOutbox) {
        outbox.push(GroupEvent::GroupDeleted {
            group_id: self.body.group_id,
            kind: GroupKind::Trade,
            former_members: vec![self.host(), self.body.other_actor_id],
        });
    }
}

// ---------------------------------------------------------------------------
// RetainerMeetingRelationGroup — player ↔ retainer pairing.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RetainerMeetingRelationGroup {
    pub group_id: u64,
    pub player_actor_id: u32,
    pub retainer_actor_id: u32,
}

impl RetainerMeetingRelationGroup {
    pub fn new(
        group_id: u64,
        player_actor_id: u32,
        retainer_actor_id: u32,
        outbox: &mut GroupOutbox,
    ) -> Self {
        outbox.push(GroupEvent::GroupCreated {
            group_id,
            kind: GroupKind::Retainer,
            type_id: GroupTypeId::RETAINER,
        });
        Self {
            group_id,
            player_actor_id,
            retainer_actor_id,
        }
    }

    pub fn build_member_list(
        &self,
        name_lookup: impl Fn(u32) -> (String, bool),
    ) -> Vec<GroupMemberRef> {
        let (pname, ponline) = name_lookup(self.player_actor_id);
        let (rname, ronline) = name_lookup(self.retainer_actor_id);
        // Retail stamps class_id = 0x83 on both members of a retainer
        // meeting group — the client reads it as "retainer context".
        let mut a = GroupMemberRef::new(self.player_actor_id, ponline, pname);
        a.class_id = 0x83;
        let mut b = GroupMemberRef::new(self.retainer_actor_id, ronline, rname);
        b.class_id = 0x83;
        vec![a, b]
    }
}

fn build_two_member_list(
    host: u32,
    other: u32,
    name_lookup: impl Fn(u32) -> (String, bool),
) -> Vec<GroupMemberRef> {
    let (hn, ho) = name_lookup(host);
    let (on, oo) = name_lookup(other);
    vec![
        GroupMemberRef::new(host, ho, hn),
        GroupMemberRef::new(other, oo, on),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(_: u32) -> (String, bool) {
        ("Foo".into(), true)
    }

    #[test]
    fn host_pack_round_trip() {
        let p = pack_host(0xA000_0001);
        assert_eq!(unpack_host(p), 0xA000_0001);
        assert_eq!(p & 0xFFFF_FFFF, RELATION_HOST_MARKER);
    }

    #[test]
    fn relation_group_basic() {
        let mut ob = GroupOutbox::new();
        let r = RelationGroup::new(42, 0xA000_0001, 0xA000_0002, 1234, 7, &mut ob);
        assert_eq!(r.host(), 0xA000_0001);
        assert_eq!(r.other(), 0xA000_0002);
        assert_eq!(r.variable_command(), 1234);
        assert_eq!(r.topic_group(), 7);
        let members = r.build_member_list(names);
        assert_eq!(members.len(), 2);
    }

    #[test]
    fn trade_group_hardcodes_command() {
        let mut ob = GroupOutbox::new();
        let t = TradeGroup::new(100, 1, 2, &mut ob);
        assert_eq!(t.host(), 1);
        assert_eq!(t.other(), 2);
        let created = ob
            .events
            .iter()
            .find(|e| matches!(e, GroupEvent::GroupCreated { .. }))
            .unwrap();
        match created {
            GroupEvent::GroupCreated { type_id, .. } => {
                assert_eq!(*type_id, GroupTypeId::TRADE_RELATION);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn retainer_meeting_group_uses_class_id_0x83() {
        let mut ob = GroupOutbox::new();
        let g = RetainerMeetingRelationGroup::new(100, 1, 2, &mut ob);
        let list = g.build_member_list(names);
        assert_eq!(list.len(), 2);
        assert!(list.iter().all(|m| m.class_id == 0x83));
    }

    #[test]
    fn delete_emits_deleted_event() {
        let mut ob = GroupOutbox::new();
        let mut r = RelationGroup::new(42, 1, 2, 0, 0, &mut ob);
        ob.drain();
        r.delete(&mut ob);
        assert!(
            ob.events
                .iter()
                .any(|e| matches!(e, GroupEvent::GroupDeleted { .. }))
        );
    }
}
