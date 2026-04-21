//! `Npc` — a non-player Character with an `ActorClass` reference.
//! Port of `Actors/Chara/Npc/Npc.cs`.
//!
//! Composition vs inheritance: the C# has `Npc : Character`. In Rust we
//! own a `Character` as `character: Character` so the game loop can drive
//! the same `status_effects` / `ai_container` paths that `Player` uses.

#![allow(dead_code)]

use std::collections::HashMap;

use crate::actor::Character;
use crate::actor::event_conditions::parse_event_conditions;

use super::actor_class::ActorClass;
use super::npc_work::NpcWork;

/// Spawn-condition bitfield — matches the C# `[Flags] NpcSpawnType`.
/// Determines when a seed actually becomes a live actor in a zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NpcSpawnType(pub u16);

impl NpcSpawnType {
    pub const NORMAL: Self = Self(0x00);
    pub const SCRIPTED: Self = Self(0x01);
    pub const NIGHTTIME: Self = Self(0x02);
    pub const EVENING: Self = Self(0x04);
    pub const DAYTIME: Self = Self(0x08);
    pub const WEATHER: Self = Self(0x10);

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

impl std::ops::BitOr for NpcSpawnType {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Back-compat alias — the old flat map isn't populated anywhere but
/// stays exported so downstream code that only looked at it by name
/// doesn't break. New callers should read
/// `crate::actor::event_conditions::EventConditionList` (also re-exported
/// as `EventConditionList` from this module for convenience).
pub type EventConditionMap = HashMap<String, String>;

// ---------------------------------------------------------------------------
// The NPC itself.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Npc {
    /// Character state (position, stats, status effects, AI container).
    pub character: Character,

    pub actor_class_id: u32,
    pub unique_id: String,
    pub npc_work: NpcWork,
    pub npc_spawn_type: NpcSpawnType,

    /// Cached `classPath` — used by the Lua instantiate packet.
    pub class_path: String,
    /// Short tail of `class_path` after the last `/` — what the client
    /// uses as the script's filename.
    pub class_name: String,

    /// True when the actor is a static map object with a layout binding
    /// rather than a moving NPC.
    pub is_map_obj: bool,
    pub layout: u32,
    pub instance: u32,
}

impl Npc {
    /// `Npc(actorNumber, ActorClass, uniqueId, spawnedArea, posX, posY,
    /// posZ, rot, actorState, animationId, customDisplayName)`. The
    /// composite actor id follows the same `(4 << 28 | areaId << 19 |
    /// actor_number)` formula as the C#.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        actor_number: u32,
        actor_class: &ActorClass,
        unique_id: impl Into<String>,
        area_id: u32,
        x: f32,
        y: f32,
        z: f32,
        rotation: f32,
        actor_state: u16,
        animation_id: u32,
        custom_display_name: Option<String>,
    ) -> Self {
        let composite_id = (4u32 << 28) | (area_id << 19) | (actor_number & 0x7FFFF);
        let mut character = Character::new(composite_id);
        character.base.position_x = x;
        character.base.position_y = y;
        character.base.position_z = z;
        character.base.rotation = rotation;
        character.base.current_main_state = actor_state;
        character.chara.animation_id = animation_id;
        character.base.display_name_id = actor_class.display_name_id;
        if let Some(name) = custom_display_name {
            character.base.custom_display_name = name;
        }
        character.base.zone_id = area_id;

        // Default stat line — the retail code stamps an 80-hp placeholder
        // for plain NPCs; the BattleNpc subtype overrides once it has
        // the real pool/genus data.
        character.chara.hp = 80;
        character.chara.max_hp = 80;
        character.chara.level = 1;

        let class_path = actor_class.class_path.clone();
        let class_name = class_path
            .rsplit('/')
            .next()
            .unwrap_or(&class_path)
            .to_string();
        character.base.class_name = class_name.clone();
        character.base.class_path = class_path.clone();
        // NPCs need an `actor_name` + the gamedata class id for their
        // 0x00CC ActorInstantiate LuaParam tail. Meteor's `Actor.GenerateActorName`
        // (`Map Server/Actors/Actor.cs:501`) formats it as
        //   "<classAbbrev>_<zoneAbbrev>_<numBase63>@<zoneHex:3><privLevel:2>"
        // The zone name isn't in scope here; the spawner writes it in
        // after construction via `Npc::set_generated_actor_name`.
        character.chara.actor_class_id = actor_class.actor_class_id;
        character.chara.property_flags = actor_class.property_flags;

        let npc_work = NpcWork::new_from_class(
            actor_class.push_command,
            actor_class.push_command_sub,
            actor_class.push_command_priority,
        );

        let is_map_obj = Self::class_id_is_map_obj(actor_class.actor_class_id);
        // Parse the `ActorClass::event_conditions` JSON blob into the
        // typed struct and stash it on `BaseActor` so the zone-in
        // spawn emitter can fan the right SetXxxEventCondition packets
        // per trigger. Parse failures leave the list empty, which
        // matches the C# `eventConditions == null` branch in
        // `Actor.GetEventConditionPackets`.
        if !actor_class.event_conditions.is_empty() {
            match parse_event_conditions(&actor_class.event_conditions) {
                Ok(list) => character.base.event_conditions = list,
                Err(err) => tracing::debug!(
                    actor_class = actor_class.actor_class_id,
                    %err,
                    "event_conditions parse failed"
                ),
            }
        }

        Self {
            character,
            actor_class_id: actor_class.actor_class_id,
            unique_id: unique_id.into(),
            npc_work,
            npc_spawn_type: NpcSpawnType::NORMAL,
            class_path,
            class_name,
            is_map_obj,
            layout: 0,
            instance: 0,
        }
    }

    /// `Npc(actorNumber, ActorClass, uniqueId, spawnedArea, posX, posY,
    /// posZ, 0f, regionId, layoutId)` — the map-object overload. Produces
    /// a static actor with layout/instance bindings.
    #[allow(clippy::too_many_arguments)]
    pub fn new_map_object(
        actor_number: u32,
        actor_class: &ActorClass,
        unique_id: impl Into<String>,
        area_id: u32,
        x: f32,
        y: f32,
        z: f32,
        region_id: u32,
        layout_id: u32,
    ) -> Self {
        let mut me = Self::new(
            actor_number,
            actor_class,
            unique_id,
            area_id,
            x,
            y,
            z,
            0.0,
            0,
            0,
            None,
        );
        me.is_map_obj = true;
        me.layout = region_id;
        me.instance = layout_id;
        me.character.chara.is_static = true;
        me
    }

    pub fn is_map_object(&self) -> bool {
        self.is_map_obj
    }

    pub fn unique_id(&self) -> &str {
        &self.unique_id
    }

    pub fn actor_id(&self) -> u32 {
        self.character.base.actor_id
    }

    pub fn actor_class_id(&self) -> u32 {
        self.actor_class_id
    }

    /// Matches the C# "treat these class id ranges as map objects"
    /// heuristic verbatim.
    pub fn class_id_is_map_obj(class_id: u32) -> bool {
        matches!(class_id, 1_080_078..=1_080_080)
            || (1_080_123..=1_080_135).contains(&class_id)
            || (5_000_001..=5_000_090).contains(&class_id)
            || (5_900_001..=5_900_038).contains(&class_id)
    }

    /// Composite name matching `GenerateActorName(actorNumber)` in C#:
    /// `actorName = _zoneId{zoneIdHex}_{classNameTail}_{actorNumHex}`.
    pub fn generate_actor_name(&mut self, actor_number: u32) {
        let hex = format!("{actor_number:05X}");
        self.character.base.actor_name = format!("_npc@{}", hex);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class() -> ActorClass {
        ActorClass::new(
            1_001_234,
            "/Chara/Npc/Populace/PopulaceStandard",
            100,
            0b1010,
            "",
            0,
            0,
            0,
        )
    }

    #[test]
    fn new_npc_builds_composite_actor_id() {
        let c = class();
        let npc = Npc::new(1, &c, "unique_1", 100, 0.0, 0.0, 0.0, 0.0, 0, 0, None);
        // composite = (4<<28) | (100<<19) | 1.
        let expected = (4u32 << 28) | (100u32 << 19) | 1;
        assert_eq!(npc.actor_id(), expected);
        assert_eq!(npc.class_name, "PopulaceStandard");
        assert!(!npc.is_map_object());
    }

    #[test]
    fn map_object_class_id_triggers_static_flag() {
        let c = ActorClass::new(1_080_078, "/Chara/Npc/MapObj/Static", 0, 0, "", 0, 0, 0);
        let npc = Npc::new(1, &c, "static_1", 100, 0.0, 0.0, 0.0, 0.0, 0, 0, None);
        assert!(npc.is_map_object());
    }

    fn class_with_events(blob: &str) -> ActorClass {
        ActorClass::new(
            1_001_234,
            "/Chara/Npc/Populace/PopulaceStandard",
            100,
            0b1010,
            blob,
            0,
            0,
            0,
        )
    }

    #[test]
    fn event_conditions_parse_monster_notice_only() {
        // Verbatim from `gamedata_actor_class` row for the Limsa-opening
        // jellyfish (class 2205403). Populated by Npc::new from the
        // ActorClass's JSON field; landed on BaseActor for the spawn
        // emitter to consume.
        let blob = r#"{
            "talkEventConditions": [],
            "noticeEventConditions": [
                {"unknown1": 0, "unknown2": 1, "conditionName": "noticeEvent"}
            ],
            "emoteEventConditions": [],
            "pushWithCircleEventConditions": []
        }"#;
        let c = class_with_events(blob);
        let npc = Npc::new(1, &c, "x", 100, 0.0, 0.0, 0.0, 0.0, 0, 0, None);
        let ec = &npc.character.base.event_conditions;
        assert_eq!(ec.notice.len(), 1);
        assert_eq!(ec.notice[0].condition_name, "noticeEvent");
        assert_eq!(ec.notice[0].unknown2, 1);
        assert!(ec.talk.is_empty());
    }

    #[test]
    fn event_conditions_parse_push_with_stringified_primitives() {
        // Verbatim from the door-trigger class 1090025 — demonstrates
        // Meteor's stringified `radius` / `outwards` / `silent` values.
        let blob = r#"{
            "talkEventConditions": [],
            "noticeEventConditions": [
                {"unknown1": 0, "unknown2": 1, "conditionName": "noticeEvent"}
            ],
            "pushWithCircleEventConditions": [
                {"radius": "2.0", "outwards": "false", "silent": "false", "conditionName": "pushDefault"}
            ]
        }"#;
        let c = class_with_events(blob);
        let npc = Npc::new(1, &c, "x", 100, 0.0, 0.0, 0.0, 0.0, 0, 0, None);
        let ec = &npc.character.base.event_conditions;
        assert_eq!(ec.push_circle.len(), 1);
        let pc = &ec.push_circle[0];
        assert_eq!(pc.condition_name, "pushDefault");
        assert!((pc.radius - 2.0).abs() < 1e-6);
        assert!(!pc.outwards);
        assert!(!pc.silent);
    }

    #[test]
    fn event_conditions_empty_blob_is_noop() {
        for blob in ["", "{}"] {
            let c = class_with_events(blob);
            let npc = Npc::new(1, &c, "x", 100, 0.0, 0.0, 0.0, 0.0, 0, 0, None);
            assert!(npc.character.base.event_conditions.is_empty());
        }
    }

    #[test]
    fn push_command_hints_copy_from_class() {
        let mut c = class();
        c.push_command = 42;
        c.push_command_sub = 7;
        c.push_command_priority = 3;
        let npc = Npc::new(1, &c, "x", 100, 0.0, 0.0, 0.0, 0.0, 0, 0, None);
        assert_eq!(npc.npc_work.push_command, 42);
        assert_eq!(npc.npc_work.push_command_sub, 7);
        assert_eq!(npc.npc_work.push_command_priority, 3);
    }
}
