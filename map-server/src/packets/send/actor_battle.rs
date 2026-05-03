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

// ---------------------------------------------------------------------------
// 0x0195 SetEnmityIndicator — per-mob hate UI cue.
//
// Wire format (per wiki + retail bytes from
// `ffxiv_traces/combat_skills.pcapng` 0x0195 records):
//
//   subpacket source_id = bnpc emitting hate (REQUIRED — the wiki's
//                          "Source actor ID must set" requirement;
//                          the client routes the indicator to that
//                          actor's nameplate)
//   body size = 0x08 bytes
//   0x00  u32 target_actor_id  — the actor the bnpc is focused on
//                                 (player); use NO_ENMITY_TARGET
//                                 sentinel when hate is established
//                                 but no specific lock-on yet
//   0x04  u16 hate_amount      — percentage 0..100 (0=invisible,
//                                 1-49=green, 50-79=yellow, 80+=red);
//                                 special value HATE_LOCKED (0xFFFF)
//                                 means "permanently locked"
//   0x06  u16 zero/padding
//
// Lifecycle observed in the captures (combat_skills.pcapng):
//   1. (target=0xE0000000, hate=100)  — hate established, no target
//   2. (target=player,     hate=100)  — locked onto player, red gem
//   3. (target=player,     hate=0xFFFF) — permanently locked
//   4. (target=0xE0000000, hate=0xFFFF) — disengage from target,
//                                          retain max hate
//
// Project Meteor never emits this opcode (their `HateContainer.cs` is
// stubbed with a "todo: actually implement enmity properly" comment);
// the C# fork's combat targeting comes through entirely via 0x00DB
// `SetActorTarget`, which the client renders as a target indicator
// rather than the colored enmity gem.

/// Sentinel `target_actor_id` for "hate established but not locked
/// onto a specific actor". Same `0xE0000000` constant the 1.x client
/// uses for `SetTargetPacket.attackTarget` when no target.
pub const NO_ENMITY_TARGET: u32 = 0xE000_0000;

/// `hate_amount` sentinel for "permanent lock". Captured retail value;
/// out-of-range vs. the 0..100 percentage but the client interprets it
/// as the strongest possible hate.
pub const HATE_AMOUNT_LOCKED: u16 = 0xFFFF;

/// 0x0195 SetEnmityIndicator. `bnpc_actor_id` MUST be the bnpc emitting
/// hate — the client routes the gem update to that actor's nameplate
/// via the SubPacket source_id.
pub fn build_set_enmity_indicator(
    bnpc_actor_id: u32,
    target_actor_id: u32,
    hate_amount: u16,
) -> SubPacket {
    let mut data = body(0x28);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(target_actor_id).unwrap();
    c.write_u16::<LittleEndian>(hate_amount).unwrap();
    // 2 bytes trailing padding stay zero.
    SubPacket::new(OP_SET_ENMITY_INDICATOR, bnpc_actor_id, data)
}

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Reproduce the four 0x0195 packets captured from
    /// `ffxiv_traces/combat_skills.pcapng` — full hate lifecycle from
    /// "established, no target" → "locked on player at 100%" →
    /// "permanently locked" → "released target, retain max hate".
    /// Mob actor id 0x44D035D5; player 0x029B2941.
    #[test]
    fn enmity_indicator_lifecycle_matches_retail_capture() {
        // #1: hate established, no target lock yet (red gem, 100%).
        let p1 = build_set_enmity_indicator(0x44D0_35D5, NO_ENMITY_TARGET, 100);
        assert_eq!(p1.data, [0x00, 0x00, 0x00, 0xE0, 0x64, 0x00, 0x00, 0x00]);
        assert_eq!(p1.header.source_id, 0x44D0_35D5);
        assert_eq!(p1.game_message.opcode, OP_SET_ENMITY_INDICATOR);

        // #2: locked onto player, hate=100 (red gem).
        let p2 = build_set_enmity_indicator(0x44D0_35D5, 0x029B_2941, 100);
        assert_eq!(p2.data, [0x41, 0x29, 0x9B, 0x02, 0x64, 0x00, 0x00, 0x00]);

        // #3: permanently locked (HATE_AMOUNT_LOCKED sentinel).
        let p3 = build_set_enmity_indicator(0x44D0_35D5, 0x029B_2941, HATE_AMOUNT_LOCKED);
        assert_eq!(p3.data, [0x41, 0x29, 0x9B, 0x02, 0xFF, 0xFF, 0x00, 0x00]);

        // #4: target released, max hate retained.
        let p4 = build_set_enmity_indicator(0x44D0_35D5, NO_ENMITY_TARGET, HATE_AMOUNT_LOCKED);
        assert_eq!(p4.data, [0x00, 0x00, 0x00, 0xE0, 0xFF, 0xFF, 0x00, 0x00]);
    }

    #[test]
    fn enmity_indicator_body_is_eight_bytes() {
        let p = build_set_enmity_indicator(0x1, 0x2, 50);
        assert_eq!(p.data.len(), 8);
        // Wiki's gem-color thresholds: 0=invisible, 1-49=green,
        // 50-79=yellow, 80+=red. We pass 50 — boundary into yellow.
        let hate = u16::from_le_bytes([p.data[4], p.data[5]]);
        assert_eq!(hate, 50);
    }
}
