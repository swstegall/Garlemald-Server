use std::io::Cursor;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::error::PacketError;
use crate::utils;

pub const SUBPACKET_SIZE: usize = 0x10;
pub const GAMEMESSAGE_SIZE: usize = 0x10;

pub const SUBPACKET_TYPE_GAMEMESSAGE: u16 = 0x03;

#[derive(Debug, Clone, Copy, Default)]
pub struct SubPacketHeader {
    pub subpacket_size: u16,
    pub r#type: u16,
    pub source_id: u32,
    pub target_id: u32,
    pub unknown1: u32,
}

impl SubPacketHeader {
    pub fn read(buf: &[u8]) -> Result<Self, PacketError> {
        if buf.len() < SUBPACKET_SIZE {
            return Err(PacketError::TooSmall {
                needed: SUBPACKET_SIZE,
                have: buf.len(),
            });
        }
        let mut c = Cursor::new(buf);
        Ok(Self {
            subpacket_size: c.read_u16::<LittleEndian>()?,
            r#type: c.read_u16::<LittleEndian>()?,
            source_id: c.read_u32::<LittleEndian>()?,
            target_id: c.read_u32::<LittleEndian>()?,
            unknown1: c.read_u32::<LittleEndian>()?,
        })
    }

    pub fn write(&self, out: &mut [u8; SUBPACKET_SIZE]) {
        let mut c = Cursor::new(&mut out[..]);
        c.write_u16::<LittleEndian>(self.subpacket_size).unwrap();
        c.write_u16::<LittleEndian>(self.r#type).unwrap();
        c.write_u32::<LittleEndian>(self.source_id).unwrap();
        c.write_u32::<LittleEndian>(self.target_id).unwrap();
        c.write_u32::<LittleEndian>(self.unknown1).unwrap();
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GameMessageHeader {
    pub unknown4: u16,
    pub opcode: u16,
    pub unknown5: u32,
    pub timestamp: u32,
    pub unknown6: u32,
}

impl GameMessageHeader {
    pub fn read(buf: &[u8]) -> Result<Self, PacketError> {
        if buf.len() < GAMEMESSAGE_SIZE {
            return Err(PacketError::TooSmall {
                needed: GAMEMESSAGE_SIZE,
                have: buf.len(),
            });
        }
        let mut c = Cursor::new(buf);
        Ok(Self {
            unknown4: c.read_u16::<LittleEndian>()?,
            opcode: c.read_u16::<LittleEndian>()?,
            unknown5: c.read_u32::<LittleEndian>()?,
            timestamp: c.read_u32::<LittleEndian>()?,
            unknown6: c.read_u32::<LittleEndian>()?,
        })
    }

    pub fn write(&self, out: &mut [u8; GAMEMESSAGE_SIZE]) {
        let mut c = Cursor::new(&mut out[..]);
        c.write_u16::<LittleEndian>(self.unknown4).unwrap();
        c.write_u16::<LittleEndian>(self.opcode).unwrap();
        c.write_u32::<LittleEndian>(self.unknown5).unwrap();
        c.write_u32::<LittleEndian>(self.timestamp).unwrap();
        c.write_u32::<LittleEndian>(self.unknown6).unwrap();
    }
}

#[derive(Debug, Clone)]
pub struct SubPacket {
    pub header: SubPacketHeader,
    pub game_message: GameMessageHeader,
    pub data: Vec<u8>,
}

impl SubPacket {
    /// Mirrors the C# `SubPacket(bool isGameMessage, ushort opcode, uint sourceId, byte[] data)`.
    /// When `is_game_message` is true, the 16-byte game-message header is
    /// prefixed and the subpacket `type` is forced to 0x03 (GameMessage).
    pub fn new_with_flag(
        is_game_message: bool,
        opcode: u16,
        source_id: u32,
        data: Vec<u8>,
    ) -> Self {
        let mut header = SubPacketHeader::default();
        let mut game_message = GameMessageHeader::default();

        if is_game_message {
            game_message.opcode = opcode;
            game_message.timestamp = utils::unix_timestamp();
            game_message.unknown4 = 0x14;
        }

        header.source_id = source_id;
        header.r#type = if is_game_message {
            SUBPACKET_TYPE_GAMEMESSAGE
        } else {
            opcode
        };
        header.subpacket_size = (SUBPACKET_SIZE + data.len()) as u16;
        if is_game_message {
            header.subpacket_size += GAMEMESSAGE_SIZE as u16;
        }

        SubPacket {
            header,
            game_message,
            data,
        }
    }

    /// Convenience constructor: game-message subpacket.
    pub fn new(opcode: u16, source_id: u32, data: Vec<u8>) -> Self {
        Self::new_with_flag(true, opcode, source_id, data)
    }

    /// Re-target an existing subpacket (used when relaying to a different actor).
    pub fn with_target(other: &SubPacket, new_target: u32) -> Self {
        let mut header = other.header;
        header.target_id = new_target;
        SubPacket {
            header,
            game_message: other.game_message,
            data: other.data.clone(),
        }
    }

    pub fn set_target_id(&mut self, target: u32) {
        self.header.target_id = target;
    }

    pub fn parse(bytes: &[u8], offset: &mut usize) -> Result<SubPacket, PacketError> {
        if bytes.len() < *offset + SUBPACKET_SIZE {
            return Err(PacketError::TooSmall {
                needed: *offset + SUBPACKET_SIZE,
                have: bytes.len(),
            });
        }

        let header = SubPacketHeader::read(&bytes[*offset..*offset + SUBPACKET_SIZE])?;

        if bytes.len() < *offset + header.subpacket_size as usize {
            return Err(PacketError::SizeMismatch {
                declared: header.subpacket_size as usize,
                available: bytes.len() - *offset,
            });
        }

        let mut game_message = GameMessageHeader::default();
        let data_start;
        if header.r#type == SUBPACKET_TYPE_GAMEMESSAGE {
            game_message = GameMessageHeader::read(
                &bytes[*offset + SUBPACKET_SIZE..*offset + SUBPACKET_SIZE + GAMEMESSAGE_SIZE],
            )?;
            data_start = *offset + SUBPACKET_SIZE + GAMEMESSAGE_SIZE;
        } else {
            data_start = *offset + SUBPACKET_SIZE;
        }

        let data_end = *offset + header.subpacket_size as usize;
        let data = bytes[data_start..data_end].to_vec();

        *offset += header.subpacket_size as usize;
        Ok(SubPacket {
            header,
            game_message,
            data,
        })
    }

    /// Try to parse a single subpacket from a buffer slice, returning `None`
    /// if there isn't enough data yet. Offset is advanced on success.
    pub fn try_parse(buffer: &[u8], offset: &mut usize, bytes_read: usize) -> Option<SubPacket> {
        if bytes_read <= *offset || buffer.len() < *offset + 2 {
            return None;
        }
        let size = u16::from_le_bytes([buffer[*offset], buffer[*offset + 1]]) as usize;
        if bytes_read < *offset + size || buffer.len() < *offset + size {
            return None;
        }
        SubPacket::parse(buffer, offset).ok()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let total = self.header.subpacket_size as usize;
        let mut out = vec![0u8; total];

        let mut hdr = [0u8; SUBPACKET_SIZE];
        self.header.write(&mut hdr);
        out[..SUBPACKET_SIZE].copy_from_slice(&hdr);

        let body_start = if self.header.r#type == SUBPACKET_TYPE_GAMEMESSAGE {
            let mut gm = [0u8; GAMEMESSAGE_SIZE];
            self.game_message.write(&mut gm);
            out[SUBPACKET_SIZE..SUBPACKET_SIZE + GAMEMESSAGE_SIZE].copy_from_slice(&gm);
            SUBPACKET_SIZE + GAMEMESSAGE_SIZE
        } else {
            SUBPACKET_SIZE
        };

        out[body_start..body_start + self.data.len()].copy_from_slice(&self.data);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_message_roundtrip() {
        let sub = SubPacket::new(0xBEEF, 0xDEADBEEF, vec![1, 2, 3, 4]);
        let bytes = sub.to_bytes();
        assert_eq!(bytes.len(), sub.header.subpacket_size as usize);

        let mut off = 0;
        let parsed = SubPacket::parse(&bytes, &mut off).unwrap();
        assert_eq!(parsed.header.source_id, 0xDEADBEEF);
        assert_eq!(parsed.header.r#type, SUBPACKET_TYPE_GAMEMESSAGE);
        assert_eq!(parsed.game_message.opcode, 0xBEEF);
        assert_eq!(parsed.data, vec![1, 2, 3, 4]);
        assert_eq!(off, bytes.len());
    }

    #[test]
    fn raw_subpacket_roundtrip() {
        let sub = SubPacket::new_with_flag(false, 0x01, 0x11, vec![9, 8, 7]);
        let bytes = sub.to_bytes();
        let mut off = 0;
        let parsed = SubPacket::parse(&bytes, &mut off).unwrap();
        assert_eq!(parsed.header.r#type, 0x01);
        assert_eq!(parsed.data, vec![9, 8, 7]);
    }
}
