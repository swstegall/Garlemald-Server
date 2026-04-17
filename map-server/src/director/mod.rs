//! Content director runtime. Port of
//! `Map Server/Actors/Director/*`.
//!
//! A `Director` is a disembodied "actor" that orchestrates scripted
//! content: a guildleve run, an instanced trial, a weather cycle. It has
//! its own actor id (composite `6 << 28 | zone_id << 19 | id`), holds a
//! member list (players + NPCs inside the content), optionally owns a
//! `ContentGroup`, and dispatches Lua hooks (`init`, `main`, `onTalkEvent`,
//! `onEventStarted`, …) as members enter/leave and events fire.
//!
//! This phase keeps the runtime pure: mutations record `DirectorEvent`s
//! on a `DirectorOutbox`, and the game loop's dispatcher fans them into
//! real packet sends + Lua calls (matching the pattern used by inventory,
//! status, battle, area, and event).

#![allow(dead_code, unused_imports, clippy::module_inception)]

pub mod director;
pub mod guildleve;
pub mod guildleve_work;
pub mod outbox;

pub use director::{Director, DirectorKind};
pub use guildleve::{
    guildleve_start_animation, guildleve_script_for_plate, GuildleveDirector, GuildleveLocationMusic,
    GL_TEXT_ABANDON, GL_TEXT_COMPLETE, GL_TEXT_REWARD_EXP, GL_TEXT_REWARD_GIL, GL_TEXT_START,
    GL_TEXT_TIME_LIMIT,
};
pub use guildleve_work::GuildleveWork;
pub use outbox::{DirectorEvent, DirectorOutbox};

// ---------------------------------------------------------------------------
// Integration tests — Area → Director, Player → Director, guildleve lifecycle.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::actor::player::PlayerHelperState;
    use crate::zone::area::{AreaCore, AreaKind};

    fn new_area() -> AreaCore {
        AreaCore::new(
            100, "FieldCoastline", 103, "/Area/Zone/Coastline", 0, 0, 0,
            false, false, false, false, false, AreaKind::Zone,
        )
    }

    #[test]
    fn area_creates_generic_director() {
        let mut area = new_area();
        let id = area.create_director("Weather/Default", false);
        assert!(area.director(id).is_some());
        assert_eq!(area.director_count(), 1);
    }

    #[test]
    fn area_creates_guildleve_director_with_routed_script() {
        let mut area = new_area();
        let id = area.create_guildleve_director(
            123_456, 3, 0xA000_0001, 20_024, 1, 600, [5, 0, 0, 0],
        );
        let gl = area.guildleve_director(id).expect("present");
        assert_eq!(
            gl.base.director_script_path,
            "Guildleve/PrivateGLBattleHuntNormal"
        );
        assert!(matches!(gl.base.kind, DirectorKind::Guildleve));
    }

    #[test]
    fn full_guildleve_lifecycle_end_to_end() {
        let mut area = new_area();
        let id = area.create_guildleve_director(
            123_456, 3, 0xA000_0001, 20_024, 1, 600, [5, 0, 0, 0],
        );

        // Start + add player + start the leve itself.
        let mut outbox = DirectorOutbox::new();
        {
            let gl = area.guildleve_director_mut(id).unwrap();
            gl.base.start(
                Some("/Area/Director/Guildleve/PrivateGLBattleHuntNormal".into()),
                true,
                &mut outbox,
            );
            gl.base.add_member(0xA000_0001, /* player */ true, &mut outbox);
            gl.start_guildleve(1_000, &mut outbox);
        }
        outbox.drain();

        // Advance an objective twice.
        {
            let gl = area.guildleve_director_mut(id).unwrap();
            gl.update_aim_num_now(0, 1, &mut outbox);
            gl.update_aim_num_now(0, 5, &mut outbox);
        }
        let events = outbox.drain();
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, DirectorEvent::GuildleveAimUpdated { .. }))
                .count(),
            2
        );

        // Player plumbing — a PlayerHelperState picks up membership.
        let mut helper = PlayerHelperState::default();
        let mut ob2 = DirectorOutbox::new();
        helper.add_director(id, /* is_guildleve */ true, &mut ob2, 0xA000_0001);
        assert_eq!(helper.guildleve_director(), Some(id));
        assert!(helper.owned_directors.contains(&id));

        // Complete + delete.
        let mut ob3 = DirectorOutbox::new();
        {
            let gl = area.guildleve_director_mut(id).unwrap();
            gl.end_guildleve(1_450, /* completed */ true, &mut ob3);
            gl.base.end(&mut ob3);
        }
        let end_events = ob3.drain();
        assert!(end_events
            .iter()
            .any(|e| matches!(e, DirectorEvent::GuildleveEnded { was_completed: true, .. })));
        assert!(end_events
            .iter()
            .any(|e| matches!(e, DirectorEvent::DirectorEnded { .. })));

        helper.remove_director(id, &mut ob2, 0xA000_0001);
        assert!(helper.guildleve_director().is_none());

        assert!(area.delete_director(id));
        assert_eq!(area.director_count(), 0);
    }

    #[test]
    fn guildleve_abandon_tears_director_down() {
        let mut area = new_area();
        let id = area.create_guildleve_director(
            999_999, 1, 0xA000_0001, 20_021, 2, 300, [3, 0, 0, 0],
        );
        let mut ob = DirectorOutbox::new();
        {
            let gl = area.guildleve_director_mut(id).unwrap();
            gl.base.start(None, true, &mut ob);
            gl.base.add_member(0xA000_0001, true, &mut ob);
            gl.start_guildleve(1_000, &mut ob);
            ob.drain();
            gl.abandon_guildleve(1_100, &mut ob);
        }
        let events = ob.drain();
        assert!(events
            .iter()
            .any(|e| matches!(e, DirectorEvent::GuildleveAbandoned { .. })));
        assert!(events
            .iter()
            .any(|e| matches!(e, DirectorEvent::DirectorEnded { .. })));
    }
}
