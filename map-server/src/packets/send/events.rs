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

/// 0x012F KickEventPacket — start an NPC-driven event.
pub fn build_kick_event(
    trigger_actor_id: u32,
    owner_actor_id: u32,
    event_name: &str,
    event_type: u8,
    lua_params: &[LuaParam],
) -> SubPacket {
    let mut data = body(0x90);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(owner_actor_id).unwrap();
    c.write_u8(event_type).unwrap();
    c.write_u8(0).unwrap();
    c.write_u16::<LittleEndian>(0).unwrap();
    write_padded_ascii(&mut c, event_name, 0x20);
    // Reserve 4-byte "opening alignment" like the C# writes before the lua blob.
    c.write_u32::<LittleEndian>(0).unwrap();
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
