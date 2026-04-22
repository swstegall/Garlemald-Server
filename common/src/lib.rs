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

//! Shared primitives for the Garlemald FFXIV 1.23b server emulator.
//!
//! Ported from Project Meteor's `Common Class Lib` (C#) for Rust 1.95.
//! Wire-format semantics (packet layout, Blowfish key schedule, endianness)
//! are preserved verbatim so this crate can interoperate with the original
//! client without re-sniffing traffic.

pub mod bitfield;
pub mod bitstream;
pub mod blowfish;
pub mod db;
pub mod error;
pub mod hash_table;
pub mod logging;
pub mod luaparam;
pub mod math;
pub mod migrations;
pub mod packet;
pub mod packet_log;
pub mod subpacket;
pub mod utils;

pub use blowfish::Blowfish;
pub use error::PacketError;
pub use luaparam::LuaParam;
pub use math::Vector3;
pub use packet::{
    BasePacket, BasePacketHeader, PACKET_TYPE_CHAT, PACKET_TYPE_ZONE, wrap_subpackets_in_basepacket,
};
pub use subpacket::{GameMessageHeader, SubPacket, SubPacketHeader};
