//! Turn `GroupEvent`s into the chunked packet bundle the client expects.
//!
//! The C# `Group.SendGroupPackets` pumps `Header → Begin → MembersXnn…→
//! End` per session. We do the same here, routing through whichever
//! chunk size matches the remaining member count.

#![allow(dead_code)]

use crate::packets::send as tx;
use crate::runtime::actor_registry::ActorRegistry;
use crate::world_manager::WorldManager;

use super::outbox::GroupEvent;
use super::party::Party;
use super::types::{ChunkBucket, GroupKind, GroupMemberRef, GroupTypeId, chunk_bucket};

/// Dispatch one `GroupEvent`. `resolver` supplies the per-group
/// member list + type id so the dispatcher doesn't have to own the
/// group state itself (it lives on the world manager / registry).
pub async fn dispatch_group_event(
    event: &GroupEvent,
    registry: &ActorRegistry,
    world: &WorldManager,
    resolver: &dyn GroupResolver,
) {
    // Any roster mutation triggers a full re-broadcast to every
    // current member. The resolver supplies the live list.
    let roster_change_group_id = match event {
        GroupEvent::GroupCreated { group_id, .. }
        | GroupEvent::MemberAdded { group_id, .. }
        | GroupEvent::MemberRemoved { group_id, .. }
        | GroupEvent::LeaderChanged { group_id, .. } => Some(*group_id),
        _ => None,
    };
    if let Some(group_id) = roster_change_group_id {
        let Some(members) = resolver.members(group_id) else {
            return;
        };
        let kind = match event {
            GroupEvent::GroupCreated { kind, .. }
            | GroupEvent::MemberAdded { kind, .. }
            | GroupEvent::MemberRemoved { kind, .. } => *kind,
            _ => resolver.kind(group_id).unwrap_or(GroupKind::Party),
        };
        let resolved_type_id = match event {
            GroupEvent::GroupCreated { type_id, .. } => *type_id,
            _ => resolver.type_id(group_id).unwrap_or(GroupTypeId::NONE),
        };

        for recipient_actor_id in &members {
            broadcast_member_list(
                registry,
                world,
                resolver,
                *recipient_actor_id,
                group_id,
                kind,
                resolved_type_id,
            )
            .await;
        }
        return;
    }

    match event {
        GroupEvent::GroupCreated { .. }
        | GroupEvent::MemberAdded { .. }
        | GroupEvent::MemberRemoved { .. }
        | GroupEvent::LeaderChanged { .. } => {
            // Handled above.
        }
        GroupEvent::GroupDeleted {
            group_id,
            kind: _,
            former_members,
        } => {
            for actor_id in former_members {
                let Some(handle) = registry.get(*actor_id).await else {
                    continue;
                };
                let sub = tx::build_delete_group(handle.session_id, *group_id);
                if let Some(client) = world.client(handle.session_id).await {
                    client.send_bytes(sub.to_bytes()).await;
                }
            }
        }
        GroupEvent::SynchWorkValues { group_id, .. } => {
            // The retail SynchGroupWorkValues packet carries a work
            // struct serialised with a small property table. Phase 6
            // emits an empty blob; callers that need real property
            // updates push the Director/Guildleve path, which has its
            // own dispatcher.
            let Some(members) = resolver.members(*group_id) else {
                return;
            };
            for actor_id in members {
                let Some(handle) = registry.get(actor_id).await else {
                    continue;
                };
                let sub = tx::build_synch_group_work_values(handle.session_id, *group_id, &[]);
                if let Some(client) = world.client(handle.session_id).await {
                    client.send_bytes(sub.to_bytes()).await;
                }
            }
        }
        GroupEvent::PartyEmptied { .. } | GroupEvent::ContentGroupAutoDelete { .. } => {
            // Pure state-machine signals — the WorldManager is expected
            // to emit a matching `GroupDeleted` event when it sweeps.
        }
    }
}

/// Trait the dispatcher consults to resolve live group state. The
/// WorldManager (or a test harness) implements it to hand back the
/// authoritative roster.
pub trait GroupResolver: Send + Sync {
    fn members(&self, group_id: u64) -> Option<Vec<u32>>;
    fn kind(&self, group_id: u64) -> Option<GroupKind>;
    fn type_id(&self, group_id: u64) -> Option<GroupTypeId>;
    /// Look up a display name for a member actor id. Used when the
    /// wire packets include per-member names (party). Defaults to
    /// empty when the actor isn't resolvable.
    fn name_of(&self, actor_id: u32) -> String {
        let _ = actor_id;
        String::new()
    }
}

/// Single-party resolver — small wrapper the tests use to plug a live
/// `Party` into the dispatcher without going through WorldManager.
pub struct PartyResolver<'a> {
    pub party: &'a Party,
}

impl GroupResolver for PartyResolver<'_> {
    fn members(&self, group_id: u64) -> Option<Vec<u32>> {
        if self.party.group_id == group_id {
            Some(self.party.members.clone())
        } else {
            None
        }
    }
    fn kind(&self, group_id: u64) -> Option<GroupKind> {
        if self.party.group_id == group_id {
            Some(GroupKind::Party)
        } else {
            None
        }
    }
    fn type_id(&self, group_id: u64) -> Option<GroupTypeId> {
        if self.party.group_id == group_id {
            Some(GroupTypeId::PARTY)
        } else {
            None
        }
    }
}

async fn broadcast_member_list(
    registry: &ActorRegistry,
    world: &WorldManager,
    resolver: &dyn GroupResolver,
    recipient_actor_id: u32,
    group_id: u64,
    kind: GroupKind,
    type_id: GroupTypeId,
) {
    let Some(handle) = registry.get(recipient_actor_id).await else {
        return;
    };
    let session_id = handle.session_id;
    let Some(client) = world.client(session_id).await else {
        return;
    };
    let Some(members) = resolver.members(group_id) else {
        return;
    };
    let members_refs = build_member_refs(recipient_actor_id, &members, resolver);

    // 1. Header.
    let header = tx::build_group_header(
        session_id,
        group_id,
        type_id.bits() as u16,
        members_refs.len() as u16,
    );
    client.send_bytes(header.to_bytes()).await;
    // 2. Begin.
    let begin = tx::build_group_members_begin(session_id, group_id);
    client.send_bytes(begin.to_bytes()).await;
    // 3. Chunked body.
    let mut offset = 0usize;
    loop {
        let remaining = members_refs.len().saturating_sub(offset);
        let bucket = chunk_bucket(remaining);
        if matches!(bucket, ChunkBucket::None) {
            break;
        }
        let sub = match (kind, bucket) {
            (GroupKind::Party, ChunkBucket::X08) => {
                tx::build_group_members_x08(session_id, group_id, &members_refs, &mut offset)
            }
            (GroupKind::Party, ChunkBucket::X16) => {
                tx::build_group_members_x16(session_id, group_id, &members_refs, &mut offset)
            }
            (GroupKind::Party, ChunkBucket::X32) => {
                tx::build_group_members_x32(session_id, group_id, &members_refs, &mut offset)
            }
            (GroupKind::Party, ChunkBucket::X64) => {
                tx::build_group_members_x64(session_id, group_id, &members_refs, &mut offset)
            }
            (_, ChunkBucket::X08) => {
                tx::build_content_members_x08(session_id, group_id, &members_refs, &mut offset)
            }
            (_, ChunkBucket::X16) => {
                tx::build_content_members_x16(session_id, group_id, &members_refs, &mut offset)
            }
            (_, ChunkBucket::X32) => {
                tx::build_content_members_x32(session_id, group_id, &members_refs, &mut offset)
            }
            (_, ChunkBucket::X64) => {
                tx::build_content_members_x64(session_id, group_id, &members_refs, &mut offset)
            }
            _ => break,
        };
        client.send_bytes(sub.to_bytes()).await;
    }
    // 4. End.
    let end = tx::build_group_members_end(session_id, group_id);
    client.send_bytes(end.to_bytes()).await;
}

fn build_member_refs(
    requester: u32,
    members: &[u32],
    resolver: &dyn GroupResolver,
) -> Vec<tx::groups::GroupMember> {
    let mut out = Vec::with_capacity(members.len());
    // Requester first (matches the C# `BuildMemberList`).
    out.push(to_wire(&GroupMemberRef::new(
        requester,
        true,
        resolver.name_of(requester),
    )));
    for &id in members {
        if id != requester {
            out.push(to_wire(&GroupMemberRef::new(
                id,
                true,
                resolver.name_of(id),
            )));
        }
    }
    out
}

fn to_wire(r: &GroupMemberRef) -> tx::groups::GroupMember {
    tx::groups::GroupMember {
        actor_id: r.actor_id,
        ally_actor_id: 0,
        name: r.name.clone(),
        class_or_job: 0,
        level: r.level as u8,
        hp: 0,
        hp_max: 0,
        mp: 0,
        mp_max: 0,
        current_zone_id: 0,
        leader_flags: if r.is_leader { 1 } else { 0 },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::Character;
    use crate::data::ClientHandle;
    use crate::group::{Party, PartyWork};
    use crate::runtime::actor_registry::{ActorHandle, ActorKindTag};
    use std::sync::Arc;
    use tokio::sync::mpsc;

    fn character(id: u32, name: &str) -> Character {
        let mut c = Character::new(id);
        c.base.actor_name = name.to_string();
        c
    }

    struct DummyResolver(Party);
    impl GroupResolver for DummyResolver {
        fn members(&self, id: u64) -> Option<Vec<u32>> {
            if self.0.group_id == id {
                Some(self.0.members.clone())
            } else {
                None
            }
        }
        fn kind(&self, _: u64) -> Option<GroupKind> {
            Some(GroupKind::Party)
        }
        fn type_id(&self, _: u64) -> Option<GroupTypeId> {
            Some(GroupTypeId::PARTY)
        }
        fn name_of(&self, id: u32) -> String {
            format!("Actor{id}")
        }
    }

    #[tokio::test]
    async fn group_created_broadcasts_header_begin_members_end() {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        let mut ob = super::super::GroupOutbox::new();
        let party = Party::new(1, 100, &mut ob);

        registry
            .insert(ActorHandle::new(
                100,
                ActorKindTag::Player,
                0,
                11,
                character(100, "Leader"),
            ))
            .await;
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(16);
        world.register_client(11, ClientHandle::new(11, tx)).await;

        let resolver = DummyResolver(party);
        dispatch_group_event(
            &GroupEvent::GroupCreated {
                group_id: 1,
                kind: GroupKind::Party,
                type_id: GroupTypeId::PARTY,
            },
            &registry,
            &world,
            &resolver,
        )
        .await;

        // Expect: Header + Begin + one X08 chunk + End = 4 packets.
        for _ in 0..4 {
            let got = rx.recv().await.expect("group sync packet");
            assert!(!got.is_empty());
        }
    }

    #[tokio::test]
    async fn group_deleted_sends_delete_to_each_former_member() {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        registry
            .insert(ActorHandle::new(
                100,
                ActorKindTag::Player,
                0,
                11,
                character(100, "A"),
            ))
            .await;
        registry
            .insert(ActorHandle::new(
                200,
                ActorKindTag::Player,
                0,
                22,
                character(200, "B"),
            ))
            .await;
        let (tx1, mut rx1) = mpsc::channel::<Vec<u8>>(4);
        let (tx2, mut rx2) = mpsc::channel::<Vec<u8>>(4);
        world.register_client(11, ClientHandle::new(11, tx1)).await;
        world.register_client(22, ClientHandle::new(22, tx2)).await;

        let mut ob = super::super::GroupOutbox::new();
        let party = Party::new(1, 100, &mut ob);
        let resolver = DummyResolver(party);

        dispatch_group_event(
            &GroupEvent::GroupDeleted {
                group_id: 1,
                kind: GroupKind::Party,
                former_members: vec![100, 200],
            },
            &registry,
            &world,
            &resolver,
        )
        .await;

        assert!(!rx1.recv().await.unwrap().is_empty());
        assert!(!rx2.recv().await.unwrap().is_empty());
        // Silence unused PartyWork import.
        let _ = PartyWork::default();
    }
}
