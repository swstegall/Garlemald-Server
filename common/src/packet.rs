use std::io::{Cursor, Read, Write};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;

use crate::blowfish::Blowfish;
use crate::error::PacketError;
use crate::subpacket::{SUBPACKET_SIZE, SubPacket};
use crate::utils;

pub const PACKET_TYPE_ZONE: u16 = 1;
pub const PACKET_TYPE_CHAT: u16 = 2;
pub const BASEPACKET_SIZE: usize = 0x10;

#[derive(Debug, Clone, Copy, Default)]
pub struct BasePacketHeader {
    pub is_authenticated: u8,
    pub is_compressed: u8,
    pub connection_type: u16,
    pub packet_size: u16,
    pub num_subpackets: u16,
    pub timestamp: u64,
}

impl BasePacketHeader {
    pub fn read(buf: &[u8]) -> Result<Self, PacketError> {
        if buf.len() < BASEPACKET_SIZE {
            return Err(PacketError::TooSmall { needed: BASEPACKET_SIZE, have: buf.len() });
        }
        let mut c = Cursor::new(buf);
        Ok(Self {
            is_authenticated: c.read_u8()?,
            is_compressed: c.read_u8()?,
            connection_type: c.read_u16::<LittleEndian>()?,
            packet_size: c.read_u16::<LittleEndian>()?,
            num_subpackets: c.read_u16::<LittleEndian>()?,
            timestamp: c.read_u64::<LittleEndian>()?,
        })
    }

    pub fn write(&self, out: &mut [u8; BASEPACKET_SIZE]) {
        let mut c = Cursor::new(&mut out[..]);
        c.write_u8(self.is_authenticated).unwrap();
        c.write_u8(self.is_compressed).unwrap();
        c.write_u16::<LittleEndian>(self.connection_type).unwrap();
        c.write_u16::<LittleEndian>(self.packet_size).unwrap();
        c.write_u16::<LittleEndian>(self.num_subpackets).unwrap();
        c.write_u64::<LittleEndian>(self.timestamp).unwrap();
    }
}

#[derive(Debug, Clone)]
pub struct BasePacket {
    pub header: BasePacketHeader,
    pub data: Vec<u8>,
}

impl BasePacket {
    pub fn from_bytes(bytes: &[u8]) -> Result<BasePacket, PacketError> {
        if bytes.len() < BASEPACKET_SIZE {
            return Err(PacketError::TooSmall { needed: BASEPACKET_SIZE, have: bytes.len() });
        }
        let header = BasePacketHeader::read(&bytes[..BASEPACKET_SIZE])?;

        if bytes.len() < header.packet_size as usize {
            return Err(PacketError::SizeMismatch {
                declared: header.packet_size as usize,
                available: bytes.len(),
            });
        }

        let data = bytes[BASEPACKET_SIZE..header.packet_size as usize].to_vec();
        Ok(BasePacket { header, data })
    }

    /// Parse one packet from a framing buffer. Advances `offset` on success.
    pub fn from_buffer(bytes: &[u8], offset: &mut usize) -> Result<BasePacket, PacketError> {
        if bytes.len() < *offset + BASEPACKET_SIZE {
            return Err(PacketError::TooSmall {
                needed: *offset + BASEPACKET_SIZE,
                have: bytes.len(),
            });
        }
        let header = BasePacketHeader::read(&bytes[*offset..*offset + BASEPACKET_SIZE])?;

        if bytes.len() < *offset + header.packet_size as usize {
            return Err(PacketError::SizeMismatch {
                declared: header.packet_size as usize,
                available: bytes.len() - *offset,
            });
        }

        let packet_end = *offset + header.packet_size as usize;
        let data = bytes[*offset + BASEPACKET_SIZE..packet_end].to_vec();
        *offset = packet_end;
        Ok(BasePacket { header, data })
    }

    /// Non-destructive header peek used by streaming readers that still need
    /// to decide whether the whole packet is on the wire.
    pub fn peek_header(bytes: &[u8]) -> Result<BasePacketHeader, PacketError> {
        BasePacketHeader::read(bytes)
    }

    /// Try to build a packet from `buffer` starting at `offset`. Returns
    /// `None` if the buffer doesn't have the full packet yet. `offset` is
    /// advanced when a packet is produced, matching the C# semantics.
    pub fn try_from_buffer(
        buffer: &[u8],
        offset: &mut usize,
        bytes_read: usize,
    ) -> Option<BasePacket> {
        if bytes_read <= *offset || buffer.len() < *offset + 2 {
            return None;
        }
        let size = u16::from_le_bytes([buffer[*offset], buffer[*offset + 1]]) as usize;
        if bytes_read < *offset + size || buffer.len() < *offset + size {
            return None;
        }
        BasePacket::from_buffer(buffer, offset).ok()
    }

    pub fn header_bytes(&self) -> [u8; BASEPACKET_SIZE] {
        let mut out = [0u8; BASEPACKET_SIZE];
        self.header.write(&mut out);
        out
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; self.header.packet_size as usize];
        out[..BASEPACKET_SIZE].copy_from_slice(&self.header_bytes());
        out[BASEPACKET_SIZE..BASEPACKET_SIZE + self.data.len()].copy_from_slice(&self.data);
        out
    }

    pub fn get_subpackets(&self) -> Result<Vec<SubPacket>, PacketError> {
        let mut subs = Vec::with_capacity(self.header.num_subpackets as usize);
        let mut offset = 0;
        while offset < self.data.len() {
            subs.push(SubPacket::parse(&self.data, &mut offset)?);
        }
        Ok(subs)
    }

    /// Replace every 4-byte window matching one of the sniffed default actor
    /// IDs with `actor_id`. Ported verbatim from the C# helper, which is used
    /// to re-target captured packets for replay.
    pub fn replace_actor_id(&mut self, actor_id: u32) {
        const ORIGINAL_IDS: &[u32] = &[
            0x029B2941, 0x02977DC7, 0x0297D2C8, 0x0230d573, 0x23317df, 0x23344a3, 0x1730bdb, 0x6c,
        ];
        self.replace_matching_ids(ORIGINAL_IDS, actor_id);
    }

    pub fn replace_actor_id_from(&mut self, from_actor_id: u32, actor_id: u32) {
        self.replace_matching_ids(&[from_actor_id], actor_id);
    }

    fn replace_matching_ids(&mut self, needles: &[u32], replacement: u32) {
        let data_len = self.data.len();
        if data_len < 4 {
            return;
        }
        let mut pos = 0;
        while pos + 4 < data_len {
            let val = u32::from_le_bytes([
                self.data[pos],
                self.data[pos + 1],
                self.data[pos + 2],
                self.data[pos + 3],
            ]);
            if needles.contains(&val) {
                self.data[pos..pos + 4].copy_from_slice(&replacement.to_le_bytes());
            }
            pos += 4;
        }
    }

    pub fn create_from_subpackets(
        subpackets: &[SubPacket],
        is_authed: bool,
        is_compressed: bool,
    ) -> Result<BasePacket, PacketError> {
        let body_size: usize = subpackets.iter().map(|s| s.header.subpacket_size as usize).sum();

        let mut data = vec![0u8; body_size];
        let mut offset = 0;
        for s in subpackets {
            let bytes = s.to_bytes();
            data[offset..offset + bytes.len()].copy_from_slice(&bytes);
            offset += bytes.len();
        }

        let (data, packet_size) = if is_compressed {
            let compressed = compress_data(&data)?;
            let size = (BASEPACKET_SIZE + compressed.len()) as u16;
            (compressed, size)
        } else {
            let size = (BASEPACKET_SIZE + body_size) as u16;
            (data, size)
        };

        let header = BasePacketHeader {
            is_authenticated: is_authed as u8,
            is_compressed: is_compressed as u8,
            connection_type: 0,
            packet_size,
            num_subpackets: subpackets.len() as u16,
            timestamp: utils::millis_unix_timestamp(),
        };

        Ok(BasePacket { header, data })
    }

    pub fn create_from_subpacket(
        subpacket: &SubPacket,
        is_authed: bool,
        is_compressed: bool,
    ) -> Result<BasePacket, PacketError> {
        BasePacket::create_from_subpackets(std::slice::from_ref(subpacket), is_authed, is_compressed)
    }

    pub fn create_from_data(
        data: &[u8],
        is_authed: bool,
        is_compressed: bool,
    ) -> Result<BasePacket, PacketError> {
        let data = if is_compressed { compress_data(data)? } else { data.to_vec() };

        let header = BasePacketHeader {
            is_authenticated: is_authed as u8,
            is_compressed: is_compressed as u8,
            connection_type: 0,
            packet_size: (BASEPACKET_SIZE + data.len()) as u16,
            num_subpackets: 1,
            timestamp: utils::millis_unix_timestamp(),
        };

        Ok(BasePacket { header, data })
    }

    /// Encrypt every subpacket body in place. Walks the packet using the
    /// declared subpacket sizes, matching the C# implementation exactly.
    pub fn encrypt(&mut self, blowfish: &Blowfish) -> Result<(), PacketError> {
        let mut offset = 0;
        while offset < self.data.len() {
            if self.data.len() < offset + SUBPACKET_SIZE {
                return Err(PacketError::TooSmall {
                    needed: offset + SUBPACKET_SIZE,
                    have: self.data.len(),
                });
            }
            let size = u16::from_le_bytes([self.data[offset], self.data[offset + 1]]) as usize;

            if self.data.len() < offset + size {
                return Err(PacketError::SizeMismatch {
                    declared: size,
                    available: self.data.len() - offset,
                });
            }

            blowfish.encipher(&mut self.data, offset + SUBPACKET_SIZE, size - SUBPACKET_SIZE)?;
            offset += size;
        }
        Ok(())
    }

    pub fn decrypt(&mut self, blowfish: &Blowfish) -> Result<(), PacketError> {
        let mut offset = 0;
        while offset < self.data.len() {
            if self.data.len() < offset + SUBPACKET_SIZE {
                return Err(PacketError::TooSmall {
                    needed: offset + SUBPACKET_SIZE,
                    have: self.data.len(),
                });
            }
            let size = u16::from_le_bytes([self.data[offset], self.data[offset + 1]]) as usize;

            if self.data.len() < offset + size {
                return Err(PacketError::SizeMismatch {
                    declared: size,
                    available: self.data.len() - offset,
                });
            }

            blowfish.decipher(&mut self.data, offset + SUBPACKET_SIZE, size - SUBPACKET_SIZE)?;
            offset += size;
        }
        Ok(())
    }

    pub fn decompress(&mut self) -> Result<(), PacketError> {
        let mut decoder = ZlibDecoder::new(&self.data[..]);
        let mut out = Vec::new();
        decoder.read_to_end(&mut out)?;
        self.data = out;
        self.header.is_compressed = 0;
        self.header.packet_size = (BASEPACKET_SIZE + self.data.len()) as u16;
        Ok(())
    }

    pub fn compress(&self) -> Result<BasePacket, PacketError> {
        let compressed = compress_data(&self.data)?;
        BasePacket::create_from_data(&compressed, self.header.is_authenticated == 1, true)
    }
}

fn compress_data(data: &[u8]) -> Result<Vec<u8>, PacketError> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_roundtrip() {
        let h = BasePacketHeader {
            is_authenticated: 1,
            is_compressed: 0,
            connection_type: 2,
            packet_size: 0x30,
            num_subpackets: 2,
            timestamp: 0x0123456789abcdef,
        };
        let mut buf = [0u8; BASEPACKET_SIZE];
        h.write(&mut buf);
        let parsed = BasePacketHeader::read(&buf).unwrap();
        assert_eq!(parsed.is_authenticated, 1);
        assert_eq!(parsed.packet_size, 0x30);
        assert_eq!(parsed.timestamp, 0x0123456789abcdef);
    }

    #[test]
    fn subpacket_roundtrip_through_base() {
        let sub = SubPacket::new_with_flag(false, 0x02, 0xDEAD, vec![0xAA, 0xBB, 0xCC, 0xDD]);
        let packet = BasePacket::create_from_subpacket(&sub, true, false).unwrap();
        let bytes = packet.to_bytes();

        let parsed = BasePacket::from_bytes(&bytes).unwrap();
        let subs = parsed.get_subpackets().unwrap();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].header.source_id, 0xDEAD);
        assert_eq!(subs[0].data, vec![0xAA, 0xBB, 0xCC, 0xDD]);
    }
}
