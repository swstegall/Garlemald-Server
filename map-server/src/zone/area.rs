//! `Area` — the spatial-grid container that every zone type builds on.
//! Port of `Actors/Area/Area.cs`.
//!
//! The C# keeps a `Dictionary<uint, Actor>` plus a 2-D `List<Actor>[,]`
//! grid for proximity queries. We mirror the design with:
//!
//! * `actors: HashMap<u32, StoredActor>` — O(1) lookup by actor id.
//! * `grid: Vec<Vec<u32>>` — flat-indexed 2-D grid; each cell holds the
//!   ids of actors whose XZ position lies inside that 50-yalm square.
//!
//! Actors never live directly inside the area — we only track enough
//! state to answer range queries (position, kind, alive/isolation flags).
//! The real `Character`/`Player`/`Npc` rows live on the game loop's
//! actor registry and are looked up by id.

#![allow(dead_code)]

use std::collections::HashMap;

use common::Vector3;

use super::outbox::{AreaEvent, AreaOutbox};

// ---------------------------------------------------------------------------
// Grid constants — identical to the C#.
// ---------------------------------------------------------------------------

pub const BOUNDING_GRID_SIZE: i32 = 50;
pub const AREA_MIN: i32 = -5000;
pub const AREA_MAX: i32 = 5000;
pub const NUM_X_BLOCKS: i32 = (AREA_MAX - AREA_MIN) / BOUNDING_GRID_SIZE;
pub const NUM_Y_BLOCKS: i32 = (AREA_MAX - AREA_MIN) / BOUNDING_GRID_SIZE;
pub const HALF_WIDTH: i32 = NUM_X_BLOCKS / 2;
pub const HALF_HEIGHT: i32 = NUM_Y_BLOCKS / 2;

/// `BroadcastPacketAroundActor` visibility radius — 50 yalms in retail.
pub const BROADCAST_RADIUS: f32 = 50.0;

// ---------------------------------------------------------------------------
// AreaKind — which variant of Area we're looking at. The C# hierarchy
// (Area ← Zone, PrivateArea ← Area, PrivateAreaContent ← PrivateArea)
// becomes a tag so range queries can route correctly.
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AreaKind {
    #[default]
    Zone = 0,
    PrivateArea = 1,
    PrivateAreaContent = 2,
}

/// Which actor kind a `StoredActor` represents. Mirrors the C# runtime
/// type checks (`is Player`, `is BattleNpc`, `is Ally`, …).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActorKind {
    #[default]
    Other = 0,
    Player = 1,
    Npc = 2,
    BattleNpc = 3,
    Ally = 4,
    Pet = 5,
}

/// The projection of a live actor the area keeps for range queries.
/// Updated whenever the actor moves; deleted on `RemoveActorFromZone`.
#[derive(Debug, Clone, Copy)]
pub struct StoredActor {
    pub actor_id: u32,
    pub kind: ActorKind,
    pub position: Vector3,
    /// Last-known "grid cell" — used by `UpdateActorPosition` to detect
    /// cell crossings cheaply.
    pub grid: (i32, i32),
    pub is_alive: bool,
}

// ---------------------------------------------------------------------------
// AreaCore — the shared body composed by Zone / PrivateArea /
// PrivateAreaContent.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AreaCore {
    pub actor_id: u32,
    pub zone_name: String,
    pub region_id: u16,
    pub class_path: String,
    pub class_name: String,
    pub kind: AreaKind,

    pub is_isolated: bool,
    pub can_stealth: bool,
    pub is_inn: bool,
    pub can_ride_chocobo: bool,
    pub is_instance_raid: bool,

    pub weather_normal: u16,
    pub weather_common: u16,
    pub weather_rare: u16,
    pub bgm_day: u16,
    pub bgm_night: u16,
    pub bgm_battle: u16,

    pub actors: HashMap<u32, StoredActor>,
    grid: Vec<Vec<u32>>,

    /// Generic directors keyed by composite actor id
    /// (`6 << 28 | zone << 19 | local`).
    directors: HashMap<u32, crate::director::Director>,
    /// GuildleveDirectors keyed the same way. Kept in a separate map so
    /// lookups don't need a branch on the variant tag.
    guildleve_directors: HashMap<u32, crate::director::GuildleveDirector>,
    /// Running counter for director local ids — matches the C#
    /// `directorIdCount`. Atomic per-area.
    next_director_id: u32,
}

impl AreaCore {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        actor_id: u32,
        zone_name: impl Into<String>,
        region_id: u16,
        class_path: impl Into<String>,
        bgm_day: u16,
        bgm_night: u16,
        bgm_battle: u16,
        is_isolated: bool,
        is_inn: bool,
        can_ride_chocobo: bool,
        can_stealth: bool,
        is_instance_raid: bool,
        kind: AreaKind,
    ) -> Self {
        let class_path: String = class_path.into();
        let class_name = class_path
            .rsplit('/')
            .next()
            .unwrap_or(&class_path)
            .to_string();
        let grid_cells = (NUM_X_BLOCKS * NUM_Y_BLOCKS) as usize;
        Self {
            actor_id,
            zone_name: zone_name.into(),
            region_id,
            class_path,
            class_name,
            kind,
            is_isolated,
            can_stealth,
            is_inn,
            can_ride_chocobo,
            is_instance_raid,
            weather_normal: 0,
            weather_common: 0,
            weather_rare: 0,
            bgm_day,
            bgm_night,
            bgm_battle,
            actors: HashMap::new(),
            grid: vec![Vec::new(); grid_cells],
            directors: HashMap::new(),
            guildleve_directors: HashMap::new(),
            next_director_id: 1,
        }
    }

    // -----------------------------------------------------------------
    // Director registry
    // -----------------------------------------------------------------

    /// Port of `Area::CreateDirector(path, hasContentGroup, args)`.
    /// Returns the composite actor id of the new director. The caller
    /// then looks up the Director via `director_mut` to drive `start`.
    pub fn create_director(
        &mut self,
        script_path: impl Into<String>,
        has_content_group: bool,
    ) -> u32 {
        let local_id = self.alloc_director_id();
        let director =
            crate::director::Director::new(local_id, self.actor_id, script_path, has_content_group);
        let id = director.actor_id;
        self.directors.insert(id, director);
        id
    }

    /// Port of `Area::CreateGuildleveDirector(glid, difficulty, owner, args)`.
    /// `plate_id` / `location` / `time_limit` / `aim_num_template` come
    /// from `GuildleveGamedata`; the caller resolves them (Phase 5c stays
    /// gamedata-free).
    #[allow(clippy::too_many_arguments)]
    pub fn create_guildleve_director(
        &mut self,
        guildleve_id: u32,
        difficulty: u8,
        owner_actor_id: u32,
        plate_id: u32,
        location: u32,
        time_limit_seconds: u32,
        aim_num_template: [i8; 4],
    ) -> u32 {
        let local_id = self.alloc_director_id();
        let director = crate::director::GuildleveDirector::new(
            local_id,
            self.actor_id,
            guildleve_id,
            difficulty,
            owner_actor_id,
            plate_id,
            location,
            time_limit_seconds,
            aim_num_template,
        );
        let id = director.director_id();
        self.guildleve_directors.insert(id, director);
        id
    }

    fn alloc_director_id(&mut self) -> u32 {
        let id = self.next_director_id;
        self.next_director_id = self.next_director_id.saturating_add(1);
        id
    }

    pub fn director(&self, actor_id: u32) -> Option<&crate::director::Director> {
        self.directors.get(&actor_id)
    }

    pub fn director_mut(&mut self, actor_id: u32) -> Option<&mut crate::director::Director> {
        self.directors.get_mut(&actor_id)
    }

    pub fn guildleve_director(&self, actor_id: u32) -> Option<&crate::director::GuildleveDirector> {
        self.guildleve_directors.get(&actor_id)
    }

    pub fn guildleve_director_mut(
        &mut self,
        actor_id: u32,
    ) -> Option<&mut crate::director::GuildleveDirector> {
        self.guildleve_directors.get_mut(&actor_id)
    }

    /// Mirrors `Area::DeleteDirector(id)` — the caller is expected to
    /// have already called `Director::end(outbox)` to fire the bookkeeping
    /// events. This just drops the entry from whichever registry holds it.
    pub fn delete_director(&mut self, actor_id: u32) -> bool {
        if self.directors.remove(&actor_id).is_some() {
            return true;
        }
        self.guildleve_directors.remove(&actor_id).is_some()
    }

    pub fn director_count(&self) -> usize {
        self.directors.len() + self.guildleve_directors.len()
    }

    pub fn director_ids(&self) -> Vec<u32> {
        self.directors
            .keys()
            .chain(self.guildleve_directors.keys())
            .copied()
            .collect()
    }

    pub fn actor_count(&self) -> usize {
        self.actors.len()
    }

    // -----------------------------------------------------------------
    // Grid math
    // -----------------------------------------------------------------

    /// Convert world coords to grid cell, clamped to the area's bounds.
    pub fn pos_to_grid(x: f32, z: f32) -> (i32, i32) {
        let gx = (x as i32) / BOUNDING_GRID_SIZE + HALF_WIDTH;
        let gy = (z as i32) / BOUNDING_GRID_SIZE + HALF_HEIGHT;
        (gx.clamp(0, NUM_X_BLOCKS - 1), gy.clamp(0, NUM_Y_BLOCKS - 1))
    }

    fn grid_index(cell: (i32, i32)) -> usize {
        (cell.1 * NUM_X_BLOCKS + cell.0) as usize
    }

    fn cell_mut(&mut self, cell: (i32, i32)) -> &mut Vec<u32> {
        let idx = Self::grid_index(cell);
        &mut self.grid[idx]
    }

    fn cell(&self, cell: (i32, i32)) -> &Vec<u32> {
        &self.grid[Self::grid_index(cell)]
    }

    // -----------------------------------------------------------------
    // Actor add / remove / update
    // -----------------------------------------------------------------

    /// `AddActorToZone`. Idempotent — re-adding an existing actor just
    /// updates their cached row.
    pub fn add_actor(&mut self, actor: StoredActor, outbox: &mut AreaOutbox) {
        let cell = Self::pos_to_grid(actor.position.x, actor.position.z);
        let mut actor = actor;
        actor.grid = cell;

        // If we're re-adding, remove the prior cell entry first.
        if let Some(prev) = self.actors.insert(actor.actor_id, actor) {
            let prev_cell = prev.grid;
            if prev_cell != cell
                && let Some(pos) = self
                    .cell(prev_cell)
                    .iter()
                    .position(|&id| id == actor.actor_id)
            {
                self.cell_mut(prev_cell).remove(pos);
            } else {
                // Same cell — don't re-push below.
                return;
            }
        }
        self.cell_mut(cell).push(actor.actor_id);
        outbox.push(AreaEvent::ActorAdded {
            area_id: self.actor_id,
            actor_id: actor.actor_id,
        });
    }

    /// `RemoveActorFromZone`.
    pub fn remove_actor(&mut self, actor_id: u32, outbox: &mut AreaOutbox) -> bool {
        let Some(prev) = self.actors.remove(&actor_id) else {
            return false;
        };
        if let Some(pos) = self.cell(prev.grid).iter().position(|&id| id == actor_id) {
            self.cell_mut(prev.grid).remove(pos);
        }
        outbox.push(AreaEvent::ActorRemoved {
            area_id: self.actor_id,
            actor_id,
        });
        true
    }

    /// `UpdateActorPosition`. If the actor crossed a cell boundary we
    /// move it in the grid and emit `ActorMoved` so the game loop can
    /// recompute visibility.
    pub fn update_actor_position(
        &mut self,
        actor_id: u32,
        new_position: Vector3,
        outbox: &mut AreaOutbox,
    ) {
        let Some(actor) = self.actors.get_mut(&actor_id) else {
            return;
        };
        let new_cell = Self::pos_to_grid(new_position.x, new_position.z);
        let old_cell = actor.grid;
        actor.position = new_position;

        if new_cell == old_cell {
            return;
        }
        actor.grid = new_cell;

        if let Some(pos) = self.cell(old_cell).iter().position(|&id| id == actor_id) {
            self.cell_mut(old_cell).remove(pos);
        }
        self.cell_mut(new_cell).push(actor_id);

        outbox.push(AreaEvent::ActorMoved {
            area_id: self.actor_id,
            actor_id,
            old_grid: old_cell,
            new_grid: new_cell,
        });
    }

    /// Update alive/dead without moving.
    pub fn set_actor_alive(&mut self, actor_id: u32, is_alive: bool) {
        if let Some(a) = self.actors.get_mut(&actor_id) {
            a.is_alive = is_alive;
        }
    }

    /// `Clear` — wipe the actor table + grid without emitting events.
    pub fn clear(&mut self) {
        self.actors.clear();
        for cell in &mut self.grid {
            cell.clear();
        }
    }

    // -----------------------------------------------------------------
    // Lookups
    // -----------------------------------------------------------------

    pub fn find_actor(&self, actor_id: u32) -> Option<StoredActor> {
        self.actors.get(&actor_id).copied()
    }

    pub fn contains(&self, actor_id: u32) -> bool {
        self.actors.contains_key(&actor_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &StoredActor> {
        self.actors.values()
    }

    /// All actors whose `kind` matches `filter`.
    pub fn all_of_kind(&self, filter: ActorKind) -> Vec<StoredActor> {
        self.actors
            .values()
            .filter(|a| a.kind == filter)
            .copied()
            .collect()
    }

    pub fn all_players(&self) -> Vec<StoredActor> {
        self.all_of_kind(ActorKind::Player)
    }

    pub fn all_battle_npcs(&self) -> Vec<StoredActor> {
        self.all_of_kind(ActorKind::BattleNpc)
    }

    pub fn all_allies(&self) -> Vec<StoredActor> {
        self.all_of_kind(ActorKind::Ally)
    }

    // -----------------------------------------------------------------
    // Range queries — the heart of battle / visibility integration.
    // -----------------------------------------------------------------

    /// `GetActorsAroundPoint(x, z, checkDistance)`.
    pub fn actors_around_point(&self, x: f32, z: f32, check_distance: f32) -> Vec<StoredActor> {
        let radius_cells = ((check_distance / BOUNDING_GRID_SIZE as f32).ceil() as i32).max(0);
        let (gx, gy) = Self::pos_to_grid(x, z);
        self.collect_actors_in_cells(gx, gy, radius_cells, None)
    }

    /// `GetActorsAroundActor(actor, checkDistance)`.
    pub fn actors_around(&self, center_actor_id: u32, check_distance: f32) -> Vec<StoredActor> {
        let Some(center) = self.actors.get(&center_actor_id) else {
            return Vec::new();
        };
        let radius_cells = ((check_distance / BOUNDING_GRID_SIZE as f32).ceil() as i32).max(0);
        self.collect_actors_in_cells(
            center.grid.0,
            center.grid.1,
            radius_cells,
            Some(center_actor_id),
        )
    }

    /// Filter-specialized range query — same as `actors_around` but only
    /// returns actors whose kind matches.
    pub fn actors_around_of_kind(
        &self,
        center_actor_id: u32,
        check_distance: f32,
        kind: ActorKind,
    ) -> Vec<StoredActor> {
        self.actors_around(center_actor_id, check_distance)
            .into_iter()
            .filter(|a| a.kind == kind)
            .collect()
    }

    fn collect_actors_in_cells(
        &self,
        gx: i32,
        gy: i32,
        radius_cells: i32,
        exclude_actor_id: Option<u32>,
    ) -> Vec<StoredActor> {
        let mut out = Vec::new();
        let y_min = (gy - radius_cells).max(0);
        let y_max = (gy + radius_cells).min(NUM_Y_BLOCKS - 1);
        let x_min = (gx - radius_cells).max(0);
        let x_max = (gx + radius_cells).min(NUM_X_BLOCKS - 1);
        for y in y_min..=y_max {
            for x in x_min..=x_max {
                for &id in self.cell((x, y)) {
                    if Some(id) == exclude_actor_id {
                        continue;
                    }
                    if let Some(a) = self.actors.get(&id) {
                        if self.is_isolated && a.kind == ActorKind::Player {
                            continue;
                        }
                        out.push(*a);
                    }
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Area — thin wrapper that matches the C# class name. Zone / PrivateArea
// compose an `AreaCore` plus their own state; this struct is just for
// the "a plain area" case (mostly tests + legacy zone records without
// navmesh).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Area {
    pub core: AreaCore,
}

impl Area {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        actor_id: u32,
        zone_name: impl Into<String>,
        region_id: u16,
        class_path: impl Into<String>,
        bgm_day: u16,
        bgm_night: u16,
        bgm_battle: u16,
        is_isolated: bool,
        is_inn: bool,
        can_ride_chocobo: bool,
        can_stealth: bool,
        is_instance_raid: bool,
    ) -> Self {
        Self {
            core: AreaCore::new(
                actor_id,
                zone_name,
                region_id,
                class_path,
                bgm_day,
                bgm_night,
                bgm_battle,
                is_isolated,
                is_inn,
                can_ride_chocobo,
                can_stealth,
                is_instance_raid,
                AreaKind::Zone,
            ),
        }
    }

    /// `ChangeWeather(weather, transitionTime)` — caches new weather and
    /// pushes a `WeatherChange` event. Caller decides whether it's
    /// player-targeted or zone-wide.
    pub fn change_weather(
        &mut self,
        weather_id: u16,
        transition_time: u16,
        target_actor_id: Option<u32>,
        zone_wide: bool,
        outbox: &mut AreaOutbox,
    ) {
        self.core.weather_normal = weather_id;
        outbox.push(AreaEvent::WeatherChange {
            area_id: self.core.actor_id,
            weather_id,
            transition_time,
            target_actor_id,
            zone_wide,
        });
    }

    /// `BroadcastPacketAroundActor`. The outbox event carries opcode +
    /// raw bytes; the game loop turns that into real SubPacket sends.
    pub fn broadcast_around_actor(
        &self,
        source_actor_id: u32,
        opcode: u16,
        payload: Vec<u8>,
        outbox: &mut AreaOutbox,
    ) {
        if self.core.is_isolated {
            return;
        }
        outbox.push(AreaEvent::BroadcastAroundActor {
            area_id: self.core.actor_id,
            source_actor_id,
            check_distance: BROADCAST_RADIUS,
            opcode,
            payload,
        });
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn actor(id: u32, x: f32, z: f32, kind: ActorKind) -> StoredActor {
        StoredActor {
            actor_id: id,
            kind,
            position: Vector3::new(x, 0.0, z),
            grid: (0, 0),
            is_alive: true,
        }
    }

    fn mk_area() -> Area {
        Area::new(
            100,
            "test_zone",
            1,
            "/Area/Zone/TestZone",
            0,
            0,
            0,
            false,
            false,
            false,
            false,
            false,
        )
    }

    #[test]
    fn grid_math_clamps_to_bounds() {
        assert_eq!(AreaCore::pos_to_grid(0.0, 0.0), (HALF_WIDTH, HALF_HEIGHT));
        // Past the max edge.
        let (gx, gy) = AreaCore::pos_to_grid(99999.0, 99999.0);
        assert_eq!(gx, NUM_X_BLOCKS - 1);
        assert_eq!(gy, NUM_Y_BLOCKS - 1);
        // Past the min edge.
        let (gx, gy) = AreaCore::pos_to_grid(-99999.0, -99999.0);
        assert_eq!(gx, 0);
        assert_eq!(gy, 0);
    }

    #[test]
    fn add_and_remove_actor() {
        let mut area = mk_area();
        let mut ob = AreaOutbox::new();
        area.core
            .add_actor(actor(1, 0.0, 0.0, ActorKind::Player), &mut ob);
        assert_eq!(area.core.actor_count(), 1);
        assert!(area.core.contains(1));
        area.core.remove_actor(1, &mut ob);
        assert_eq!(area.core.actor_count(), 0);
        // Both an Added and Removed event should be in the outbox.
        let events = ob.drain();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn update_position_crosses_cell_boundary() {
        let mut area = mk_area();
        let mut ob = AreaOutbox::new();
        area.core
            .add_actor(actor(1, 0.0, 0.0, ActorKind::Player), &mut ob);
        ob.drain();

        // Move within the same cell — no ActorMoved event.
        area.core
            .update_actor_position(1, Vector3::new(10.0, 0.0, 10.0), &mut ob);
        assert!(ob.events.is_empty());

        // Move across a cell boundary.
        area.core
            .update_actor_position(1, Vector3::new(60.0, 0.0, 0.0), &mut ob);
        assert!(
            ob.events
                .iter()
                .any(|e| matches!(e, AreaEvent::ActorMoved { .. }))
        );
    }

    #[test]
    fn actors_around_finds_neighbors_only() {
        let mut area = mk_area();
        let mut ob = AreaOutbox::new();
        area.core
            .add_actor(actor(1, 0.0, 0.0, ActorKind::Player), &mut ob);
        area.core
            .add_actor(actor(2, 10.0, 0.0, ActorKind::BattleNpc), &mut ob);
        area.core
            .add_actor(actor(3, 1000.0, 0.0, ActorKind::BattleNpc), &mut ob);
        ob.drain();

        let near = area.core.actors_around(1, 50.0);
        let ids: Vec<u32> = near.iter().map(|a| a.actor_id).collect();
        assert!(ids.contains(&2));
        assert!(!ids.contains(&3));
        assert!(!ids.contains(&1)); // excluded as center
    }

    #[test]
    fn isolation_flag_filters_players() {
        let mut area = Area::new(
            100,
            "instance",
            1,
            "/Area/Zone/Instance",
            0,
            0,
            0,
            /* is_isolated */ true,
            false,
            false,
            false,
            false,
        );
        let mut ob = AreaOutbox::new();
        area.core
            .add_actor(actor(1, 0.0, 0.0, ActorKind::BattleNpc), &mut ob);
        area.core
            .add_actor(actor(2, 10.0, 0.0, ActorKind::Player), &mut ob);
        ob.drain();

        let near = area.core.actors_around(1, 50.0);
        assert!(near.iter().all(|a| a.kind != ActorKind::Player));
    }

    #[test]
    fn broadcast_suppressed_in_isolated_area() {
        let area = Area::new(
            100,
            "instance",
            1,
            "/Area/Zone/Instance",
            0,
            0,
            0,
            true,
            false,
            false,
            false,
            false,
        );
        let mut ob = AreaOutbox::new();
        area.broadcast_around_actor(1, 0x1234, vec![1, 2, 3], &mut ob);
        assert!(ob.events.is_empty());
    }

    #[test]
    fn all_of_kind_filters_correctly() {
        let mut area = mk_area();
        let mut ob = AreaOutbox::new();
        area.core
            .add_actor(actor(1, 0.0, 0.0, ActorKind::Player), &mut ob);
        area.core
            .add_actor(actor(2, 10.0, 0.0, ActorKind::BattleNpc), &mut ob);
        area.core
            .add_actor(actor(3, 20.0, 0.0, ActorKind::BattleNpc), &mut ob);
        assert_eq!(area.core.all_players().len(), 1);
        assert_eq!(area.core.all_battle_npcs().len(), 2);
    }

    #[test]
    fn change_weather_emits_event() {
        let mut area = mk_area();
        let mut ob = AreaOutbox::new();
        area.change_weather(5, 10, Some(42), false, &mut ob);
        assert_eq!(area.core.weather_normal, 5);
        assert!(matches!(
            ob.events[0],
            AreaEvent::WeatherChange {
                weather_id: 5,
                target_actor_id: Some(42),
                ..
            }
        ));
    }
}
