//! `ActorClass` — metadata for every NPC class loaded from
//! `gamedata_actor_class` (+ the `gamedata_actor_pushcommand` join).
//! Port of `Actors/Chara/Npc/ActorClass.cs`.
//!
//! The fields are immutable after DB load. Actors reference an
//! `ActorClass` by id; the class supplies the Lua script path, the
//! client-side display name, property bits, and the JSON event-
//! condition map that binds packet opcodes to Lua function names.

#![allow(dead_code)]

#[derive(Debug, Clone, Default)]
pub struct ActorClass {
    pub actor_class_id: u32,
    /// `/Chara/Npc/Populace/PopulaceStandard` etc. Doubles as the Lua
    /// script path.
    pub class_path: String,
    pub display_name_id: u32,
    pub property_flags: u32,
    /// JSON blob: `{"opcode": "functionName", ...}`. Parsed on demand.
    pub event_conditions: String,

    pub push_command: u16,
    pub push_command_sub: u16,
    pub push_command_priority: u8,
}

impl ActorClass {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        actor_class_id: u32,
        class_path: impl Into<String>,
        display_name_id: u32,
        property_flags: u32,
        event_conditions: impl Into<String>,
        push_command: u16,
        push_command_sub: u16,
        push_command_priority: u8,
    ) -> Self {
        Self {
            actor_class_id,
            class_path: class_path.into(),
            display_name_id,
            property_flags,
            event_conditions: event_conditions.into(),
            push_command,
            push_command_sub,
            push_command_priority,
        }
    }

    /// Test a single property-flag bit. Matches the C# bit-style checks.
    pub fn has_property(&self, bit: u8) -> bool {
        debug_assert!(bit < 32);
        (self.property_flags & (1 << bit)) != 0
    }
}
