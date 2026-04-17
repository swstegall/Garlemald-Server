//! AoE target resolution. Port of `Actors/Chara/Ai/Helpers/TargetFind.cs`.
//!
//! The C# `TargetFind` takes references to concrete `Character` objects
//! and walks them through the zone. We decouple that with a small
//! `ActorView` trait + `ActorArena` so the engine, tests, and scripts
//! can use any backend that can answer "where is actor X" and "what
//! actors are within R of actor Y".

#![allow(dead_code)]

use std::f32::consts::PI;

use common::Vector3;

/// ValidTarget bitfield. Ported from
/// `Actors/Chara/Ai/Helpers/TargetFind.cs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ValidTarget(pub u16);

impl ValidTarget {
    pub const NONE: ValidTarget = ValidTarget(0x0000);
    pub const SELF: ValidTarget = ValidTarget(0x0001);
    pub const SELF_ONLY: ValidTarget = ValidTarget(0x0002);
    pub const PARTY: ValidTarget = ValidTarget(0x0004);
    pub const PARTY_ONLY: ValidTarget = ValidTarget(0x0008);
    pub const ALLY: ValidTarget = ValidTarget(0x0010);
    pub const ALLY_ONLY: ValidTarget = ValidTarget(0x0020);
    pub const NPC: ValidTarget = ValidTarget(0x0040);
    pub const NPC_ONLY: ValidTarget = ValidTarget(0x0080);
    pub const ENEMY: ValidTarget = ValidTarget(0x0100);
    pub const ENEMY_ONLY: ValidTarget = ValidTarget(0x0200);
    pub const OBJECT: ValidTarget = ValidTarget(0x0400);
    pub const OBJECT_ONLY: ValidTarget = ValidTarget(0x0800);
    pub const CORPSE: ValidTarget = ValidTarget(0x1000);
    pub const CORPSE_ONLY: ValidTarget = ValidTarget(0x2000);

    pub const MAIN_TARGET_PARTY: ValidTarget = ValidTarget(0x4000);
    pub const MAIN_TARGET_PARTY_ONLY: ValidTarget = ValidTarget(0x8000);

    pub const fn bits(self) -> u16 {
        self.0
    }
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
    pub const fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }
}

impl std::ops::BitOr for ValidTarget {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}
impl std::ops::BitOrAssign for ValidTarget {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TargetFindAOEType {
    #[default]
    None = 0,
    /// Cylinder: XZ distance plus half-height on Y.
    Circle = 1,
    /// Cone: angle in radians around owner/target rotation.
    Cone = 2,
    /// Axis-aligned box relative to owner rotation + aoe rotate angle.
    Box = 3,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TargetFindAOETarget {
    #[default]
    Target = 0,
    Self_ = 1,
}

// ---------------------------------------------------------------------------
// ActorView + ActorArena — what the target finder needs from its universe.
// ---------------------------------------------------------------------------

/// Minimal projection of a `Character` that `CanTarget` needs to do its
/// filtering. Real callers will hand over a wrapper around the live
/// Character; tests can build one by hand.
#[derive(Debug, Clone, Copy)]
pub struct ActorView {
    pub actor_id: u32,
    pub position: Vector3,
    pub rotation: f32,
    pub is_alive: bool,
    pub is_static: bool,
    /// Whatever side the actor is on — parity/ownership flag in the C#.
    pub allegiance: u32,
    /// Party id (0 = not in a party). The retail code compares these for
    /// PartyOnly/AllyOnly logic.
    pub party_id: u64,
    pub zone_id: u32,
    pub is_updates_locked: bool,
    pub is_player: bool,
    pub is_battle_npc: bool,
}

/// Anything that can answer "who is at actor id X" and "who is within R of
/// actor Y". The zone/world module implements this; tests use `HashMap`.
pub trait ActorArena {
    fn get(&self, actor_id: u32) -> Option<ActorView>;
    fn actors_around(&self, center: u32, radius: f32) -> Vec<ActorView>;
}

impl ActorArena for std::collections::HashMap<u32, ActorView> {
    fn get(&self, actor_id: u32) -> Option<ActorView> {
        self.get(&actor_id).copied()
    }
    fn actors_around(&self, center: u32, radius: f32) -> Vec<ActorView> {
        let Some(origin) = self.get(&center).copied() else {
            return Vec::new();
        };
        let r2 = radius * radius;
        self.values()
            .filter(|v| {
                let dx = v.position.x - origin.position.x;
                let dz = v.position.z - origin.position.z;
                (dx * dx + dz * dz) <= r2
            })
            .copied()
            .collect()
    }
}

// ---------------------------------------------------------------------------
// TargetFind itself.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TargetFind {
    owner: ActorView,
    main_target: ActorView,

    pub valid_target: ValidTarget,
    pub aoe_type: TargetFindAOEType,
    pub aoe_target: TargetFindAOETarget,

    /// Set by `find_within_area` — origin of the AoE region.
    aoe_target_position: Vector3,
    aoe_target_rotation: f32,

    pub max_distance: f32,
    pub min_distance: f32,
    pub width: f32,
    pub height: f32,
    pub aoe_rotate_angle: f32,
    pub cone_angle: f32,
    pub param: f32,

    targets: Vec<ActorView>,
}

impl TargetFind {
    pub fn new(owner: ActorView, main_target: Option<ActorView>) -> Self {
        let main_target = main_target.unwrap_or(owner);
        Self {
            owner,
            main_target,
            valid_target: ValidTarget::ENEMY,
            aoe_type: TargetFindAOEType::None,
            aoe_target: TargetFindAOETarget::Target,
            aoe_target_position: Vector3::ZERO,
            aoe_target_rotation: 0.0,
            max_distance: 0.0,
            min_distance: 0.0,
            width: 0.0,
            height: 0.0,
            aoe_rotate_angle: 0.0,
            cone_angle: 0.0,
            param: 0.0,
            targets: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        self.main_target = self.owner;
        self.valid_target = ValidTarget::ENEMY;
        self.aoe_type = TargetFindAOEType::None;
        self.aoe_target = TargetFindAOETarget::Target;
        self.aoe_target_position = Vector3::ZERO;
        self.aoe_target_rotation = 0.0;
        self.max_distance = 0.0;
        self.min_distance = 0.0;
        self.width = 0.0;
        self.height = 0.0;
        self.aoe_rotate_angle = 0.0;
        self.cone_angle = 0.0;
        self.param = 0.0;
        self.targets.clear();
    }

    pub fn targets(&self) -> &[ActorView] {
        &self.targets
    }

    /// Port of `SetAOEType(validTarget, aoeType, aoeTarget, maxDistance,
    /// minDistance, height, aoeRotate, coneAngle, param=0)`.
    #[allow(clippy::too_many_arguments)]
    pub fn set_aoe_type(
        &mut self,
        valid_target: ValidTarget,
        aoe_type: TargetFindAOEType,
        aoe_target: TargetFindAOETarget,
        max_distance: f32,
        min_distance: f32,
        height: f32,
        aoe_rotate: f32,
        cone_angle: f32,
    ) {
        self.valid_target = valid_target;
        self.aoe_type = aoe_type;
        self.aoe_target = aoe_target;
        self.max_distance = max_distance;
        self.min_distance = min_distance;
        self.height = height;
        self.aoe_rotate_angle = aoe_rotate;
        self.cone_angle = cone_angle;
    }

    /// Port of `SetAOEBox(validTarget, aoeTarget, length, width, rotate)`.
    pub fn set_aoe_box(
        &mut self,
        valid_target: ValidTarget,
        aoe_target: TargetFindAOETarget,
        length: f32,
        width: f32,
        aoe_rotate_angle: f32,
    ) {
        self.valid_target = valid_target;
        self.aoe_type = TargetFindAOEType::Box;
        self.aoe_target = aoe_target;
        self.aoe_rotate_angle = aoe_rotate_angle;
        self.max_distance = length;
        self.width = width;
    }

    /// Single-target resolver. Matches `FindTarget(target, flags)`.
    pub fn find_target(&mut self, target: ActorView, flags: ValidTarget) {
        self.valid_target = flags;
        self.add_target(target);
    }

    /// Port of `FindWithinArea(target, flags, aoeTarget)`. Iterates the
    /// arena for anything within range of the AoE origin.
    pub fn find_within_area(
        &mut self,
        target: ActorView,
        flags: ValidTarget,
        aoe_target: TargetFindAOETarget,
        arena: &dyn ActorArena,
    ) {
        self.targets.clear();
        self.valid_target = flags;
        self.aoe_target = aoe_target;

        let (origin, origin_rot) = match aoe_target {
            TargetFindAOETarget::Self_ => (
                self.owner.position,
                self.owner.rotation + (self.aoe_rotate_angle * PI),
            ),
            TargetFindAOETarget::Target => (
                target.position,
                target.rotation + (self.aoe_rotate_angle * PI),
            ),
        };
        self.aoe_target_position = origin;
        self.aoe_target_rotation = origin_rot;

        if self.can_target(target, false) {
            self.targets.push(target);
        }

        if self.aoe_type != TargetFindAOEType::None {
            self.add_all_in_range(target, arena);
        }
    }

    fn add_target(&mut self, target: ActorView) {
        if self.can_target(target, false) {
            self.targets.push(target);
        }
    }

    fn add_all_in_range(&mut self, target: ActorView, arena: &dyn ActorArena) {
        let dist = self.max_distance;
        for actor in arena.actors_around(target.actor_id, dist) {
            if actor.actor_id == target.actor_id {
                continue;
            }
            self.add_target(actor);
        }
    }

    /// Full `CanTarget` port — all the bitmask checks followed by the
    /// geometry check.
    pub fn can_target(&self, target: ActorView, ignore_aoe: bool) -> bool {
        // Already targeted?
        if self.targets.iter().any(|a| a.actor_id == target.actor_id) {
            return false;
        }

        // Corpse flags.
        if !self.valid_target.intersects(ValidTarget::CORPSE) && !target.is_alive {
            return false;
        }
        if self.valid_target.intersects(ValidTarget::CORPSE_ONLY) && target.is_alive {
            return false;
        }

        // Self flags.
        let is_self = target.actor_id == self.owner.actor_id;
        if !self.valid_target.intersects(ValidTarget::SELF) && is_self {
            return false;
        }
        if self.valid_target.intersects(ValidTarget::SELF_ONLY) && !is_self {
            return false;
        }

        // Ally / enemy — "Ally" means same allegiance as owner.
        let is_ally = target.allegiance == self.owner.allegiance;
        if !self.valid_target.intersects(ValidTarget::ALLY) && is_ally && !is_self {
            return false;
        }
        if self.valid_target.intersects(ValidTarget::ALLY_ONLY) && !is_ally {
            return false;
        }
        if !self.valid_target.intersects(ValidTarget::ENEMY) && !is_ally {
            return false;
        }
        if self.valid_target.intersects(ValidTarget::ENEMY_ONLY) && is_ally {
            return false;
        }

        // Party flags.
        let in_same_party = self.owner.party_id != 0 && target.party_id == self.owner.party_id;
        if !self.valid_target.intersects(ValidTarget::PARTY) && in_same_party {
            return false;
        }
        if self.valid_target.intersects(ValidTarget::PARTY_ONLY) && !in_same_party {
            return false;
        }

        // NPC flags.
        if !self.valid_target.intersects(ValidTarget::NPC) && target.is_static {
            return false;
        }
        if self.valid_target.intersects(ValidTarget::NPC_ONLY) && !target.is_static {
            return false;
        }

        // Zoning / zone match.
        if target.is_updates_locked {
            return false;
        }
        if target.zone_id != self.owner.zone_id {
            return false;
        }

        // Pure self-only with no AoE — can't splash bystanders.
        if self.valid_target == ValidTarget::SELF && self.aoe_type == TargetFindAOEType::None && !is_self {
            return false;
        }

        // MainTargetParty flags.
        let in_main_party = self.main_target.party_id != 0 && target.party_id == self.main_target.party_id;
        if !self.valid_target.intersects(ValidTarget::MAIN_TARGET_PARTY) && in_main_party && !is_self {
            return false;
        }
        if self.valid_target.intersects(ValidTarget::MAIN_TARGET_PARTY_ONLY) && !in_main_party {
            return false;
        }

        // Geometry.
        if !ignore_aoe {
            if self.param == -1.0 {
                return false;
            }
            match self.aoe_type {
                TargetFindAOEType::None => {
                    // Only the main target survives a None AoE; anything else
                    // would have been skipped earlier.
                    if !self.targets.is_empty() {
                        return false;
                    }
                }
                TargetFindAOEType::Circle => {
                    if !self.is_within_circle(target) {
                        return false;
                    }
                }
                TargetFindAOEType::Cone => {
                    if !self.is_within_cone(target) {
                        return false;
                    }
                }
                TargetFindAOEType::Box => {
                    if !self.is_within_box(target) {
                        return false;
                    }
                }
            }
        }
        true
    }

    // ---- Geometry ---------------------------------------------------------

    fn is_within_circle(&self, target: ActorView) -> bool {
        let dx = target.position.x - self.aoe_target_position.x;
        let dz = target.position.z - self.aoe_target_position.z;
        let dist_sq = dx * dx + dz * dz;
        let max_sq = self.max_distance * self.max_distance;
        let min_sq = self.min_distance * self.min_distance;
        let y_ok = (self.owner.position.y - target.position.y).abs() <= (self.height / 2.0);
        dist_sq <= max_sq && dist_sq >= min_sq && y_ok
    }

    fn is_within_cone(&self, target: ActorView) -> bool {
        if !self.is_within_circle(target) {
            return false;
        }
        // Relative angle from origin to target.
        let dx = target.position.x - self.aoe_target_position.x;
        let dz = target.position.z - self.aoe_target_position.z;
        let angle_to = dx.atan2(dz);
        // Minimum angular gap (wrapped).
        let diff = (angle_to - self.aoe_target_rotation).rem_euclid(std::f32::consts::TAU);
        let gap = diff.min(std::f32::consts::TAU - diff);
        gap <= (self.cone_angle / 2.0)
    }

    fn is_within_box(&self, target: ActorView) -> bool {
        let vx = target.position.x - self.aoe_target_position.x;
        let vz = target.position.z - self.aoe_target_position.z;
        // Rotate into the box's local frame.
        let sin = self.aoe_target_rotation.sin();
        let cos = self.aoe_target_rotation.cos();
        let rel_x = vx * cos - vz * sin;
        let rel_z = vx * sin + vz * cos;
        let half_w = self.width / 2.0;
        let half_h = self.height / 2.0;
        let y_diff = (self.owner.position.y - target.position.y).abs();
        rel_x >= -half_w
            && rel_x <= half_w
            && rel_z >= self.min_distance
            && rel_z <= self.max_distance
            && y_diff <= half_h
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn view(id: u32, x: f32, z: f32, allegiance: u32) -> ActorView {
        ActorView {
            actor_id: id,
            position: Vector3::new(x, 0.0, z),
            rotation: 0.0,
            is_alive: true,
            is_static: false,
            allegiance,
            party_id: 0,
            zone_id: 1,
            is_updates_locked: false,
            is_player: allegiance == 1,
            is_battle_npc: allegiance != 1,
        }
    }

    fn arena() -> HashMap<u32, ActorView> {
        let mut a = HashMap::new();
        a.insert(1, view(1, 0.0, 0.0, 1)); // owner
        a.insert(2, view(2, 5.0, 0.0, 2)); // enemy east
        a.insert(3, view(3, -5.0, 0.0, 2)); // enemy west
        a.insert(4, view(4, 0.0, 15.0, 2)); // enemy far north
        a
    }

    #[test]
    fn circle_aoe_finds_near_enemies() {
        let a = arena();
        let mut tf = TargetFind::new(*a.get(&1).unwrap(), None);
        tf.set_aoe_type(
            ValidTarget::ENEMY,
            TargetFindAOEType::Circle,
            TargetFindAOETarget::Self_,
            10.0,
            0.0,
            10.0,
            0.0,
            0.0,
        );
        tf.find_within_area(*a.get(&2).unwrap(), ValidTarget::ENEMY, TargetFindAOETarget::Self_, &a);
        let ids: Vec<u32> = tf.targets().iter().map(|v| v.actor_id).collect();
        assert!(ids.contains(&2));
        assert!(ids.contains(&3));
        assert!(!ids.contains(&4));
    }

    #[test]
    fn single_target_respects_enemy_flag() {
        let a = arena();
        let mut tf = TargetFind::new(*a.get(&1).unwrap(), None);
        tf.find_target(*a.get(&2).unwrap(), ValidTarget::ENEMY);
        assert_eq!(tf.targets().len(), 1);

        let mut tf = TargetFind::new(*a.get(&1).unwrap(), None);
        tf.find_target(*a.get(&1).unwrap(), ValidTarget::ENEMY);
        assert!(tf.targets().is_empty()); // can't target self when flag is ENEMY
    }

    #[test]
    fn cone_aoe_forward_only() {
        let mut a = arena();
        // Add target behind owner.
        a.insert(5, view(5, 0.0, -5.0, 2));
        let mut tf = TargetFind::new(*a.get(&1).unwrap(), None);
        tf.set_aoe_type(
            ValidTarget::ENEMY,
            TargetFindAOEType::Cone,
            TargetFindAOETarget::Self_,
            10.0,
            0.0,
            10.0,
            0.0,
            std::f32::consts::PI / 2.0,
        );
        // Owner facing rotation=0 (positive-z in our coord system)
        // so target at z=+15 is in front; z=-5 is behind.
        tf.find_within_area(*a.get(&4).unwrap(), ValidTarget::ENEMY, TargetFindAOETarget::Self_, &a);
        let ids: Vec<u32> = tf.targets().iter().map(|v| v.actor_id).collect();
        // Target 4 is at z=15 which is outside the 10-yalm circle; shouldn't match.
        assert!(!ids.contains(&5)); // behind
    }
}
