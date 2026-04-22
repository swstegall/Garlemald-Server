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

//! `NpcWork` — transient per-NPC state. Port of
//! `Actors/Chara/Npc/NpcWork.cs`.

#![allow(dead_code)]

pub const HATE_TYPE_NONE: u8 = 0;
pub const HATE_TYPE_ENGAGED: u8 = 2;
pub const HATE_TYPE_ENGAGED_PARTY: u8 = 3;

#[derive(Debug, Clone)]
pub struct NpcWork {
    pub push_command: u16,
    pub push_command_sub: i32,
    pub push_command_priority: u8,
    /// Defaults to `1` — matches the C# initializer. Actors flip to
    /// `HATE_TYPE_ENGAGED` / `HATE_TYPE_ENGAGED_PARTY` when they grab a
    /// target.
    pub hate_type: u8,
}

impl Default for NpcWork {
    fn default() -> Self {
        Self {
            push_command: 0,
            push_command_sub: 0,
            push_command_priority: 0,
            hate_type: 1,
        }
    }
}

impl NpcWork {
    pub fn new_from_class(push: u16, push_sub: u16, priority: u8) -> Self {
        Self {
            push_command: push,
            push_command_sub: push_sub as i32,
            push_command_priority: priority,
            hate_type: 1,
        }
    }
}
