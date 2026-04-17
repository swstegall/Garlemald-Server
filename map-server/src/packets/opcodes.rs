//! Opcode registry. Names match the C# Packets/Send and Packets/Receive
//! hierarchy to simplify cross-referencing.
#![allow(dead_code)]

// Session/handshake (raw client frames — not game messages)
pub const OP_PING: u16 = 0x0007;
pub const OP_PONG: u16 = 0x0008;
pub const OP_HANDSHAKE_RESPONSE: u16 = 0x0002;

// World↔map-server session control (>= 0x1000 subpackets)
pub const OP_SESSION_BEGIN: u16 = 0x1000;
pub const OP_SESSION_END: u16 = 0x1001;
pub const OP_WORLD_ZONE_CHANGE_REQUEST: u16 = 0x1002;

// Actor management (game messages — opcode lives in game-message header)
pub const OP_SET_ACTOR_STATE: u16 = 0x0134;
pub const OP_ADD_ACTOR: u16 = 0x0138;
pub const OP_REMOVE_ACTOR: u16 = 0x0139;
pub const OP_DELETE_ALL_ACTORS: u16 = 0x013A;
pub const OP_ACTOR_INIT: u16 = 0x013B;
pub const OP_SET_ACTOR_NAME: u16 = 0x013C;
pub const OP_SET_ACTOR_POSITION: u16 = 0x013D;
pub const OP_SET_ACTOR_SPEED: u16 = 0x013E;
pub const OP_SET_ACTOR_ICON: u16 = 0x013F;
pub const OP_SET_ACTOR_APPEARANCE: u16 = 0x014A;
pub const OP_SET_ACTOR_IS_ZONING: u16 = 0x0147;
pub const OP_SET_ACTOR_TARGET: u16 = 0x0146;
pub const OP_SET_ACTOR_STATUS: u16 = 0x0148;
pub const OP_SET_ACTOR_PROPERTY: u16 = 0x012F;

pub const OP_MOVE_ACTOR_TO_POSITION: u16 = 0x017C;
pub const OP_PLAY_ANIMATION_ON_ACTOR: u16 = 0x0163;

// Chat / game messages
pub const OP_SEND_MESSAGE: u16 = 0x00CA;
pub const OP_GAME_MESSAGE: u16 = 0x01FD;

// Inventory / item package
pub const OP_INVENTORY_BEGIN_CHANGE: u16 = 0x01F4;
pub const OP_INVENTORY_SET_BEGIN: u16 = 0x01F5;
pub const OP_INVENTORY_ADD_ITEM: u16 = 0x01F6;
pub const OP_INVENTORY_REMOVE_ITEM: u16 = 0x01F7;
pub const OP_INVENTORY_END_CHANGE: u16 = 0x01F8;

// Events
pub const OP_EVENT_START: u16 = 0x012D;
pub const OP_EVENT_END: u16 = 0x012E;
pub const OP_EVENT_RUN_LUA: u16 = 0x0130;

// Group work
pub const OP_GROUP_HEADER: u16 = 0x017D;
pub const OP_GROUP_MEMBER: u16 = 0x017E;
pub const OP_SYNCH_GROUP_WORK: u16 = 0x017F;
