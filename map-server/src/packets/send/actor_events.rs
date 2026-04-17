//! Event-condition packets. These tell the client which interactions an NPC
//! supports (emote / talk / push-to-enter / notice icon).
//!
//! The C# `EventList.*Condition` struct tree hasn't been ported yet; these
//! builders take flat structs for now and Phase-6+ will hand the full types
//! in.

use std::io::Cursor;

use byteorder::{LittleEndian, WriteBytesExt};
use common::subpacket::SubPacket;

use super::super::opcodes::*;
use super::{body, write_padded_ascii};

#[derive(Debug, Clone, Default)]
pub struct EventCondition {
    pub name: String,
    pub emote_id: u32,
    pub enabled: bool,
    pub silent: bool,
}

/// 0x012E SetTalkEventCondition.
pub fn build_set_talk_event_condition(actor_id: u32, condition: &EventCondition) -> SubPacket {
    let mut data = body(0x48);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u8(condition.enabled as u8).unwrap();
    c.write_u8(condition.silent as u8).unwrap();
    write_padded_ascii(&mut c, &condition.name, 0x20);
    SubPacket::new(OP_SET_TALK_EVENT_CONDITION, actor_id, data)
}

/// 0x016B SetNoticeEventCondition.
pub fn build_set_notice_event_condition(actor_id: u32, condition: &EventCondition) -> SubPacket {
    let mut data = body(0x48);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u8(condition.enabled as u8).unwrap();
    c.write_u8(0).unwrap();
    c.write_u32::<LittleEndian>(condition.emote_id).unwrap();
    write_padded_ascii(&mut c, &condition.name, 0x20);
    SubPacket::new(OP_SET_NOTICE_EVENT_CONDITION, actor_id, data)
}

/// 0x016C SetEmoteEventCondition.
pub fn build_set_emote_event_condition(actor_id: u32, condition: &EventCondition) -> SubPacket {
    let mut data = body(0x48);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u8(condition.enabled as u8).unwrap();
    c.write_u8(condition.silent as u8).unwrap();
    c.write_u32::<LittleEndian>(condition.emote_id).unwrap();
    write_padded_ascii(&mut c, &condition.name, 0x20);
    SubPacket::new(OP_SET_EMOTE_EVENT_CONDITION, actor_id, data)
}

/// 0x016F SetPushEventConditionWithCircle — (x, y, z, radius).
#[allow(clippy::too_many_arguments)]
pub fn build_set_push_circle_event_condition(
    actor_id: u32,
    name: &str,
    enabled: bool,
    x: f32,
    y: f32,
    z: f32,
    radius: f32,
) -> SubPacket {
    let mut data = body(0x58);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u8(enabled as u8).unwrap();
    c.write_u8(0).unwrap();
    c.write_f32::<LittleEndian>(x).unwrap();
    c.write_f32::<LittleEndian>(y).unwrap();
    c.write_f32::<LittleEndian>(z).unwrap();
    c.write_f32::<LittleEndian>(radius).unwrap();
    write_padded_ascii(&mut c, name, 0x20);
    SubPacket::new(OP_SET_PUSH_CIRCLE_EVENT_CONDITION, actor_id, data)
}

/// 0x0170 SetPushEventConditionWithFan — (x, y, z, radius, angle, rotation).
#[allow(clippy::too_many_arguments)]
pub fn build_set_push_fan_event_condition(
    actor_id: u32,
    name: &str,
    enabled: bool,
    x: f32,
    y: f32,
    z: f32,
    radius: f32,
    angle: f32,
    rotation: f32,
) -> SubPacket {
    let mut data = body(0x60);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u8(enabled as u8).unwrap();
    c.write_u8(0).unwrap();
    c.write_f32::<LittleEndian>(x).unwrap();
    c.write_f32::<LittleEndian>(y).unwrap();
    c.write_f32::<LittleEndian>(z).unwrap();
    c.write_f32::<LittleEndian>(radius).unwrap();
    c.write_f32::<LittleEndian>(angle).unwrap();
    c.write_f32::<LittleEndian>(rotation).unwrap();
    write_padded_ascii(&mut c, name, 0x20);
    SubPacket::new(OP_SET_PUSH_FAN_EVENT_CONDITION, actor_id, data)
}

/// 0x0175 SetPushEventConditionWithTriggerBox — AABB collision volume.
#[allow(clippy::too_many_arguments)]
pub fn build_set_push_box_event_condition(
    actor_id: u32,
    name: &str,
    enabled: bool,
    x: f32,
    y: f32,
    z: f32,
    extent_x: f32,
    extent_y: f32,
    extent_z: f32,
) -> SubPacket {
    let mut data = body(0x60);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u8(enabled as u8).unwrap();
    c.write_u8(0).unwrap();
    c.write_f32::<LittleEndian>(x).unwrap();
    c.write_f32::<LittleEndian>(y).unwrap();
    c.write_f32::<LittleEndian>(z).unwrap();
    c.write_f32::<LittleEndian>(extent_x).unwrap();
    c.write_f32::<LittleEndian>(extent_y).unwrap();
    c.write_f32::<LittleEndian>(extent_z).unwrap();
    write_padded_ascii(&mut c, name, 0x20);
    SubPacket::new(OP_SET_PUSH_BOX_EVENT_CONDITION, actor_id, data)
}
