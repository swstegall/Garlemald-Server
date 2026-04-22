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

//! Per-player friendlist + blacklist state. Retail keeps these as
//! small lists with name + id + online flag.

#![allow(dead_code)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendlistEntry {
    pub character_id: u64,
    pub name: String,
    pub is_online: bool,
}

impl FriendlistEntry {
    pub fn new(character_id: u64, name: impl Into<String>, is_online: bool) -> Self {
        Self {
            character_id,
            name: name.into(),
            is_online,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlacklistEntry {
    pub name: String,
}

impl BlacklistEntry {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}
