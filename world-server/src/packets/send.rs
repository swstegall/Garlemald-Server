//! Outgoing world-server subpackets. Byte-for-byte compatible with the C#
//! builders. Each function produces a single `SubPacket` that the caller
//! wraps in a `BasePacket` before transmission.
//!
//! A subset of constants / helpers (`MESSAGE_TYPE_*`, `GameMessageOptions`,
//! `build_error`) are part of the public wire surface but not yet consumed
//! from the processor — Map Server phase will light those up.
#![allow(dead_code)]

use std::io::{Cursor, Write};

use byteorder::{LittleEndian, WriteBytesExt};
use common::luaparam::{self, LuaParam};
use common::subpacket::SubPacket;
use common::utils;

fn write_padded_ascii<W: Write>(w: &mut W, s: &str, width: usize) {
    let bytes = s.as_bytes();
    let n = bytes.len().min(width);
    w.write_all(&bytes[..n]).unwrap();
    for _ in n..width {
        w.write_u8(0).unwrap();
    }
}

// ---------------------------------------------------------------------------
// Raw client-frame subpackets (non-gamemessage; bypass the 0x10 game-message
// header). These all use SubPacket::new_with_flag(false, ...).
// ---------------------------------------------------------------------------

pub const OP_CLIENT_0X2: u16 = 0x0002;
pub const OP_CLIENT_0X7: u16 = 0x0007;
pub const OP_CLIENT_0X8: u16 = 0x0008;

/// `_0x2Packet` — handshake-complete response. Fixed-layout 0x28-byte blob,
/// with the caller's actor id patched into the first 4 bytes.
pub fn build_0x2_packet(actor_id: u32) -> SubPacket {
    let mut data = vec![
        0x6c, 0x00, 0x00, 0x00, 0xC8, 0xD6, 0xAF, 0x2B, 0x38, 0x2B, 0x5F, 0x26, 0xB8, 0x8D, 0xF0,
        0x2B, 0xC8, 0xFD, 0x85, 0xFE, 0xA8, 0x7C, 0x5B, 0x09, 0x38, 0x2B, 0x5F, 0x26, 0xC8, 0xD6,
        0xAF, 0x2B, 0xB8, 0x8D, 0xF0, 0x2B, 0x88, 0xAF, 0x5E, 0x26,
    ];
    data[0..4].copy_from_slice(&actor_id.to_le_bytes());
    SubPacket::new_with_flag(false, OP_CLIENT_0X2, 0, data)
}

/// `_0x7Packet` — handshake stage; actor id + unix timestamp.
pub fn build_0x7_packet(actor_id: u32) -> SubPacket {
    let mut data = vec![0u8; 0x08];
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(actor_id).unwrap();
    c.write_u32::<LittleEndian>(utils::unix_timestamp()).unwrap();
    SubPacket::new_with_flag(false, OP_CLIENT_0X7, 0, data)
}

/// `_0x8PingPacket` — keep-alive ping reply. Same 8-byte layout as 0x7.
pub fn build_0x8_ping_packet(actor_id: u32) -> SubPacket {
    let mut data = vec![0u8; 0x08];
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(actor_id).unwrap();
    c.write_u32::<LittleEndian>(utils::unix_timestamp()).unwrap();
    SubPacket::new_with_flag(false, OP_CLIENT_0X8, 0, data)
}

// ---------------------------------------------------------------------------
// World↔zone control frames (opcodes >= 0x1000, NO gamemessage header)
// ---------------------------------------------------------------------------

pub const OP_SESSION_BEGIN: u16 = 0x1000;
pub const OP_SESSION_END: u16 = 0x1001;
pub const OP_ERROR: u16 = 0x100A;
pub const OP_LINKSHELL_RESULT: u16 = 0x1025;

pub fn build_session_begin(session_id: u32, is_login: bool) -> SubPacket {
    // C# writes a single 1-byte flag when is_login, otherwise zeroes.
    let data = vec![if is_login { 1u8 } else { 0u8 }; 1];
    let mut sub = SubPacket::new_with_flag(false, OP_SESSION_BEGIN, session_id, data);
    sub.set_target_id(session_id);
    sub
}

pub fn build_session_end(session_id: u32) -> SubPacket {
    let data = vec![0u8; 4];
    let mut sub = SubPacket::new_with_flag(false, OP_SESSION_END, session_id, data);
    sub.set_target_id(session_id);
    sub
}

pub fn build_session_end_with_zone(
    session_id: u32,
    destination_zone_id: u32,
    spawn_type: u8,
    x: f32,
    y: f32,
    z: f32,
    rotation: f32,
) -> SubPacket {
    let mut data = vec![0u8; 22];
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(destination_zone_id).unwrap();
    c.write_u16::<LittleEndian>(spawn_type as u16).unwrap();
    c.write_f32::<LittleEndian>(x).unwrap();
    c.write_f32::<LittleEndian>(y).unwrap();
    c.write_f32::<LittleEndian>(z).unwrap();
    c.write_f32::<LittleEndian>(rotation).unwrap();
    let mut sub = SubPacket::new_with_flag(false, OP_SESSION_END, session_id, data);
    sub.set_target_id(session_id);
    sub
}

pub fn build_error(session_id: u32, error_code: u32) -> SubPacket {
    let mut data = vec![0u8; 4];
    data[..4].copy_from_slice(&error_code.to_le_bytes());
    let mut sub = SubPacket::new_with_flag(false, OP_ERROR, session_id, data);
    sub.set_target_id(session_id);
    sub
}

pub fn build_linkshell_result(session_id: u32, result: i32) -> SubPacket {
    let mut data = vec![0u8; 4];
    data[..4].copy_from_slice(&result.to_le_bytes());
    let mut sub = SubPacket::new_with_flag(false, OP_LINKSHELL_RESULT, session_id, data);
    sub.set_target_id(session_id);
    sub
}

// ---------------------------------------------------------------------------
// GameMessagePacket (opcode 0x03 subpacket type, carries the game-message
// header). This is THE packet the server uses to speak in the chat/event log.
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

/// Build a game-message subpacket (opcode 0x1FD).
pub fn build_game_message(source_actor_id: u32, opts: GameMessageOptions) -> SubPacket {
    // Layout mirrored from World Server/Packets/Send/Subpackets/GameMessagePacket.cs:
    //   u32 receiver, u32 sender, u16 textId, u8 log, u8 pad, [optional u32 displayId
    //   or padded 0x20 custom-sender string], then luaparams.
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

    SubPacket::new(0x01FD, source_actor_id, body)
}

// ---------------------------------------------------------------------------
// SendMessagePacket (opcode 0xCA) — chat relay.
// ---------------------------------------------------------------------------

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
    // Original C# packs: [u64 0, u32 0, u8 type, u8 0, u16 0, name(0x20), text].
    let mut body = Vec::<u8>::with_capacity(0x40 + message.len());
    body.write_u64::<LittleEndian>(0).unwrap();
    body.write_u32::<LittleEndian>(0).unwrap();
    body.write_u8(message_type).unwrap();
    body.write_u8(0).unwrap();
    body.write_u16::<LittleEndian>(0).unwrap();
    write_padded_ascii(&mut body, sender_name, 0x20);
    body.write_all(message.as_bytes()).unwrap();
    body.write_u8(0).unwrap();

    let mut sub = SubPacket::new(0x00CA, source_session, body);
    sub.set_target_id(target_session);
    sub
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshake_0x2_patches_actor_id() {
        let sub = build_0x2_packet(0xDEADBEEF);
        assert_eq!(&sub.data[..4], &[0xEF, 0xBE, 0xAD, 0xDE]);
    }

    #[test]
    fn session_begin_login_flag() {
        let sub = build_session_begin(123, true);
        assert_eq!(sub.data, vec![1]);
        assert_eq!(sub.header.source_id, 123);
        assert_eq!(sub.header.target_id, 123);
    }

    #[test]
    fn error_packet_carries_code() {
        let sub = build_error(7, 0xBADF00D);
        assert_eq!(sub.data, 0xBADF00Du32.to_le_bytes().to_vec());
    }
}
