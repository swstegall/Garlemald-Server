//! Shared primitives for the Garlemald FFXIV 1.23b server emulator.
//!
//! Ported from Project Meteor's `Common Class Lib` (C#) for Rust 1.95.
//! Wire-format semantics (packet layout, Blowfish key schedule, endianness)
//! are preserved verbatim so this crate can interoperate with the original
//! client without re-sniffing traffic.

pub mod bitfield;
pub mod blowfish;
pub mod error;
pub mod hash_table;
pub mod ini;
pub mod math;
pub mod packet;
pub mod subpacket;
pub mod utils;

pub use blowfish::Blowfish;
pub use error::PacketError;
pub use math::Vector3;
pub use packet::{BasePacket, BasePacketHeader, PACKET_TYPE_CHAT, PACKET_TYPE_ZONE};
pub use subpacket::{GameMessageHeader, SubPacket, SubPacketHeader};
