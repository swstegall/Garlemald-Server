//! Incoming world-server packets.
#![allow(dead_code)]

use std::io::{Cursor, Read};

use anyhow::Result;
use byteorder::{LittleEndian, ReadBytesExt};

fn read_fixed_string(c: &mut Cursor<&[u8]>, len: usize) -> Result<String> {
    let mut buf = vec![0u8; len];
    c.read_exact(&mut buf)?;
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    Ok(String::from_utf8_lossy(&buf[..end]).into_owned())
}

/// Initial `0x01` hello frame from client. The session id arrives as an ASCII
/// decimal string at offset 0x14 of the base packet body; we parse that into
/// a u32.
#[derive(Debug, Clone)]
pub struct HelloPacket {
    pub session_id: u32,
}

impl HelloPacket {
    pub fn parse(base_packet_body: &[u8]) -> Result<Self> {
        if base_packet_body.len() < 0x14 + 12 {
            anyhow::bail!("hello packet too small");
        }
        let ascii = &base_packet_body[0x14..0x14 + 12];
        let s: String = ascii.iter().take_while(|&&b| b != 0).map(|&b| b as char).collect();
        let session_id: u32 = s.trim().parse().unwrap_or(0);
        Ok(Self { session_id })
    }
}

// ---------------------------------------------------------------------------
// World-server confirmation/notification frames (opcode >= 0x1000)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SessionBeginConfirmPacket {
    pub session_id: u32,
    pub error_code: u16,
}

impl SessionBeginConfirmPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        Ok(Self {
            session_id: c.read_u32::<LittleEndian>()?,
            error_code: c.read_u16::<LittleEndian>()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SessionEndConfirmPacket {
    pub session_id: u32,
    pub error_code: u16,
    pub destination_zone: u32,
}

impl SessionEndConfirmPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        Ok(Self {
            session_id: c.read_u32::<LittleEndian>()?,
            error_code: c.read_u16::<LittleEndian>()?,
            destination_zone: c.read_u32::<LittleEndian>()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct WorldRequestZoneChangePacket {
    pub session_id: u32,
    pub destination_zone_id: u32,
    pub destination_spawn_type: u8,
    pub destination_x: f32,
    pub destination_y: f32,
    pub destination_z: f32,
    pub destination_rot: f32,
}

impl WorldRequestZoneChangePacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        let session_id = c.read_u32::<LittleEndian>()?;
        let destination_zone_id = c.read_u32::<LittleEndian>()?;
        let destination_spawn_type = c.read_u16::<LittleEndian>()? as u8;
        let destination_x = c.read_f32::<LittleEndian>()?;
        let destination_y = c.read_f32::<LittleEndian>()?;
        let destination_z = c.read_f32::<LittleEndian>()?;
        let destination_rot = c.read_f32::<LittleEndian>()?;
        Ok(Self {
            session_id,
            destination_zone_id,
            destination_spawn_type,
            destination_x,
            destination_y,
            destination_z,
            destination_rot,
        })
    }
}

// ---------------------------------------------------------------------------
// Party
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PartyModifyPacket {
    pub command: u16,
    pub actor_id: u32,
    pub name: String,
}

impl PartyModifyPacket {
    pub const MODIFY_LEADER: u16 = 0;
    pub const MODIFY_KICKPLAYER: u16 = 1;

    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        let command = c.read_u16::<LittleEndian>()?;
        let (actor_id, name) = if command >= 2 {
            (c.read_u32::<LittleEndian>()?, String::new())
        } else {
            (0, read_fixed_string(&mut c, 0x20)?)
        };
        Ok(Self { command, actor_id, name })
    }
}

#[derive(Debug, Clone)]
pub struct PartyLeavePacket {
    pub is_disband: bool,
}

impl PartyLeavePacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        let flag = c.read_u8()?;
        Ok(Self { is_disband: flag == 1 })
    }
}

#[derive(Debug, Clone)]
pub struct PartyInvitePacket {
    pub command: u16,
    pub actor_id: u32,
    pub name: String,
}

impl PartyInvitePacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        let command = c.read_u16::<LittleEndian>()?;
        if command == 1 {
            Ok(Self { command, actor_id: c.read_u32::<LittleEndian>()?, name: String::new() })
        } else {
            Ok(Self { command, actor_id: 0, name: read_fixed_string(&mut c, 0x20)? })
        }
    }
}

#[derive(Debug, Clone)]
pub struct GroupInviteResultPacket {
    pub group_type: u32,
    pub result: u32,
}

impl GroupInviteResultPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        Ok(Self {
            group_type: c.read_u32::<LittleEndian>()?,
            result: c.read_u32::<LittleEndian>()?,
        })
    }
}

// ---------------------------------------------------------------------------
// Linkshells
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CreateLinkshellPacket {
    pub name: String,
    pub crest_id: u16,
    pub master: u32,
}

impl CreateLinkshellPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        let name = read_fixed_string(&mut c, 0x20)?;
        let crest_id = c.read_u16::<LittleEndian>()?;
        let master = c.read_u32::<LittleEndian>()?;
        Ok(Self { name, crest_id, master })
    }
}

#[derive(Debug, Clone)]
pub struct ModifyLinkshellPacket {
    pub current_name: String,
    pub arg_code: u16,
    pub new_name: String,
    pub crest_id: u16,
    pub master: u32,
}

impl ModifyLinkshellPacket {
    pub const CODE_RENAME: u16 = 0;
    pub const CODE_CRESTCHANGE: u16 = 1;
    pub const CODE_MASTERCHANGE: u16 = 2;

    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        let current_name = read_fixed_string(&mut c, 0x20)?;
        let arg_code = c.read_u16::<LittleEndian>()?;
        let mut new_name = String::new();
        let mut crest_id = 0u16;
        let mut master = 0u32;
        match arg_code {
            Self::CODE_RENAME => new_name = read_fixed_string(&mut c, 0x20)?,
            Self::CODE_CRESTCHANGE => crest_id = c.read_u16::<LittleEndian>()?,
            Self::CODE_MASTERCHANGE => master = c.read_u32::<LittleEndian>()?,
            _ => {}
        }
        Ok(Self { current_name, arg_code, new_name, crest_id, master })
    }
}

#[derive(Debug, Clone)]
pub struct DeleteLinkshellPacket {
    pub name: String,
}

impl DeleteLinkshellPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        Ok(Self { name: read_fixed_string(&mut c, 0x20)? })
    }
}

#[derive(Debug, Clone)]
pub struct LinkshellChangePacket {
    pub ls_name: String,
}

impl LinkshellChangePacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        Ok(Self { ls_name: read_fixed_string(&mut c, 0x20)? })
    }
}

#[derive(Debug, Clone)]
pub struct LinkshellInvitePacket {
    pub actor_id: u32,
    pub ls_name: String,
}

impl LinkshellInvitePacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        let actor_id = c.read_u32::<LittleEndian>()?;
        let ls_name = read_fixed_string(&mut c, 0x20)?;
        Ok(Self { actor_id, ls_name })
    }
}

#[derive(Debug, Clone)]
pub struct LinkshellLeavePacket {
    pub is_kicked: bool,
    pub kicked_name: String,
    pub ls_name: String,
}

impl LinkshellLeavePacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        let is_kicked = c.read_u16::<LittleEndian>()? == 1;
        let kicked_name = read_fixed_string(&mut c, 0x20)?;
        let ls_name = read_fixed_string(&mut c, 0x20)?;
        Ok(Self { is_kicked, kicked_name, ls_name })
    }
}

#[derive(Debug, Clone)]
pub struct LinkshellRankChangePacket {
    pub name: String,
    pub ls_name: String,
    pub rank: u8,
}

impl LinkshellRankChangePacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        let name = read_fixed_string(&mut c, 0x20)?;
        let ls_name = read_fixed_string(&mut c, 0x20)?;
        let rank = c.read_u8()?;
        Ok(Self { name, ls_name, rank })
    }
}

// ---------------------------------------------------------------------------
// GameMessage inline intercepts (opcode == 3 game messages)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PartyChatMessagePacket {
    pub message: String,
}

impl PartyChatMessagePacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        // PartyChatMessagePacket.cs reads a null-terminated string from offset 0.
        let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
        Ok(Self { message: String::from_utf8_lossy(&data[..end]).into_owned() })
    }
}

#[derive(Debug, Clone)]
pub struct GroupCreatedPacket {
    pub group_id: u64,
}

impl GroupCreatedPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        Ok(Self { group_id: c.read_u64::<LittleEndian>()? })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use byteorder::WriteBytesExt;

    #[test]
    fn parses_session_end_confirm() {
        let mut buf = Vec::new();
        buf.write_u32::<LittleEndian>(42).unwrap();
        buf.write_u16::<LittleEndian>(1).unwrap();
        buf.write_u32::<LittleEndian>(166).unwrap();
        let p = SessionEndConfirmPacket::parse(&buf).unwrap();
        assert_eq!(p.session_id, 42);
        assert_eq!(p.error_code, 1);
        assert_eq!(p.destination_zone, 166);
    }

    #[test]
    fn parses_party_modify_name() {
        let mut buf = Vec::new();
        buf.write_u16::<LittleEndian>(0).unwrap();
        let mut name = b"Alice".to_vec();
        name.resize(0x20, 0);
        buf.extend_from_slice(&name);
        let p = PartyModifyPacket::parse(&buf).unwrap();
        assert_eq!(p.command, 0);
        assert_eq!(p.name, "Alice");
    }
}
