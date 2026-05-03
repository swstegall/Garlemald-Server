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

// ---------------------------------------------------------------------------
// 0x0166-0x016A "Text Sheet Message (No Source Actor)" family — system
// messages routed through a static sender (WorldMaster, gamedata id, etc.)
// rather than a runtime actor in the world.
//
// Wire format (per retail bytes from `ffxiv_traces/gather_wood.pcapng`,
// `ffxiv_traces/accept_quest.pcapng`, etc., decoded via
// `packet-diff/cargo run --bin pcap-survey -- … --dump-opcode 0x016X`):
//
//   u32 sender_actor_id   (4 bytes — typically 0x5FF80001 WorldMaster
//                          or a 0xA0F-prefixed static gamedata id)
//   u16 text_id           (2 bytes — index into the client's text-sheet
//                          table)
//   u8  log_flag          (1 byte — captured 0x20, matches the existing
//                          MESSAGE_TYPE_SYSTEM constant for system log)
//   u8  pad               (1 byte, zero)
//   LuaParams             (variable — 0..N tiers per opcode, see table)
//
// Tier table (size figures are SubPacket total = 0x10 header + 0x10 GMHeader
// + body):
//   0x0166 (28b) — body  8, params capacity  0  — header-only message
//   0x0167 (38b) — body 24, params capacity 16  — ~2 params
//   0x0168 (38b) — body 24, params capacity 16  — ~2 params (alt routing,
//                   captured in different captures than 0x0167; no
//                   semantic difference confirmed yet)
//   0x0169 (48b) — body 40, params capacity 32  — ~4 params
//   0x016A (68b) — body 72, params capacity 64  — ~8 params
//
// Project Meteor never implemented this family. Garlemald's existing
// `build_game_message_with_actors` covers the 0x0157-0x015B "Source Actor"
// variants but those require a runtime actor as the message subject;
// the No-Source variants are what retail uses for system feedback like
// "You harvest a Maple Log", "Quest accepted", etc.

/// Common 8-byte header for the Text Sheet (No Source Actor) family.
fn write_text_sheet_no_source_header(
    out: &mut Vec<u8>,
    sender_actor_id: u32,
    text_id: u16,
    log_flag: u8,
) {
    out.write_u32::<LittleEndian>(sender_actor_id).unwrap();
    out.write_u16::<LittleEndian>(text_id).unwrap();
    out.write_u8(log_flag).unwrap();
    out.write_u8(0).unwrap();
}

/// 0x0166 Text Sheet Message (No Source Actor) (28b) — header only;
/// no LuaParams. Smallest tier; the simplest "fire a system text id"
/// emission.
pub fn build_text_sheet_no_source_x28(
    receiver_actor_id: u32,
    sender_actor_id: u32,
    text_id: u16,
    log_flag: u8,
) -> SubPacket {
    let mut body_buf = Vec::<u8>::with_capacity(8);
    write_text_sheet_no_source_header(&mut body_buf, sender_actor_id, text_id, log_flag);
    let mut data = body(0x28);
    data[..body_buf.len()].copy_from_slice(&body_buf);
    SubPacket::new(OP_TEXT_SHEET_NO_ACTOR_X28, receiver_actor_id, data)
}

/// 0x0167 Text Sheet Message (No Source Actor) (38b). Up to 16 bytes of
/// LuaParams (~2 typical 8-byte params).
pub fn build_text_sheet_no_source_x38(
    receiver_actor_id: u32,
    sender_actor_id: u32,
    text_id: u16,
    log_flag: u8,
    lua_params: &[LuaParam],
) -> SubPacket {
    build_text_sheet_no_source_n(
        receiver_actor_id,
        sender_actor_id,
        text_id,
        log_flag,
        lua_params,
        OP_TEXT_SHEET_NO_ACTOR_X38,
        0x38,
    )
}

/// 0x0168 Text Sheet Message (No Source Actor) (38b alt). Same body
/// size as 0x0167; the captures don't reveal an unambiguous semantic
/// distinction. Captured in different feature areas than 0x0167
/// (`gather_wood`, `harvest`, `local_leve_complete` for 0x0168 vs.
/// `accept_leve`, `accept_quest`, `sell_item` for 0x0167). Caller
/// picks based on the message's intended display / log routing.
pub fn build_text_sheet_no_source_x38_alt(
    receiver_actor_id: u32,
    sender_actor_id: u32,
    text_id: u16,
    log_flag: u8,
    lua_params: &[LuaParam],
) -> SubPacket {
    build_text_sheet_no_source_n(
        receiver_actor_id,
        sender_actor_id,
        text_id,
        log_flag,
        lua_params,
        OP_TEXT_SHEET_NO_ACTOR_X38_ALT,
        0x38,
    )
}

/// 0x0169 Text Sheet Message (No Source Actor) (48b). Up to 32 bytes
/// of LuaParams.
pub fn build_text_sheet_no_source_x48(
    receiver_actor_id: u32,
    sender_actor_id: u32,
    text_id: u16,
    log_flag: u8,
    lua_params: &[LuaParam],
) -> SubPacket {
    build_text_sheet_no_source_n(
        receiver_actor_id,
        sender_actor_id,
        text_id,
        log_flag,
        lua_params,
        OP_TEXT_SHEET_NO_ACTOR_X48,
        0x48,
    )
}

/// 0x016A Text Sheet Message (No Source Actor) (68b). Up to 64 bytes
/// of LuaParams. Not observed in the survey but defined for symmetry.
pub fn build_text_sheet_no_source_x68(
    receiver_actor_id: u32,
    sender_actor_id: u32,
    text_id: u16,
    log_flag: u8,
    lua_params: &[LuaParam],
) -> SubPacket {
    build_text_sheet_no_source_n(
        receiver_actor_id,
        sender_actor_id,
        text_id,
        log_flag,
        lua_params,
        OP_TEXT_SHEET_NO_ACTOR_X68,
        0x68,
    )
}

fn build_text_sheet_no_source_n(
    receiver_actor_id: u32,
    sender_actor_id: u32,
    text_id: u16,
    log_flag: u8,
    lua_params: &[LuaParam],
    opcode: u16,
    packet_size: usize,
) -> SubPacket {
    let mut body_buf = Vec::<u8>::with_capacity(packet_size.saturating_sub(0x20));
    write_text_sheet_no_source_header(&mut body_buf, sender_actor_id, text_id, log_flag);
    luaparam::write_lua_params(&mut body_buf, lua_params).unwrap();
    let mut data = body(packet_size);
    let n = body_buf.len().min(data.len());
    data[..n].copy_from_slice(&body_buf[..n]);
    SubPacket::new(opcode, receiver_actor_id, data)
}

/// Convenience: pick the smallest tier that fits the LuaParam payload.
/// Captures show retail uses 0x0167 vs. 0x0168 with the same body size
/// for routing reasons — the auto-tier picker defaults to the
/// "primary" 0x0167 / 0x0168 style based on `prefer_alt`.
pub fn build_text_sheet_no_source_auto(
    receiver_actor_id: u32,
    sender_actor_id: u32,
    text_id: u16,
    log_flag: u8,
    lua_params: &[LuaParam],
    prefer_alt: bool,
) -> SubPacket {
    if lua_params.is_empty() {
        return build_text_sheet_no_source_x28(receiver_actor_id, sender_actor_id, text_id, log_flag);
    }
    // Probe param byte length by serializing into a temp buffer.
    let mut probe = Vec::<u8>::new();
    luaparam::write_lua_params(&mut probe, lua_params).unwrap();
    let p_len = probe.len();
    if p_len <= 16 {
        return if prefer_alt {
            build_text_sheet_no_source_x38_alt(
                receiver_actor_id,
                sender_actor_id,
                text_id,
                log_flag,
                lua_params,
            )
        } else {
            build_text_sheet_no_source_x38(
                receiver_actor_id,
                sender_actor_id,
                text_id,
                log_flag,
                lua_params,
            )
        };
    }
    if p_len <= 32 {
        return build_text_sheet_no_source_x48(
            receiver_actor_id,
            sender_actor_id,
            text_id,
            log_flag,
            lua_params,
        );
    }
    build_text_sheet_no_source_x68(
        receiver_actor_id,
        sender_actor_id,
        text_id,
        log_flag,
        lua_params,
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Reproduce the body bytes of `gather_wood.pcapng` 0x0166 record #1
    /// — sender = 0xA0F4E204 (gamedata static actor), text_id = 0x0024
    /// (decimal 36), log_flag = 0x20 (system message). Header-only,
    /// no LuaParams.
    #[test]
    fn text_sheet_no_source_x28_matches_retail_capture() {
        let pkt = build_text_sheet_no_source_x28(0x029B_2941, 0xA0F4_E204, 0x0024, 0x20);
        assert_eq!(pkt.data.len(), 8);
        assert_eq!(
            pkt.data,
            [0x04, 0xE2, 0xF4, 0xA0, 0x24, 0x00, 0x20, 0x00]
        );
        assert_eq!(pkt.game_message.opcode, OP_TEXT_SHEET_NO_ACTOR_X28);
    }

    /// Verify the 8-byte header for the larger tiers — captured retail
    /// 0x0167 record from `accept_quest.pcapng`:
    ///   sender = 0x5FF80001 (WorldMaster), text_id = 0x6288, log = 0x20.
    #[test]
    fn text_sheet_no_source_x38_header_matches_retail() {
        let pkt = build_text_sheet_no_source_x38(0x029B_2941, 0x5FF8_0001, 0x6288, 0x20, &[]);
        assert_eq!(pkt.data.len(), 24);
        assert_eq!(pkt.game_message.opcode, OP_TEXT_SHEET_NO_ACTOR_X38);
        assert_eq!(
            &pkt.data[..8],
            &[0x01, 0x00, 0xF8, 0x5F, 0x88, 0x62, 0x20, 0x00]
        );
    }

    #[test]
    fn text_sheet_no_source_x38_alt_uses_separate_opcode() {
        let pkt = build_text_sheet_no_source_x38_alt(0x029B_2941, 0x5FF8_0001, 1, 0x20, &[]);
        assert_eq!(pkt.data.len(), 24);
        assert_eq!(pkt.game_message.opcode, OP_TEXT_SHEET_NO_ACTOR_X38_ALT);
    }

    #[test]
    fn text_sheet_no_source_x48_size() {
        let pkt = build_text_sheet_no_source_x48(0x029B_2941, 0x5FF8_0001, 1, 0x20, &[]);
        assert_eq!(pkt.data.len(), 40);
        assert_eq!(pkt.game_message.opcode, OP_TEXT_SHEET_NO_ACTOR_X48);
    }

    #[test]
    fn text_sheet_no_source_x68_size() {
        let pkt = build_text_sheet_no_source_x68(0x029B_2941, 0x5FF8_0001, 1, 0x20, &[]);
        assert_eq!(pkt.data.len(), 72);
        assert_eq!(pkt.game_message.opcode, OP_TEXT_SHEET_NO_ACTOR_X68);
    }

    #[test]
    fn text_sheet_no_source_auto_picks_smallest_tier() {
        // No params → 0x0166 (28b)
        let p0 = build_text_sheet_no_source_auto(1, 2, 3, 0x20, &[], false);
        assert_eq!(p0.game_message.opcode, OP_TEXT_SHEET_NO_ACTOR_X28);
        assert_eq!(p0.data.len(), 8);

        // One Int32 param → 0x0167 (38b)
        let p1 = build_text_sheet_no_source_auto(
            1,
            2,
            3,
            0x20,
            &[LuaParam::Int32(42)],
            false,
        );
        assert_eq!(p1.game_message.opcode, OP_TEXT_SHEET_NO_ACTOR_X38);
        assert_eq!(p1.data.len(), 24);

        // prefer_alt swaps to 0x0168.
        let p1a = build_text_sheet_no_source_auto(
            1,
            2,
            3,
            0x20,
            &[LuaParam::Int32(42)],
            true,
        );
        assert_eq!(p1a.game_message.opcode, OP_TEXT_SHEET_NO_ACTOR_X38_ALT);

        // Many params → 0x0169 (48b) or 0x016A (68b).
        let many = vec![LuaParam::Int32(1); 4]; // 4 × 6 bytes + 1 LUA_END = 25 bytes
        let pn = build_text_sheet_no_source_auto(1, 2, 3, 0x20, &many, false);
        assert_eq!(pn.game_message.opcode, OP_TEXT_SHEET_NO_ACTOR_X48);

        let huge = vec![LuaParam::Int32(1); 8]; // 8 × 6 + 1 = 49 bytes
        let ph = build_text_sheet_no_source_auto(1, 2, 3, 0x20, &huge, false);
        assert_eq!(ph.game_message.opcode, OP_TEXT_SHEET_NO_ACTOR_X68);
    }
}
