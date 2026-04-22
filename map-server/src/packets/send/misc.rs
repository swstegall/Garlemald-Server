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

//! Leftover root-level packets: SetMap/Music/Weather/Dalamud + GameMessage +
//! SendMessage.

use std::io::Cursor;
use std::io::Write as _;

use byteorder::{LittleEndian, WriteBytesExt};
use common::luaparam::{self, LuaParam};
use common::subpacket::SubPacket;

use super::super::opcodes::*;
use super::{body, write_padded_ascii};

/// 0x0005 SetMap — loads a zone/region map on the client side. Wire layout
/// mirrors `Map Server/Packets/Send/SetMapPacket.cs`: `region_id` first,
/// `zone_actor_id` second, then the magic 0x28 at offset 0x08. The C# param
/// names are misleading — its `mapID` parameter actually receives `zone.regionId`
/// and its `regionID` receives `zone.actorId`. Built as a game-message subpacket
/// (the C# `new SubPacket(OPCODE, ...)` overload defaults to `isGameMessage=true`).
pub fn build_set_map(actor_id: u32, region_id: u32, zone_actor_id: u32) -> SubPacket {
    let mut data = body(0x30);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(region_id).unwrap();
    c.write_u32::<LittleEndian>(zone_actor_id).unwrap();
    c.write_u32::<LittleEndian>(0x28).unwrap();
    SubPacket::new(OP_SET_MAP, actor_id, data)
}

/// 0x000C SetMusic. Built as a game-message subpacket — the C# `new SubPacket(
/// OPCODE, ...)` overload defaults to `isGameMessage=true`, so the client
/// expects a type=0x03 frame with the opcode in the game-message header.
pub fn build_set_music(actor_id: u32, music_id: u16, music_track_mode: u16) -> SubPacket {
    let mut data = body(0x28);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u16::<LittleEndian>(music_id).unwrap();
    c.write_u16::<LittleEndian>(music_track_mode).unwrap();
    SubPacket::new(OP_SET_MUSIC, actor_id, data)
}

/// 0x000D SetWeather. Game-message subpacket (same reasoning as SetMusic).
pub fn build_set_weather(actor_id: u32, weather_id: u16, transition_time: u16) -> SubPacket {
    let mut data = body(0x28);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u16::<LittleEndian>(weather_id).unwrap();
    c.write_u16::<LittleEndian>(transition_time).unwrap();
    SubPacket::new(OP_SET_WEATHER, actor_id, data)
}

/// 0x0010 SetDalamud — gating for Dalamud features, one signed byte.
/// Game-message subpacket (same reasoning as SetMusic).
pub fn build_set_dalamud(actor_id: u32, dalamud_level: i8) -> SubPacket {
    let mut data = body(0x28);
    data[0] = dalamud_level as u8;
    SubPacket::new(OP_SET_DALAMUD, actor_id, data)
}

// --- Game messages / chat ---------------------------------------------------

/// Game-message options shared with the older `build_game_message`.
pub struct GameMessageOptions {
    pub sender_actor_id: u32,
    pub receiver_actor_id: u32,
    pub text_id: u16,
    pub log: u8,
    pub display_id: Option<u32>,
    pub custom_sender: Option<String>,
    pub lua_params: Vec<LuaParam>,
}

/// 0x01FD GameMessagePacket (default).
pub fn build_game_message(source_actor_id: u32, opts: GameMessageOptions) -> SubPacket {
    let mut body = Vec::<u8>::with_capacity(0x40 + opts.lua_params.len() * 8);
    body.write_u32::<LittleEndian>(opts.receiver_actor_id)
        .unwrap();
    body.write_u32::<LittleEndian>(opts.sender_actor_id)
        .unwrap();
    body.write_u16::<LittleEndian>(opts.text_id).unwrap();
    body.write_u8(opts.log).unwrap();
    body.write_u8(0).unwrap();
    if let Some(id) = opts.display_id {
        body.write_u32::<LittleEndian>(id).unwrap();
    } else if let Some(ref name) = opts.custom_sender {
        write_padded_ascii(&mut body, name, 0x20);
    }
    luaparam::write_lua_params(&mut body, &opts.lua_params).unwrap();
    SubPacket::new(OP_GAME_MESSAGE, source_actor_id, body)
}

/// 0x0157..0x015B GameMessageWithActor1..5 — actor-scoped variants.
#[allow(clippy::too_many_arguments)]
pub fn build_game_message_with_actors(
    source_actor_id: u32,
    actor_count: u8,
    actors: &[u32; 5],
    text_id: u16,
    log: u8,
    params: &[LuaParam],
) -> SubPacket {
    let opcode = match actor_count {
        1 => OP_GAME_MESSAGE_ACTOR1,
        2 => OP_GAME_MESSAGE_ACTOR2,
        3 => OP_GAME_MESSAGE_ACTOR3,
        4 => OP_GAME_MESSAGE_ACTOR4,
        _ => OP_GAME_MESSAGE_ACTOR5,
    };
    let mut body = Vec::<u8>::with_capacity(0x40 + params.len() * 8);
    for i in 0..actor_count.min(5) {
        body.write_u32::<LittleEndian>(actors[i as usize]).unwrap();
    }
    body.write_u16::<LittleEndian>(text_id).unwrap();
    body.write_u8(log).unwrap();
    body.write_u8(0).unwrap();
    luaparam::write_lua_params(&mut body, params).unwrap();
    SubPacket::new(opcode, source_actor_id, body)
}

pub const MESSAGE_TYPE_SAY: u8 = 0x01;
pub const MESSAGE_TYPE_SHOUT: u8 = 0x02;
pub const MESSAGE_TYPE_TELL: u8 = 0x03;
pub const MESSAGE_TYPE_PARTY: u8 = 0x04;
pub const MESSAGE_TYPE_LS: u8 = 0x05;
pub const MESSAGE_TYPE_YELL: u8 = 0x1D;
pub const MESSAGE_TYPE_SYSTEM: u8 = 0x20;
pub const MESSAGE_TYPE_SYSTEM_ERROR: u8 = 0x21;

/// 0x00CA SendMessagePacket — the general chat relay.
pub fn build_send_message(
    source_session: u32,
    target_session: u32,
    message_type: u8,
    sender_name: &str,
    message: &str,
) -> SubPacket {
    let mut body = Vec::<u8>::with_capacity(0x40 + message.len());
    body.write_u64::<LittleEndian>(0).unwrap();
    body.write_u32::<LittleEndian>(0).unwrap();
    body.write_u8(message_type).unwrap();
    body.write_u8(0).unwrap();
    body.write_u16::<LittleEndian>(0).unwrap();
    write_padded_ascii(&mut body, sender_name, 0x20);
    body.write_all(message.as_bytes()).unwrap();
    body.write_u8(0).unwrap();
    let mut sub = SubPacket::new(OP_SEND_MESSAGE, source_session, body);
    sub.set_target_id(target_session);
    sub
}

/// 0x0003 SendMessagePublic — system-wide (login greetings, shutdown notice).
pub fn build_send_message_public(
    source_actor_id: u32,
    message_type: u32,
    sender: &str,
    message: &str,
) -> SubPacket {
    let mut body = Vec::<u8>::with_capacity(0x248);
    body.write_u32::<LittleEndian>(message_type).unwrap();
    write_padded_ascii(&mut body, sender, 0x20);
    write_padded_ascii(&mut body, message, 0x200);
    SubPacket::new_with_flag(false, OP_SEND_MESSAGE_PUBLIC, source_actor_id, body)
}
