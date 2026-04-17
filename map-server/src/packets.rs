//! Map server wire packets.
//!
//! FFXIV 1.23b has ~200 opcodes documented across the C# source. Phase 4
//! ports the core handshake + session + actor-spawn family; the rest are
//! kept as opcode constants so the processor can log them by name even if
//! no builder exists yet.

pub mod opcodes;
pub mod receive;
pub mod send;
