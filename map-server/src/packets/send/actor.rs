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
    command_border: u8,
    tribe: u8,
    guardian: u8,
    birthday_day: u8,
    birthday_month: u8,
    initial_town: u8,
    rest_bonus_exp_rate: i32,
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
    // `skillPoint` is int[] per C# BattleSave.
    let skill_slot = main_skill.saturating_sub(1);
    b.add_int(
        &format!("charaWork.battleSave.skillPoint[{}]", skill_slot),
        0,
    );

    b.add_byte("charaWork.commandBorder", command_border);
    // `negotiationFlag` is bool[] — serialized as byte. Project Meteor's
    // Player ctor sets `negotiationFlag[0] = true`; we were sending
    // false, which the client reads as "no default haggling behaviour"
    // and (per the DepictionJudge stack trace) can leave nameplate
    // state partially uninitialised.
    b.add_byte("charaWork.battleSave.negotiationFlag[0]", 1);

    // Project Meteor's Player ctor pre-binds `charaWork.command[0..15]`
    // with 16 starter commands (`0xA0F00000 | id`). Emitting those caused
    // the 1.23b client to advance *past* the DepictionJudge nameplate
    // error but fail a step later in `ActionMenuWidget:addSlot()` —
    // `DesktopWidget:isStackIntoActionMenu()` line 12448 calls
    // `processCanFireWithoutTarget` on a nil command, which means the
    // id->command lookup in the client's own data archive returned nil
    // for at least one of our bound ids. Leaving the command slots
    // unpopulated takes the ActionMenu down an empty-slot branch instead
    // of an invalid-slot branch. We still emit `commandAcquired` and
    // `additionalCommandAcquired` because those are plain flag arrays
    // that don't require the client to resolve any command id.
    b.add_byte("charaWork.commandAcquired[1150]", 1);
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

    b.done()
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
