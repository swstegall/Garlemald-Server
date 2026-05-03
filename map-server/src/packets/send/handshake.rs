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

//! Non-gamemessage frames exchanged during the client handshake/session
//! lifecycle: ping/pong, 0x02, 0x07 delete-all, 0x0F, session begin/end.

use std::io::Cursor;

use byteorder::{LittleEndian, WriteBytesExt};
use common::subpacket::SubPacket;
use common::utils;

use super::super::opcodes::*;
use super::body;

/// OP_PONG (0x0008) — ping reply with actor id + unix timestamp.
pub fn build_ping_response(actor_id: u32) -> SubPacket {
    let mut data = vec![0u8; 8];
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(actor_id).unwrap();
    c.write_u32::<LittleEndian>(utils::unix_timestamp())
        .unwrap();
    SubPacket::new_with_flag(false, OP_PONG, 0, data)
}

/// OP_PONG_RESPONSE (0x0001) — server reply to the client's game-message Ping.
/// Mirrors `Map Server/Packets/Send/PongPacket.cs`: a 0x40-byte game-message
/// subpacket with `pingTicks` at offset 0x00 and the magic constant 0x14D
/// at offset 0x04. Wrap as `SubPacket::new(..)` (game-message form) so the
/// client's game-message reader sees opcode 0x0001; set `target_id` to the
/// session so the world-server's proxy router forwards it back to the
/// client instead of dropping it.
pub fn build_pong(session_id: u32, ping_ticks: u32) -> SubPacket {
    let mut data = body(0x40);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(ping_ticks).unwrap();
    c.write_u32::<LittleEndian>(0x14D).unwrap();
    let mut sub = SubPacket::new(OP_PONG_RESPONSE, session_id, data);
    sub.set_target_id(session_id);
    sub
}

/// OP_HANDSHAKE_RESPONSE (0x0002) — 40-byte canned blob with actor id patched
/// into the first four bytes.
pub fn build_handshake_response(actor_id: u32) -> SubPacket {
    let mut data = vec![
        0x6c, 0x00, 0x00, 0x00, 0xC8, 0xD6, 0xAF, 0x2B, 0x38, 0x2B, 0x5F, 0x26, 0xB8, 0x8D, 0xF0,
        0x2B, 0xC8, 0xFD, 0x85, 0xFE, 0xA8, 0x7C, 0x5B, 0x09, 0x38, 0x2B, 0x5F, 0x26, 0xC8, 0xD6,
        0xAF, 0x2B, 0xB8, 0x8D, 0xF0, 0x2B, 0x88, 0xAF, 0x5E, 0x26,
    ];
    data[0..4].copy_from_slice(&actor_id.to_le_bytes());
    SubPacket::new_with_flag(false, OP_HANDSHAKE_RESPONSE, 0, data)
}

/// OP_HANDSHAKE_RESPONSE (0x0002) in the "map-login" variant — takes a single
/// u32 `val` payload.
pub fn build_0x02(actor_id: u32, val: i32) -> SubPacket {
    let mut data = body(0x30);
    data[..4].copy_from_slice(&val.to_le_bytes());
    SubPacket::new_with_flag(false, OP_HANDSHAKE_RESPONSE, actor_id, data)
}

/// Map-server reply to the client's 0x0002 game-message handshake ack.
/// Matches `Map Server/Packets/Send/Login/0x2Packet.cs`: a 0x10-byte body
/// with `source_id` at offset 0x8, wrapped as a game-message subpacket.
/// Sets `target_id = session_id` so the world server's proxy router
/// (`world-server/src/server.rs`) forwards it back to the right client.
pub fn build_gm_0x02_ack(session_id: u32) -> SubPacket {
    let mut data = body(0x30);
    data[0x08..0x0C].copy_from_slice(&session_id.to_le_bytes());
    let mut sub = SubPacket::new(OP_HANDSHAKE_RESPONSE, session_id, data);
    sub.set_target_id(session_id);
    sub
}

/// OP_0XE2 (0x00E2) — mystery client-signalling frame; carries a single int.
pub fn build_0xe2(actor_id: u32, val: i32) -> SubPacket {
    let mut data = body(0x28);
    data[..4].copy_from_slice(&val.to_le_bytes());
    SubPacket::new(OP_0XE2_PACKET, actor_id, data)
}

/// OP_DELETE_ALL_ACTORS (0x0007) — server-initiated world wipe.
///
/// The wiki names this "Mass Delete Actor End": when sent ALONE it
/// wipes everything, but it can also close a Mass-Delete sequence
/// opened by `build_mass_delete_actor_start` with intervening Body
/// packets that exempt specific actors.
pub fn build_delete_all_actors(actor_id: u32) -> SubPacket {
    SubPacket::new_with_flag(false, OP_DELETE_ALL_ACTORS, actor_id, vec![0u8; 8])
}

/// OP_MASS_DELETE_ACTOR_START (0x0006) — opens a Mass Delete Actor
/// sequence. Body is 8 bytes of zero. Followed by zero or more Body
/// packets (0x0008/0x0009/0x000A/0x000B — the actors listed there
/// are *exempted* from the impending wipe), then closed by
/// `build_delete_all_actors` (0x0007 = "End").
///
/// Captured retail bytes (`ffxiv_traces/from_gridania_to_blackshroud.pcapng`
/// + `gridania_to_coerthas.pcapng`): SubPacket size 0x28, body 8
/// zero bytes. Same opcode as `OP_RX_LANGUAGE_CODE` — direction
/// disambiguates.
pub fn build_mass_delete_actor_start(actor_id: u32) -> SubPacket {
    SubPacket::new(OP_MASS_DELETE_ACTOR_START, actor_id, body(0x28))
}

/// OP_MASS_DELETE_ACTOR_X16 (0x0009) — Mass Delete Actor Body
/// holding up to 16 actor ids to exempt from the impending wipe
/// (wiki labels this "(x10)"; the `x10` is HEX). Body = 80 bytes
/// (16×u32 actor + 16-byte zero pad). Empty slots stay zero.
pub fn build_mass_delete_actor_x16(actor_id: u32, exempt_actors: &[u32]) -> SubPacket {
    build_mass_delete_actor_body(
        actor_id,
        exempt_actors,
        16,
        OP_MASS_DELETE_ACTOR_X16,
        0x70,
    )
}

/// OP_MASS_DELETE_ACTOR_X32 (0x000A) — same shape, 32 actor slots.
/// Body = 160 bytes (32×u32 actor + 32-byte pad).
pub fn build_mass_delete_actor_x32(actor_id: u32, exempt_actors: &[u32]) -> SubPacket {
    build_mass_delete_actor_body(
        actor_id,
        exempt_actors,
        32,
        OP_MASS_DELETE_ACTOR_X32,
        0xC0,
    )
}

/// OP_MASS_DELETE_ACTOR_X64 (0x000B) — same shape, 64 actor slots.
/// Body = 320 bytes (64×u32 actor + 64-byte pad). Not observed in
/// the 56-capture survey, but defined for symmetry.
pub fn build_mass_delete_actor_x64(actor_id: u32, exempt_actors: &[u32]) -> SubPacket {
    build_mass_delete_actor_body(
        actor_id,
        exempt_actors,
        64,
        OP_MASS_DELETE_ACTOR_X64,
        0x160,
    )
}

fn build_mass_delete_actor_body(
    actor_id: u32,
    exempt_actors: &[u32],
    cap: usize,
    opcode: u16,
    packet_size: usize,
) -> SubPacket {
    let mut data = body(packet_size);
    let n = exempt_actors.len().min(cap);
    for (i, exempt) in exempt_actors.iter().take(n).enumerate() {
        let off = i * 4;
        data[off..off + 4].copy_from_slice(&exempt.to_le_bytes());
    }
    SubPacket::new(opcode, actor_id, data)
}

/// OP_0XF_PACKET (0x000F) — terminator/init marker in the login sequence.
/// Built as a game-message subpacket (C# `_0xFPacket` uses the
/// `new SubPacket(OPCODE, ...)` overload, which defaults to isGameMessage=true).
pub fn build_0xf(actor_id: u32) -> SubPacket {
    SubPacket::new(OP_0XF_PACKET, actor_id, body(0x38))
}

/// OP_LOGOUT (0x000E) — server-side logout trigger.
pub fn build_logout(actor_id: u32) -> SubPacket {
    SubPacket::new_with_flag(false, OP_LOGOUT, actor_id, body(0x28))
}

/// OP_QUIT (0x0011) — client-close command.
pub fn build_quit(actor_id: u32) -> SubPacket {
    SubPacket::new_with_flag(false, OP_QUIT, actor_id, body(0x28))
}

// --- Session control (opcode >= 0x1000, non-gamemessage) --------------------

pub fn build_session_begin(session_id: u32, error_code: u16) -> SubPacket {
    let mut data = vec![0u8; 6];
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(session_id).unwrap();
    c.write_u16::<LittleEndian>(error_code).unwrap();
    let mut sub = SubPacket::new_with_flag(false, OP_SESSION_BEGIN, session_id, data);
    sub.set_target_id(session_id);
    sub
}

pub fn build_session_end(session_id: u32, error_code: u16, destination_zone: u32) -> SubPacket {
    let mut data = vec![0u8; 10];
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(session_id).unwrap();
    c.write_u16::<LittleEndian>(error_code).unwrap();
    c.write_u32::<LittleEndian>(destination_zone).unwrap();
    let mut sub = SubPacket::new_with_flag(false, OP_SESSION_END, session_id, data);
    sub.set_target_id(session_id);
    sub
}

#[cfg(test)]
mod mass_delete_actor_tests {
    use super::*;

    /// 0x0006 Mass Delete Actor Start. Body must be 8 zero bytes —
    /// matches `from_gridania_to_blackshroud.pcapng` 0x0006 record #1.
    #[test]
    fn mass_delete_actor_start_matches_retail_capture() {
        let pkt = build_mass_delete_actor_start(0x029B_2941);
        assert_eq!(pkt.data.len(), 8);
        assert!(pkt.data.iter().all(|b| *b == 0));
        assert_eq!(pkt.game_message.opcode, OP_MASS_DELETE_ACTOR_START);
    }

    /// 0x0009 Mass Delete Actor Body x16 — capacity decoded from
    /// `moving_around_gridania.pcapng` 0x0009 record #1 (16 actor IDs
    /// in 80-byte body, no trailing pad). Wiki labels this "(x10)"
    /// where `x10` is HEX (= 16).
    #[test]
    fn mass_delete_actor_x16_writes_actors_in_order() {
        let actors = vec![
            0x029B_2941,
            0x4670_0002,
            0x5FF8_0002,
            0x5FF8_0001,
            0x4670_00C2,
            0x4670_0001,
            0x4670_0004,
            0x4670_004B,
            0x4670_008F,
            0x4670_0090,
            0x4670_0091,
            0x4670_0092,
            0x4670_0093,
            0x4670_0015,
            0x4670_0097,
            0x4670_0026,
        ];
        let pkt = build_mass_delete_actor_x16(0x029B_2941, &actors);
        assert_eq!(pkt.data.len(), 80);
        // First 16 u32 slots populated; trailing 16 bytes zero.
        for (i, expected) in actors.iter().enumerate() {
            let actual = u32::from_le_bytes(pkt.data[i * 4..i * 4 + 4].try_into().unwrap());
            assert_eq!(actual, *expected, "slot {i}");
        }
        assert!(pkt.data[64..80].iter().all(|b| *b == 0));
    }

    /// Truncates extra actors above the per-tier cap.
    #[test]
    fn mass_delete_actor_x16_truncates_overflow() {
        let actors: Vec<u32> = (0..32).map(|i| 0x4670_0000 | i).collect();
        let pkt = build_mass_delete_actor_x16(1, &actors);
        // First 16 fit; the rest are dropped on the wire.
        assert_eq!(
            u32::from_le_bytes(pkt.data[60..64].try_into().unwrap()),
            0x4670_000F
        );
        // Trailing pad is zero (slot 16 onwards is unused → padding).
        assert!(pkt.data[64..80].iter().all(|b| *b == 0));
    }

    #[test]
    fn mass_delete_actor_x32_size_and_first_slot() {
        let pkt = build_mass_delete_actor_x32(1, &[0xDEAD_BEEF]);
        assert_eq!(pkt.data.len(), 160);
        assert_eq!(
            u32::from_le_bytes(pkt.data[0..4].try_into().unwrap()),
            0xDEAD_BEEF
        );
        assert_eq!(pkt.game_message.opcode, OP_MASS_DELETE_ACTOR_X32);
    }

    #[test]
    fn mass_delete_actor_x64_size() {
        let pkt = build_mass_delete_actor_x64(1, &[]);
        assert_eq!(pkt.data.len(), 320);
        assert!(pkt.data.iter().all(|b| *b == 0));
    }
}
