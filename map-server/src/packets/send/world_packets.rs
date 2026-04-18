//! Map-server → world-server request packets (opcodes 0x1000-0x1032). These
//! ride the same game-message channel as ordinary client frames, but the
//! peer is the world server rather than a game client.
//!
//! Ports `Map Server/Packets/WorldPackets/Send/*` (top-level session control)
//! + `Send/Group/*` (party + linkshell RPCs).

use std::io::{Cursor, Seek, SeekFrom};

use byteorder::{LittleEndian, WriteBytesExt};
use common::subpacket::SubPacket;

use super::super::opcodes::*;
use super::{body, write_padded_ascii};

// ---------------------------------------------------------------------------
// Session control
// ---------------------------------------------------------------------------

/// `SessionBeginConfirmPacket` (0x1000) — ack a `SessionBegin` from world.
pub fn build_session_begin_confirm(session_id: u32, error_code: u16) -> SubPacket {
    let mut data = body(0x28);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(session_id).unwrap();
    c.write_u16::<LittleEndian>(error_code).unwrap();
    SubPacket::new(OP_SESSION_BEGIN, session_id, data)
}

/// `SessionEndConfirmPacket` (0x1001) — ack a `SessionEnd` with the zone id
/// we are handing the player off to.
pub fn build_session_end_confirm(
    session_id: u32,
    destination_zone: u32,
    error_code: u16,
) -> SubPacket {
    let mut data = body(0x30);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(session_id).unwrap();
    c.write_u16::<LittleEndian>(error_code).unwrap();
    c.write_u32::<LittleEndian>(destination_zone).unwrap();
    SubPacket::new(OP_SESSION_END, session_id, data)
}

/// `WorldRequestZoneChangePacket` (0x1002) — map server asks world to hand
/// a session off to another zone (and possibly another map server).
pub fn build_world_request_zone_change(
    session_id: u32,
    destination_zone_id: u32,
    spawn_type: u8,
    spawn_x: f32,
    spawn_y: f32,
    spawn_z: f32,
    spawn_rotation: f32,
) -> SubPacket {
    let mut data = body(0x48);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(session_id).unwrap();
    c.write_u32::<LittleEndian>(destination_zone_id).unwrap();
    c.write_u16::<LittleEndian>(spawn_type as u16).unwrap();
    c.write_f32::<LittleEndian>(spawn_x).unwrap();
    c.write_f32::<LittleEndian>(spawn_y).unwrap();
    c.write_f32::<LittleEndian>(spawn_z).unwrap();
    c.write_f32::<LittleEndian>(spawn_rotation).unwrap();
    SubPacket::new(OP_WORLD_ZONE_CHANGE_REQUEST, session_id, data)
}

// ---------------------------------------------------------------------------
// Party RPCs
// ---------------------------------------------------------------------------

/// `PartyModifyPacket` (0x1020) — by-name overload.
pub fn build_party_modify_by_name(session_id: u32, command: u16, name: &str) -> SubPacket {
    let mut data = body(0x48);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u16::<LittleEndian>(command).unwrap();
        write_padded_ascii(&mut c, name, 0x20);
    }
    SubPacket::new(OP_WORLD_PARTY_MODIFY, session_id, data)
}

/// `PartyModifyPacket` (0x1020) — by-actor-id overload. C# bumps the command
/// by 2 before writing it on this path.
pub fn build_party_modify_by_actor(session_id: u32, command: u16, actor_id: u32) -> SubPacket {
    let mut data = body(0x48);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u16::<LittleEndian>(command + 2).unwrap();
        c.write_u32::<LittleEndian>(actor_id).unwrap();
    }
    SubPacket::new(OP_WORLD_PARTY_MODIFY, session_id, data)
}

/// `PartyLeavePacket` (0x1021).
pub fn build_party_leave(session_id: u32, is_disband: bool) -> SubPacket {
    let mut data = body(0x28);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u16::<LittleEndian>(if is_disband { 1 } else { 0 })
            .unwrap();
    }
    SubPacket::new(OP_WORLD_PARTY_LEAVE, session_id, data)
}

/// `PartyInvitePacket` (0x1022) — by-name overload.
pub fn build_party_invite_by_name(session_id: u32, name: &str) -> SubPacket {
    let mut data = body(0x48);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u16::<LittleEndian>(0).unwrap();
        write_padded_ascii(&mut c, name, 0x20);
    }
    SubPacket::new(OP_WORLD_PARTY_INVITE, session_id, data)
}

/// `PartyInvitePacket` (0x1022) — by-actor-id overload.
pub fn build_party_invite_by_actor(session_id: u32, actor_id: u32) -> SubPacket {
    let mut data = body(0x48);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u16::<LittleEndian>(1).unwrap();
        c.write_u32::<LittleEndian>(actor_id).unwrap();
    }
    SubPacket::new(OP_WORLD_PARTY_INVITE, session_id, data)
}

/// `GroupInviteResultPacket` (0x1023).
pub fn build_group_invite_result(session_id: u32, group_type: u32, result: u32) -> SubPacket {
    let mut data = body(0x28);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u32::<LittleEndian>(group_type).unwrap();
        c.write_u32::<LittleEndian>(result).unwrap();
    }
    SubPacket::new(OP_WORLD_GROUP_INVITE_RESULT, session_id, data)
}

// ---------------------------------------------------------------------------
// Linkshell RPCs
// ---------------------------------------------------------------------------

/// `CreateLinkshellPacket` (0x1025). C# writes the 32-byte name from offset 0,
/// then seeks to 0x20 and writes `(crest u16, master u32)`.
pub fn build_create_linkshell(session_id: u32, name: &str, crest: u16, master: u32) -> SubPacket {
    let mut data = body(0x48);
    {
        let mut c = Cursor::new(&mut data[..]);
        write_padded_ascii(&mut c, name, 0x20);
        c.seek(SeekFrom::Start(0x20)).unwrap();
        c.write_u16::<LittleEndian>(crest).unwrap();
        c.write_u32::<LittleEndian>(master).unwrap();
    }
    SubPacket::new(OP_WORLD_CREATE_LINKSHELL, session_id, data)
}

/// `ModifyLinkshellPacket` (0x1026). `change_arg` selects which field is
/// being changed: 0 = rename, 1 = crest, 2 = master.
///
/// Matches the C# layout byte-for-byte: names are written *truncated* (up
/// to 0x20 bytes each) rather than zero-padded — a 0x40-byte body cannot
/// fit two padded 0x20 names plus the u16 selector.
pub fn build_modify_linkshell(
    session_id: u32,
    change_arg: u16,
    name: &str,
    new_name: &str,
    crest: u16,
    master: u32,
) -> SubPacket {
    use std::io::Write as _;
    let mut data = body(0x60);
    {
        let mut c = Cursor::new(&mut data[..]);
        let name_bytes = name.as_bytes();
        let n = name_bytes.len().min(0x20);
        c.write_all(&name_bytes[..n]).unwrap();
        c.write_u16::<LittleEndian>(change_arg).unwrap();
        match change_arg {
            0 => {
                let nn = new_name.as_bytes();
                let m = nn.len().min(0x20);
                c.write_all(&nn[..m]).unwrap();
            }
            1 => c.write_u16::<LittleEndian>(crest).unwrap(),
            2 => c.write_u32::<LittleEndian>(master).unwrap(),
            _ => {}
        }
    }
    SubPacket::new(OP_WORLD_MODIFY_LINKSHELL, session_id, data)
}

/// `DeleteLinkshellPacket` (0x1027).
pub fn build_delete_linkshell(session_id: u32, name: &str) -> SubPacket {
    let mut data = body(0x40);
    {
        let mut c = Cursor::new(&mut data[..]);
        write_padded_ascii(&mut c, name, 0x20);
    }
    SubPacket::new(OP_WORLD_DELETE_LINKSHELL, session_id, data)
}

/// `LinkshellChangePacket` (0x1028) — player switches active linkshell.
pub fn build_linkshell_change(session_id: u32, ls_name: &str) -> SubPacket {
    let mut data = body(0x48);
    {
        let mut c = Cursor::new(&mut data[..]);
        write_padded_ascii(&mut c, ls_name, 0x20);
    }
    SubPacket::new(OP_WORLD_LINKSHELL_CHANGE, session_id, data)
}

/// `LinkshellInvitePacket` (0x1029).
pub fn build_linkshell_invite(session_id: u32, actor_id: u32, linkshell_name: &str) -> SubPacket {
    let mut data = body(0x48);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u32::<LittleEndian>(actor_id).unwrap();
        write_padded_ascii(&mut c, linkshell_name, 0x20);
    }
    SubPacket::new(OP_WORLD_LINKSHELL_INVITE, session_id, data)
}

/// `LinkshellInviteCancelPacket` (0x1030) — empty body.
pub fn build_linkshell_invite_cancel(session_id: u32) -> SubPacket {
    let data = body(0x28);
    SubPacket::new(OP_WORLD_LINKSHELL_INVITE_CANCEL, session_id, data)
}

/// `LinkshellLeavePacket` (0x1031). When `is_kicked` is true the kicked
/// member's name is written right after the flag; the linkshell name lands
/// at offset 0x22 either way.
pub fn build_linkshell_leave(
    session_id: u32,
    ls_name: &str,
    kicked_name: Option<&str>,
    is_kicked: bool,
) -> SubPacket {
    let mut data = body(0x68);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u16::<LittleEndian>(if is_kicked { 1 } else { 0 })
            .unwrap();
        if is_kicked && let Some(k) = kicked_name {
            write_padded_ascii(&mut c, k, 0x20);
        }
        c.seek(SeekFrom::Start(0x22)).unwrap();
        write_padded_ascii(&mut c, ls_name, 0x20);
    }
    SubPacket::new(OP_WORLD_LINKSHELL_LEAVE, session_id, data)
}

/// `LinkshellRankChangePacket` (0x1032). Member name at 0, ls name at 0x20,
/// rank byte at 0x40.
pub fn build_linkshell_rank_change(
    session_id: u32,
    name: &str,
    ls_name: &str,
    rank: u8,
) -> SubPacket {
    let mut data = body(0x68);
    {
        let mut c = Cursor::new(&mut data[..]);
        write_padded_ascii(&mut c, name, 0x20);
        c.seek(SeekFrom::Start(0x20)).unwrap();
        write_padded_ascii(&mut c, ls_name, 0x20);
        c.seek(SeekFrom::Start(0x40)).unwrap();
        c.write_u8(rank).unwrap();
    }
    SubPacket::new(OP_WORLD_LINKSHELL_RANK_CHANGE, session_id, data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game_message_body(p: &SubPacket) -> &[u8] {
        &p.data
    }

    #[test]
    fn session_begin_confirm_has_session_id_and_error() {
        let p = build_session_begin_confirm(0xDEAD_BEEF, 0x0042);
        let b = game_message_body(&p);
        assert_eq!(&b[0..4], &0xDEAD_BEEFu32.to_le_bytes());
        assert_eq!(&b[4..6], &0x0042u16.to_le_bytes());
    }

    #[test]
    fn session_end_confirm_carries_destination_zone() {
        let p = build_session_end_confirm(1, 230, 0);
        let b = game_message_body(&p);
        assert_eq!(&b[0..4], &1u32.to_le_bytes());
        assert_eq!(&b[6..10], &230u32.to_le_bytes());
    }

    #[test]
    fn zone_change_request_writes_position() {
        let p = build_world_request_zone_change(1, 133, 0x2A, 10.0, 20.0, 30.0, 1.5);
        let b = game_message_body(&p);
        assert_eq!(&b[0..4], &1u32.to_le_bytes());
        assert_eq!(&b[4..8], &133u32.to_le_bytes());
        assert_eq!(&b[8..10], &0x002Au16.to_le_bytes());
        assert_eq!(&b[10..14], &10.0f32.to_le_bytes());
    }

    #[test]
    fn party_modify_by_actor_bumps_command() {
        let p = build_party_modify_by_actor(7, 10, 0xA000_0001);
        let b = game_message_body(&p);
        assert_eq!(&b[0..2], &12u16.to_le_bytes()); // 10 + 2
        assert_eq!(&b[2..6], &0xA000_0001u32.to_le_bytes());
    }

    #[test]
    fn party_invite_by_name_writes_type_0() {
        let p = build_party_invite_by_name(7, "Alice");
        let b = game_message_body(&p);
        assert_eq!(&b[0..2], &0u16.to_le_bytes());
        assert_eq!(&b[2..7], b"Alice");
    }

    #[test]
    fn party_invite_by_actor_writes_type_1() {
        let p = build_party_invite_by_actor(7, 0xC0FFEE);
        let b = game_message_body(&p);
        assert_eq!(&b[0..2], &1u16.to_le_bytes());
        assert_eq!(&b[2..6], &0xC0FFEEu32.to_le_bytes());
    }

    #[test]
    fn create_linkshell_places_crest_at_0x20() {
        let p = build_create_linkshell(5, "FreeCompany", 0xAB, 0xDEADBEEF);
        let b = game_message_body(&p);
        assert_eq!(&b[0..11], b"FreeCompany");
        assert_eq!(&b[0x20..0x22], &0x00ABu16.to_le_bytes());
        assert_eq!(&b[0x22..0x26], &0xDEADBEEFu32.to_le_bytes());
    }

    #[test]
    fn modify_linkshell_dispatches_on_change_arg() {
        // Names are written truncated (no padding), so the selector lands
        // at offset `name.len()` and the payload at `name.len()+2`.
        let rename = build_modify_linkshell(1, 0, "Old", "New", 0, 0);
        assert_eq!(&rename.data[0..3], b"Old");
        assert_eq!(&rename.data[3..5], &0u16.to_le_bytes());
        assert_eq!(&rename.data[5..8], b"New");

        let crest = build_modify_linkshell(1, 1, "X", "", 0x1234, 0);
        assert_eq!(&crest.data[0..1], b"X");
        assert_eq!(&crest.data[1..3], &1u16.to_le_bytes());
        assert_eq!(&crest.data[3..5], &0x1234u16.to_le_bytes());

        let master = build_modify_linkshell(1, 2, "X", "", 0, 0xFEEDFACE);
        assert_eq!(&master.data[3..7], &0xFEEDFACEu32.to_le_bytes());
    }

    #[test]
    fn linkshell_leave_writes_ls_name_at_0x22() {
        let p = build_linkshell_leave(1, "Guild", Some("Bob"), true);
        let b = game_message_body(&p);
        assert_eq!(&b[0..2], &1u16.to_le_bytes());
        assert_eq!(&b[2..5], b"Bob");
        assert_eq!(&b[0x22..0x27], b"Guild");
    }

    #[test]
    fn linkshell_rank_change_layout() {
        let p = build_linkshell_rank_change(1, "Alice", "Guild", 7);
        let b = game_message_body(&p);
        assert_eq!(&b[0..5], b"Alice");
        assert_eq!(&b[0x20..0x25], b"Guild");
        assert_eq!(b[0x40], 7);
    }

    #[test]
    fn linkshell_invite_cancel_is_empty() {
        let p = build_linkshell_invite_cancel(42);
        assert_eq!(p.data.len(), 0x28 - 0x20);
        assert!(p.data.iter().all(|&b| b == 0));
    }
}
