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

/// 0x0197 SetCurrentMountChocobo.
pub fn build_set_current_mount_chocobo(
    actor_id: u32,
    chocobo_appearance: u8,
    rental_expire_time: u32,
    rental_min_left: u8,
) -> SubPacket {
    let mut data = body(0x28);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u8(chocobo_appearance).unwrap();
    c.write_u8(rental_min_left).unwrap();
    c.write_u16::<LittleEndian>(0).unwrap();
    c.write_u32::<LittleEndian>(rental_expire_time).unwrap();
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

/// 0x0194 SetGrandCompany.
pub fn build_set_grand_company(
    actor_id: u32,
    current_allegiance: u16,
    rank_limsa: u16,
    rank_gridania: u16,
    rank_uldah: u16,
) -> SubPacket {
    let mut data = body(0x28);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u16::<LittleEndian>(current_allegiance).unwrap();
    c.write_u16::<LittleEndian>(rank_limsa).unwrap();
    c.write_u16::<LittleEndian>(rank_gridania).unwrap();
    c.write_u16::<LittleEndian>(rank_uldah).unwrap();
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

/// 0x01A3 SetCutsceneBook — last-watched cutscene plus NPC hint.
pub fn build_set_cutscene_book(
    actor_id: u32,
    npc_name: &str,
    npc_actor_id_offset: i16,
    npc_skin: u8,
    npc_personality: u8,
) -> SubPacket {
    let mut data = body(0x150);
    let mut c = Cursor::new(&mut data[..]);
    write_padded_ascii(&mut c, npc_name, 0x20);
    c.write_i16::<LittleEndian>(npc_actor_id_offset).unwrap();
    c.write_u8(npc_skin).unwrap();
    c.write_u8(npc_personality).unwrap();
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
