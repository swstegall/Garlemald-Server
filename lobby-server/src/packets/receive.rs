//! Incoming lobby packets. Field layouts mirror the C# receivers in
//! Lobby Server/Packets/Receive — byte offsets preserved verbatim.
//!
//! Some fields (`unknown_id`, `person_type`) aren't consumed by any handler
//! today but are part of the wire format, so we keep them rather than skip
//! bytes blindly on parse.
#![allow(dead_code)]

use std::io::{Cursor, Read, Seek, SeekFrom};

use anyhow::Result;
use byteorder::{LittleEndian, ReadBytesExt};

fn read_fixed_string(c: &mut Cursor<&[u8]>, len: usize) -> Result<String> {
    let mut buf = vec![0u8; len];
    c.read_exact(&mut buf)?;
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    Ok(String::from_utf8_lossy(&buf[..end]).into_owned())
}

#[derive(Debug, Clone)]
pub struct SecurityHandshakePacket {
    pub ticket_phrase: String,
    pub client_number: u32,
}

impl SecurityHandshakePacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        c.seek(SeekFrom::Start(0x34))?;
        let ticket_phrase = read_fixed_string(&mut c, 0x40)?;
        let client_number = c.read_u32::<LittleEndian>()?;
        Ok(Self { ticket_phrase, client_number })
    }
}

#[derive(Debug, Clone)]
pub struct SessionPacket {
    pub sequence: u64,
    pub session: String,
    pub version: String,
}

impl SessionPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        let sequence = c.read_u64::<LittleEndian>()?;
        let _ = c.read_u32::<LittleEndian>()?;
        let _ = c.read_u32::<LittleEndian>()?;
        let session = read_fixed_string(&mut c, 0x40)?;
        let version = read_fixed_string(&mut c, 0x20)?;
        Ok(Self { sequence, session, version })
    }
}

#[derive(Debug, Clone)]
pub struct SelectCharacterPacket {
    pub sequence: u64,
    pub character_id: u32,
    pub unknown_id: u32,
    pub ticket: u64,
}

impl SelectCharacterPacket {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        let sequence = c.read_u64::<LittleEndian>()?;
        let character_id = c.read_u32::<LittleEndian>()?;
        let unknown_id = c.read_u32::<LittleEndian>()?;
        let ticket = c.read_u64::<LittleEndian>()?;
        Ok(Self { sequence, character_id, unknown_id, ticket })
    }
}

#[derive(Debug, Clone)]
pub struct CharacterModifyPacket {
    pub sequence: u64,
    pub character_id: u32,
    pub person_type: u32,
    pub slot: u8,
    pub command: u8,
    pub world_id: u16,
    pub character_name: String,
    pub character_info_encoded: String,
}

impl CharacterModifyPacket {
    pub const CMD_RESERVE: u8 = 0x01;
    pub const CMD_MAKE: u8 = 0x02;
    pub const CMD_RENAME: u8 = 0x03;
    pub const CMD_DELETE: u8 = 0x04;
    pub const CMD_RENAME_RETAINER: u8 = 0x06;

    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(data);
        let sequence = c.read_u64::<LittleEndian>()?;
        let character_id = c.read_u32::<LittleEndian>()?;
        let person_type = c.read_u32::<LittleEndian>()?;
        let slot = c.read_u8()?;
        let command = c.read_u8()?;
        let world_id = c.read_u16::<LittleEndian>()?;
        let character_name = read_fixed_string(&mut c, 0x20)?;
        let character_info_encoded = read_fixed_string(&mut c, 0x190)?;
        Ok(Self {
            sequence,
            character_id,
            person_type,
            slot,
            command,
            world_id,
            character_name,
            character_info_encoded,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use byteorder::{LittleEndian, WriteBytesExt};
    use std::io::Write;

    #[test]
    fn parses_select_character() {
        let mut buf = Vec::new();
        buf.write_u64::<LittleEndian>(0xdeadbeef).unwrap();
        buf.write_u32::<LittleEndian>(42).unwrap();
        buf.write_u32::<LittleEndian>(0).unwrap();
        buf.write_u64::<LittleEndian>(0x1234).unwrap();
        let packet = SelectCharacterPacket::parse(&buf).unwrap();
        assert_eq!(packet.sequence, 0xdeadbeef);
        assert_eq!(packet.character_id, 42);
        assert_eq!(packet.ticket, 0x1234);
    }

    #[test]
    fn parses_session_packet_trims_nulls() {
        let mut buf = Vec::new();
        buf.write_u64::<LittleEndian>(1).unwrap();
        buf.write_u32::<LittleEndian>(0).unwrap();
        buf.write_u32::<LittleEndian>(0).unwrap();
        let session = b"abc";
        let mut session_field = session.to_vec();
        session_field.resize(0x40, 0);
        buf.write_all(&session_field).unwrap();
        let version = b"1.23b";
        let mut version_field = version.to_vec();
        version_field.resize(0x20, 0);
        buf.write_all(&version_field).unwrap();

        let pkt = SessionPacket::parse(&buf).unwrap();
        assert_eq!(pkt.session, "abc");
        assert_eq!(pkt.version, "1.23b");
    }
}
