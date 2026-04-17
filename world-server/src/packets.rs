//! World server wire packets. Opcodes preserved from project-meteor-mirror.
//!
//! Convention: `receive.rs` parses client/zone → world bytes; `send.rs` builds
//! server-issued subpackets.

pub mod receive;
pub mod send;
