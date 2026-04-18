//! Movement helper. Port of `Actors/Chara/Ai/Helpers/PathFind.cs`.
//!
//! The retail code calls directly into the zone's navmesh and mutates the
//! owner's position via `QueuePositionUpdate`. We abstract both:
//!
//! * `NavmeshProvider` — plug in the real navmesh in the game loop; the
//!   default is a straight-line fallback so tests and the integration
//!   stub work without a mesh.
//! * `MovementSink` — receives per-tick position updates. The game loop
//!   implementation forwards them to the actor and queues
//!   `SetActorPositionPacket` emissions.

#![allow(dead_code)]

use common::Vector3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PathFindFlags(pub u32);

impl PathFindFlags {
    pub const NONE: Self = Self(0);
    pub const SCRIPTED: Self = Self(0x01);
    pub const IGNORE_NAV: Self = Self(0x02);

    pub const fn bits(self) -> u32 {
        self.0
    }
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl std::ops::BitOr for PathFindFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Supplies waypoints for a navmesh path. The default fallback
/// (`StraightLineNavmesh`) returns a single waypoint equal to the
/// destination.
pub trait NavmeshProvider {
    fn get_path(
        &self,
        start: Vector3,
        end: Vector3,
        step_size: f32,
        max_points: usize,
    ) -> Vec<Vector3>;
}

/// Fallback "navmesh" — straight-line, no obstacle avoidance.
pub struct StraightLineNavmesh;

impl NavmeshProvider for StraightLineNavmesh {
    fn get_path(
        &self,
        start: Vector3,
        end: Vector3,
        step_size: f32,
        max_points: usize,
    ) -> Vec<Vector3> {
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let dz = end.z - start.z;
        let total = (dx * dx + dz * dz).sqrt().max(1e-6);
        let steps = ((total / step_size).ceil() as usize).max(1).min(max_points);
        (1..=steps)
            .map(|i| {
                let t = i as f32 / steps as f32;
                Vector3::new(start.x + dx * t, start.y + dy * t, start.z + dz * t)
            })
            .collect()
    }
}

/// Sink where `step_to` publishes position updates. Real implementations
/// update the actor and emit `SetActorPositionPacket`s; tests can inspect
/// the latest position directly.
pub trait MovementSink {
    fn queue_position_update(&mut self, pos: Vector3);
    fn look_at(&mut self, pos: Vector3);
    fn get_position(&self) -> Vector3;
    fn get_rotation(&self) -> f32;
    fn get_speed(&self) -> f32;
    fn get_attack_range(&self) -> f32;
}

// ---------------------------------------------------------------------------
// PathFind.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct PathFind {
    path: Vec<Vector3>,
    can_follow_path: bool,
    distance_from_point: f32,
    pub flags: PathFindFlags,
}

impl PathFind {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn path(&self) -> &[Vector3] {
        &self.path
    }

    pub fn is_following_path(&self) -> bool {
        !self.path.is_empty()
    }

    pub fn is_following_scripted_path(&self) -> bool {
        self.flags.contains(PathFindFlags::SCRIPTED)
    }

    pub fn clear(&mut self) {
        self.path.clear();
        self.flags = PathFindFlags::NONE;
        self.distance_from_point = 0.0;
    }

    pub fn set_path_flags(&mut self, flags: PathFindFlags) {
        self.flags = flags;
    }

    /// `PreparePath(dest, step, maxPath)`. If `IGNORE_NAV` is set we skip
    /// the navmesh and jump straight to the destination.
    pub fn prepare_path(
        &mut self,
        start: Vector3,
        dest: Vector3,
        step_size: f32,
        max_points: usize,
        navmesh: &dyn NavmeshProvider,
    ) {
        if self.flags.contains(PathFindFlags::IGNORE_NAV) {
            self.path = vec![dest];
        } else {
            self.path = navmesh.get_path(start, dest, step_size, max_points);
        }
    }

    /// `PathInRange` — in the C# it picks a random point in `[minRange,
    /// maxRange]` around `dest`; tests typically inject a deterministic
    /// picker by setting `dest` to the pre-computed point themselves.
    pub fn path_in_range(
        &mut self,
        start: Vector3,
        dest: Vector3,
        attack_range: f32,
        step_size: f32,
        max_points: usize,
        navmesh: &dyn NavmeshProvider,
    ) {
        self.distance_from_point = attack_range;
        self.prepare_path(start, dest, step_size, max_points, navmesh);
    }

    /// `FollowPath` — advance along the current path by one tick.
    pub fn follow_path(&mut self, sink: &mut dyn MovementSink) {
        if self.path.is_empty() {
            return;
        }
        let point = self.path[0];
        self.step_to(sink, point);
        if self.at_point(sink, Some(point)) {
            self.path.remove(0);
        }
    }

    /// `AtPoint(point)` — true if the owner is close enough to `point`.
    pub fn at_point(&self, sink: &dyn MovementSink, point: Option<Vector3>) -> bool {
        let point = point.or_else(|| self.path.last().copied());
        let Some(point) = point else {
            return true;
        };
        let pos = sink.get_position();
        if self.distance_from_point == 0.0 {
            (pos.x - point.x).abs() < f32::EPSILON && (pos.z - point.z).abs() < f32::EPSILON
        } else {
            let d = common::utils::xz_distance(pos.x, pos.z, point.x, point.z);
            d <= (self.distance_from_point + 4.5)
        }
    }

    /// `StepTo(point)` — publish one movement tick toward `point`.
    pub fn step_to(&self, sink: &mut dyn MovementSink, point: Vector3) {
        let speed = sink.get_speed();
        let step = speed / 3.0;
        let pos = sink.get_position();
        let distance_to = common::utils::xz_distance(pos.x, pos.z, point.x, point.z);

        sink.look_at(point);

        if distance_to <= self.distance_from_point + step {
            if self.distance_from_point <= sink.get_attack_range() {
                sink.queue_position_update(point);
            } else {
                let gap = distance_to - self.distance_from_point;
                let new_pos = lerp_toward(pos, point, gap);
                sink.queue_position_update(new_pos);
            }
        } else {
            let gap = distance_to - self.distance_from_point;
            let new_pos = lerp_toward(pos, point, gap);
            sink.queue_position_update(new_pos);
        }
    }

    pub fn set_distance_from_point(&mut self, d: f32) {
        self.distance_from_point = d;
    }
}

fn lerp_toward(from: Vector3, to: Vector3, distance: f32) -> Vector3 {
    let dx = to.x - from.x;
    let dz = to.z - from.z;
    let total = (dx * dx + dz * dz).sqrt().max(1e-6);
    let ratio = (distance / total).clamp(0.0, 1.0);
    Vector3 {
        x: from.x + dx * ratio,
        y: from.y,
        z: from.z + dz * ratio,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    struct TestSink {
        pos: Vector3,
        rotation: f32,
        speed: f32,
        attack_range: f32,
        last_look: Vector3,
    }

    impl MovementSink for TestSink {
        fn queue_position_update(&mut self, pos: Vector3) {
            self.pos = pos;
        }
        fn look_at(&mut self, pos: Vector3) {
            self.last_look = pos;
        }
        fn get_position(&self) -> Vector3 {
            self.pos
        }
        fn get_rotation(&self) -> f32 {
            self.rotation
        }
        fn get_speed(&self) -> f32 {
            self.speed
        }
        fn get_attack_range(&self) -> f32 {
            self.attack_range
        }
    }

    fn sink_at(x: f32, z: f32) -> TestSink {
        TestSink {
            pos: Vector3::new(x, 0.0, z),
            rotation: 0.0,
            speed: 5.0,
            attack_range: 3.0,
            last_look: Vector3::ZERO,
        }
    }

    #[test]
    fn straight_line_path_steps_toward_target() {
        let mut pf = PathFind::new();
        pf.prepare_path(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(10.0, 0.0, 0.0),
            1.0,
            20,
            &StraightLineNavmesh,
        );
        assert!(pf.is_following_path());
        assert!(pf.path().len() >= 2);
    }

    #[test]
    fn ignore_nav_returns_single_waypoint() {
        let mut pf = PathFind::new();
        pf.set_path_flags(PathFindFlags::IGNORE_NAV);
        pf.prepare_path(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(10.0, 0.0, 0.0),
            1.0,
            20,
            &StraightLineNavmesh,
        );
        assert_eq!(pf.path().len(), 1);
    }

    #[test]
    fn follow_path_advances() {
        let mut pf = PathFind::new();
        pf.prepare_path(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(2.0, 0.0, 0.0),
            2.0,
            10,
            &StraightLineNavmesh,
        );
        let mut sink = sink_at(0.0, 0.0);
        pf.follow_path(&mut sink);
        // Position should have moved toward the waypoint.
        assert!(sink.pos.x > 0.0);
    }
}
