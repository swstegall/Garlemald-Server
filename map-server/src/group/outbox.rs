//! Events emitted by group mutations. The game-loop dispatcher
//! drains them and turns each into the right packet bundle.

#![allow(dead_code)]

use super::types::{GroupKind, GroupTypeId};

#[derive(Debug, Clone)]
pub enum GroupEvent {
    /// A new group was created. The dispatcher queues the full
    /// `Header → Begin → Members → End` sweep to every member's client.
    GroupCreated {
        group_id: u64,
        kind: GroupKind,
        type_id: GroupTypeId,
    },
    /// The group was disbanded. `SendDeletePacket` to every former
    /// member.
    GroupDeleted {
        group_id: u64,
        kind: GroupKind,
        former_members: Vec<u32>,
    },
    /// An actor joined. Dispatcher re-broadcasts the full roster to
    /// everyone still in the group.
    MemberAdded {
        group_id: u64,
        kind: GroupKind,
        actor_id: u32,
        is_leader: bool,
    },
    MemberRemoved {
        group_id: u64,
        kind: GroupKind,
        actor_id: u32,
    },
    /// Leader changed — client needs a fresh work-values sync so the
    /// UI marker moves.
    LeaderChanged {
        group_id: u64,
        new_leader_actor_id: u32,
    },
    /// Sync the group's work-struct values. Used on `_init` and after
    /// significant state changes (director id, host id, trade command).
    SynchWorkValues { group_id: u64, kind: GroupKind },
    /// `Server.NoMembersInParty(party)` — world manager hook to tear
    /// down a party that emptied out.
    PartyEmptied { group_id: u64 },
    /// `ContentGroup.CheckDestroy` decided nobody is still connected.
    ContentGroupAutoDelete { group_id: u64 },
}

#[derive(Debug, Default)]
pub struct GroupOutbox {
    pub events: Vec<GroupEvent>,
}

impl GroupOutbox {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, event: GroupEvent) {
        self.events.push(event);
    }

    pub fn drain(&mut self) -> Vec<GroupEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }
}
