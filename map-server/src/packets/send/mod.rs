//! Outgoing packet builders, organized by subsystem.
//!
//! Every packet follows the same contract: fixed-capacity byte buffer
//! pre-zeroed to the wire size, bytes written in LE at the offsets the
//! 1.23b client expects, then wrapped in a game-message `SubPacket` via
//! `SubPacket::new(opcode, source, body)`. Raw (non-gamemessage) frames use
//! `SubPacket::new_with_flag(false, …)` instead.
//!
//! Capacity numbers ported from `PACKET_SIZE` constants in the C# source;
//! each builder pre-subtracts the 0x20 header overhead (0x10 subpacket
//! header + 0x10 game-message header) so the body buffer matches exactly.

#![allow(dead_code)]

pub mod actor;
pub mod actor_battle;
pub mod actor_events;
pub mod actor_inventory;
pub mod events;
pub mod groups;
pub mod handshake;
pub mod misc;
pub mod player;
pub mod recruitment;
pub mod search;
pub mod social;
pub mod supportdesk;

use std::io::Write;

use byteorder::WriteBytesExt;

/// Helper: write `s.as_bytes()` truncated to `width`, zero-padded.
pub(crate) fn write_padded_ascii<W: Write>(w: &mut W, s: &str, width: usize) {
    let bytes = s.as_bytes();
    let n = bytes.len().min(width);
    w.write_all(&bytes[..n]).unwrap();
    for _ in n..width {
        w.write_u8(0).unwrap();
    }
}

/// Helper: zero-filled body buffer of the right game-message capacity.
/// C# `new byte[PACKET_SIZE - 0x20]`.
pub(crate) fn body(total_packet_size: usize) -> Vec<u8> {
    vec![0u8; total_packet_size.saturating_sub(0x20)]
}

// Public re-exports so callers can `use packets::send::*`. Some submodules
// have no consumer yet — flag them as allow(unused_imports) until the
// processor and game loop grow their own dispatch call sites.
#[allow(unused_imports)]
pub use actor::*;
#[allow(unused_imports)]
pub use actor_battle::*;
#[allow(unused_imports)]
pub use actor_events::*;
#[allow(unused_imports)]
pub use actor_inventory::*;
#[allow(unused_imports)]
pub use events::*;
#[allow(unused_imports)]
pub use groups::*;
#[allow(unused_imports)]
pub use handshake::*;
#[allow(unused_imports)]
pub use misc::*;
#[allow(unused_imports)]
pub use player::*;
#[allow(unused_imports)]
pub use recruitment::*;
#[allow(unused_imports)]
pub use search::*;
#[allow(unused_imports)]
pub use social::*;
#[allow(unused_imports)]
pub use supportdesk::*;
