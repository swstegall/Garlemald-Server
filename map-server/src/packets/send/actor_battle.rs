//! Battle packets. The client recognizes two containers for combat results:
//! `CommandResultX10` (up to 16 results) and `CommandResultX18` (up to 24).
//! `BattleAction` is the server's version of the same container, used for
//! server-initiated actions (cutscenes, traps, etc.). Each entry serializes
//! a `CommandResult` / `BattleAction` struct with target id, amount, type,
//! flags, animation id.
//!
//! The full wire format has ~40 flags per action; Phase 7 ports the
//! container + per-entry header. Deeper fields (proc flags, miss/parry/
//! block results, reflected damage) pass through as zero-padded bytes
//! until the battle math lands.

use std::io::Cursor;

use byteorder::{LittleEndian, WriteBytesExt};
use common::subpacket::SubPacket;

use super::super::opcodes::*;
use super::body;

/// One unit of battle action serialized into the X10/X18 containers.
#[derive(Debug, Clone, Default)]
pub struct BattleAction {
    pub target_id: u32,
    pub amount: i32,
    pub amount_mitigated: i32,
    pub effect_id: u32,
    pub param: u32,
    pub animation: u16,
    pub flag: u8,
}

/// One unit of command result (player-initiated action).
#[derive(Debug, Clone, Default)]
pub struct CommandResult {
    pub target_id: u32,
    pub hit_num: u8,
    pub sub_command: u8,
    pub hit_effect: u32,
    pub mitigated_amount: u32,
    pub amount: u32,
    pub command_type: u8,
    pub animation_id: u32,
    pub worldmaster_text_id: u16,
    pub param: u32,
    pub action_property: u8,
}

fn encode_command_result(c: &mut Cursor<&mut [u8]>, entry: &CommandResult) {
    c.write_u32::<LittleEndian>(entry.target_id).unwrap();
    c.write_u8(entry.hit_num).unwrap();
    c.write_u8(entry.sub_command).unwrap();
    c.write_u16::<LittleEndian>(entry.worldmaster_text_id)
        .unwrap();
    c.write_u32::<LittleEndian>(entry.hit_effect).unwrap();
    c.write_u32::<LittleEndian>(entry.mitigated_amount).unwrap();
    c.write_u32::<LittleEndian>(entry.amount).unwrap();
    c.write_u8(entry.command_type).unwrap();
    c.write_u8(entry.action_property).unwrap();
    c.write_u16::<LittleEndian>(0).unwrap();
    c.write_u32::<LittleEndian>(entry.animation_id).unwrap();
    c.write_u32::<LittleEndian>(entry.param).unwrap();
}

fn encode_battle_action(c: &mut Cursor<&mut [u8]>, entry: &BattleAction) {
    c.write_u32::<LittleEndian>(entry.target_id).unwrap();
    c.write_i32::<LittleEndian>(entry.amount).unwrap();
    c.write_i32::<LittleEndian>(entry.amount_mitigated).unwrap();
    c.write_u32::<LittleEndian>(entry.effect_id).unwrap();
    c.write_u32::<LittleEndian>(entry.param).unwrap();
    c.write_u16::<LittleEndian>(entry.animation).unwrap();
    c.write_u8(entry.flag).unwrap();
    c.write_u8(0).unwrap();
}

/// 0x013C CommandResultX00 — "no hits" confirmation.
pub fn build_command_result_x00(actor_id: u32, animation_id: u32, command_id: u16) -> SubPacket {
    let mut data = body(0x48);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(animation_id).unwrap();
    c.write_u16::<LittleEndian>(command_id).unwrap();
    SubPacket::new(OP_COMMAND_RESULT_X00, actor_id, data)
}

/// 0x0139 CommandResultX01 — one hit.
pub fn build_command_result_x01(
    actor_id: u32,
    animation_id: u32,
    command_id: u16,
    action: &CommandResult,
) -> SubPacket {
    let mut data = body(0x58);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u32::<LittleEndian>(animation_id).unwrap();
        c.write_u16::<LittleEndian>(command_id).unwrap();
        c.write_u16::<LittleEndian>(1).unwrap();
        encode_command_result(&mut c, action);
    }
    SubPacket::new(OP_COMMAND_RESULT_X01, actor_id, data)
}

/// 0x013A CommandResultX10 — up to 16 hits in one packet.
pub fn build_command_result_x10(
    actor_id: u32,
    animation_id: u32,
    command_id: u16,
    actions: &[CommandResult],
    list_offset: &mut usize,
) -> SubPacket {
    build_command_result_container(
        actor_id,
        animation_id,
        command_id,
        actions,
        list_offset,
        16,
        OP_COMMAND_RESULT_X10,
        0xD8,
    )
}

/// 0x013B CommandResultX18 — up to 24 hits.
pub fn build_command_result_x18(
    actor_id: u32,
    animation_id: u32,
    command_id: u16,
    actions: &[CommandResult],
    list_offset: &mut usize,
) -> SubPacket {
    build_command_result_container(
        actor_id,
        animation_id,
        command_id,
        actions,
        list_offset,
        24,
        OP_COMMAND_RESULT_X18,
        0x148,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_command_result_container(
    actor_id: u32,
    animation_id: u32,
    command_id: u16,
    actions: &[CommandResult],
    list_offset: &mut usize,
    cap: usize,
    opcode: u16,
    packet_size: usize,
) -> SubPacket {
    let mut data = body(packet_size);
    let max = actions.len().saturating_sub(*list_offset).min(cap);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u32::<LittleEndian>(animation_id).unwrap();
        c.write_u16::<LittleEndian>(command_id).unwrap();
        c.write_u16::<LittleEndian>(max as u16).unwrap();
        for i in 0..max {
            encode_command_result(&mut c, &actions[*list_offset + i]);
        }
    }
    *list_offset += max;
    SubPacket::new(opcode, actor_id, data)
}

/// 0x013A BattleActionX10 — server-initiated variant using `BattleAction`
/// entries. Opcode collides with `CommandResultX10`; distinguished by the
/// source actor and the payload layout.
pub fn build_battle_action_x10(
    player_actor_id: u32,
    source_actor_id: u32,
    animation_id: u32,
    command_id: u16,
    actions: &[BattleAction],
    list_offset: &mut usize,
) -> SubPacket {
    build_battle_action_container(
        player_actor_id,
        source_actor_id,
        animation_id,
        command_id,
        actions,
        list_offset,
        16,
        OP_BATTLE_ACTION_X10,
        0xD8,
    )
}

pub fn build_battle_action_x18(
    player_actor_id: u32,
    source_actor_id: u32,
    animation_id: u32,
    command_id: u16,
    actions: &[BattleAction],
    list_offset: &mut usize,
) -> SubPacket {
    build_battle_action_container(
        player_actor_id,
        source_actor_id,
        animation_id,
        command_id,
        actions,
        list_offset,
        24,
        OP_BATTLE_ACTION_X18,
        0x148,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_battle_action_container(
    player_actor_id: u32,
    source_actor_id: u32,
    animation_id: u32,
    command_id: u16,
    actions: &[BattleAction],
    list_offset: &mut usize,
    cap: usize,
    opcode: u16,
    packet_size: usize,
) -> SubPacket {
    let mut data = body(packet_size);
    let max = actions.len().saturating_sub(*list_offset).min(cap);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u32::<LittleEndian>(source_actor_id).unwrap();
        c.write_u32::<LittleEndian>(animation_id).unwrap();
        c.write_u16::<LittleEndian>(command_id).unwrap();
        c.write_u16::<LittleEndian>(max as u16).unwrap();
        for i in 0..max {
            encode_battle_action(&mut c, &actions[*list_offset + i]);
        }
    }
    *list_offset += max;
    SubPacket::new(opcode, player_actor_id, data)
}
