//! Incoming packets parsed by the map server.
//!
//! Clients send a fairly small number of packet types; zone servers relay
//! the player's actions via game-message opcodes. Phase 4 covers the
//! handshake + session-control receive shapes — most in-game actions are
//! parsed opportunistically by the processor using raw byte readers.
#![allow(dead_code)]

use std::io::{Cursor, Read};

use anyhow::Result;
use byteorder::{LittleEndian, ReadBytesExt};

/// Session-begin request, identical to the world-server packet.
#[derive(Debug, Clone)]
pub struct SessionBeginRequest {
    pub session_id: u32,
    pub is_login: bool,
}

impl SessionBeginRequest {
    pub fn parse(sub_source_id: u32, data: &[u8]) -> Result<Self> {
        // The data blob is 1 byte: non-zero when the session represents a
        // fresh login (vs a zone change).
        let is_login = data.first().copied().unwrap_or(0) != 0;
        Ok(Self { session_id: sub_source_id, is_login })
    }
}

/// Session-end request from the world server.
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
        let destination_zone_id = c.read_u32::<LittleEndian>()?;
        let destination_spawn_type = c.read_u16::<LittleEndian>().unwrap_or(0) as u8;
        let destination_x = c.read_f32::<LittleEndian>().unwrap_or(0.0);
        let destination_y = c.read_f32::<LittleEndian>().unwrap_or(0.0);
        let destination_z = c.read_f32::<LittleEndian>().unwrap_or(0.0);
        let destination_rot = c.read_f32::<LittleEndian>().unwrap_or(0.0);
        Ok(Self {
            session_id: sub_source_id,
            destination_zone_id,
            destination_spawn_type,
            destination_x,
            destination_y,
            destination_z,
            destination_rot,
        })
    }
}

/// Generic game-message payload the client pushes: `[actor_id LE u32, …]`.
/// Most in-game actions route through this — the opcode lives in the
/// game-message header of the subpacket, not in the body.
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
        Ok(Self { sender_actor_id, body })
    }
}

/// Parsed client chat message.
#[derive(Debug, Clone)]
pub struct IncomingChatMessage {
    pub message_type: u8,
    pub message: String,
}

impl IncomingChatMessage {
    pub fn parse(data: &[u8]) -> Result<Self> {
        // Mirrors Packets/Receive/Subpackets/ChatMessagePacket.cs:
        //   u64 _pad, u32 _pad, u8 type, u8 _, u16 _, name(0x20), message (null-term).
        if data.len() < 0x10 {
            return Ok(Self { message_type: 0, message: String::new() });
        }
        let message_type = data[0x0C];
        let msg_start = 0x10 + 0x20;
        let msg_bytes = &data[msg_start.min(data.len())..];
        let end = msg_bytes.iter().position(|&b| b == 0).unwrap_or(msg_bytes.len());
        let message = String::from_utf8_lossy(&msg_bytes[..end]).into_owned();
        Ok(Self { message_type, message })
    }
}
