//! `GuildleveDirector` — extends `Director` with guildleve-specific
//! state + lifecycle. Port of `Actors/Director/GuildleveDirector.cs`.

#![allow(dead_code)]

use super::director::{Director, DirectorKind};
use super::guildleve_work::GuildleveWork;
use super::outbox::{DirectorEvent, DirectorOutbox};

// ---------------------------------------------------------------------------
// Text ids — the world-master game messages the C# emits. Kept as
// constants so callers can reference them outside the outbox layer.
// ---------------------------------------------------------------------------

pub const GL_TEXT_START: u16 = 50022;
pub const GL_TEXT_COMPLETE: u16 = 50023;
pub const GL_TEXT_TIME_LIMIT: u16 = 50026;
pub const GL_TEXT_REWARD_EXP: u16 = 50029;
pub const GL_TEXT_REWARD_GIL: u16 = 50032;
pub const GL_TEXT_ABANDON: u16 = 50147;

/// Background-music ids the C# picks based on `guildleveData.location`.
pub struct GuildleveLocationMusic;

impl GuildleveLocationMusic {
    pub fn pick(location: u32) -> Option<u16> {
        match location {
            1 => Some(22),
            2 => Some(14),
            3 => Some(26),
            4 => Some(16),
            _ => None,
        }
    }

    /// Victory music id — plays on successful completion.
    pub const VICTORY: u16 = 81;
}

/// Resolve a `GuildleveGamedata.plateId` to the retail director script
/// path. Matches the big `switch` in `Area.CreateGuildleveDirector`.
pub fn guildleve_script_for_plate(guildleve_id: u32, plate_id: u32) -> Option<&'static str> {
    if matches!(guildleve_id, 10801 | 12401 | 11601) {
        return Some("Guildleve/PrivateGLBattleTutorial");
    }
    Some(match plate_id {
        20021 => "Guildleve/PrivateGLBattleSweepNormal",
        20022 => "Guildleve/PrivateGLBattleChaseNormal",
        20023 => "Guildleve/PrivateGLBattleOrbNormal",
        20024 => "Guildleve/PrivateGLBattleHuntNormal",
        20025 => "Guildleve/PrivateGLBattleGatherNormal",
        20026 => "Guildleve/PrivateGLBattleRoundNormal",
        20027 => "Guildleve/PrivateGLBattleSurviveNormal",
        20028 => "Guildleve/PrivateGLBattleDetectNormal",
        _ => return None,
    })
}

/// `GetGLStartAnimation(border, plate, isBoost)` port. Matches the bit
/// packing in the C# `GuildleveDirector.GetGLStartAnimation`.
pub fn guildleve_start_animation(border_icon: u32, plate_icon: u32, is_boost: bool) -> u32 {
    let border = border_icon.saturating_sub(20_000);
    let plate = plate_icon.saturating_sub(20_020) << 7;
    let boost = if is_boost { 0x8000 } else { 0 };
    0x0B00_0000 | boost | plate | border
}

// ---------------------------------------------------------------------------
// The director itself.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct GuildleveDirector {
    pub base: Director,
    pub guildleve_id: u32,
    pub difficulty: u8,
    pub owner_actor_id: u32,

    /// Copy of `GuildleveGamedata.location` — 1..=4 for the four music
    /// buckets.
    pub location: u32,
    pub time_limit_seconds: u32,
    /// 4-slot objective target copied from `GuildleveGamedata.aim_num`.
    pub aim_num_template: [i8; 4],
    /// `GuildleveGamedata.plate_id` — drives the retail script path.
    pub plate_id: u32,

    pub work: GuildleveWork,
    pub is_ended: bool,
    pub completion_time_seconds: u32,
}

impl GuildleveDirector {
    /// Build a guildleve director. `plate_id` / `aim_num_template` /
    /// `location` / `time_limit_seconds` would normally be read from
    /// `GuildleveGamedata` at boot time; the caller passes them in so the
    /// director layer stays gamedata-free.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        local_id: u32,
        zone_id: u32,
        guildleve_id: u32,
        difficulty: u8,
        owner_actor_id: u32,
        plate_id: u32,
        location: u32,
        time_limit_seconds: u32,
        aim_num_template: [i8; 4],
    ) -> Self {
        let script = guildleve_script_for_plate(guildleve_id, plate_id)
            .unwrap_or("Guildleve/PrivateGLBattleTutorial");
        let mut base = Director::new(local_id, zone_id, script, /* has_content_group */ true);
        base.kind = DirectorKind::Guildleve;

        let mut work = GuildleveWork::new();
        work.reset_from_gamedata(aim_num_template);

        Self {
            base,
            guildleve_id,
            difficulty,
            owner_actor_id,
            location,
            time_limit_seconds,
            aim_num_template,
            plate_id,
            work,
            is_ended: false,
            completion_time_seconds: 0,
        }
    }

    /// Composite actor id (6-prefixed). Distinct from the local
    /// sequence number on `base.director_id`.
    #[allow(clippy::misnamed_getters)]
    pub fn director_id(&self) -> u32 {
        self.base.actor_id
    }

    /// `StartGuildleve` — record start time, emit a `GuildleveStarted`
    /// event that the dispatcher turns into music + world-master text +
    /// property packets.
    pub fn start_guildleve(&mut self, now_unix_s: u32, outbox: &mut DirectorOutbox) {
        self.work.start_time = now_unix_s;
        outbox.push(DirectorEvent::GuildleveStarted {
            director_id: self.base.actor_id,
            guildleve_id: self.guildleve_id,
            difficulty: self.difficulty,
            location: self.location,
            time_limit_seconds: self.time_limit_seconds,
            start_time_unix: now_unix_s,
        });
        // Property sync so the client's guildleveWork state matches.
        outbox.push(DirectorEvent::GuildleveSyncAll {
            director_id: self.base.actor_id,
        });
    }

    /// `EndGuildleve(wasCompleted)`.
    pub fn end_guildleve(
        &mut self,
        now_unix_s: u32,
        was_completed: bool,
        outbox: &mut DirectorOutbox,
    ) {
        if self.is_ended {
            return;
        }
        self.is_ended = true;
        self.completion_time_seconds = now_unix_s.saturating_sub(self.work.start_time);
        self.work.clear_on_end();
        outbox.push(DirectorEvent::GuildleveEnded {
            director_id: self.base.actor_id,
            guildleve_id: self.guildleve_id,
            was_completed,
            completion_time_seconds: self.completion_time_seconds,
        });
    }

    /// `AbandonGuildleve()` — emits the abandon game-message event,
    /// ends the leve as "not completed", and tears the director down.
    pub fn abandon_guildleve(&mut self, now_unix_s: u32, outbox: &mut DirectorOutbox) {
        outbox.push(DirectorEvent::GuildleveAbandoned {
            director_id: self.base.actor_id,
            guildleve_id: self.guildleve_id,
        });
        self.end_guildleve(now_unix_s, /* was_completed */ false, outbox);
        self.base.end(outbox);
    }

    pub fn update_aim_num_now(&mut self, index: u8, value: i8, outbox: &mut DirectorOutbox) {
        let idx = index as usize;
        if idx >= self.work.aim_num_now.len() {
            return;
        }
        self.work.aim_num_now[idx] = value;
        outbox.push(DirectorEvent::GuildleveAimUpdated {
            director_id: self.base.actor_id,
            index,
            value,
        });
    }

    pub fn update_ui_state(&mut self, index: u8, value: i8, outbox: &mut DirectorOutbox) {
        let idx = index as usize;
        if idx >= self.work.ui_state.len() {
            return;
        }
        self.work.ui_state[idx] = value;
        outbox.push(DirectorEvent::GuildleveUiUpdated {
            director_id: self.base.actor_id,
            index,
            value,
        });
    }

    pub fn update_marker(
        &mut self,
        index: u8,
        x: f32,
        y: f32,
        z: f32,
        outbox: &mut DirectorOutbox,
    ) {
        let idx = index as usize;
        if idx >= self.work.marker_x.len() {
            return;
        }
        self.work.marker_x[idx] = x;
        self.work.marker_y[idx] = y;
        self.work.marker_z[idx] = z;
        outbox.push(DirectorEvent::GuildleveMarkerUpdated {
            director_id: self.base.actor_id,
            index,
            x,
            y,
            z,
        });
    }

    /// `SyncAllInfo()` — the dispatcher re-pushes the full property
    /// bundle (aim_num, aim_num_now, ui_state) to every player member.
    pub fn sync_all(&self, outbox: &mut DirectorOutbox) {
        outbox.push(DirectorEvent::GuildleveSyncAll {
            director_id: self.base.actor_id,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build() -> GuildleveDirector {
        GuildleveDirector::new(
            1,
            100,
            /* guildleve_id */ 123_456,
            /* difficulty */ 3,
            /* owner */ 0xA000_0001,
            /* plate_id */ 20024,
            /* location */ 1,
            /* time_limit */ 600,
            /* aim_num */ [5, 0, 0, 0],
        )
    }

    #[test]
    fn constructor_sets_script_and_work() {
        let gl = build();
        assert_eq!(
            gl.base.director_script_path,
            "Guildleve/PrivateGLBattleHuntNormal"
        );
        assert_eq!(gl.work.aim_num, [5, 0, 0, 0]);
        assert_eq!(gl.work.ui_state, [1, 0, 0, 0]);
        assert!(matches!(gl.base.kind, DirectorKind::Guildleve));
    }

    #[test]
    fn start_guildleve_emits_started_and_sync() {
        let mut gl = build();
        let mut ob = DirectorOutbox::new();
        gl.start_guildleve(1_000, &mut ob);
        let events = ob.drain();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, DirectorEvent::GuildleveStarted { .. }))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, DirectorEvent::GuildleveSyncAll { .. }))
        );
        assert_eq!(gl.work.start_time, 1_000);
    }

    #[test]
    fn end_guildleve_is_idempotent_and_clears_work() {
        let mut gl = build();
        let mut ob = DirectorOutbox::new();
        gl.start_guildleve(1_000, &mut ob);
        ob.drain();
        gl.end_guildleve(1_500, /* completed */ true, &mut ob);
        assert_eq!(gl.completion_time_seconds, 500);
        assert!(gl.is_ended);
        assert_eq!(gl.work.start_time, 0);
        assert_eq!(gl.work.signal, -1);
        let events_first = ob.drain();
        gl.end_guildleve(9_000, true, &mut ob);
        assert!(ob.is_empty(), "second end_guildleve is a no-op");
        assert!(events_first.iter().any(|e| matches!(
            e,
            DirectorEvent::GuildleveEnded {
                was_completed: true,
                ..
            }
        )));
    }

    #[test]
    fn abandon_tears_down_director() {
        let mut gl = build();
        let mut ob = DirectorOutbox::new();
        gl.start_guildleve(1_000, &mut ob);
        ob.drain();
        gl.abandon_guildleve(1_200, &mut ob);
        let events = ob.drain();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, DirectorEvent::GuildleveAbandoned { .. }))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, DirectorEvent::DirectorEnded { .. }))
        );
        assert!(gl.base.is_deleted());
    }

    #[test]
    fn update_aim_emits_property_packet() {
        let mut gl = build();
        let mut ob = DirectorOutbox::new();
        gl.update_aim_num_now(0, 3, &mut ob);
        match ob.events[0] {
            DirectorEvent::GuildleveAimUpdated { index, value, .. } => {
                assert_eq!(index, 0);
                assert_eq!(value, 3);
            }
            _ => panic!("wrong variant"),
        }
        assert_eq!(gl.work.aim_num_now[0], 3);
    }

    #[test]
    fn plate_id_routing() {
        assert_eq!(
            guildleve_script_for_plate(999, 20021),
            Some("Guildleve/PrivateGLBattleSweepNormal")
        );
        assert_eq!(
            guildleve_script_for_plate(999, 20028),
            Some("Guildleve/PrivateGLBattleDetectNormal")
        );
        // Tutorial-ids short-circuit regardless of plate.
        assert_eq!(
            guildleve_script_for_plate(10801, 20027),
            Some("Guildleve/PrivateGLBattleTutorial")
        );
        assert_eq!(guildleve_script_for_plate(999, 99999), None);
    }

    #[test]
    fn start_animation_encoding_matches_reference() {
        // Retail test vector: border=20010, plate=20024, boost=true
        // → 0x0B000000 | 0x8000 | ((4) << 7) | 10 = 0x0B00820A
        assert_eq!(guildleve_start_animation(20_010, 20_024, true), 0x0B00_820A);
    }

    #[test]
    fn location_music_picker() {
        assert_eq!(GuildleveLocationMusic::pick(1), Some(22));
        assert_eq!(GuildleveLocationMusic::pick(4), Some(16));
        assert_eq!(GuildleveLocationMusic::pick(99), None);
        assert_eq!(GuildleveLocationMusic::VICTORY, 81);
    }
}
