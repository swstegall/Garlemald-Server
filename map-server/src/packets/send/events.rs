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

//! Event flow packets. Scripts kick events with `player:KickEvent(actor,
//! trigger, …)`, scripts advance them with `player:RunEventFunction(fn, …)`,
//! and `player:EndEvent()` closes them out.

use std::io::Cursor;

use byteorder::{LittleEndian, WriteBytesExt};
use common::luaparam::{self, LuaParam};
use common::subpacket::SubPacket;

use super::super::opcodes::*;
use super::{body, write_padded_ascii};

/// 0x0131 EndEventPacket — scripted event teardown.
pub fn build_end_event(
    source_player: u32,
    event_owner_actor_id: u32,
    event_name: &str,
    event_type: u8,
) -> SubPacket {
    let mut data = body(0x50);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(event_owner_actor_id).unwrap();
    c.write_u8(event_type).unwrap();
    c.write_u8(0).unwrap();
    c.write_u16::<LittleEndian>(0).unwrap();
    write_padded_ascii(&mut c, event_name, 0x20);
    SubPacket::new(OP_END_EVENT, source_player, data)
}

/// 0x012F KickEventPacket — tells the client to start a scripted event
/// ("noticeEvent", "talkDefault", etc.) owned by a target actor. On the
/// tutorial login path the server emits this against the OpeningDirector
/// to kick off the intro cutscene.
///
/// Byte layout mirrors C# `Map Server/Packets/Send/Events/KickEventPacket.cs`
/// exactly — the earlier port omitted the trigger id + magic bytes and
/// used a fixed-width event-name field; both must be rewritten for the
/// 1.23b client to dispatch the event.
/// - 0x00..0x04: `trigger_actor_id` (u32)
/// - 0x04..0x08: `owner_actor_id`   (u32)
/// - 0x08     : `event_type`       (u8, `5` from `Player.KickEvent`, `0`
///                                  from `KickEventSpecial`)
/// - 0x09     : 0x17                (C# magic byte)
/// - 0x0A..0x0C: 0x75DC             (C# magic u16)
/// - 0x0C..0x10: 0x30400000         (C# server codes)
/// - 0x10..?  : null-terminated event name
/// - 0x30..   : Lua-param stream
pub fn build_kick_event(
    trigger_actor_id: u32,
    owner_actor_id: u32,
    event_name: &str,
    event_type: u8,
    lua_params: &[LuaParam],
) -> SubPacket {
    let mut data = body(0x90);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(trigger_actor_id).unwrap();
    c.write_u32::<LittleEndian>(owner_actor_id).unwrap();
    c.write_u8(event_type).unwrap();
    c.write_u8(0x17).unwrap();
    c.write_u16::<LittleEndian>(0x75DC).unwrap();
    c.write_u32::<LittleEndian>(0x3040_0000).unwrap();
    // Null-terminated event name starting at 0x10. C# uses
    // `Utils.WriteNullTermString` which writes the bytes followed by a
    // single 0 terminator. Body is zero-initialised so the terminator
    // is implicit as long as we don't write past it.
    let name_bytes = event_name.as_bytes();
    let max_name_len = 0x30usize - 0x10 - 1; // leave room for the NUL
    let n = name_bytes.len().min(max_name_len);
    use std::io::Write as _;
    c.write_all(&name_bytes[..n]).unwrap();
    // Lua params land at 0x30 regardless of event-name length.
    c.set_position(0x30);
    let _ = luaparam::write_lua_params(&mut c, lua_params);
    SubPacket::new(OP_KICK_EVENT, trigger_actor_id, data)
}

/// 0x0130 RunEventFunctionPacket.
pub fn build_run_event_function(
    trigger_actor_id: u32,
    owner_actor_id: u32,
    event_name: &str,
    event_type: u8,
    function_name: &str,
    lua_params: &[LuaParam],
) -> SubPacket {
    let mut data = body(0x2B8);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(owner_actor_id).unwrap();
    c.write_u8(event_type).unwrap();
    c.write_u8(0).unwrap();
    c.write_u16::<LittleEndian>(0).unwrap();
    write_padded_ascii(&mut c, event_name, 0x20);
    write_padded_ascii(&mut c, function_name, 0x20);
    let _ = luaparam::write_lua_params(&mut c, lua_params);
    SubPacket::new(OP_RUN_EVENT_FUNCTION, trigger_actor_id, data)
}
