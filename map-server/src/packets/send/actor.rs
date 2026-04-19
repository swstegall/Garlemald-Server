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

pub fn build_set_actor_speed_default(actor_id: u32) -> SubPacket {
    build_set_actor_speed(actor_id, 0.0, 2.0, 5.0, 5.0)
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

/// 0x00E1 ActorDoEmotePacket.
pub fn build_actor_do_emote(
    actor_id: u32,
    real_anim_id: u32,
    targeted_actor_id: u32,
    description_id: u32,
) -> SubPacket {
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

/// 0x0136 SetEventStatusPacket.
pub fn build_set_event_status(
    actor_id: u32,
    enabled: bool,
    ty: u8,
    condition_name: &str,
) -> SubPacket {
    let mut data = body(0x48);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u8(enabled as u8).unwrap();
    c.write_u8(ty).unwrap();
    write_padded_ascii(&mut c, condition_name, 0x20);
    SubPacket::new(OP_SET_EVENT_STATUS, actor_id, data)
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

/// Player-specific `/_init` property dump, modelled on C#
/// `Player.GetInitPackets()` + `ActorPropertyPacketUtil`. The 1.23b client
/// treats the minimal Actor init (3 anonymous flags) as enough to render a
/// generic NPC, but the Player UI needs HP/MP/class info before it will
/// leave "Now Loading" and instantiate the HUD. Property ids are the
/// Murmur2 hash of the path string; the byte prefixes match
/// `SetActorPropetyPacket.AddByte/AddShort/AddInt`.
///
/// Fields and field widths follow the C# reflection path:
/// - `charaWork.parameterSave.hp[0]`                 u16 (AddShort, type 2)
/// - `charaWork.parameterSave.hpMax[0]`              u16
/// - `charaWork.parameterSave.mp`                    u16
/// - `charaWork.parameterSave.mpMax`                 u16
/// - `charaWork.parameterTemp.tp`                    u16
/// - `charaWork.parameterSave.state_mainSkill[0]`    u8 (AddByte, type 1)
/// - `charaWork.parameterSave.state_mainSkillLevel`  u8
/// - `charaWork.commandBorder`                       u8
/// - `playerWork.tribe`                              u8
/// - `playerWork.guardian`                           u8
/// - `playerWork.birthdayDay`                        u8
/// - `playerWork.birthdayMonth`                      u8
/// - `playerWork.initialTown`                        u8
/// - `playerWork.restBonusExpRate`                   i32 (AddInt, type 4)
///
/// Total wire cost: 9×AddByte (54) + 5×AddShort (35) + 1×AddInt (9) +
/// target marker (7) = 105 bytes. Stays under the C# MAXBYTES=125 limit
/// so it all fits in a single packet.
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
    command_border: u8,
    tribe: u8,
    guardian: u8,
    birthday_day: u8,
    birthday_month: u8,
    initial_town: u8,
    rest_bonus_exp_rate: i32,
) -> SubPacket {
    let mut data = body(0xA8);
    let mut c = Cursor::new(&mut data[..]);
    c.set_position(1);

    // AddShort(id, value) — type=2, then u32 id, then u16 value. 7 bytes.
    let add_short = |c: &mut Cursor<&mut [u8]>, name: &str, value: u16| {
        let id = common::utils::murmur_hash2(name, 0);
        c.write_u8(2).unwrap();
        c.write_u32::<LittleEndian>(id).unwrap();
        c.write_u16::<LittleEndian>(value).unwrap();
    };
    // AddByte(id, value) — type=1, u32 id, u8 value. 6 bytes.
    let add_byte = |c: &mut Cursor<&mut [u8]>, name: &str, value: u8| {
        let id = common::utils::murmur_hash2(name, 0);
        c.write_u8(1).unwrap();
        c.write_u32::<LittleEndian>(id).unwrap();
        c.write_u8(value).unwrap();
    };
    // AddInt(id, value) — type=4, u32 id, u32 value. 9 bytes.
    let add_int = |c: &mut Cursor<&mut [u8]>, name: &str, value: u32| {
        let id = common::utils::murmur_hash2(name, 0);
        c.write_u8(4).unwrap();
        c.write_u32::<LittleEndian>(id).unwrap();
        c.write_u32::<LittleEndian>(value).unwrap();
    };

    add_short(&mut c, "charaWork.parameterSave.hp[0]", hp);
    add_short(&mut c, "charaWork.parameterSave.hpMax[0]", hp_max);
    add_short(&mut c, "charaWork.parameterSave.mp", mp);
    add_short(&mut c, "charaWork.parameterSave.mpMax", mp_max);
    add_short(&mut c, "charaWork.parameterTemp.tp", tp);
    add_byte(&mut c, "charaWork.parameterSave.state_mainSkill[0]", main_skill);
    add_byte(
        &mut c,
        "charaWork.parameterSave.state_mainSkillLevel",
        main_skill_level,
    );
    add_byte(&mut c, "charaWork.commandBorder", command_border);
    add_byte(&mut c, "playerWork.tribe", tribe);
    add_byte(&mut c, "playerWork.guardian", guardian);
    add_byte(&mut c, "playerWork.birthdayDay", birthday_day);
    add_byte(&mut c, "playerWork.birthdayMonth", birthday_month);
    add_byte(&mut c, "playerWork.initialTown", initial_town);
    add_int(
        &mut c,
        "playerWork.restBonusExpRate",
        rest_bonus_exp_rate as u32,
    );

    let target = b"/_init";
    c.write_u8(0x82u8 + target.len() as u8).unwrap();
    c.write_all(target).unwrap();

    let running_total = 5 * 7 + 9 * 6 + 1 * 9 + 1 + target.len();
    data[0] = running_total as u8;
    SubPacket::new(OP_SET_ACTOR_PROPERTY, actor_id, data)
}

use std::io::Write as _;
