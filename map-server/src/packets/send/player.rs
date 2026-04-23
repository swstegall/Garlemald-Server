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

//! Player-specific state packets. Most carry a tiny payload (one u32, a bool,
//! or a fixed-size name). The bigger ones (SetCutsceneBook,
//! SetPlayerItemStorage, SetCompletedAchievements) carry structured blobs.

use std::io::Cursor;

use byteorder::{LittleEndian, WriteBytesExt};
use common::luaparam::{self, LuaParam};
use common::subpacket::SubPacket;

use super::super::opcodes::*;
use super::{body, write_padded_ascii};

// --- Achievements -----------------------------------------------------------

/// 0x019E AchievementEarnedPacket.
pub fn build_achievement_earned(actor_id: u32, achievement_id: u32) -> SubPacket {
    let mut data = body(0x28);
    data[..4].copy_from_slice(&achievement_id.to_le_bytes());
    SubPacket::new(OP_ACHIEVEMENT_EARNED, actor_id, data)
}

/// 0x019C SetAchievementPoints.
pub fn build_set_achievement_points(actor_id: u32, num_points: u32) -> SubPacket {
    let mut data = body(0x28);
    data[..4].copy_from_slice(&num_points.to_le_bytes());
    SubPacket::new(OP_SET_ACHIEVEMENT_POINTS, actor_id, data)
}

/// 0x019F SendAchievementRate.
pub fn build_send_achievement_rate(
    actor_id: u32,
    achievement_id: u32,
    progress_count: u32,
    progress_flags: u32,
) -> SubPacket {
    let mut data = body(0x30);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(achievement_id).unwrap();
    c.write_u32::<LittleEndian>(progress_count).unwrap();
    c.write_u32::<LittleEndian>(progress_flags).unwrap();
    SubPacket::new(OP_SEND_ACHIEVEMENT_RATE, actor_id, data)
}

/// 0x019B SetLatestAchievements — 5 recent IDs.
pub fn build_set_latest_achievements(actor_id: u32, latest: &[u32; 5]) -> SubPacket {
    let mut data = body(0x40);
    let mut c = Cursor::new(&mut data[..]);
    for id in latest {
        c.write_u32::<LittleEndian>(*id).unwrap();
    }
    SubPacket::new(OP_SET_LATEST_ACHIEVEMENTS, actor_id, data)
}

/// 0x019A SetCompletedAchievements — packed bitfield of completed-offsets.
/// The C# container struct tracks a `bool[]` of length 0x240.
pub fn build_set_completed_achievements(actor_id: u32, bits: &[bool]) -> SubPacket {
    let mut data = body(0xA0);
    for (i, bit) in bits.iter().take(8 * (data.len())).enumerate() {
        if *bit {
            data[i / 8] |= 1 << (i % 8);
        }
    }
    SubPacket::new(OP_SET_COMPLETED_ACHIEVEMENTS, actor_id, data)
}

// --- Mounts / Chocobo -------------------------------------------------------

/// 0x0197 SetCurrentMountChocobo. Matches Meteor's commit `8687e431`
/// body layout exactly: u32 rentalExpireTime at 0, u8 rentalMinLeft at
/// 4, u8 chocoboAppearance at 5. The earlier `appearance, minLeft,
/// u16 0, expire` ordering in garlemald was out-of-order vs. Meteor and
/// the client would have rendered a garbled mount + timer.
pub fn build_set_current_mount_chocobo(
    actor_id: u32,
    chocobo_appearance: u8,
    rental_expire_time: u32,
    rental_min_left: u8,
) -> SubPacket {
    let mut data = body(0x28);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(rental_expire_time).unwrap();
    c.write_u8(rental_min_left).unwrap();
    c.write_u8(chocobo_appearance).unwrap();
    SubPacket::new(OP_SET_CURRENT_MOUNT_CHOCOBO, actor_id, data)
}

/// 0x01A0 SetCurrentMountGoobbue.
pub fn build_set_current_mount_goobbue(actor_id: u32, appearance_id: i32) -> SubPacket {
    let mut data = body(0x28);
    data[..4].copy_from_slice(&appearance_id.to_le_bytes());
    SubPacket::new(OP_SET_CURRENT_MOUNT_GOOBBUE, actor_id, data)
}

/// 0x0198 SetChocoboName — padded 0x20 ASCII.
pub fn build_set_chocobo_name(actor_id: u32, name: &str) -> SubPacket {
    let mut data = body(0x40);
    let mut c = Cursor::new(&mut data[..]);
    write_padded_ascii(&mut c, name, 0x20);
    SubPacket::new(OP_SET_CHOCOBO_NAME, actor_id, data)
}

/// 0x0199 SetHasChocobo.
pub fn build_set_has_chocobo(actor_id: u32, has_chocobo: bool) -> SubPacket {
    let mut data = body(0x28);
    data[0] = has_chocobo as u8;
    SubPacket::new(OP_SET_HAS_CHOCOBO, actor_id, data)
}

/// 0x01A1 SetHasGoobbue.
pub fn build_set_has_goobbue(actor_id: u32, has_goobbue: bool) -> SubPacket {
    let mut data = body(0x28);
    data[0] = has_goobbue as u8;
    SubPacket::new(OP_SET_HAS_GOOBBUE, actor_id, data)
}

// --- Grand Company / Titles / Misc ------------------------------------------

/// 0x0194 SetGrandCompany. 4 bytes: currentAllegiance, rank_limsa,
/// rank_gridania, rank_uldah — each a single byte, matching Meteor's
/// `Byte()` writes. The prior u16 variant would have stuffed the
/// rank_uldah byte into the low half of a misaligned u16 and left
/// garbage in every nibble past the first byte.
pub fn build_set_grand_company(
    actor_id: u32,
    current_allegiance: u8,
    rank_limsa: u8,
    rank_gridania: u8,
    rank_uldah: u8,
) -> SubPacket {
    let mut data = body(0x28);
    data[0] = current_allegiance;
    data[1] = rank_limsa;
    data[2] = rank_gridania;
    data[3] = rank_uldah;
    SubPacket::new(OP_SET_GRAND_COMPANY, actor_id, data)
}

/// 0x019D SetPlayerTitle.
pub fn build_set_player_title(actor_id: u32, title_id: u32) -> SubPacket {
    let mut data = body(0x28);
    data[..4].copy_from_slice(&title_id.to_le_bytes());
    SubPacket::new(OP_SET_PLAYER_TITLE, actor_id, data)
}

/// 0x01A4 SetCurrentJob.
pub fn build_set_current_job(actor_id: u32, job_id: u32) -> SubPacket {
    let mut data = body(0x28);
    data[..4].copy_from_slice(&job_id.to_le_bytes());
    SubPacket::new(OP_SET_CURRENT_JOB, actor_id, data)
}

/// 0x01A7 SetPlayerDream.
pub fn build_set_player_dream(actor_id: u32, dream_id: u8, inn_id: u8) -> SubPacket {
    let mut data = body(0x28);
    data[0] = dream_id;
    data[1] = inn_id;
    SubPacket::new(OP_SET_PLAYER_DREAM, actor_id, data)
}

/// 0x0196 SetSpecialEventWork. C#
/// `Packets/Send/Player/SetSpecialEventWorkPacket.cs` writes two `UInt16`
/// values at offsets 0x00 and 0x02: `0` (unknown) and `18` (a comment in
/// the C# source notes this is the "Bomb Festival" event code, which
/// unlocks the Bombdance emote). Our earlier builder emitted an empty
/// body, which Project Meteor never does — omitting the 18 leaves the
/// client without a baseline special-event state and its player-init
/// code path waits for this packet before clearing the loading screen.
pub fn build_set_special_event_work(actor_id: u32) -> SubPacket {
    let mut data = body(0x38);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u16::<LittleEndian>(0).unwrap();
    c.write_u16::<LittleEndian>(18).unwrap();
    SubPacket::new(OP_SET_SPECIAL_EVENT_WORK, actor_id, data)
}

/// 0x01A5 SetPlayerItemStorage — empty marker used at login.
pub fn build_set_player_item_storage(actor_id: u32) -> SubPacket {
    SubPacket::new(OP_SET_PLAYER_ITEM_STORAGE, actor_id, body(0x50))
}

/// 0x01A3 SetCutsceneBook — "rewatch-cutscene" inventory the Path
/// Companion NPC uses at inns. Matches Meteor's wire layout exactly:
///
///   offset 0x01  Int16 magic `2`
///   offset 0x03  Byte  padding
///   offset 0x04  Int16 npcActorIdOffset
///   offset 0x06  Byte  npcSkin
///   offset 0x07  Byte  npcPersonality
///   offset 0x08  256 bytes of packed cutscene flags (2048 bits)
///   offset 0x109 null-terminated ASCII npcName
///
/// `cutscene_flags` is `Some(&bits)` with up to 2048 entries (MSB-first
/// per byte, matching `Utils.ConvertBoolArrayToBinaryStream`). Passing
/// `None` leaves the flag region zeroed — used when we just want to
/// refresh the NPC name.
pub fn build_set_cutscene_book(
    actor_id: u32,
    npc_name: &str,
    npc_actor_id_offset: i16,
    npc_skin: u8,
    npc_personality: u8,
    cutscene_flags: Option<&[bool]>,
) -> SubPacket {
    let mut data = body(0x150);
    // Header triple at 0x01.
    data[0x01] = 2;
    data[0x02] = 0;
    data[0x03] = 0;
    let offset_bytes = npc_actor_id_offset.to_le_bytes();
    data[0x04] = offset_bytes[0];
    data[0x05] = offset_bytes[1];
    data[0x06] = npc_skin;
    data[0x07] = npc_personality;
    // Packed flag bitstream at 0x08. 2048 bits / 8 = 256 bytes; clamp
    // if the caller passes fewer.
    if let Some(flags) = cutscene_flags {
        let mut bytes = [0u8; 256];
        for (i, flag) in flags.iter().enumerate().take(2048) {
            if *flag {
                // MSB-first within each byte (matches C# `ConvertBoolArrayToBinaryStream`).
                bytes[i >> 3] |= 0x80 >> (i & 7);
            }
        }
        data[0x08..0x08 + 256].copy_from_slice(&bytes);
    }
    // NPC name at 0x109, null-terminated.
    let bytes = npc_name.as_bytes();
    let max = (data.len().saturating_sub(0x109)).saturating_sub(1); // leave room for null
    let copy = bytes.len().min(max);
    data[0x109..0x109 + copy].copy_from_slice(&bytes[..copy]);
    // Terminator is already 0.
    SubPacket::new(OP_SET_CUTSCENE_BOOK, actor_id, data)
}

// --- Generic data ----------------------------------------------------------

/// 0x0133 GenericDataPacket — free-form lua-param payload.
pub fn build_generic_data(actor_id: u32, params: &[LuaParam]) -> SubPacket {
    let mut data = body(0xE0);
    {
        let mut c = Cursor::new(&mut data[..]);
        let _ = luaparam::write_lua_params(&mut c, params);
    }
    SubPacket::new(OP_GENERIC_DATA, actor_id, data)
}
