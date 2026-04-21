//! Event-condition packets. These tell the client which interactions an NPC
//! supports (talk / notice-icon / emote / push-to-enter fan / circle / box).
//!
//! Wire layouts ported verbatim from
//! `Map Server/Packets/Send/Actor/Events/Set*Condition.cs`. Each builder
//! takes the typed condition struct from `actor::event_conditions` so
//! callers round-trip the parsed `ActorClass` JSON straight through.

use std::io::{Cursor, Write};

use byteorder::{LittleEndian, WriteBytesExt};
use common::subpacket::SubPacket;

use super::super::opcodes::*;
use super::body;

use crate::actor::event_conditions::{
    EmoteCondition, NoticeCondition, PushBoxCondition, PushCircleCondition, PushFanCondition,
    TalkCondition,
};

/// Max ASCII bytes the client accepts for a `conditionName` field.
/// Matches the `Encoding.ASCII.GetByteCount(...) >= 0x24 ? 0x24 : ...`
/// clamp in every Meteor builder.
const CONDITION_NAME_MAX: usize = 0x24;

fn write_condition_name(c: &mut Cursor<&mut [u8]>, name: &str) {
    let bytes = name.as_bytes();
    let n = bytes.len().min(CONDITION_NAME_MAX);
    c.write_all(&bytes[..n]).unwrap();
}

/// 0x012E `SetTalkEventCondition` — port of `SetTalkEventCondition.cs`.
///
/// Layout (0x48 packet / 0x28 body):
/// * 0x00: u8 unknown1 — Meteor hardcodes `4` regardless of struct value.
/// * 0x01: u8 isDisabled (0 or 1).
/// * 0x02..0x26: ASCII condition name, zero-padded, max 0x24 bytes.
pub fn build_set_talk_event_condition(actor_id: u32, condition: &TalkCondition) -> SubPacket {
    let mut data = body(0x48);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u8(4).unwrap();
    c.write_u8(if condition.is_disabled { 1 } else { 0 }).unwrap();
    write_condition_name(&mut c, &condition.condition_name);
    SubPacket::new(OP_SET_TALK_EVENT_CONDITION, actor_id, data)
}

/// 0x016B `SetNoticeEventCondition` — port of `SetNoticeEventCondition.cs`.
///
/// Layout (0x48 packet / 0x28 body):
/// * 0x00: u8 unknown1 (observed: 0, 1, 0xE).
/// * 0x01: u8 unknown2 (observed: 0, 1).
/// * 0x02..0x26: ASCII condition name, zero-padded, max 0x24 bytes.
///
/// Must be emitted in the zone-in spawn bundle (between `AddActor` and
/// `Speed`) so the 1.23b client binds the condition name before the
/// `ScriptBind`; without it a later `KickEventPacket("noticeEvent")` is
/// ignored and the client fires an error EventStart the server has no
/// handler for.
pub fn build_set_notice_event_condition(actor_id: u32, condition: &NoticeCondition) -> SubPacket {
    build_set_notice_event_condition_raw(
        actor_id,
        condition.unknown1,
        condition.unknown2,
        &condition.condition_name,
    )
}

/// Lower-level notice-condition builder, kept for callers that construct
/// conditions ad-hoc (test + legacy code paths).
pub fn build_set_notice_event_condition_raw(
    actor_id: u32,
    unknown1: u8,
    unknown2: u8,
    name: &str,
) -> SubPacket {
    let mut data = body(0x48);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u8(unknown1).unwrap();
    c.write_u8(unknown2).unwrap();
    write_condition_name(&mut c, name);
    SubPacket::new(OP_SET_NOTICE_EVENT_CONDITION, actor_id, data)
}

/// 0x016C `SetEmoteEventCondition` — port of `SetEmoteEventCondition.cs`.
///
/// Layout (0x48 packet / 0x28 body):
/// * 0x00: u8 unknown1 (4 in every Meteor capture).
/// * 0x01: u16 emoteId (0x82, 0x76, 0x6E observed).
/// * 0x03..0x27: ASCII condition name.
pub fn build_set_emote_event_condition(actor_id: u32, condition: &EmoteCondition) -> SubPacket {
    let mut data = body(0x48);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u8(condition.unknown1).unwrap();
    c.write_u16::<LittleEndian>(condition.emote_id as u16).unwrap();
    write_condition_name(&mut c, &condition.condition_name);
    SubPacket::new(OP_SET_EMOTE_EVENT_CONDITION, actor_id, data)
}

/// 0x016F `SetPushEventConditionWithCircle` — port of
/// `SetPushEventConditionWithCircle.cs`.
///
/// Layout (0x58 packet / 0x38 body):
/// * 0x00: f32 radius
/// * 0x04: u32 magic = 0x44533088
/// * 0x08: f32 100.0
/// * 0x0C: 4-byte skip
/// * 0x10: u8 flag — `outwards ? 0x11 : 0x01` (0x10 = inverted AABB)
/// * 0x11: u8 0
/// * 0x12: u8 silent
/// * 0x13..0x37: ASCII condition name.
pub fn build_set_push_circle_event_condition(
    actor_id: u32,
    condition: &PushCircleCondition,
) -> SubPacket {
    let mut data = body(0x58);
    let mut c = Cursor::new(&mut data[..]);
    c.write_f32::<LittleEndian>(condition.radius).unwrap();
    c.write_u32::<LittleEndian>(0x4453_3088).unwrap();
    c.write_f32::<LittleEndian>(100.0).unwrap();
    c.write_u32::<LittleEndian>(0).unwrap();
    c.write_u8(if condition.outwards { 0x11 } else { 0x01 })
        .unwrap();
    c.write_u8(0).unwrap();
    c.write_u8(if condition.silent { 1 } else { 0 }).unwrap();
    write_condition_name(&mut c, &condition.condition_name);
    SubPacket::new(OP_SET_PUSH_CIRCLE_EVENT_CONDITION, actor_id, data)
}

/// 0x0170 `SetPushEventConditionWithFan` — port of
/// `SetPushEventConditionWithFan.cs`. Same on-wire preamble as the
/// circle variant (radius + magic + 100.0 + skip + outwards/silent/name),
/// differentiated only by opcode.
pub fn build_set_push_fan_event_condition(
    actor_id: u32,
    condition: &PushFanCondition,
) -> SubPacket {
    let mut data = body(0x58);
    let mut c = Cursor::new(&mut data[..]);
    c.write_f32::<LittleEndian>(condition.radius).unwrap();
    c.write_u32::<LittleEndian>(0x4453_3088).unwrap();
    c.write_f32::<LittleEndian>(100.0).unwrap();
    c.write_u32::<LittleEndian>(0).unwrap();
    c.write_u8(if condition.outwards { 0x11 } else { 0x01 })
        .unwrap();
    c.write_u8(0).unwrap();
    c.write_u8(if condition.silent { 1 } else { 0 }).unwrap();
    write_condition_name(&mut c, &condition.condition_name);
    SubPacket::new(OP_SET_PUSH_FAN_EVENT_CONDITION, actor_id, data)
}

/// 0x0175 `SetPushEventConditionWithTriggerBox` — port of
/// `SetPushEventConditionWithTriggerBox.cs`. Layout skipped for now;
/// box-push triggers aren't emitted from any path that currently runs.
#[allow(clippy::needless_pass_by_value)]
pub fn build_set_push_box_event_condition(
    actor_id: u32,
    condition: &PushBoxCondition,
) -> SubPacket {
    // Layout: bgObj (u32), layout (u32), outwards/silent bytes, then
    // two ASCII strings (conditionName + reactName). Pad to 0x60 to
    // match the C# packet size; detailed byte offsets can be tightened
    // when a caller actually needs box-push triggers.
    let mut data = body(0x60);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(condition.bg_obj).unwrap();
    c.write_u32::<LittleEndian>(condition.layout).unwrap();
    c.write_u8(if condition.outwards { 0x11 } else { 0x01 })
        .unwrap();
    c.write_u8(0).unwrap();
    c.write_u8(if condition.silent { 1 } else { 0 }).unwrap();
    write_condition_name(&mut c, &condition.condition_name);
    SubPacket::new(OP_SET_PUSH_BOX_EVENT_CONDITION, actor_id, data)
}

/// Convenience: fan out every event-condition packet for an NPC in the
/// same order Meteor's `Actor.GetEventConditionPackets()` produces them
/// (talk → notice → emote → push-circle → push-fan → push-box). Returns
/// the subpackets so callers can splice them into a spawn bundle.
pub fn build_event_condition_packets(
    actor_id: u32,
    list: &crate::actor::event_conditions::EventConditionList,
) -> Vec<SubPacket> {
    let mut out = Vec::new();
    for cond in &list.talk {
        out.push(build_set_talk_event_condition(actor_id, cond));
    }
    for cond in &list.notice {
        out.push(build_set_notice_event_condition(actor_id, cond));
    }
    for cond in &list.emote {
        out.push(build_set_emote_event_condition(actor_id, cond));
    }
    for cond in &list.push_circle {
        out.push(build_set_push_circle_event_condition(actor_id, cond));
    }
    for cond in &list.push_fan {
        out.push(build_set_push_fan_event_condition(actor_id, cond));
    }
    for cond in &list.push_box {
        out.push(build_set_push_box_event_condition(actor_id, cond));
    }
    out
}
