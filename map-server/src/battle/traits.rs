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

//! `BattleTrait` — passive bonus granted by class/level. Ported from
//! `Actors/Chara/Ai/BattleTrait.cs`.

#![allow(dead_code)]

#[derive(Debug, Clone)]
pub struct BattleTrait {
    pub id: u16,
    pub name: String,
    pub job: u8,
    pub level: u8,
    /// `Modifier` ordinal granted by this trait — matches the u32 key used
    /// inside `ModifierMap`.
    pub modifier: u32,
    pub bonus: i32,
}

impl BattleTrait {
    pub fn new(
        id: u16,
        name: impl Into<String>,
        job: u8,
        level: u8,
        modifier: u32,
        bonus: i32,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            job,
            level,
            modifier,
            bonus,
        }
    }
}
