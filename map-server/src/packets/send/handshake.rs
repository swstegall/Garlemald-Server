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
    c.write_u32::<LittleEndian>(utils::unix_timestamp()).unwrap();
    SubPacket::new_with_flag(false, OP_PONG, 0, data)
}

/// OP_PONG_RESPONSE (0x0001) — client ping.
pub fn build_pong(actor_id: u32, ping_ticks: u32) -> SubPacket {
    let mut data = body(0x40);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(actor_id).unwrap();
    c.write_u32::<LittleEndian>(ping_ticks).unwrap();
    SubPacket::new_with_flag(false, OP_PONG_RESPONSE, actor_id, data)
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

/// OP_0XE2 (0x00E2) — mystery client-signalling frame; carries a single int.
pub fn build_0xe2(actor_id: u32, val: i32) -> SubPacket {
    let mut data = body(0x28);
    data[..4].copy_from_slice(&val.to_le_bytes());
    SubPacket::new(OP_0XE2_PACKET, actor_id, data)
}

/// OP_DELETE_ALL_ACTORS (0x0007) — server-initiated world wipe.
pub fn build_delete_all_actors(actor_id: u32) -> SubPacket {
    SubPacket::new_with_flag(false, OP_DELETE_ALL_ACTORS, actor_id, vec![0u8; 8])
}

/// OP_0XF_PACKET (0x000F) — terminator/init marker in the login sequence.
pub fn build_0xf(actor_id: u32) -> SubPacket {
    SubPacket::new_with_flag(false, OP_0XF_PACKET, actor_id, body(0x38))
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
