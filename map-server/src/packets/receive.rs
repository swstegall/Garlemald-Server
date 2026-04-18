//! Incoming packets parsed by the map server. One Rust struct per C# receive
//! class, with `parse(&[u8]) -> Result<Self>`.
#![allow(dead_code)]

use std::io::{Cursor, Read, Seek, SeekFrom};

use anyhow::Result;
use byteorder::{LittleEndian, ReadBytesExt};
use common::luaparam::{self, LuaParam};

fn read_null_term_ascii(c: &mut Cursor<&[u8]>, max: usize) -> String {
    let mut out = Vec::with_capacity(max);
    let mut buf = [0u8; 1];
    while out.len() < max && c.read_exact(&mut buf).is_ok() {
        if buf[0] == 0 {
            break;
        }
        out.push(buf[0]);
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ---------------------------------------------------------------------------
// Session + handshake
// ---------------------------------------------------------------------------

/// Handshake (initial client hello). Session id lives at offset 4 as an
/// ASCII decimal string up to 10 chars.
#[derive(Debug, Clone)]
pub struct HandshakePacket {
    pub actor_id: u32,
}

impl HandshakePacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        c.seek(SeekFrom::Start(4))?;
        let s = read_null_term_ascii(&mut c, 10);
        Ok(Self {
            actor_id: s.trim().parse().unwrap_or(0),
        })
    }
}

/// PingPacket — client sends a u32 timestamp.
#[derive(Debug, Clone)]
pub struct PingPacket {
    pub time: u32,
}

impl PingPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        Ok(Self {
            time: c.read_u32::<LittleEndian>()?,
        })
    }
}

/// `_0x02ReceivePacket` — mirror of the C# handshake ack-completion frame.
#[derive(Debug, Clone)]
pub struct Handshake0x02Packet {
    pub unknown: u32,
}

impl Handshake0x02Packet {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        c.seek(SeekFrom::Start(0x14))?;
        Ok(Self {
            unknown: c.read_u32::<LittleEndian>()?,
        })
    }
}

// ---------------------------------------------------------------------------
// World session control (from World Server)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SessionBeginRequest {
    pub session_id: u32,
    pub is_login: bool,
}

impl SessionBeginRequest {
    pub fn parse(sub_source_id: u32, data: &[u8]) -> Result<Self> {
        Ok(Self {
            session_id: sub_source_id,
            is_login: data.first().copied().unwrap_or(0) != 0,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SessionEndRequest {
    pub session_id: u32,
    pub destination_zone_id: u32,
    pub destination_spawn_type: u8,
    pub destination_x: f32,
    pub destination_y: f32,
    pub destination_z: f32,
    pub destination_rot: f32,
}

impl SessionEndRequest {
    pub fn parse(sub_source_id: u32, data: &[u8]) -> Result<Self> {
        if data.len() < 4 {
            return Ok(Self {
                session_id: sub_source_id,
                destination_zone_id: 0,
                destination_spawn_type: 0,
                destination_x: 0.0,
                destination_y: 0.0,
                destination_z: 0.0,
                destination_rot: 0.0,
            });
        }
        let mut c = Cursor::new(data);
        Ok(Self {
            session_id: sub_source_id,
            destination_zone_id: c.read_u32::<LittleEndian>()?,
            destination_spawn_type: c.read_u16::<LittleEndian>().unwrap_or(0) as u8,
            destination_x: c.read_f32::<LittleEndian>().unwrap_or(0.0),
            destination_y: c.read_f32::<LittleEndian>().unwrap_or(0.0),
            destination_z: c.read_f32::<LittleEndian>().unwrap_or(0.0),
            destination_rot: c.read_f32::<LittleEndian>().unwrap_or(0.0),
        })
    }
}

/// `PartySyncPacket` — world server pushes a party roster snapshot to the
/// map server (C# `Packets/WorldPackets/Receive/PartySyncPacket.cs`).
#[derive(Debug, Clone)]
pub struct PartySyncPacket {
    pub party_group_id: u64,
    pub owner: u32,
    pub member_actor_ids: Vec<u32>,
}

impl PartySyncPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        let party_group_id = c.read_u64::<LittleEndian>()?;
        let owner = c.read_u32::<LittleEndian>()?;
        let n = c.read_u32::<LittleEndian>()? as usize;
        let mut member_actor_ids = Vec::with_capacity(n);
        for _ in 0..n {
            member_actor_ids.push(c.read_u32::<LittleEndian>()?);
        }
        Ok(Self {
            party_group_id,
            owner,
            member_actor_ids,
        })
    }
}

/// `LinkshellResultPacket` — world server echoes the result of a
/// linkshell-mutation RPC back to map server.
#[derive(Debug, Clone)]
pub struct LinkshellResultPacket {
    pub result_code: i32,
}

impl LinkshellResultPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        Ok(Self {
            result_code: c.read_i32::<LittleEndian>()?,
        })
    }
}

/// `ErrorPacket` — generic world-server error frame.
#[derive(Debug, Clone)]
pub struct WorldErrorPacket {
    pub error_code: u32,
}

impl WorldErrorPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        Ok(Self {
            error_code: c.read_u32::<LittleEndian>()?,
        })
    }
}

// ---------------------------------------------------------------------------
// Misc client frames
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LanguageCodePacket {
    pub language_code: u32,
}
impl LanguageCodePacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        let _ = c.read_u32::<LittleEndian>()?;
        Ok(Self {
            language_code: c.read_u32::<LittleEndian>()?,
        })
    }
}

/// UpdatePlayerPositionPacket — client position heartbeat.
#[derive(Debug, Clone)]
pub struct UpdatePlayerPositionPacket {
    pub timestamp: u64,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rot: f32,
    pub move_state: u16,
}

impl UpdatePlayerPositionPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        Ok(Self {
            timestamp: c.read_u64::<LittleEndian>()?,
            x: c.read_f32::<LittleEndian>()?,
            y: c.read_f32::<LittleEndian>()?,
            z: c.read_f32::<LittleEndian>()?,
            rot: c.read_f32::<LittleEndian>()?,
            move_state: c.read_u16::<LittleEndian>()?,
        })
    }
}

/// ZoneInCompletePacket — client signals "I finished loading the zone".
#[derive(Debug, Clone)]
pub struct ZoneInCompletePacket {
    pub timestamp: u32,
    pub unknown: i32,
}

impl ZoneInCompletePacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        Ok(Self {
            timestamp: c.read_u32::<LittleEndian>()?,
            unknown: c.read_i32::<LittleEndian>()?,
        })
    }
}

/// SetTargetPacket — client picks a target.
#[derive(Debug, Clone)]
pub struct SetTargetPacket {
    pub actor_id: u32,
    pub attack_target: u32,
}
impl SetTargetPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        Ok(Self {
            actor_id: c.read_u32::<LittleEndian>()?,
            attack_target: c.read_u32::<LittleEndian>()?,
        })
    }
}

/// LockTargetPacket.
#[derive(Debug, Clone)]
pub struct LockTargetPacket {
    pub actor_id: u32,
    pub other_val: u32,
}
impl LockTargetPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        Ok(Self {
            actor_id: c.read_u32::<LittleEndian>()?,
            other_val: c.read_u32::<LittleEndian>()?,
        })
    }
}

/// CountdownRequestPacket — player wants to start a `/countdown` in-game.
#[derive(Debug, Clone)]
pub struct CountdownRequestPacket {
    pub countdown_length: u8,
    pub sync_time: u64,
}
impl CountdownRequestPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        let countdown_length = c.read_u8()?;
        c.seek(SeekFrom::Start(8))?;
        let sync_time = c.read_u64::<LittleEndian>()?;
        Ok(Self {
            countdown_length,
            sync_time,
        })
    }
}

/// UpdateItemPackagePacket — client requests a refresh of an inventory bucket.
#[derive(Debug, Clone)]
pub struct UpdateItemPackagePacket {
    pub actor_id: u32,
    pub package_id: u32,
}
impl UpdateItemPackagePacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        Ok(Self {
            actor_id: c.read_u32::<LittleEndian>()?,
            package_id: c.read_u32::<LittleEndian>()?,
        })
    }
}

// ---------------------------------------------------------------------------
// Chat
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ChatMessagePacket {
    pub pos_x: f32,
    pub pos_y: f32,
    pub pos_z: f32,
    pub pos_rot: f32,
    pub log_type: u32,
    pub message: String,
}

impl ChatMessagePacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        let _ = c.read_u64::<LittleEndian>()?;
        let pos_x = c.read_f32::<LittleEndian>()?;
        let pos_y = c.read_f32::<LittleEndian>()?;
        let pos_z = c.read_f32::<LittleEndian>()?;
        let pos_rot = c.read_f32::<LittleEndian>()?;
        let log_type = c.read_u32::<LittleEndian>()?;
        let message = read_null_term_ascii(&mut c, 0x200);
        Ok(Self {
            pos_x,
            pos_y,
            pos_z,
            pos_rot,
            log_type,
            message,
        })
    }
}

/// Legacy alias still referenced by the Phase-4 processor stub.
pub type IncomingChatMessage = ChatMessagePacket;

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct EventStartPacket {
    pub trigger_actor_id: u32,
    pub owner_actor_id: u32,
    pub server_codes: u32,
    pub unknown: u32,
    pub event_type: u8,
    /// Null-terminated event name (e.g. `"quest_man0l0"`). Read from
    /// offset 0x11; bounded by the packet body length.
    pub event_name: String,
    pub lua_params: Vec<LuaParam>,
}

impl EventStartPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        let trigger_actor_id = c.read_u32::<LittleEndian>()?;
        let owner_actor_id = c.read_u32::<LittleEndian>()?;
        let server_codes = c.read_u32::<LittleEndian>()?;
        let unknown = c.read_u32::<LittleEndian>()?;
        let event_type = c.read_u8()?;
        // Matches the C# parser: read null-term ASCII for the event name,
        // then — if the next byte isn't 0x01 — decode the LuaParam tail.
        let event_name = read_null_term_ascii(&mut c, 256);
        let pos = c.position() as usize;
        let lua_params = if pos < data.len() && data[pos] == 0x01 {
            Vec::new()
        } else {
            luaparam::read_lua_params(&data[pos..]).unwrap_or_default()
        };
        Ok(Self {
            trigger_actor_id,
            owner_actor_id,
            server_codes,
            unknown,
            event_type,
            event_name,
            lua_params,
        })
    }
}

#[derive(Debug, Clone)]
pub struct EventUpdatePacket {
    pub trigger_actor_id: u32,
    pub server_codes: u32,
    pub unknown1: u32,
    pub unknown2: u32,
    pub event_type: u8,
    pub lua_params: Vec<LuaParam>,
}

impl EventUpdatePacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        let trigger_actor_id = c.read_u32::<LittleEndian>()?;
        let server_codes = c.read_u32::<LittleEndian>()?;
        let unknown1 = c.read_u32::<LittleEndian>()?;
        let unknown2 = c.read_u32::<LittleEndian>()?;
        let event_type = c.read_u8()?;
        let pos = c.position() as usize;
        let lua_params = luaparam::read_lua_params(&data[pos..]).unwrap_or_default();
        Ok(Self {
            trigger_actor_id,
            server_codes,
            unknown1,
            unknown2,
            event_type,
            lua_params,
        })
    }
}

/// GroupCreatedPacket — echo from client when it finishes wiring a group.
#[derive(Debug, Clone)]
pub struct GroupCreatedPacket {
    pub group_id: u64,
    pub work_string: String,
}

impl GroupCreatedPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        let group_id = c.read_u64::<LittleEndian>()?;
        let work_string = read_null_term_ascii(&mut c, 0x200);
        Ok(Self {
            group_id,
            work_string,
        })
    }
}

// ---------------------------------------------------------------------------
// Social
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AddRemoveSocialPacket {
    pub name: String,
}
impl AddRemoveSocialPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        Ok(Self {
            name: read_null_term_ascii(&mut c, 0x20),
        })
    }
}

#[derive(Debug, Clone)]
pub struct FriendlistRequestPacket {
    pub num1: u32,
    pub num2: u32,
}
impl FriendlistRequestPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        Ok(Self {
            num1: c.read_u32::<LittleEndian>()?,
            num2: c.read_u32::<LittleEndian>()?,
        })
    }
}

// ---------------------------------------------------------------------------
// Support desk
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FaqBodyRequestPacket {
    pub faq_index: u32,
    pub lang_code: u32,
}
impl FaqBodyRequestPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        Ok(Self {
            faq_index: c.read_u32::<LittleEndian>()?,
            lang_code: c.read_u32::<LittleEndian>()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct FaqListRequestPacket {
    pub lang_code: u32,
    pub unknown: u32,
}
impl FaqListRequestPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        Ok(Self {
            lang_code: c.read_u32::<LittleEndian>()?,
            unknown: c.read_u32::<LittleEndian>()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct GMSupportTicketPacket {
    pub lang_code: u32,
    pub ticket_issue_index: u32,
    pub ticket_title: String,
    pub ticket_body: String,
}
impl GMSupportTicketPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        let lang_code = c.read_u32::<LittleEndian>()?;
        let ticket_issue_index = c.read_u32::<LittleEndian>()?;
        let ticket_title = read_null_term_ascii(&mut c, 0x80);
        let ticket_body = read_null_term_ascii(&mut c, 0x800);
        Ok(Self {
            lang_code,
            ticket_issue_index,
            ticket_title,
            ticket_body,
        })
    }
}

#[derive(Debug, Clone)]
pub struct GMTicketIssuesRequestPacket {
    pub lang_code: u32,
}
impl GMTicketIssuesRequestPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        Ok(Self {
            lang_code: c.read_u32::<LittleEndian>()?,
        })
    }
}

// ---------------------------------------------------------------------------
// Recruitment
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RecruitmentDetailsRequestPacket {
    pub recruitment_id: u64,
}
impl RecruitmentDetailsRequestPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        Ok(Self {
            recruitment_id: c.read_u64::<LittleEndian>()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct RecruitmentSearchRequestPacket {
    pub purpose_id: u32,
    pub location_id: u32,
    pub disciple_id: u32,
    pub classjob_id: u32,
    pub unknown1: u8,
    pub unknown2: u8,
    pub text: String,
}
impl RecruitmentSearchRequestPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        let purpose_id = c.read_u32::<LittleEndian>()?;
        let location_id = c.read_u32::<LittleEndian>()?;
        let disciple_id = c.read_u32::<LittleEndian>()?;
        let classjob_id = c.read_u32::<LittleEndian>()?;
        let unknown1 = c.read_u8()?;
        let unknown2 = c.read_u8()?;
        let text = read_null_term_ascii(&mut c, 0x80);
        Ok(Self {
            purpose_id,
            location_id,
            disciple_id,
            classjob_id,
            unknown1,
            unknown2,
            text,
        })
    }
}

#[derive(Debug, Clone)]
pub struct StartRecruitingRequestPacket {
    pub purpose_id: u32,
    pub location_id: u32,
    pub sub_task_id: u32,
    pub disciple_id: [u32; 4],
    pub classjob_id: [u32; 4],
    pub min_lvl: [u8; 4],
    pub max_lvl: [u8; 4],
    pub num: [u8; 4],
}
impl StartRecruitingRequestPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        let purpose_id = c.read_u32::<LittleEndian>()?;
        let location_id = c.read_u32::<LittleEndian>()?;
        let sub_task_id = c.read_u32::<LittleEndian>()?;
        let mut disciple_id = [0u32; 4];
        let mut classjob_id = [0u32; 4];
        let mut min_lvl = [0u8; 4];
        let mut max_lvl = [0u8; 4];
        let mut num = [0u8; 4];
        for i in 0..4 {
            disciple_id[i] = c.read_u32::<LittleEndian>()?;
            classjob_id[i] = c.read_u32::<LittleEndian>()?;
            min_lvl[i] = c.read_u8()?;
            max_lvl[i] = c.read_u8()?;
            num[i] = c.read_u8()?;
            let _ = c.read_u8()?; // 1-byte padding
        }
        Ok(Self {
            purpose_id,
            location_id,
            sub_task_id,
            disciple_id,
            classjob_id,
            min_lvl,
            max_lvl,
            num,
        })
    }
}

// ---------------------------------------------------------------------------
// Achievements / misc
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AchievementProgressRequestPacket {
    pub achievement_id: u32,
    pub response_type: u32,
}
impl AchievementProgressRequestPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        Ok(Self {
            achievement_id: c.read_u32::<LittleEndian>()?,
            response_type: c.read_u32::<LittleEndian>()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ParameterDataRequestPacket {
    pub actor_id: u32,
    pub param_name: String,
}
impl ParameterDataRequestPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        let actor_id = c.read_u32::<LittleEndian>()?;
        let param_name = read_null_term_ascii(&mut c, 0x20);
        Ok(Self {
            actor_id,
            param_name,
        })
    }
}

// ---------------------------------------------------------------------------
// GameMessage envelope — generic wrapper around the per-subpacket payload.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct GameMessageEnvelope {
    pub sender_actor_id: u32,
    pub body: Vec<u8>,
}

impl GameMessageEnvelope {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        let sender_actor_id = c.read_u32::<LittleEndian>().unwrap_or(0);
        let mut body = Vec::new();
        c.read_to_end(&mut body)?;
        Ok(Self {
            sender_actor_id,
            body,
        })
    }
}
