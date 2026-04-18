//! Group runtime. Port of `Map Server/Actors/Group/*`.
//!
//! The C# has one abstract `Group` with six concrete subclasses:
//!
//! * `Party` — up to 8 players, persistent across zones.
//! * `ContentGroup` / `GLContentGroup` — roster for a Director's
//!   instance (guildleve, trial, duty).
//! * `RelationGroup` — two-player binding (invitation, etc.).
//! * `TradeGroup` — two-player trade binding.
//! * `MonsterParty` — NPC-only party.
//! * `RetainerMeetingRelationGroup` — player + retainer pairing.
//!
//! Each kind has its own constructor + `build_member_list` (what the
//! client sees), but they share the chunked-packet broadcast pattern:
//! `Header → Begin → MembersX{08,16,32,64}* → End`. Mutations emit
//! typed `GroupEvent`s on a `GroupOutbox`; the game-loop dispatcher
//! turns those into the right packet sequence per session.

#![allow(dead_code, unused_imports, clippy::module_inception)]

pub mod content;
pub mod dispatcher;
pub mod monster;
pub mod outbox;
pub mod party;
pub mod relation;
pub mod types;

pub use dispatcher::{GroupResolver, PartyResolver, dispatch_group_event};

pub use content::{ContentGroup, GLContentGroup, new_guildleve_content_group};
pub use monster::MonsterParty;
pub use outbox::{GroupEvent, GroupOutbox};
pub use party::{Party, PartyWork};
pub use relation::{RELATION_HOST_MARKER, RelationGroup, RetainerMeetingRelationGroup, TradeGroup};
pub use types::{
    ChunkBucket, GroupKind, GroupMemberRef, GroupTypeId, PARTY_MAX_MEMBERS, RELATION_COMMAND_TRADE,
    chunk_bucket,
};

// ---------------------------------------------------------------------------
// Integration tests — Party↔Player, ContentGroup↔Director, chunk bucket.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::actor::player::PlayerHelperState;
    use crate::director::{DirectorOutbox, GuildleveDirector};
    use crate::zone::area::{AreaCore, AreaKind};

    #[test]
    fn party_lifecycle_with_player_caches() {
        let mut ob = GroupOutbox::new();
        let mut party = Party::new(1, 0xA000_0001, &mut ob);
        assert!(party.add_member(0xA000_0002, &mut ob));
        assert!(party.add_member(0xA000_0003, &mut ob));
        ob.drain();

        let mut leader = PlayerHelperState::default();
        let mut invitee = PlayerHelperState::default();
        let members = party.members.clone();
        leader.set_party(party.group_id, members.clone(), true);
        invitee.set_party(party.group_id, members, false);
        assert!(leader.current_party_is_leader);
        assert!(!invitee.current_party_is_leader);
        assert_eq!(leader.current_party_members.len(), 3);

        // Invitee leaves; leader transfers; last one out fires PartyEmptied.
        party.remove_member(0xA000_0003, &mut ob);
        assert_eq!(party.member_count(), 2);
        party.remove_member(0xA000_0001, &mut ob);
        assert_eq!(party.leader(), 0xA000_0002);
        leader.clear_party();
        assert!(leader.current_party_id.is_none());
        party.remove_member(0xA000_0002, &mut ob);
        assert!(
            ob.events
                .iter()
                .any(|e| matches!(e, GroupEvent::PartyEmptied { .. }))
        );
    }

    #[test]
    fn content_group_ties_to_director_and_player_state() {
        let mut area = AreaCore::new(
            100,
            "FieldCoastline",
            103,
            "/Area/Zone/Coastline",
            0,
            0,
            0,
            false,
            false,
            false,
            false,
            false,
            AreaKind::Zone,
        );
        let director_id =
            area.create_guildleve_director(123_456, 3, 0xA000_0001, 20_024, 1, 600, [5, 0, 0, 0]);
        let mut dir_ob = DirectorOutbox::new();
        {
            let gl: &mut GuildleveDirector = area.guildleve_director_mut(director_id).unwrap();
            gl.base.start(None, true, &mut dir_ob);
        }

        let mut ob = GroupOutbox::new();
        let mut cg = new_guildleve_content_group(0xA0A0, director_id, &[], &mut ob);
        assert_eq!(cg.type_id, GroupTypeId::CONTENT_GUILDLEVE);
        cg.add_member(0xA000_0001, &mut ob);
        cg.add_member(0xA000_0002, &mut ob);
        cg.start(&mut ob);

        let mut helper = PlayerHelperState::default();
        helper.set_content_group(Some(cg.group_id));
        assert_eq!(helper.current_content_group_id, Some(0xA0A0));

        {
            let gl = area.guildleve_director_mut(director_id).unwrap();
            gl.base.set_content_group_id(cg.group_id as u32);
            assert_eq!(gl.base.content_group_id(), cg.group_id as u32);
        }

        cg.check_destroy(|_| false, &mut ob);
        assert!(
            ob.events
                .iter()
                .any(|e| matches!(e, GroupEvent::ContentGroupAutoDelete { .. }))
        );
    }

    #[test]
    fn chunk_bucket_drives_broadcast_shape() {
        let mut ob = GroupOutbox::new();
        let mut p = Party::new(1, 1, &mut ob);
        for i in 2u32..=8 {
            p.add_member(i, &mut ob);
        }
        assert_eq!(chunk_bucket(p.member_count()), ChunkBucket::X08);

        let mut ob2 = GroupOutbox::new();
        let cg = ContentGroup::new(2, 100, &(0..20u32).collect::<Vec<_>>(), &mut ob2);
        assert_eq!(chunk_bucket(cg.member_count()), ChunkBucket::X16);

        let mut ob3 = GroupOutbox::new();
        let cg = ContentGroup::new(3, 100, &(0..40u32).collect::<Vec<_>>(), &mut ob3);
        assert_eq!(chunk_bucket(cg.member_count()), ChunkBucket::X32);

        let mut ob4 = GroupOutbox::new();
        let cg = ContentGroup::new(4, 100, &(0..70u32).collect::<Vec<_>>(), &mut ob4);
        assert_eq!(chunk_bucket(cg.member_count()), ChunkBucket::X64);
    }

    #[test]
    fn relation_group_between_two_players() {
        let mut ob = GroupOutbox::new();
        // Generic relation — party invitation (command = 0, topic = 7).
        let r = RelationGroup::new(1, 0xA000_0001, 0xA000_0002, 0, 7, &mut ob);
        assert_eq!(r.host(), 0xA000_0001);
        assert_eq!(r.other(), 0xA000_0002);

        // Trade — variable command = 30001 per retail.
        let t = TradeGroup::new(2, 0xA000_0001, 0xA000_0002, &mut ob);
        assert_eq!(t.host(), 0xA000_0001);
        // Both groups should have emitted a GroupCreated event.
        let created = ob
            .events
            .iter()
            .filter(|e| matches!(e, GroupEvent::GroupCreated { .. }))
            .count();
        assert_eq!(created, 2);
    }
}
