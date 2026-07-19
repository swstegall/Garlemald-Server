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

//! Actor-state packet builders (gamemessage opcodes, 1-on-1 with
//! `Map Server/Packets/Send/Actor/*.cs`).

use std::io::Cursor;

use byteorder::{LittleEndian, WriteBytesExt};
use common::subpacket::SubPacket;

use super::super::opcodes::*;
use super::{body, write_padded_ascii};

// ---------------------------------------------------------------------------
// Core actor management
// ---------------------------------------------------------------------------

/// 0x00CA AddActorPacket — body is a single u8 instantiation flag.
pub fn build_add_actor(actor_id: u32, flag: u8) -> SubPacket {
    let mut data = body(0x28);
    data[0] = flag;
    SubPacket::new(OP_ADD_ACTOR, actor_id, data)
}

/// 0x00CB RemoveActorPacket — removes the actor by id.
pub fn build_remove_actor(actor_id: u32) -> SubPacket {
    let mut data = body(0x28);
    data[..4].copy_from_slice(&actor_id.to_le_bytes());
    SubPacket::new(OP_REMOVE_ACTOR, actor_id, data)
}

/// 0x00CC ActorInstantiatePacket — the "script-bind" packet that tells the
/// client which Lua class to attach to an actor. Without a valid
/// `lua_params` list starting with the class path string (e.g.
/// `"/Chara/Player/Player_work"`), the client exits Now Loading but fails
/// to initialise the actor's script state and raises error 40000 before
/// the game UI comes up.
///
/// Wire layout mirrors `Map Server/Packets/Send/Actor/ActorInstantiatePacket.cs`:
/// - offset 0x00: `value1` (i16) — usually 0 (instance id hint)
/// - offset 0x02: `value2` (i16) — hardcoded 0x3040 for players in the C#
///   reference; the earlier port passed 0 here, which the client treats as
///   an invalid instantiation and aborts
/// - offset 0x04..0x24: `object_name` (actor name, e.g. `_pc00000001`)
/// - offset 0x24..0x44: `class_name` (e.g. `Player`)
/// - offset 0x44+    : Lua params stream (type byte + value), no count prefix
pub fn build_actor_instantiate(
    actor_id: u32,
    value1: i16,
    value2: i16,
    object_name: &str,
    class_name: &str,
    lua_params: &[common::luaparam::LuaParam],
) -> SubPacket {
    let mut data = body(0x128);
    let mut c = Cursor::new(&mut data[..]);
    c.write_i16::<LittleEndian>(value1).unwrap();
    c.write_i16::<LittleEndian>(value2).unwrap();
    write_padded_ascii(&mut c, object_name, 0x20);
    c.set_position(0x24);
    write_padded_ascii(&mut c, class_name, 0x20);
    c.set_position(0x44);
    common::luaparam::write_lua_params(&mut c, lua_params).unwrap();
    SubPacket::new(OP_ACTOR_INSTANTIATE, actor_id, data)
}

/// 0x00CE SetActorPositionPacket. C# seeks to offset 0x24 before writing
/// `spawnType` + `isZoningPlayer` — the floats stop at 0x18 but the u16
/// tail lives at 0x24/0x26. Writing them contiguously after the rotation
/// floats (as the earlier port did) puts spawn_type at 0x18 and leaves
/// 0x24 zero, which the client reads as SPAWNTYPE_FADEIN and ignores the
/// intended login spawn — a subtle desync that can trip later state checks.
#[allow(clippy::too_many_arguments)]
pub fn build_set_actor_position(
    actor_id: u32,
    target_actor_id: i32,
    x: f32,
    y: f32,
    z: f32,
    rotation: f32,
    spawn_type: u16,
    is_zoning_player: bool,
) -> SubPacket {
    let mut data = body(0x48);
    let mut c = Cursor::new(&mut data[..]);
    c.write_i32::<LittleEndian>(0).unwrap();
    c.write_i32::<LittleEndian>(target_actor_id).unwrap();
    c.write_f32::<LittleEndian>(x).unwrap();
    c.write_f32::<LittleEndian>(y).unwrap();
    c.write_f32::<LittleEndian>(z).unwrap();
    c.write_f32::<LittleEndian>(rotation).unwrap();
    c.set_position(0x24);
    c.write_u16::<LittleEndian>(spawn_type).unwrap();
    c.write_u16::<LittleEndian>(is_zoning_player as u16)
        .unwrap();
    SubPacket::new(OP_SET_ACTOR_POSITION, actor_id, data)
}

/// 0x00CF MoveActorToPositionPacket — server-driven path-to.
///
/// Body layout (wiki "Move Actor to Position" + pmeteor
/// `MoveActorToPositionPacket.cs:36-50`, which seeks to 0x8 before the
/// float writes; byte-verified against the pmeteor capture's Yda
/// warp-in 0x00CF, map-packets.log:33617):
///   +0x00  u32 ×2  unknown, zero
///   +0x08  f32     x
///   +0x0C  f32     y
///   +0x10  f32     z
///   +0x14  f32     rotation
///   +0x18  u16     move_state — 0 = standing, 1 = walking, 2 = running
///   +0x24  f32     floatingHeight — stays 0 for ground mobs
pub fn build_move_actor_to_position(
    actor_id: u32,
    x: f32,
    y: f32,
    z: f32,
    rot: f32,
    move_state: u16,
) -> SubPacket {
    let mut data = body(0x50);
    let mut c = Cursor::new(&mut data[..]);
    c.set_position(0x08);
    c.write_f32::<LittleEndian>(x).unwrap();
    c.write_f32::<LittleEndian>(y).unwrap();
    c.write_f32::<LittleEndian>(z).unwrap();
    c.write_f32::<LittleEndian>(rot).unwrap();
    c.write_u16::<LittleEndian>(move_state).unwrap();
    SubPacket::new(OP_MOVE_ACTOR_TO_POSITION, actor_id, data)
}

/// 0x00D0 SetActorSpeedPacket — four speed bands (stop/walk/run/active).
pub fn build_set_actor_speed(
    actor_id: u32,
    stop: f32,
    walk: f32,
    run: f32,
    active: f32,
) -> SubPacket {
    let mut data = body(0xA8);
    let mut c = Cursor::new(&mut data[..]);
    for (speed, slot) in [(stop, 0u32), (walk, 1), (run, 2), (active, 3)] {
        c.write_f32::<LittleEndian>(speed).unwrap();
        c.write_u32::<LittleEndian>(slot).unwrap();
    }
    c.write_u32::<LittleEndian>(4).unwrap();
    SubPacket::new(OP_SET_ACTOR_SPEED, actor_id, data)
}

/// Default speed bands, mirroring C# `SetActorSpeedPacket.cs:33-36`
/// (DEFAULT_STOP / DEFAULT_WALK / DEFAULT_RUN / DEFAULT_ACTIVE).
pub const SPEED_DEFAULT_STOP: f32 = 0.0;
pub const SPEED_DEFAULT_WALK: f32 = 2.0;
pub const SPEED_DEFAULT_RUN: f32 = 5.0;
pub const SPEED_DEFAULT_ACTIVE: f32 = 5.0;

pub fn build_set_actor_speed_default(actor_id: u32) -> SubPacket {
    build_set_actor_speed_scaled(actor_id, 1.0)
}

/// Default bands scaled by a movement-speed multiplier (stop stays 0);
/// 1.0 reproduces the defaults exactly.
pub fn build_set_actor_speed_scaled(actor_id: u32, multiplier: f32) -> SubPacket {
    build_set_actor_speed(
        actor_id,
        SPEED_DEFAULT_STOP,
        SPEED_DEFAULT_WALK * multiplier,
        SPEED_DEFAULT_RUN * multiplier,
        SPEED_DEFAULT_ACTIVE * multiplier,
    )
}

/// 0x00D3 SetActorTargetAnimatedPacket — played w/ animation lock.
pub fn build_set_actor_target_animated(actor_id: u32, target_id: u32) -> SubPacket {
    let mut data = body(0x28);
    data[..8].copy_from_slice(&(target_id as u64).to_le_bytes());
    SubPacket::new(OP_SET_ACTOR_TARGET_ANIMATED, actor_id, data)
}

/// 0x00D6 SetActorAppearancePacket — 28 appearance slots.
pub fn build_set_actor_appearance(
    actor_id: u32,
    model_id: u32,
    appearance: &[u32; 28],
) -> SubPacket {
    let mut data = body(0x128);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(model_id).unwrap();
    for (i, id) in appearance.iter().enumerate() {
        c.write_u32::<LittleEndian>(i as u32).unwrap();
        c.write_u32::<LittleEndian>(*id).unwrap();
    }
    // C# writes appearanceIDs.Length at offset 0x100.
    let len = appearance.len() as u32;
    data[0x100..0x104].copy_from_slice(&len.to_le_bytes());
    SubPacket::new(OP_SET_ACTOR_APPEARANCE, actor_id, data)
}

/// 0x00D8 SetActorBGPropertiesPacket.
pub fn build_set_actor_bg_properties(actor_id: u32, val1: u32, val2: u32) -> SubPacket {
    let mut data = body(0x28);
    data[..4].copy_from_slice(&val1.to_le_bytes());
    data[4..8].copy_from_slice(&val2.to_le_bytes());
    SubPacket::new(OP_SET_ACTOR_BG_PROPERTIES, actor_id, data)
}

/// 0x00D9 PlayBGAnimation — ASCII name (max 8 chars) of a background anim.
pub fn build_play_bg_animation(actor_id: u32, anim_name: &str) -> SubPacket {
    let mut data = body(0x28);
    let n = anim_name.len().min(8);
    data[..n].copy_from_slice(&anim_name.as_bytes()[..n]);
    SubPacket::new(OP_PLAY_BG_ANIMATION, actor_id, data)
}

/// 0x00DA PlayAnimationOnActorPacket.
pub fn build_play_animation_on_actor(actor_id: u32, animation_id: u32) -> SubPacket {
    let mut data = body(0x28);
    data[..8].copy_from_slice(&(animation_id as u64).to_le_bytes());
    SubPacket::new(OP_PLAY_ANIMATION_ON_ACTOR, actor_id, data)
}

/// 0x00DB SetActorTargetPacket.
pub fn build_set_actor_target(actor_id: u32, target_id: u32) -> SubPacket {
    let mut data = body(0x28);
    data[..8].copy_from_slice(&(target_id as u64).to_le_bytes());
    SubPacket::new(OP_SET_ACTOR_TARGET, actor_id, data)
}

/// 0x00DE ResetHeadPacket — clears any active head-tracking on the
/// actor (set via 0x00DB Set Head to Actor / 0x00DC Set Head to
/// Position / 0x00DD Set Actor Head Orientation), returning the
/// head to its origin/neutral pose.
///
/// Wire format: 8-byte zero body. The wiki claims "0x4 bytes" but
/// captures (`ffxiv_traces/combat_skills.pcapng`) confirm the
/// actual SubPacket size is 0x28 (40 bytes total) with an 8-byte
/// zero body — i.e. the smallest valid game-message body. Project
/// Meteor never implements this opcode; the C# fork leaves the
/// mob's head locked to its last target indefinitely.
///
/// Retail emits this when a mob disengages combat (alongside
/// 0x0195 SetEnmityIndicator clearing the gem) so the mob's head
/// stops following the player. `actor_id` is the mob whose head
/// should reset.
pub fn build_reset_head(actor_id: u32) -> SubPacket {
    SubPacket::new(OP_RESET_HEAD, actor_id, body(0x28))
}

/// 0x00E1 ActorDoEmotePacket — port of pmeteor `ActorDoEmotePacket.cs`.
///
/// `animation_id` is the bare emote id the script passes (e.g. 5 = /bow). The
/// client expects it packed as `realAnimID = 0x5000000 | (animation_id << 12)`
/// — writing the raw id makes the client resolve the description text (so the
/// "You bow…" log line still prints) but play NO animation. Also mirrors
/// pmeteor's `targetedActorId == 0` fallback (retarget to self + bump the
/// description id, except the 10105 "generic" id). (Garlemald-Server #46.)
pub fn build_actor_do_emote(
    actor_id: u32,
    animation_id: u32,
    targeted_actor_id: u32,
    description_id: u32,
) -> SubPacket {
    let (targeted_actor_id, description_id) = if targeted_actor_id == 0 {
        let desc = if description_id != 10105 {
            description_id + 1
        } else {
            description_id
        };
        (actor_id, desc)
    } else {
        (targeted_actor_id, description_id)
    };
    let real_anim_id = 0x0500_0000 | (animation_id << 12);
    let mut data = body(0x30);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(real_anim_id).unwrap();
    c.write_u32::<LittleEndian>(targeted_actor_id).unwrap();
    c.write_u32::<LittleEndian>(description_id).unwrap();
    SubPacket::new(OP_ACTOR_DO_EMOTE, actor_id, data)
}

/// 0x00E3 ActorSpecialGraphicPacket.
pub fn build_actor_special_graphic(actor_id: u32, icon_code: i32) -> SubPacket {
    let mut data = body(0x28);
    data[..4].copy_from_slice(&icon_code.to_le_bytes());
    SubPacket::new(OP_ACTOR_SPECIAL_GRAPHIC, actor_id, data)
}

/// 0x00E5 StartCountdownPacket — `countdown_length` seconds, synced off
/// `sync_time` (u64 unix ms), and a 0x20-byte ASCII message.
pub fn build_start_countdown(
    actor_id: u32,
    countdown_length: u8,
    sync_time: u64,
    message: &str,
) -> SubPacket {
    let mut data = body(0x48);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u8(countdown_length).unwrap();
    c.write_u64::<LittleEndian>(sync_time).unwrap();
    write_padded_ascii(&mut c, message, 0x20);
    SubPacket::new(OP_START_COUNTDOWN, actor_id, data)
}

/// 0x0134 SetActorStatePacket — packs `(main_state | sub_state << 8)` into a
/// single u64.
pub fn build_set_actor_state(actor_id: u32, main_state: u8, sub_state: u8) -> SubPacket {
    let combined = (main_state as u64) | ((sub_state as u64) << 8);
    SubPacket::new(
        OP_SET_ACTOR_STATE,
        actor_id,
        combined.to_le_bytes().to_vec(),
    )
}

/// 0x013D SetActorNamePacket — custom display name override. Size 0x19 per
/// C# to avoid overwriting the trailing flag byte.
pub fn build_set_actor_name(actor_id: u32, display_name_id: u32, custom_name: &str) -> SubPacket {
    let mut data = body(0x48);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(display_name_id).unwrap();
    let bytes = custom_name.as_bytes();
    let n = bytes.len().min(0x19);
    c.write_all(&bytes[..n]).unwrap();
    SubPacket::new(OP_SET_ACTOR_NAME, actor_id, data)
}

/// 0x0144 SetActorSubStatePacket.
pub fn build_set_actor_sub_state(
    actor_id: u32,
    breakage: u8,
    chant_id: u8,
    guard: u8,
    waste: u8,
    mode: u8,
    motion_pack: u16,
) -> SubPacket {
    let mut data = body(0x28);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u8(breakage).unwrap();
    c.write_u8(chant_id).unwrap();
    c.write_u8(guard & 0xF).unwrap();
    c.write_u8(waste).unwrap();
    c.write_u8(mode).unwrap();
    c.write_u8(0).unwrap();
    c.write_u16::<LittleEndian>(motion_pack).unwrap();
    SubPacket::new(OP_SET_ACTOR_SUB_STATE, actor_id, data)
}

/// 0x0145 SetActorIconPacket.
pub const ICON_DISCONNECTING: u32 = 0x00010000;
pub const ICON_IS_GM: u32 = 0x00020000;
pub const ICON_IS_AFK: u32 = 0x00000100;
pub fn build_set_actor_icon(actor_id: u32, icon_code: u32) -> SubPacket {
    let mut data = body(0x28);
    data[..4].copy_from_slice(&icon_code.to_le_bytes());
    SubPacket::new(OP_SET_ACTOR_ICON, actor_id, data)
}

/// 0x0177 SetActorStatusPacket — one (index, code) update.
pub fn build_set_actor_status(actor_id: u32, index: u16, status_code: u16) -> SubPacket {
    let mut data = body(0x28);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u16::<LittleEndian>(index).unwrap();
    c.write_u16::<LittleEndian>(status_code).unwrap();
    SubPacket::new(OP_SET_ACTOR_STATUS, actor_id, data)
}

/// 0x0179 SetActorStatusAllPacket — up to N status ids in one shot.
pub fn build_set_actor_status_all(actor_id: u32, status_ids: &[u16]) -> SubPacket {
    let mut data = body(0x48);
    let mut c = Cursor::new(&mut data[..]);
    for id in status_ids {
        c.write_u16::<LittleEndian>(*id).unwrap();
    }
    SubPacket::new(OP_SET_ACTOR_STATUS_ALL, actor_id, data)
}

/// 0x017B SetActorIsZoningPacket.
pub fn build_set_actor_is_zoning(actor_id: u32, is_zoning: bool) -> SubPacket {
    let mut data = body(0x28);
    data[0] = is_zoning as u8;
    SubPacket::new(OP_SET_ACTOR_IS_ZONING, actor_id, data)
}

/// 0x0132 _0x132Packet — scripted RunEvent trigger with function name.
pub fn build_0x132(actor_id: u32, number: u16, function: &str) -> SubPacket {
    let mut data = body(0x48);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u16::<LittleEndian>(number).unwrap();
    write_padded_ascii(&mut c, function, 0x20);
    SubPacket::new(OP_0X132_PACKET, actor_id, data)
}

/// 0x0136 SetEventStatusPacket — port of `SetEventStatusPacket.cs`.
///
/// Layout (0x48 packet / 0x28 body):
/// * 0x00..0x04: u32 enabled (Meteor writes `UInt32`, NOT `Byte`)
/// * 0x04:       u8 type (1=talk, 2=push, 3=emote, 5=notice)
/// * 0x05..0x29: ASCII condition name, max 0x24 bytes (no padding past
///   the bytes actually written, but our `write_padded_ascii`
///   zero-fills to 0x20 which is fine — the condition-name
///   compare on the client stops at the NUL terminator).
///
/// Previous garlemald port wrote `enabled` as a single byte, which
/// shifted `type` and `condition_name` left by 3 bytes. The 1.x client
/// then read the type from inside the (now-misaligned) condition-name
/// buffer and the talk/push trigger silently failed to enable. Visible
/// symptom: clicking Yda after the opening cinematic never fired
/// `EventStart(eventType=1, owner=Yda)` because the talk condition was
/// disabled client-side, so `man0g0::seq000_onTalk` couldn't run.
pub fn build_set_event_status(
    actor_id: u32,
    enabled: bool,
    ty: u8,
    condition_name: &str,
) -> SubPacket {
    let mut data = body(0x48);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(if enabled { 1 } else { 0 })
        .unwrap();
    c.write_u8(ty).unwrap();
    write_padded_ascii(&mut c, condition_name, 0x20);
    SubPacket::new(OP_SET_EVENT_STATUS, actor_id, data)
}

/// 0x00E3 `SetActorQuestGraphicPacket` — port of Meteor's
/// `SetActorQuestGraphicPacket.cs` (`ActorSpecialGraphicPacket.cs` on the
/// `ioncannon/quest_system` branch). Wire body is a single 4-byte LE
/// `iconCode`; the whole subpacket is the standard 0x28-byte shape.
///
/// `icon_code` values the client recognises:
/// * `0` / `NONE`            — no marker
/// * `2` / `QUEST`           — ordinary quest (`!` marker)
/// * `3` / `NOGRAPHIC`       — suppress marker even when quest-active
/// * `4` / `QUEST_IMPORTANT` — priority quest marker
///
/// Emitted alongside [`build_set_event_status`] packets whenever a quest
/// script registers or clears an active ENPC (`quest:SetENpc(classId, ...)`
/// / stale-ENPC drain after `onStateChange`).
pub fn build_set_actor_quest_graphic(actor_id: u32, icon_code: u8) -> SubPacket {
    let mut data = body(0x28);
    let mut c = Cursor::new(&mut data[..]);
    c.write_i32::<LittleEndian>(icon_code as i32).unwrap();
    SubPacket::new(OP_ACTOR_SPECIAL_GRAPHIC, actor_id, data)
}

/// `Actor.GetSetEventStatusPackets(talk, emote, push, notice)` —
/// fan out one [`build_set_event_status`] subpacket per entry in the
/// NPC's event-condition list, enabling/disabling each per the flags.
///
/// The `ty` byte per Meteor's classification:
/// * talk   → 1
/// * notice → 5
/// * emote  → 3
/// * push   → 2
pub fn build_actor_event_status_packets(
    actor_id: u32,
    conditions: &crate::actor::event_conditions::EventConditionList,
    talk_enabled: bool,
    emote_enabled: bool,
    push_enabled: Option<bool>,
    notice_enabled: bool,
) -> Vec<SubPacket> {
    let mut out = Vec::new();
    for cond in &conditions.talk {
        out.push(build_set_event_status(
            actor_id,
            talk_enabled,
            1,
            &cond.condition_name,
        ));
    }
    for cond in &conditions.notice {
        out.push(build_set_event_status(
            actor_id,
            notice_enabled,
            5,
            &cond.condition_name,
        ));
    }
    for cond in &conditions.emote {
        out.push(build_set_event_status(
            actor_id,
            emote_enabled,
            3,
            &cond.condition_name,
        ));
    }
    // Meteor's C# branched on `push_enabled ?? condition.isEnabled`
    // (Map Server/Actors/Actor.cs::GetSetEventStatusPackets). The garlemald
    // push-condition structs now carry that per-condition `is_enabled`, so
    // we replicate the exact fallback: an explicit `Some(b)` from a
    // `quest:SetENpc(..)` broadcast wins; otherwise each circle/fan/box
    // streams in at its actor-class default (`isEnabled`, which is `false`
    // for every quest trigger). Previously this defaulted to `true`, which
    // force-enabled every streamed push trigger regardless of the owning
    // quest's state — a trigger like man0l1's ECHO_EXIT could then fire the
    // moment it streamed and warp the player out mid-sequence.
    for cond in &conditions.push_circle {
        out.push(build_set_event_status(
            actor_id,
            push_enabled.unwrap_or(cond.is_enabled),
            2,
            &cond.condition_name,
        ));
    }
    for cond in &conditions.push_fan {
        out.push(build_set_event_status(
            actor_id,
            push_enabled.unwrap_or(cond.is_enabled),
            2,
            &cond.condition_name,
        ));
    }
    for cond in &conditions.push_box {
        out.push(build_set_event_status(
            actor_id,
            push_enabled.unwrap_or(cond.is_enabled),
            2,
            &cond.condition_name,
        ));
    }
    out
}

/// 0x0137 SetActorPropertyPacket — byte 0 is the running length written last,
/// then each AddXxx call emits `(type_tag, u32 id, value)`, and finally
/// AddTarget appends `(0x82 + name_len, ascii name)` for the non-array /
/// isMore=false case. Matches `Map Server/Packets/Send/Actor/SetActorPropetyPacket.cs`.
pub fn build_set_actor_property_u32(actor_id: u32, target: &str, id: u32, value: u32) -> SubPacket {
    let mut data = body(0xA8);
    let mut c = Cursor::new(&mut data[..]);
    c.set_position(1);
    c.write_u8(4).unwrap();
    c.write_u32::<LittleEndian>(id).unwrap();
    c.write_u32::<LittleEndian>(value).unwrap();
    let tbytes = target.as_bytes();
    c.write_u8(0x82u8 + tbytes.len() as u8).unwrap();
    c.write_all(tbytes).unwrap();
    let running_total = 9 + 1 + tbytes.len();
    data[0] = running_total as u8;
    SubPacket::new(OP_SET_ACTOR_PROPERTY, actor_id, data)
}

/// 0x0137 SetActorPropertyPacket for the `/_init` target. Emits the exact
/// three byte flags that Meteor's `Actor.GetInitPackets()` pushes — they tell
/// the client the actor is fully initialised and safe to render, which is
/// the last signal the client waits for before leaving "Now loading…".
pub fn build_actor_property_init(actor_id: u32) -> SubPacket {
    let mut data = body(0xA8);
    let mut c = Cursor::new(&mut data[..]);
    c.set_position(1);
    for (id, value) in [(0xE14B0CA8u32, 1u8), (0x2138FD71, 1), (0xFBFBCFB1, 1)] {
        c.write_u8(1).unwrap();
        c.write_u32::<LittleEndian>(id).unwrap();
        c.write_u8(value).unwrap();
    }
    let target = b"/_init";
    c.write_u8(0x82u8 + target.len() as u8).unwrap();
    c.write_all(target).unwrap();
    let running_total = 3 * 6 + 1 + target.len();
    data[0] = running_total as u8;
    SubPacket::new(OP_SET_ACTOR_PROPERTY, actor_id, data)
}

/// Property-packet builder that mirrors C# `ActorPropertyPacketUtil` +
/// `SetActorPropetyPacket`. Callers stage property writes via
/// `add_byte / add_short / add_int`; when a single packet would exceed the
/// 0x7D byte budget (including the 1-byte target marker + target path),
/// `flush_if_needed` seals the current packet with the "more follows"
/// target marker (`0x60 + len`) and starts a fresh one. The final packet
/// gets the "done" marker (`0x82 + len`) via `done()`. Property ids are
/// the Murmur2 hash of the `/` path string, matching the C# reflection
/// path.
pub struct ActorPropertyPacketBuilder<'a> {
    actor_id: u32,
    target: &'a str,
    packets: Vec<SubPacket>,
    /// Staged bytes for the current packet, starting at offset 1 (offset
    /// 0 reserves one byte for the running-total header `runningByteTotal`).
    buf: Vec<u8>,
}

impl<'a> ActorPropertyPacketBuilder<'a> {
    const MAX_BYTES: usize = 0x7D;

    pub fn new(actor_id: u32, target: &'a str) -> Self {
        Self {
            actor_id,
            target,
            packets: Vec::new(),
            buf: Vec::new(),
        }
    }

    fn target_marker_cost(&self) -> usize {
        1 + self.target.len()
    }

    /// Seal the current packet with the given target marker byte.
    fn seal_current(&mut self, marker: u8) {
        let running_total = self.buf.len() + self.target_marker_cost();
        // Allocate the 0xA8-sized body with zero padding beyond the used
        // range — matches the fixed C# PACKET_SIZE.
        let mut data = body(0xA8);
        data[0] = running_total as u8;
        data[1..1 + self.buf.len()].copy_from_slice(&self.buf);
        let target_start = 1 + self.buf.len();
        data[target_start] = marker;
        data[target_start + 1..target_start + 1 + self.target.len()]
            .copy_from_slice(self.target.as_bytes());
        self.packets
            .push(SubPacket::new(OP_SET_ACTOR_PROPERTY, self.actor_id, data));
        self.buf.clear();
    }

    /// If `needed` more bytes wouldn't fit in the current packet, seal it
    /// with the "more follows" marker (`0x60 + len`) and start a fresh
    /// staging buffer.
    fn flush_if_needed(&mut self, needed: usize) {
        if self.buf.len() + needed + self.target_marker_cost() > Self::MAX_BYTES {
            let marker = 0x60u8 + self.target.len() as u8;
            self.seal_current(marker);
        }
    }

    /// Stage a 1-byte property (`AddByte`). Type byte 1, id u32 LE, value u8.
    pub fn add_byte(&mut self, name: &str, value: u8) {
        self.flush_if_needed(6);
        let id = common::utils::murmur_hash2(name, 0);
        self.buf.push(1);
        self.buf.extend_from_slice(&id.to_le_bytes());
        self.buf.push(value);
    }

    /// Stage a 2-byte property (`AddShort`). Type byte 2, id u32 LE, value u16.
    pub fn add_short(&mut self, name: &str, value: u16) {
        self.flush_if_needed(7);
        let id = common::utils::murmur_hash2(name, 0);
        self.buf.push(2);
        self.buf.extend_from_slice(&id.to_le_bytes());
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    /// Stage a 4-byte property (`AddInt`). Type byte 4, id u32 LE, value u32.
    pub fn add_int(&mut self, name: &str, value: u32) {
        self.flush_if_needed(9);
        let id = common::utils::murmur_hash2(name, 0);
        self.buf.push(4);
        self.buf.extend_from_slice(&id.to_le_bytes());
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    /// Stage a 4-byte float (`AddBuffer` with a 4-byte payload). C# writes
    /// the buffer length as the type byte (4) and the float's IEEE-754
    /// bytes as the value — same wire shape as `AddInt`.
    pub fn add_float(&mut self, name: &str, value: f32) {
        self.add_int(name, value.to_bits());
    }

    /// Seal the final packet with the "done" marker (`0x82 + len`) and
    /// return the full packet list.
    pub fn done(mut self) -> Vec<SubPacket> {
        let marker = 0x82u8 + self.target.len() as u8;
        self.seal_current(marker);
        self.packets
    }
}

/// Player-specific `/_init` property dump, modelled on C#
/// `Player.GetInitPackets()` + `ActorPropertyPacketUtil`. Emits the
/// "always-sent" property set for a fresh character: HP/MP/class state,
/// command categories (forced 1 for 0..64), command-slot compatibilities
/// (forced true for 0..40), `otherClassAbilityCount`/`giftCount` sentinel
/// values the C# code hardcodes, the `depictionJudge` constant, and the
/// player profile fields. Properties are packed across multiple
/// `SetActorProperty` subpackets when the MAXBYTES cap is exceeded —
/// the first N packets carry the "more follows" target marker and the
/// last carries the "done" marker.
#[allow(clippy::too_many_arguments)]
pub fn build_player_property_init(
    actor_id: u32,
    hp: u16,
    hp_max: u16,
    mp: u16,
    mp_max: u16,
    tp: u16,
    main_skill: u8,
    main_skill_level: u8,
    // Current skill points of the active class — feeds the XP bar at
    // zone-in. Previously hardcoded 0 on the wire, which rendered any
    // pre-warp EXP as a reset to 0 (the level companion was hardcoded
    // 1 at the call site).
    skill_point: i32,
    command_border: u8,
    tribe: u8,
    guardian: u8,
    birthday_day: u8,
    birthday_month: u8,
    initial_town: u8,
    rest_bonus_exp_rate: i32,
    // (slot, quest_actor_id) pairs for every active scenario quest. C#
    // `Player.GetInitPackets` (line 540-545) iterates `playerWork.questScenario`
    // and calls `AddProperty("playerWork.questScenario[i]")` for each non-zero
    // slot — that's the wire that puts active quests in the client's journal
    // and lets `seq000_onTalk` actually find them. Without this the client
    // never sees the scenario quest a fresh-character `onBeginLogin` adds, so
    // the journal stays empty and Yda/Papalymo's quest icons never light up.
    active_quests: &[(u32, u32)],
    // Local levequest slots `(slot, raw_quest_id)`. C# `Player.GetInitPackets`
    // (Player.cs:547-551) emits `playerWork.questGuildleve[slot]` (uint) per
    // non-zero slot; the wire VALUE carries the `0xA0F00000` quest mask that
    // C# `Database.cs:1285` bakes in on load. garlemald stores the raw quest
    // id, so the mask is OR'd in at emit time below.
    local_leves: &[(u16, u32)],
    // Regional levequest slots `(slot, guildleveId, abandoned, completed)`.
    // C# (Player.cs:553-563) emits `work.guildleveId[slot]` (ushort, raw — NO
    // mask, Database.cs:1305) plus the `guildleveDone` (garlemald `abandoned`)
    // / `guildleveChecked` (garlemald `completed`) bool companions when set.
    regional_leves: &[(u16, u16, bool, bool)],
    // Pre-resolved equipped hotbar: `(slot0, masked_command_id,
    // max_recast_seconds, recast_end_unix)` per populated slot. pmeteor
    // `Database.LoadHotbar` + `Player.GetInitPackets` (Player.cs:474-505)
    // emit these as `charaWork.command[32+slot]` with the recast
    // companions at the 0-based index; without them the client's action
    // bar is empty and a press never produces the `0x012D EventStart
    // owner=0xA0F0xxxx commandDefault` the skill path dispatches on.
    // (Garlemald-Server #28 S3.1.)
    hotbar: &[(u16, u32, u16, u32)],
) -> Vec<SubPacket> {
    let mut b = ActorPropertyPacketBuilder::new(actor_id, "/_init");

    // Base charaWork state. Values match Project Meteor's Player ctor:
    // `bazaarTax = 5` (byte, not 0), `potencial = 6.6f`. The default 0 we
    // were sending for bazaarTax is what the client sees as "tax rate
    // unknown"; bits of nameplate logic can read this.
    b.add_byte("charaWork.eventSave.bazaarTax", 5);
    b.add_float("charaWork.battleSave.potencial", 6.6);

    // Nameplate-visibility flags. Project Meteor's Player ctor sets
    // `charaWork.property[0/1/2/4] = 1` and `GetInitPackets` emits any
    // non-zero property slot. `CharaWork.cs:26` defines the constant
    // `PROPERTY_NAMEPLATE_VISIBLE = 1` — i.e. slot 1 literally gates
    // whether the client's `DepictionJudge:judgeNameplate()` can read
    // its nameplate-config table. Without these slots emitted, that
    // method indexes a nil table at line 900 on the first frame of the
    // player's `_onUpdateWork()` tick and the client pops "An error has
    // occured. (40000)4" and punts back to character select.
    b.add_byte("charaWork.property[0]", 1);
    b.add_byte("charaWork.property[1]", 1);
    b.add_byte("charaWork.property[2]", 1);
    b.add_byte("charaWork.property[4]", 1);

    // Parameters (HP/MP/class).
    b.add_short("charaWork.parameterSave.hp[0]", hp);
    b.add_short("charaWork.parameterSave.hpMax[0]", hp_max);
    b.add_short("charaWork.parameterSave.mp", mp);
    b.add_short("charaWork.parameterSave.mpMax", mp_max);
    b.add_short("charaWork.parameterTemp.tp", tp);
    b.add_byte("charaWork.parameterSave.state_mainSkill[0]", main_skill);
    // C# `ParameterSave.state_mainSkillLevel` is `short`; reflection in
    // `AddProperty` emits it via `AddShort`. We were emitting it as a
    // byte, giving the client a 1-byte payload where it read 2 bytes of
    // the type table — the extra byte came from whatever followed, so
    // every read of this field returned a bogus high nibble.
    b.add_short(
        "charaWork.parameterSave.state_mainSkillLevel",
        main_skill_level as u16,
    );

    // Cast gauge defaults are floats (C# `float[] castGauge_speed = { 1.0f, 0.25f }`).
    b.add_float("charaWork.battleTemp.castGauge_speed[0]", 1.0);
    b.add_float("charaWork.battleTemp.castGauge_speed[1]", 0.25);
    // `skillPoint` is int[] per C# BattleSave; wire slot is class-1.
    let skill_slot = main_skill.saturating_sub(1);
    b.add_int(
        &format!("charaWork.battleSave.skillPoint[{}]", skill_slot),
        skill_point.max(0) as u32,
    );

    b.add_byte("charaWork.commandBorder", command_border);
    // `negotiationFlag` is bool[] — serialized as byte. Project Meteor's
    // Player ctor sets `negotiationFlag[0] = true`; we were sending
    // false, which the client reads as "no default haggling behaviour"
    // and (per the DepictionJudge stack trace) can leave nameplate
    // state partially uninitialised.
    b.add_byte("charaWork.battleSave.negotiationFlag[0]", 1);

    // Project Meteor's Player ctor pre-binds `charaWork.command[0..15]`
    // with 16 starter commands (`0xA0F00000 | id`, Player.cs:235-251), and
    // `GetInitPackets` emits every non-zero slot (Player.cs:478-490). This
    // table is what populates the client's ready-command map — slots 0/1
    // hold 21001, the main-state Activate toggle — so WITHOUT it the F key
    // / sword icon resolves to no command and the client never emits the
    // `0x012D EventStart owner=0xA0F05209 commandForced` that
    // `commands/ActivateCommand.lua` (and the SEQ_005 combat tutorial's
    // `waitForSignal("playerActive")`) depend on.
    //
    // History: an earlier attempt to emit these crashed the client in
    // `ActionMenuWidget:addSlot()` (nil command in
    // `processCanFireWithoutTarget`), and the emission was removed. That
    // crash predated the `state_mainSkillLevel` byte-vs-short encoding fix
    // above — the mis-sized field corrupted every property read that
    // followed it in the 0x0137 stream, which is exactly the failure shape
    // of "the id->command lookup returned nil". pmeteor ships these exact
    // bytes to the same 1.23b client without crashing.
    // (Garlemald-Server #28.)
    const STARTER_COMMANDS: [u32; 16] = [
        21001, 21001, 21002, 12004, 21005, 21006, 21007, 12009, 12010, 12005, 12007, 12011, 22012,
        22013, 29497, 22015,
    ];
    for (i, id) in STARTER_COMMANDS.iter().enumerate() {
        b.add_int(&format!("charaWork.command[{}]", i), 0xA0F0_0000 | *id);
    }
    // Equipped hotbar — pmeteor `Player.GetInitPackets` (Player.cs:484-487)
    // pairs every populated `command[i >= 32]` with that slot's
    // `maxCommandRecastTime` (u16 seconds) + `commandSlot_recastTime`
    // (u32 unix END timestamp; DB 0 = ready now). `commandAcquired` is
    // bool[4096] indexed by raw id − 26000 — pmeteor hardcoded the lone
    // Fast Blade index 1150 (Player.cs:253); emitting it per equipped
    // command generalises that. Out-of-window ids are skipped, never
    // emitted with a bogus index. (#28 S3.1.)
    for (slot0, cmd_masked, max_recast_s, recast_end) in hotbar {
        if cmd_masked & 0xFFFF == 0 {
            continue;
        }
        b.add_int(&format!("charaWork.command[{}]", 32 + slot0), *cmd_masked);
        b.add_short(
            &format!("charaWork.parameterTemp.maxCommandRecastTime[{slot0}]"),
            *max_recast_s,
        );
        b.add_int(
            &format!("charaWork.parameterSave.commandSlot_recastTime[{slot0}]"),
            *recast_end,
        );
        let acquired_index = (cmd_masked & 0xFFFF) as i32 - 26000;
        if (0..4096).contains(&acquired_index) {
            b.add_byte(&format!("charaWork.commandAcquired[{acquired_index}]"), 1);
        }
    }
    for i in 0..36 {
        b.add_byte(&format!("charaWork.additionalCommandAcquired[{}]", i), 1);
    }
    // `battleTemp.generalParameter[0..3] = 1` — the first three slots are
    // `NAMEPLATE_SHOWN` (0), `TARGETABLE` (1), `NAMEPLATE_SHOWN2` (2) per
    // Project Meteor's `BattleTemp.cs` constants; slot 3 is STR. Project
    // Meteor's `GetInitPackets` starts iterating at `i = 3` and only
    // emits non-zero entries — so slots 0/1/2 ride on a client-local
    // default. Our test client (1.23b under Wine) behaves as if those
    // defaults are nil rather than 1, so `DepictionJudge:judgeNameplate()`
    // indexes a nil visibility table at line 900. Emit all three
    // explicitly to seed the client's nameplate-visibility state before
    // the first `_onUpdateWork` tick.
    b.add_short("charaWork.battleTemp.generalParameter[0]", 1);
    b.add_short("charaWork.battleTemp.generalParameter[1]", 1);
    b.add_short("charaWork.battleTemp.generalParameter[2]", 1);
    b.add_short("charaWork.battleTemp.generalParameter[3]", 1);

    // C# forces `commandCategory[i] = 1` for all 64 slots. byte[].
    for i in 0..64 {
        b.add_byte(&format!("charaWork.commandCategory[{}]", i), 1);
    }
    // C# forces `commandSlot_compatibility[i] = true` for all 40 slots. bool[].
    for i in 0..40 {
        b.add_byte(
            &format!("charaWork.parameterSave.commandSlot_compatibility[{}]", i),
            1,
        );
    }

    // Force-control defaults C# hardcodes. `forceControl_float_*` is
    // float[] (defaults {1.0, 1.0, 0.0, 0.0}); `forceControl_int16_*` is
    // short[] (defaults {-1, -1}).
    b.add_float(
        "charaWork.parameterTemp.forceControl_float_forClientSelf[0]",
        1.0,
    );
    b.add_float(
        "charaWork.parameterTemp.forceControl_float_forClientSelf[1]",
        1.0,
    );
    b.add_short(
        "charaWork.parameterTemp.forceControl_int16_forClientSelf[0]",
        0xFFFF,
    );
    b.add_short(
        "charaWork.parameterTemp.forceControl_int16_forClientSelf[1]",
        0xFFFF,
    );
    // byte[2] sentinel values C# sets before AddProperty.
    b.add_byte("charaWork.parameterTemp.otherClassAbilityCount[0]", 4);
    b.add_byte("charaWork.parameterTemp.otherClassAbilityCount[1]", 5);
    b.add_byte("charaWork.parameterTemp.giftCount[1]", 5);
    // `depictionJudge` is a uint in C# (default 0xA0F50911).
    b.add_int("charaWork.depictionJudge", 0xA0F50911);

    // Player profile. `restBonusExpRate` is int, rest are bytes.
    b.add_int("playerWork.restBonusExpRate", rest_bonus_exp_rate as u32);
    b.add_byte("playerWork.tribe", tribe);
    b.add_byte("playerWork.guardian", guardian);
    b.add_byte("playerWork.birthdayMonth", birthday_month);
    b.add_byte("playerWork.birthdayDay", birthday_day);
    b.add_byte("playerWork.initialTown", initial_town);

    // Scenario quest slots. C# `Player.GetInitPackets` (line 540-545) emits
    // `playerWork.questScenario[i]` for each non-zero slot — that's how the
    // client learns which scenario quests are in the journal. The slot
    // payload is the quest's static-actor id (`0xA0F00000 | quest_id`).
    for (slot, quest_actor_id) in active_quests {
        b.add_int(
            &format!("playerWork.questScenario[{slot}]"),
            *quest_actor_id,
        );
    }

    // Local levequests — C# `Player.GetInitPackets` (Player.cs:547-551) emits
    // `playerWork.questGuildleve[slot]` (uint) per non-zero slot. The value
    // carries the `0xA0F00000` quest mask (C# `Database.cs:1285`); garlemald
    // stores the raw quest id, so OR it in here.
    for (slot, quest_id) in local_leves {
        b.add_int(
            &format!("playerWork.questGuildleve[{slot}]"),
            0xA0F0_0000 | *quest_id,
        );
    }
    // Regional levequests — C# (Player.cs:553-563) emits `work.guildleveId`
    // (ushort, raw) plus the `guildleveDone`/`guildleveChecked` bool
    // companions when set. No id mask (C# `Database.cs:1305`).
    for (slot, guildleve_id, abandoned, completed) in regional_leves {
        b.add_short(&format!("work.guildleveId[{slot}]"), *guildleve_id);
        if *abandoned {
            b.add_byte(&format!("work.guildleveDone[{slot}]"), 1);
        }
        if *completed {
            b.add_byte(&format!("work.guildleveChecked[{slot}]"), 1);
        }
    }

    b.done()
}

/// `playerWork/journal`-targeted SetActorProperty companion to
/// [`build_player_property_init`]. C# `Player.SendQuestClientUpdate`
/// (Map Server/Actors/Chara/Player/Player.cs:2048) emits one of these
/// per AddQuest/RemoveQuest call, and `Player.GetInitPackets` opens the
/// player session with one journal-targeted packet listing every active
/// scenario quest. Without this packet the 1.x client's journal tab
/// shows the quest-name string but the surrounding info pane (sequence
/// summary, description text from sqpack) stays blank — captured 2026-04-26
/// against pmeteor's `quest_system_mac` capture which sends 4 separate
/// `playerWork/journal` packets at zone-in (one base + 3 incremental).
///
/// The `/_init` variant in [`build_player_property_init`] also sends the
/// same `playerWork.questScenario[N]` properties, but the 1.x client
/// dispatches by target name — only the `playerWork/journal`-targeted
/// emission triggers the journal-pane refresh.
pub fn build_player_journal_property(
    actor_id: u32,
    active_quests: &[(u32, u32)],
) -> Vec<SubPacket> {
    let mut b = ActorPropertyPacketBuilder::new(actor_id, "playerWork/journal");
    for (slot, quest_actor_id) in active_quests {
        b.add_int(
            &format!("playerWork.questScenario[{slot}]"),
            *quest_actor_id,
        );
    }
    b.done()
}

/// Per-slot live hotbar refresh — pmeteor `Player.UpdateHotbar(slots)`
/// (`UpdateHotbarCommands` + `UpdateRecastTimers`, Player.cs:2502-2543):
/// one `charaWork/command`-targeted packet carrying the slot's command
/// id and category, one `charaWork/commandDetailForSelf`-targeted packet
/// carrying compatibility and recast state. Sent after Equip/Unequip/
/// Swap so live hotbar edits show without re-zoning. An empty slot
/// (`command_masked == 0`) zeroes the command and disables the slot
/// (category/compatibility 0), matching pmeteor's
/// `commandSlot_compatibility[i] = command[i] != 0`. (#28 S3.1.)
pub fn build_hotbar_slot_update(
    actor_id: u32,
    slot0: u16,
    command_masked: u32,
    max_recast_s: u16,
    recast_end: u32,
) -> Vec<SubPacket> {
    let slot = 32 + slot0;
    let occupied = (command_masked & 0xFFFF != 0) as u8;
    let mut cmd = ActorPropertyPacketBuilder::new(actor_id, "charaWork/command");
    cmd.add_int(&format!("charaWork.command[{slot}]"), command_masked);
    cmd.add_byte(&format!("charaWork.commandCategory[{slot}]"), occupied);
    let mut out = cmd.done();
    let mut detail = ActorPropertyPacketBuilder::new(actor_id, "charaWork/commandDetailForSelf");
    detail.add_byte(
        &format!("charaWork.parameterSave.commandSlot_compatibility[{slot0}]"),
        occupied,
    );
    detail.add_short(
        &format!("charaWork.parameterTemp.maxCommandRecastTime[{slot0}]"),
        max_recast_s,
    );
    detail.add_int(
        &format!("charaWork.parameterSave.commandSlot_recastTime[{slot0}]"),
        recast_end,
    );
    out.extend(detail.done());
    out
}

/// Recast-only `charaWork/commandDetailForSelf` pair — pmeteor
/// `Player.UpdateRecastTimers` (Player.cs:2530-2543), fired per used slot
/// on skill completion (`UpdateHotbarTimer`). The client renders the
/// spinner as `recast_end − now`. (#28 S3.3.)
pub fn build_hotbar_recast_update(
    actor_id: u32,
    slot0: u16,
    max_recast_s: u16,
    recast_end: u32,
) -> Vec<SubPacket> {
    let mut b = ActorPropertyPacketBuilder::new(actor_id, "charaWork/commandDetailForSelf");
    b.add_short(
        &format!("charaWork.parameterTemp.maxCommandRecastTime[{slot0}]"),
        max_recast_s,
    );
    b.add_int(
        &format!("charaWork.parameterSave.commandSlot_recastTime[{slot0}]"),
        recast_end,
    );
    b.done()
}

/// NPC `/_init` property dump, modelled on C# `Npc.GetInitPackets()`
/// (Map Server/Actors/Chara/Npc/Npc.cs:228). Emits the populace-baseline
/// property set: `charaWork.property[i]` for each non-zero bit of
/// `propertyFlags`, baseline `potencial=1.0`, HP/MP/TP, two state_mainSkill
/// entries that Meteor's Npc ctor hardcodes (`[0]=3`, `[2]=3`,
/// `state_mainSkillLevel=1`), and `npcWork.hateType`. Without this
/// dump the 1.x client keeps populace nameplates hidden and treats the
/// actor as non-collidable — the spawn-bundle SetActorName carries the
/// right displayNameId but the client only renders the nameplate when
/// `charaWork.property[1] = 1` has arrived via 0x0137.
///
/// `hate_type` is the actor's CURRENT hateType — 1 (passive white) for
/// everything at spawn, hostiles included. `judgeNameplate` RE-RUNS on
/// every WorkSync (`CharaBaseClass:_onUpdateWork`) — there is NO
/// latch, so a later `npcWork/hate` flip restyles the nameplate live.
/// The overhead HP gauge DOES exist in 1.23b (retail shutdown-day
/// screenshots + tutorial videos; the round-2 "RET 0x8 stub / gauges
/// impossible" claim was disproven): the gauge belongs to the client's
/// BATTLE nameplate branch, gated by `charaWork.property[2]` in this
/// dump, with fill computed client-side from charaWork battle state —
/// so battle NPCs must ship bit 2 in `property_flags` and a
/// SHORT-typed `state_mainSkillLevel` or the plate degenerates to a
/// sliver. `level` is the mob's real level (retail sends it; clamped
/// ≥1). (Garlemald-Server #46, round 3.)
#[allow(clippy::too_many_arguments)]
pub fn build_npc_property_init(
    actor_id: u32,
    property_flags: u32,
    hp: u16,
    hp_max: u16,
    mp: u16,
    mp_max: u16,
    tp: u16,
    hate_type: u8,
    level: u16,
) -> Vec<SubPacket> {
    let mut b = ActorPropertyPacketBuilder::new(actor_id, "/_init");
    // potencial=1.0 — Meteor stamps this in the Npc ctor (line 86).
    b.add_float("charaWork.battleSave.potencial", 1.0);
    for i in 0..32u8 {
        if (property_flags >> i) & 1 != 0 {
            b.add_byte(&format!("charaWork.property[{i}]"), 1);
        }
    }
    b.add_short("charaWork.parameterSave.hp[0]", hp);
    b.add_short("charaWork.parameterSave.hpMax[0]", hp_max);
    b.add_short("charaWork.parameterSave.mp", mp);
    b.add_short("charaWork.parameterSave.mpMax", mp_max);
    b.add_short("charaWork.parameterTemp.tp", tp);
    // Meteor's Npc ctor seeds state_mainSkill[0/2]=3 (line 90-92). The
    // AddProperty call sites are guarded on != 0, so we only emit
    // [0]/[2]/level and skip [1]/[3].
    b.add_byte("charaWork.parameterSave.state_mainSkill[0]", 3);
    b.add_byte("charaWork.parameterSave.state_mainSkill[2]", 3);
    // state_mainSkillLevel is a SHORT on the wire (retail pcaps type it
    // 0x02 and send the mob's real level) — the same byte-vs-short
    // encoding bug fixed for the player /_init on 2026-04-19 lived on
    // here untouched. A mis-typed short corrupts the client's NpcBase
    // work-struct parse for the whole init tail (the 2026-06-11
    // "property[2] crash" was this, mis-attributed). (Round-3 nameplate
    // RCA, 2026-07-02.)
    b.add_short("charaWork.parameterSave.state_mainSkillLevel", level.max(1));
    // Meteor's `NpcWork.cs:33` defaults hateType to 1. Sending 0 here
    // tells the 1.x client the actor is inert — no talk prompt, no
    // capsule collider, player walks straight through — so every
    // spawn passes 1 (passive white nameplate; hostiles too — see the
    // fn doc above). Engage-time 2/3 flips arrive via the later
    // `npcWork/hate` subpacket and re-color live (judge re-runs per
    // WorkSync, no latch).
    b.add_byte("npcWork.hateType", hate_type);
    b.done()
}

/// BattleNpc-only `npcWork/hate` property emission, modelled on Meteor's
/// `BattleNpc.GetHateTypePacket` (Actors/Chara/Npc/BattleNpc.cs:145).
/// Always emitted at the tail of the BattleNpc spawn bundle, after the
/// ScriptBind. Uses the non-`/_init` target "npcWork/hate" so the 1.x
/// client routes it through its hate-state update path instead of the
/// boot-property path.
///
/// `hate_type` values — per `DepictionJudge:judgeNameplate()` (which
/// RE-RUNS on every WorkSync via `CharaBaseClass:_onUpdateWork` — no
/// latch; hateType STYLES the plate; the overhead HP gauge itself is
/// the bit-2 battle-branch element, see `build_npc_property_init`):
///   0 = inert — treated as no-interaction (no talk prompt / no
///       collider when it lands in the `/_init` dump); avoid.
///   1 = passive WHITE — populace AND unengaged hostiles (retail
///       spawns every mob white; this is the spawn-table value).
///   2 = engaged ORANGE — in-combat tint; does NOT dereference party
///       state.
///   3 = claimed — RED only if the mob's party is the player party's
///       occupancy group (0x0187 Set Occupancy Group claim), ELSE the
///       PURPLE "claimed by another party" tint. Meteor hardcodes 3
///       unconditionally at spawn (`BattleNpc.cs:160`); doing that
///       without the 0x0187 claim renders every idle hostile purple
///       (2026-07-01 retest) — and before the solo-party 0x017C
///       registration existed it crashed the judge outright
///       ("attempt to compare number with nil", 2026-04-21).
///
///   4 = corpse plate — retail rides it with the hp=0 death state
///       (`HATE_TYPE_DEAD`, dispatcher death arm).
///
/// The round-2 claim that no overhead HP gauge exists in 1.23b was
/// DISPROVEN by retail screenshots/videos — the gauge is real and
/// belongs to the `charaWork.property[2]` battle nameplate branch;
/// enemy HP additionally renders in the target parameter widget from
/// `charaWork.parameterSave.hp`. (Garlemald-Server #46, round 3.)
pub fn build_npc_hate_type_packet(actor_id: u32, hate_type: u8) -> SubPacket {
    let mut b = ActorPropertyPacketBuilder::new(actor_id, "npcWork/hate");
    b.add_byte("npcWork.hateType", hate_type);
    let mut packets = b.done();
    packets.remove(0)
}

/// `charaWork/stateAtQuicklyForAll` emission — base (Chara) variant.
/// Mirrors C# `Character.PostUpdate` `HpTpMp` branch:
///   hp[0], hpMax[0], mp, mpMax, parameterTemp.tp
/// The target path uses `/` separators (not `.`) because the C# emits
/// this property group under a distinct namespace — the client keys
/// its nameplate HP-bar table off the slashed name.
pub fn build_chara_state_at_quickly_for_all(
    actor_id: u32,
    hp: u16,
    hp_max: u16,
    mp: u16,
    mp_max: u16,
    tp: u16,
) -> Vec<SubPacket> {
    let mut b = ActorPropertyPacketBuilder::new(actor_id, "charaWork/stateAtQuicklyForAll");
    b.add_short("charaWork.parameterSave.hp[0]", hp);
    b.add_short("charaWork.parameterSave.hpMax[0]", hp_max);
    b.add_short("charaWork.parameterSave.mp", mp);
    b.add_short("charaWork.parameterSave.mpMax", mp_max);
    b.add_short("charaWork.parameterTemp.tp", tp);
    b.done()
}

/// `charaWork/stateAtQuicklyForAll` emission — Player-override variant.
/// Mirrors C# `Player.PostUpdate` `HpTpMp` branch which emits a second
/// pass with the main-skill slot fields on top of the base pass.
pub fn build_player_state_at_quickly_for_all(
    actor_id: u32,
    hp: u16,
    hp_max: u16,
    main_skill: u8,
    main_skill_level: u16,
) -> Vec<SubPacket> {
    let mut b = ActorPropertyPacketBuilder::new(actor_id, "charaWork/stateAtQuicklyForAll");
    b.add_short("charaWork.parameterSave.hp[0]", hp);
    b.add_short("charaWork.parameterSave.hpMax[0]", hp_max);
    b.add_byte("charaWork.parameterSave.state_mainSkill[0]", main_skill);
    b.add_short(
        "charaWork.parameterSave.state_mainSkillLevel",
        main_skill_level,
    );
    b.done()
}

/// `charaWork/battleParameter` emission. Mirrors C# `Player.PostUpdate`
/// `Stats` branch which emits `charaWork.battleTemp.generalParameter[i]`
/// for each non-zero slot in 0..35. For the Asdf-shape login we emit
/// the three nameplate-visibility slots (0=NAMEPLATE_SHOWN,
/// 1=TARGETABLE, 2=NAMEPLATE_SHOWN2, plus 3=STR default 1) that the
/// client's DepictionJudge:judgeNameplate references every tick.
pub fn build_battle_parameter(actor_id: u32, general_parameter: &[i16; 35]) -> Vec<SubPacket> {
    let mut b = ActorPropertyPacketBuilder::new(actor_id, "charaWork/battleParameter");
    for (i, v) in general_parameter.iter().enumerate() {
        if *v != 0 {
            b.add_short(
                &format!("charaWork.battleTemp.generalParameter[{}]", i),
                *v as u16,
            );
        }
    }
    b.done()
}

use std::io::Write as _;

#[cfg(test)]
mod npc_property_init_tests {
    use super::*;

    /// Scan a property-packet series for the staged `add_byte` record of
    /// `name` (`[type=1][murmur2(name) LE][value]`) and return its value.
    fn find_byte_property(packets: &[SubPacket], name: &str) -> Option<u8> {
        let id = common::utils::murmur_hash2(name, 0).to_le_bytes();
        for p in packets {
            let d = &p.data;
            for i in 0..d.len().saturating_sub(5) {
                if d[i] == 1 && d[i + 1..i + 5] == id {
                    return Some(d[i + 5]);
                }
            }
        }
        None
    }

    /// The builder is value-agnostic — it stamps whatever hateType the
    /// caller resolved into the `/_init` dump. Spawn tables pass 1
    /// (passive white — retail spawns every actor white, hostiles
    /// included); 2 is the engaged-orange tint an engage-time caller
    /// would carry. The judge re-runs per WorkSync (no latch), so this
    /// only needs to be the CURRENT value, not a pre-latched combat
    /// one. (Garlemald-Server #46, round 2.)
    #[test]
    fn init_dump_carries_the_passed_hate_type() {
        let spawn = build_npc_property_init(0x4700_0001, 0x13, 100, 100, 50, 50, 0, 1, 1);
        assert_eq!(
            find_byte_property(&spawn, "npcWork.hateType"),
            Some(1),
            "spawn-table /_init carries the passive-white 1 (all actors, hostiles too)"
        );
        let engaged = build_npc_property_init(0x4700_0002, 0x13, 100, 100, 50, 50, 0, 2, 3);
        assert_eq!(
            find_byte_property(&engaged, "npcWork.hateType"),
            Some(2),
            "builder passes an engaged-orange 2 through unchanged (value-agnostic)"
        );
    }
}

#[cfg(test)]
mod reset_head_tests {
    use super::*;

    /// Reproduce the body bytes captured from
    /// `ffxiv_traces/combat_skills.pcapng` 0x00DE record #1 — actor
    /// `0x44D035D5` (mob), 8-byte zero body.
    #[test]
    fn reset_head_matches_retail_capture() {
        let pkt = build_reset_head(0x44D0_35D5);
        assert_eq!(pkt.data.len(), 8);
        assert!(pkt.data.iter().all(|b| *b == 0));
        assert_eq!(pkt.game_message.opcode, OP_RESET_HEAD);
        assert_eq!(pkt.header.source_id, 0x44D0_35D5);
    }
}

#[cfg(test)]
mod do_emote_tests {
    use super::*;

    /// The bare emote id must be packed as `0x5000000 | (id << 12)` (pmeteor
    /// `ActorDoEmotePacket`). Writing the raw id makes the client print the
    /// description text but play no animation. (Garlemald-Server #46.)
    #[test]
    fn animation_id_is_packed() {
        let pkt = build_actor_do_emote(0x0000_0001, 5, 0x4730_00D8, 21041);
        assert_eq!(pkt.game_message.opcode, OP_ACTOR_DO_EMOTE);
        let real = u32::from_le_bytes([pkt.data[0], pkt.data[1], pkt.data[2], pkt.data[3]]);
        assert_eq!(real, 0x0500_0000 | (5u32 << 12), "realAnimID = 0x5005000");
        // target + description pass through unchanged when target != 0.
        assert_eq!(
            u32::from_le_bytes([pkt.data[4], pkt.data[5], pkt.data[6], pkt.data[7]]),
            0x4730_00D8
        );
        assert_eq!(
            u32::from_le_bytes([pkt.data[8], pkt.data[9], pkt.data[10], pkt.data[11]]),
            21041
        );
    }

    /// target == 0 retargets to self and bumps the description id (pmeteor).
    #[test]
    fn target_zero_falls_back_to_self() {
        let pkt = build_actor_do_emote(0x0000_0001, 5, 0, 21041);
        assert_eq!(
            u32::from_le_bytes([pkt.data[4], pkt.data[5], pkt.data[6], pkt.data[7]]),
            0x0000_0001,
            "retargets to source actor"
        );
        assert_eq!(
            u32::from_le_bytes([pkt.data[8], pkt.data[9], pkt.data[10], pkt.data[11]]),
            21042,
            "description id +1"
        );
    }
}

#[cfg(test)]
mod event_status_push_tests {
    use super::*;
    use crate::actor::event_conditions::parse_event_conditions;

    fn push_status(packets: &[SubPacket]) -> bool {
        // Find the push (ty==2) SetEventStatus packet and read its enabled
        // flag (u32 LE at body +0x00, type byte at +0x04).
        let p = packets
            .iter()
            .find(|p| p.game_message.opcode == OP_SET_EVENT_STATUS && p.data[4] == 2)
            .expect("a push SetEventStatus packet");
        u32::from_le_bytes([p.data[0], p.data[1], p.data[2], p.data[3]]) != 0
    }

    /// A quest trigger ships `isEnabled=false`; with no explicit override the
    /// streamed packet must come in DISABLED (Meteor `push ?? isEnabled`).
    /// This is the guard against the pre-#46 default-true behavior that let
    /// a streamed trigger fire the moment it appeared.
    #[test]
    fn push_none_honours_condition_disabled_default() {
        let conds = parse_event_conditions(
            r#"{"pushWithCircleEventConditions":[{"isEnabled":"false","radius":"6.0","conditionName":"pushDefault"}]}"#,
        )
        .unwrap();
        let packets = build_actor_event_status_packets(0x4730_009D, &conds, true, true, None, true);
        assert!(
            !push_status(&packets),
            "disabled trigger must stay disabled"
        );
    }

    /// An explicit `Some(true)` from a `quest:SetENpc(.., QFLAG_PUSH)`
    /// broadcast overrides the actor-class default and enables the circle.
    #[test]
    fn push_some_true_overrides_to_enabled() {
        let conds = parse_event_conditions(
            r#"{"pushWithCircleEventConditions":[{"isEnabled":"false","radius":"6.0","conditionName":"pushDefault"}]}"#,
        )
        .unwrap();
        let packets =
            build_actor_event_status_packets(0x4730_009D, &conds, true, true, Some(true), true);
        assert!(
            push_status(&packets),
            "quest enable must win over isEnabled"
        );
    }

    /// A condition whose data default is `isEnabled=true` enables on stream
    /// even without an override.
    #[test]
    fn push_none_honours_condition_enabled_default() {
        let conds = parse_event_conditions(
            r#"{"pushWithCircleEventConditions":[{"isEnabled":"true","radius":"6.0","conditionName":"pushDefault"}]}"#,
        )
        .unwrap();
        let packets = build_actor_event_status_packets(0x4730_009D, &conds, true, true, None, true);
        assert!(
            push_status(&packets),
            "enabled-by-default trigger streams enabled"
        );
    }
}

#[cfg(test)]
mod move_actor_to_position_tests {
    use super::*;

    /// Reproduce Yda's warp-in 0x00CF from the pmeteor reference capture
    /// (`captures/pmeteor-quest/20260426-160210-gridania-manual3/
    /// map-packets.log:33617-33621`) — src 0x40080007, x=365.266 @ +0x08
    /// behind an 8-byte zero prefix, y=4.1219, z=-700.730, rot=1.5659,
    /// moveState=0. Pins the +0x08 float offset: writing x at +0x00 (the
    /// pre-#28 layout) makes the client decode garbage coordinates for
    /// every NPC movement packet.
    #[test]
    fn move_actor_to_position_matches_pmeteor_capture() {
        let x = f32::from_le_bytes([0x0C, 0xA2, 0xB6, 0x43]); // 365.266
        let y = f32::from_le_bytes([0x6D, 0xE7, 0x83, 0x40]); // 4.1219
        let z = f32::from_le_bytes([0xB8, 0x2E, 0x2F, 0xC4]); // -700.730
        let rot = f32::from_le_bytes([0x69, 0x6F, 0xC8, 0x3F]); // 1.5659
        let pkt = build_move_actor_to_position(0x4008_0007, x, y, z, rot, 0);

        assert_eq!(pkt.game_message.opcode, OP_MOVE_ACTOR_TO_POSITION);
        assert_eq!(pkt.header.source_id, 0x4008_0007);
        assert_eq!(pkt.data.len(), 0x30);
        // +0x00..0x08: the two unknown u32 stay zero.
        assert!(pkt.data[..0x08].iter().all(|b| *b == 0));
        // Floats from +0x08, capture bytes verbatim.
        assert_eq!(
            &pkt.data[0x08..0x18],
            &[
                0x0C, 0xA2, 0xB6, 0x43, // x
                0x6D, 0xE7, 0x83, 0x40, // y
                0xB8, 0x2E, 0x2F, 0xC4, // z
                0x69, 0x6F, 0xC8, 0x3F, // rot
            ],
        );
        // +0x18 moveState; everything after (incl. +0x24 floatingHeight)
        // stays zero for ground mobs.
        assert_eq!(&pkt.data[0x18..0x1A], &[0x00, 0x00]);
        assert!(pkt.data[0x1A..].iter().all(|b| *b == 0));

        // Running variant: moveState=2 lands at +0x18.
        let run = build_move_actor_to_position(0x4008_0007, x, y, z, rot, 2);
        assert_eq!(&run.data[0x18..0x1A], &[0x02, 0x00]);
    }
}

#[cfg(test)]
mod player_property_init_tests {
    use super::*;

    /// The init property stream must carry pmeteor's 16 starter command
    /// bindings (`charaWork.command[0..15]`, Player.cs:235-251). Slot 0/1
    /// are `0xA0F00000 | 21001 = 0xA0F05209` — the main-state Activate
    /// toggle, i.e. the exact actor id the client puts in its
    /// `0x012D EventStart owner=0xA0F05209 commandForced` when F / the
    /// sword icon is pressed. Without these properties the client's
    /// ready-command table is empty and F is dead (Garlemald-Server #28).
    #[test]
    fn init_properties_carry_starter_commands() {
        let subs = build_player_property_init(
            7,
            100,
            100,
            100,
            100,
            0,
            2,
            1,
            0,
            32,
            0,
            0,
            1,
            1,
            1,
            0,
            &[],
            &[],
            &[],
            &[],
        );
        let stream: Vec<u8> = subs.iter().flat_map(|s| s.to_bytes()).collect();
        let activate = 0xA0F0_5209u32.to_le_bytes();
        let hits = stream.windows(4).filter(|w| *w == activate).count();
        assert!(
            hits >= 2,
            "expected charaWork.command[0] and [1] to carry 0xA0F05209 (Activate); found {hits} occurrence(s)",
        );
    }

    /// Encode one staged property as the exact wire triple
    /// `[type_byte, murmur2(name) LE, value LE]` so the assertions below
    /// can pin both the property id AND its wire type — pmeteor's
    /// reflection emits command as int, maxCommandRecastTime as short,
    /// commandSlot_recastTime as int, commandAcquired as byte, and a
    /// mis-typed field corrupts every property that follows it in the
    /// 0x0137 stream (the historical `state_mainSkillLevel` crash).
    fn property_needle(type_byte: u8, name: &str, value_le: &[u8]) -> Vec<u8> {
        let mut needle = vec![type_byte];
        needle.extend_from_slice(&common::utils::murmur_hash2(name, 0).to_le_bytes());
        needle.extend_from_slice(value_le);
        needle
    }

    /// #28 S3.1 — a GLA hotbar `[(0, 0xA0F06A0E, 10, 0)]` (Fast Blade
    /// 27150, masked, 10 s recast, ready now) emits exactly the four
    /// pmeteor `LoadHotbar`/`GetInitPackets` properties: the masked
    /// command id at `command[32]`, the recast companions at 0-based
    /// slot 0, and `commandAcquired[1150]` (= 27150 − 26000).
    #[test]
    fn init_properties_carry_hotbar_slots() {
        let hotbar = [(0u16, 0xA0F0_6A0Eu32, 10u16, 0u32)];
        let subs = build_player_property_init(
            7,
            100,
            100,
            100,
            100,
            0,
            3,
            1,
            0,
            32,
            0,
            0,
            1,
            1,
            1,
            0,
            &[],
            &[],
            &[],
            &hotbar,
        );
        let stream: Vec<u8> = subs.iter().flat_map(|s| s.to_bytes()).collect();
        for (label, needle) in [
            (
                "charaWork.command[32] = 0xA0F06A0E (int)",
                property_needle(4, "charaWork.command[32]", &0xA0F0_6A0Eu32.to_le_bytes()),
            ),
            (
                "maxCommandRecastTime[0] = 10 (short)",
                property_needle(
                    2,
                    "charaWork.parameterTemp.maxCommandRecastTime[0]",
                    &10u16.to_le_bytes(),
                ),
            ),
            (
                "commandSlot_recastTime[0] = 0 (int)",
                property_needle(
                    4,
                    "charaWork.parameterSave.commandSlot_recastTime[0]",
                    &0u32.to_le_bytes(),
                ),
            ),
            (
                "commandAcquired[1150] = 1 (byte)",
                property_needle(1, "charaWork.commandAcquired[1150]", &[1]),
            ),
        ] {
            assert!(
                stream.windows(needle.len()).any(|w| w == needle),
                "init stream missing {label}",
            );
        }
    }

    /// Kodama parity — the `/_init` bundle emits levequest slots with the
    /// exact wire types + mask C# `Player.GetInitPackets` uses: local
    /// `playerWork.questGuildleve[slot]` as int carrying the `0xA0F00000`
    /// quest mask (Database.cs:1285), regional `work.guildleveId[slot]` as
    /// short with the RAW id (no mask, Database.cs:1305), and the
    /// `guildleveDone` byte companion only when abandoned. A `completed=false`
    /// regional slot must NOT emit `guildleveChecked`.
    #[test]
    fn init_properties_carry_levequest_slots() {
        let local = [(2u16, 0x1234u32)];
        let regional = [(3u16, 0x0056u16, true, false)];
        let subs = build_player_property_init(
            7,
            100,
            100,
            100,
            100,
            0,
            2,
            1,
            0,
            32,
            0,
            0,
            1,
            1,
            1,
            0,
            &[],
            &local,
            &regional,
            &[],
        );
        let stream: Vec<u8> = subs.iter().flat_map(|s| s.to_bytes()).collect();
        // Local leve: int, value carries the 0xA0F00000 quest mask.
        let local_needle = property_needle(
            4,
            "playerWork.questGuildleve[2]",
            &(0xA0F0_0000u32 | 0x1234).to_le_bytes(),
        );
        assert!(
            stream
                .windows(local_needle.len())
                .any(|w| w == local_needle),
            "missing masked local questGuildleve[2]",
        );
        // Regional leve: short, raw guildleve id (no mask).
        let regional_needle = property_needle(2, "work.guildleveId[3]", &0x0056u16.to_le_bytes());
        assert!(
            stream
                .windows(regional_needle.len())
                .any(|w| w == regional_needle),
            "missing raw regional guildleveId[3]",
        );
        // abandoned=true -> guildleveDone byte present.
        let done_needle = property_needle(1, "work.guildleveDone[3]", &[1]);
        assert!(
            stream.windows(done_needle.len()).any(|w| w == done_needle),
            "missing guildleveDone[3] for abandoned slot",
        );
        // completed=false -> guildleveChecked must be absent (match the
        // type+id prefix so any value would fail the search).
        let checked_prefix = property_needle(1, "work.guildleveChecked[3]", &[]);
        assert!(
            !stream
                .windows(checked_prefix.len())
                .any(|w| w == checked_prefix),
            "guildleveChecked[3] must not be emitted when completed=false",
        );
    }

    /// #28 S3.1 — the post-equip `UpdateHotbar` pair carries the
    /// command + category under `charaWork/command` and compat +
    /// recast under `charaWork/commandDetailForSelf`, and an empty
    /// slot disables rather than ghosts the button.
    #[test]
    fn hotbar_slot_update_pair_carries_both_targets() {
        let subs = build_hotbar_slot_update(7, 0, 0xA0F0_6A0E, 10, 1234);
        assert_eq!(subs.len(), 2, "one packet per property target");
        let cmd_bytes = subs[0].to_bytes();
        let detail_bytes = subs[1].to_bytes();
        assert!(
            cmd_bytes
                .windows(b"charaWork/command".len())
                .any(|w| w == b"charaWork/command"),
            "first packet targets charaWork/command",
        );
        assert!(
            detail_bytes
                .windows(b"charaWork/commandDetailForSelf".len())
                .any(|w| w == b"charaWork/commandDetailForSelf"),
            "second packet targets charaWork/commandDetailForSelf",
        );
        let cmd_needle = property_needle(4, "charaWork.command[32]", &0xA0F0_6A0Eu32.to_le_bytes());
        assert!(cmd_bytes.windows(cmd_needle.len()).any(|w| w == cmd_needle));
        let recast_needle = property_needle(
            4,
            "charaWork.parameterSave.commandSlot_recastTime[0]",
            &1234u32.to_le_bytes(),
        );
        assert!(
            detail_bytes
                .windows(recast_needle.len())
                .any(|w| w == recast_needle)
        );

        // Empty slot: command 0, category + compatibility 0.
        let empty = build_hotbar_slot_update(7, 0, 0, 0, 0);
        let empty_cmd = empty[0].to_bytes();
        let category_off = property_needle(1, "charaWork.commandCategory[32]", &[0]);
        assert!(
            empty_cmd
                .windows(category_off.len())
                .any(|w| w == category_off),
            "empty slot must emit commandCategory[32] = 0",
        );
    }
}
