//! Outgoing packet builders for the map server.
//!
//! This is the minimum viable surface needed for login/spawn handshake:
//!   - 0x0002/0x0007/0x0008 handshake echoes (identical to World Server)
//!   - 0x1000/0x1001 session begin/end
//!   - 0x013B ActorInit
//!   - 0x0134 SetActorState
//!   - 0x013D SetActorPosition
//!   - 0x01FD game message
//!
//! The full set of ~200 opcodes is declared in `opcodes.rs`. Builders can be
//! added incrementally without disrupting the processor.
#![allow(dead_code)]

use std::io::{Cursor, Write};

use byteorder::{LittleEndian, WriteBytesExt};
use common::luaparam::{self, LuaParam};
use common::subpacket::SubPacket;
use common::utils;

use super::opcodes::*;

fn write_padded_ascii<W: Write>(w: &mut W, s: &str, width: usize) {
    let bytes = s.as_bytes();
    let n = bytes.len().min(width);
    w.write_all(&bytes[..n]).unwrap();
    for _ in n..width {
        w.write_u8(0).unwrap();
    }
}

// ---------------------------------------------------------------------------
// Handshake frames — mirror World Server send.rs. Duplicated here instead of
// shared because the map server speaks the client protocol directly (so the
// subpackets are not wrapped in a world-session header).
// ---------------------------------------------------------------------------

pub fn build_ping_response(actor_id: u32) -> SubPacket {
    let mut data = vec![0u8; 8];
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(actor_id).unwrap();
    c.write_u32::<LittleEndian>(utils::unix_timestamp()).unwrap();
    SubPacket::new_with_flag(false, OP_PONG, 0, data)
}

pub fn build_handshake_response(actor_id: u32) -> SubPacket {
    // Same 0x28-byte blob as the World Server — the low 4 bytes are the
    // session/actor id and the rest is a captured server challenge.
    let mut data = vec![
        0x6c, 0x00, 0x00, 0x00, 0xC8, 0xD6, 0xAF, 0x2B, 0x38, 0x2B, 0x5F, 0x26, 0xB8, 0x8D, 0xF0,
        0x2B, 0xC8, 0xFD, 0x85, 0xFE, 0xA8, 0x7C, 0x5B, 0x09, 0x38, 0x2B, 0x5F, 0x26, 0xC8, 0xD6,
        0xAF, 0x2B, 0xB8, 0x8D, 0xF0, 0x2B, 0x88, 0xAF, 0x5E, 0x26,
    ];
    data[0..4].copy_from_slice(&actor_id.to_le_bytes());
    SubPacket::new_with_flag(false, OP_HANDSHAKE_RESPONSE, 0, data)
}

// ---------------------------------------------------------------------------
// Session control — opcode >= 0x1000
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Actor packets. All of these are game-message subpackets (type 0x03 with the
// 0x10-byte GameMessageHeader prefix).
// ---------------------------------------------------------------------------

pub fn build_set_actor_state(actor_id: u32, main_state: u8, sub_state: u8) -> SubPacket {
    // Mirrors SetActorStatePacket.cs: single u64 with low byte = main,
    // second byte = sub.
    let combined = (main_state as u64) | ((sub_state as u64) << 8);
    let data = combined.to_le_bytes().to_vec();
    SubPacket::new(OP_SET_ACTOR_STATE, actor_id, data)
}

pub fn build_set_actor_position(
    actor_id: u32,
    sequence: u16,
    x: f32,
    y: f32,
    z: f32,
    rotation: f32,
    move_state: u16,
) -> SubPacket {
    let mut data = vec![0u8; 24];
    let mut c = Cursor::new(&mut data[..]);
    c.write_u16::<LittleEndian>(sequence).unwrap();
    c.write_u16::<LittleEndian>(move_state).unwrap();
    c.write_f32::<LittleEndian>(x).unwrap();
    c.write_f32::<LittleEndian>(y).unwrap();
    c.write_f32::<LittleEndian>(z).unwrap();
    c.write_f32::<LittleEndian>(rotation).unwrap();
    SubPacket::new(OP_SET_ACTOR_POSITION, actor_id, data)
}

pub fn build_actor_init(actor_id: u32, class_name: &str) -> SubPacket {
    // ActorInitPacket.cs writes: [f32 speed, f32 speed2, f32 speed3, u32 unknown,
    // padded 0x20 class name, padded 0x20 script name]. We only need the
    // fields the client validates against today.
    let mut data = vec![0u8; 0x50];
    let mut c = Cursor::new(&mut data[..]);
    c.write_f32::<LittleEndian>(1.0).unwrap();
    c.write_f32::<LittleEndian>(1.0).unwrap();
    c.write_f32::<LittleEndian>(1.0).unwrap();
    c.write_u32::<LittleEndian>(0).unwrap();
    write_padded_ascii(&mut c, class_name, 0x20);
    write_padded_ascii(&mut c, "", 0x20);
    SubPacket::new(OP_ACTOR_INIT, actor_id, data)
}

pub fn build_set_actor_name(actor_id: u32, display_name_id: u32, custom_name: &str) -> SubPacket {
    let mut data = vec![0u8; 4 + 0x20];
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(display_name_id).unwrap();
    write_padded_ascii(&mut c, custom_name, 0x20);
    SubPacket::new(OP_SET_ACTOR_NAME, actor_id, data)
}

pub fn build_set_actor_is_zoning(actor_id: u32, is_zoning: bool) -> SubPacket {
    let data = vec![if is_zoning { 1u8 } else { 0u8 }];
    SubPacket::new(OP_SET_ACTOR_IS_ZONING, actor_id, data)
}

pub fn build_delete_all_actors(actor_id: u32) -> SubPacket {
    SubPacket::new(OP_DELETE_ALL_ACTORS, actor_id, Vec::new())
}

pub fn build_remove_actor(actor_id: u32, target_actor_id: u32) -> SubPacket {
    SubPacket::new(OP_REMOVE_ACTOR, actor_id, target_actor_id.to_le_bytes().to_vec())
}

// ---------------------------------------------------------------------------
// Game message (0x01FD) + chat send (0x00CA).
// ---------------------------------------------------------------------------

pub struct GameMessageOptions {
    pub sender_actor_id: u32,
    pub receiver_actor_id: u32,
    pub text_id: u16,
    pub log: u8,
    pub display_id: Option<u32>,
    pub custom_sender: Option<String>,
    pub lua_params: Vec<LuaParam>,
}

pub fn build_game_message(source_actor_id: u32, opts: GameMessageOptions) -> SubPacket {
    let mut body = Vec::<u8>::with_capacity(0x40 + opts.lua_params.len() * 8);
    body.write_u32::<LittleEndian>(opts.receiver_actor_id).unwrap();
    body.write_u32::<LittleEndian>(opts.sender_actor_id).unwrap();
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

pub const MESSAGE_TYPE_SAY: u8 = 0x01;
pub const MESSAGE_TYPE_SHOUT: u8 = 0x02;
pub const MESSAGE_TYPE_TELL: u8 = 0x03;
pub const MESSAGE_TYPE_PARTY: u8 = 0x04;
pub const MESSAGE_TYPE_LS: u8 = 0x05;
pub const MESSAGE_TYPE_YELL: u8 = 0x1D;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_actor_state_packs_bytes() {
        let sub = build_set_actor_state(0xDEADBEEF, 0x02, 0xBF);
        assert_eq!(sub.header.source_id, 0xDEADBEEF);
        assert_eq!(sub.data, vec![0x02, 0xBF, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn session_begin_shape() {
        let sub = build_session_begin(7, 1);
        assert_eq!(sub.data.len(), 6);
        assert_eq!(sub.header.source_id, 7);
        assert_eq!(sub.header.target_id, 7);
    }

    #[test]
    fn actor_init_packet_is_expected_size() {
        let sub = build_actor_init(1, "player");
        assert_eq!(sub.data.len(), 0x50);
    }
}
