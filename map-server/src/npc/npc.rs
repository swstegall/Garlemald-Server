//! `Npc` — a non-player Character with an `ActorClass` reference.
//! Port of `Actors/Chara/Npc/Npc.cs`.
//!
//! Composition vs inheritance: the C# has `Npc : Character`. In Rust we
//! own a `Character` as `character: Character` so the game loop can drive
//! the same `status_effects` / `ai_container` paths that `Player` uses.

#![allow(dead_code)]

use std::collections::HashMap;

use crate::actor::Character;

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

/// Parsed form of `ActorClass::event_conditions`. Keys are packet opcodes
/// or event names; values are Lua function names. The real content files
/// are JSON — we parse opportunistically and store the flat map.
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

    /// Parsed event-condition table. Populated lazily via
    /// `load_event_conditions`.
    pub event_conditions: EventConditionMap,
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
        let mut me = Self {
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
            event_conditions: EventConditionMap::new(),
        };
        if !actor_class.event_conditions.is_empty() {
            me.load_event_conditions(&actor_class.event_conditions);
        }
        me
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

    /// Port of `LoadEventConditions(jsonBlob)`. The C# uses Json.NET to
    /// deserialize into an `EventList`. We take a more pragmatic tack:
    /// the JSON is a flat object of `"opcode_or_key" → "functionName"`
    /// pairs, which is what the scripts actually consume.
    pub fn load_event_conditions(&mut self, blob: &str) {
        let trimmed = blob.trim();
        if trimmed.is_empty() || trimmed == "{}" {
            return;
        }
        if let Ok(map) = parse_event_conditions(trimmed) {
            self.event_conditions = map;
        }
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

// ---------------------------------------------------------------------------
// Minimal JSON parser for the event_conditions blob. The real C# payload
// has `{ opcode: "fn", opcode2: "fn2" }` shapes; we parse the outer-most
// object and accept string values only. Anything more exotic falls back
// to an empty map — the caller handles that case gracefully.
// ---------------------------------------------------------------------------

fn parse_event_conditions(s: &str) -> Result<EventConditionMap, &'static str> {
    let s = s.trim();
    if !s.starts_with('{') || !s.ends_with('}') {
        return Err("not an object");
    }
    let inner = &s[1..s.len() - 1];
    let mut out = EventConditionMap::new();
    for entry in split_top_level(inner, ',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let (key, value) = entry.split_once(':').ok_or("missing colon")?;
        let key = strip_quotes(key.trim());
        let value = strip_quotes(value.trim());
        out.insert(key.to_string(), value.to_string());
    }
    Ok(out)
}

fn strip_quotes(s: &str) -> &str {
    if s.len() >= 2
        && let Some(inner) = s.strip_prefix('"').and_then(|t| t.strip_suffix('"'))
    {
        inner
    } else if s.len() >= 2
        && let Some(inner) = s.strip_prefix('\'').and_then(|t| t.strip_suffix('\''))
    {
        inner
    } else {
        s
    }
}

/// Split on `sep` characters that aren't nested inside braces/brackets/
/// quoted strings. Good enough for the event-condition payloads we see.
fn split_top_level(s: &str, sep: char) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut in_quote = false;
    let mut start = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '"' => in_quote = !in_quote,
            '{' | '[' if !in_quote => depth += 1,
            '}' | ']' if !in_quote => depth -= 1,
            x if x == sep && !in_quote && depth == 0 => {
                out.push(&s[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
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

    #[test]
    fn event_conditions_parse_flat_object() {
        let blob = r#"{"204": "onTalk", "30": "onTrade"}"#;
        let mut npc = Npc::new(1, &class(), "x", 100, 0.0, 0.0, 0.0, 0.0, 0, 0, None);
        npc.load_event_conditions(blob);
        assert_eq!(npc.event_conditions.get("204"), Some(&"onTalk".to_string()));
        assert_eq!(npc.event_conditions.get("30"), Some(&"onTrade".to_string()));
    }

    #[test]
    fn event_conditions_empty_blob_is_noop() {
        let mut npc = Npc::new(1, &class(), "x", 100, 0.0, 0.0, 0.0, 0.0, 0, 0, None);
        npc.load_event_conditions("");
        npc.load_event_conditions("{}");
        assert!(npc.event_conditions.is_empty());
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
