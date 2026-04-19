//! Shared primitives for the Garlemald FFXIV 1.23b server emulator.
//!
//! Ported from Project Meteor's `Common Class Lib` (C#) for Rust 1.95.
//! Wire-format semantics (packet layout, Blowfish key schedule, endianness)
//! are preserved verbatim so this crate can interoperate with the original
//! client without re-sniffing traffic.

pub mod bitfield;
pub mod blowfish;
pub mod db;
pub mod error;
pub mod hash_table;
pub mod logging;
pub mod luaparam;
pub mod math;
pub mod migrations;
pub mod packet;
pub mod packet_log;
pub mod subpacket;
pub mod utils;

pub use blowfish::Blowfish;
pub use error::PacketError;
pub use luaparam::LuaParam;
pub use math::Vector3;
pub use packet::{
    BasePacket, BasePacketHeader, PACKET_TYPE_CHAT, PACKET_TYPE_ZONE, wrap_subpackets_in_basepacket,
};
pub use subpacket::{GameMessageHeader, SubPacket, SubPacketHeader};
