//! Friends-list, blacklist, and friend-online-status packets.

use std::io::Cursor;

use byteorder::{LittleEndian, WriteBytesExt};
use common::subpacket::SubPacket;

use super::super::opcodes::*;
use super::{body, write_padded_ascii};

fn ack(actor_id: u32, opcode: u16, is_success: bool, name: &str, size: usize) -> SubPacket {
    let mut data = body(size);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u8(is_success as u8).unwrap();
    write_padded_ascii(&mut c, name, 0x20);
    SubPacket::new(opcode, actor_id, data)
}

/// 0x01C9 BlacklistAdded.
pub fn build_blacklist_added(actor_id: u32, is_success: bool, name: &str) -> SubPacket {
    ack(actor_id, OP_BLACKLIST_ADDED, is_success, name, 0x48)
}

/// 0x01CA BlacklistRemoved.
pub fn build_blacklist_removed(actor_id: u32, is_success: bool, name: &str) -> SubPacket {
    ack(actor_id, OP_BLACKLIST_REMOVED, is_success, name, 0x48)
}

/// 0x01CD FriendlistRemoved.
pub fn build_friendlist_removed(actor_id: u32, is_success: bool, name: &str) -> SubPacket {
    ack(actor_id, OP_FRIENDLIST_REMOVED, is_success, name, 0x57)
}

/// 0x01CC FriendlistAdded — carries the friend's id + online state.
pub fn build_friendlist_added(
    actor_id: u32,
    is_success: bool,
    friend_id: i64,
    is_online: bool,
    name: &str,
) -> SubPacket {
    let mut data = body(0x67);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u8(is_success as u8).unwrap();
    c.write_u8(is_online as u8).unwrap();
    c.write_u16::<LittleEndian>(0).unwrap();
    c.write_i64::<LittleEndian>(friend_id).unwrap();
    write_padded_ascii(&mut c, name, 0x20);
    SubPacket::new(OP_FRIENDLIST_ADDED, actor_id, data)
}

/// 0x01CB SendBlacklist — up to N names per packet.
pub fn build_send_blacklist(
    actor_id: u32,
    blacklisted: &[String],
    offset: &mut usize,
) -> SubPacket {
    let mut data = body(0x686);
    let max = blacklisted.len().saturating_sub(*offset).min(48);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u16::<LittleEndian>(max as u16).unwrap();
        c.write_u16::<LittleEndian>(0).unwrap();
        for i in 0..max {
            write_padded_ascii(&mut c, &blacklisted[*offset + i], 0x20);
        }
    }
    *offset += max;
    SubPacket::new(OP_SEND_BLACKLIST, actor_id, data)
}

/// 0x01CE SendFriendlist — (id, name) pairs.
pub fn build_send_friendlist(
    actor_id: u32,
    friends: &[(i64, String)],
    offset: &mut usize,
) -> SubPacket {
    let mut data = body(0x686);
    let max = friends.len().saturating_sub(*offset).min(48);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u16::<LittleEndian>(max as u16).unwrap();
        c.write_u16::<LittleEndian>(0).unwrap();
        for i in 0..max {
            let (id, name) = &friends[*offset + i];
            c.write_i64::<LittleEndian>(*id).unwrap();
            write_padded_ascii(&mut c, name, 0x20);
        }
    }
    *offset += max;
    SubPacket::new(OP_SEND_FRIENDLIST, actor_id, data)
}

/// 0x01CF FriendStatus — batched (id, online) for the status board.
pub fn build_friend_status(actor_id: u32, friend_status: &[(i64, bool)]) -> SubPacket {
    let mut data = body(0x686);
    let n = friend_status.len().min(48);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u16::<LittleEndian>(n as u16).unwrap();
        c.write_u16::<LittleEndian>(0).unwrap();
        for (id, online) in friend_status.iter().take(n) {
            c.write_i64::<LittleEndian>(*id).unwrap();
            c.write_u8(*online as u8).unwrap();
            c.write_u8(0).unwrap();
            c.write_u16::<LittleEndian>(0).unwrap();
        }
    }
    SubPacket::new(OP_FRIEND_STATUS, actor_id, data)
}
