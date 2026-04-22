// garlemald-server — Rust port of a FINAL FANTASY XIV v1.23b server emulator (lobby/world/map)
// Copyright (C) 2026  Samuel Stegall
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: AGPL-3.0-or-later

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
