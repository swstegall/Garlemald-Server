// garlemald-server — Rust port of a FINAL FANTASY XIV v1.23b server emulator (lobby/world/map)
// Copyright (C) 2026  Samuel Stegall
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Group/party/linkshell sync packets. Similar chunked shape to inventory:
//! `GroupMembersBegin` → one or more `GroupMembers{X08,X16,X32,X64}` →
//! `GroupMembersEnd`, plus the optional named/content/sync variants.

use std::io::Cursor;

use byteorder::{LittleEndian, WriteBytesExt};
use common::subpacket::SubPacket;

use super::super::opcodes::*;
use super::{body, write_padded_ascii};

/// One per-member row serialized into the X08/X16/X32/X64 containers.
/// Fields mirror Meteor's `GroupMember` (used by `GroupMembersXnn.buildPacket`),
/// not the party-UI fields the earlier port invented. The client's
/// group-table parser expects a fixed 0x30-byte slot with this layout;
/// writing the HP/class/etc. shape we had before was what turned the
/// empty-retainer-sync packets into a hard Wine crash.
#[derive(Debug, Clone, Default)]
pub struct GroupMember {
    pub actor_id: u32,
    /// C# `localizedName` — signed 32-bit (retainer/linkshell title id).
    pub localized_name: i32,
    /// Opaque 32-bit slot — zero for retainers, class-bit-flags for LS.
    pub unknown2: u32,
    /// `flag1` in C# (leader / officer bit).
    pub flag1: bool,
    pub is_online: bool,
    /// Up to 0x20 bytes of ASCII name.
    pub name: String,
}

/// Fixed member-slot size in Meteor: u32+i32+u32+byte+byte+name[0x20] =
/// 0x2E bytes written, slot is 0x30 with two trailing pad bytes.
const GROUP_MEMBER_SLOT_BYTES: usize = 0x30;

fn encode_group_member_at(data: &mut [u8], slot_offset: usize, m: &GroupMember) {
    let mut c = Cursor::new(&mut data[slot_offset..slot_offset + GROUP_MEMBER_SLOT_BYTES]);
    c.write_u32::<LittleEndian>(m.actor_id).unwrap();
    c.write_i32::<LittleEndian>(m.localized_name).unwrap();
    c.write_u32::<LittleEndian>(m.unknown2).unwrap();
    c.write_u8(if m.flag1 { 1 } else { 0 }).unwrap();
    c.write_u8(if m.is_online { 1 } else { 0 }).unwrap();
    write_padded_ascii(&mut c, &m.name, 0x20);
}

/// 0x017C GroupHeaderPacket — first packet in a group sync.
///
/// Meteor layout (body size 0x78):
///   0x00  locationCode        u64  (zone id)
///   0x08  sequenceId          u64  (timestamp)
///   0x10  const               u64  = 3
///   0x18  group_index         u64
///   0x20  const               u64  = 0
///   0x28  group_index         u64
///   0x30  type_id             u32
///   0x40  localized_name      u32  (linkshell display id, 0 otherwise)
///   0x44  group_name (ASCII)  up to 0x20 bytes
///   0x64  const               u32  = 0x6D
///   0x68  const               u32  = 0x6D
///   0x6C  const               u32  = 0x6D
///   0x70  const               u32  = 0x6D
///   0x74  member_count        u32
pub fn build_group_header(
    source_actor_id: u32,
    location_code: u64,
    sequence_id: u64,
    group_index: u64,
    type_id: u32,
    localized_name: i32,
    group_name: &str,
    member_count: u32,
) -> SubPacket {
    let mut data = body(0x98);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u64::<LittleEndian>(location_code).unwrap();
        c.write_u64::<LittleEndian>(sequence_id).unwrap();
        c.write_u64::<LittleEndian>(3).unwrap();
        c.write_u64::<LittleEndian>(group_index).unwrap();
        c.write_u64::<LittleEndian>(0).unwrap();
        c.write_u64::<LittleEndian>(group_index).unwrap();
        c.write_u32::<LittleEndian>(type_id).unwrap();
    }
    // 0x40 region — C# seeks past the u32 type_id before writing the
    // linkshell display bits.
    {
        let mut c = Cursor::new(&mut data[..]);
        c.set_position(0x40);
        c.write_i32::<LittleEndian>(localized_name).unwrap();
        write_padded_ascii(&mut c, group_name, 0x20);
        c.set_position(0x64);
        c.write_u32::<LittleEndian>(0x6D).unwrap();
        c.write_u32::<LittleEndian>(0x6D).unwrap();
        c.write_u32::<LittleEndian>(0x6D).unwrap();
        c.write_u32::<LittleEndian>(0x6D).unwrap();
        c.write_u32::<LittleEndian>(member_count).unwrap();
    }
    SubPacket::new(OP_GROUP_HEADER, source_actor_id, data)
}

/// 0x017D GroupMembersBeginPacket. Body layout (0x20 bytes):
///   0x00 locationCode  u64
///   0x08 sequenceId    u64
///   0x10 group_index   u64
///   0x18 member_count  u32
pub fn build_group_members_begin(
    source_actor_id: u32,
    location_code: u64,
    sequence_id: u64,
    group_index: u64,
    member_count: u32,
) -> SubPacket {
    let mut data = body(0x40);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u64::<LittleEndian>(location_code).unwrap();
    c.write_u64::<LittleEndian>(sequence_id).unwrap();
    c.write_u64::<LittleEndian>(group_index).unwrap();
    c.write_u32::<LittleEndian>(member_count).unwrap();
    SubPacket::new(OP_GROUP_MEMBERS_BEGIN, source_actor_id, data)
}

/// 0x017E GroupMembersEndPacket. Body layout (0x18 bytes):
///   0x00 locationCode  u64
///   0x08 sequenceId    u64
///   0x10 group_index   u64
pub fn build_group_members_end(
    source_actor_id: u32,
    location_code: u64,
    sequence_id: u64,
    group_index: u64,
) -> SubPacket {
    let mut data = body(0x38);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u64::<LittleEndian>(location_code).unwrap();
    c.write_u64::<LittleEndian>(sequence_id).unwrap();
    c.write_u64::<LittleEndian>(group_index).unwrap();
    SubPacket::new(OP_GROUP_MEMBERS_END, source_actor_id, data)
}

/// 0x017F GroupMembersX08Packet — up to 8 members per packet.
///
/// Meteor's `GroupMembersX08Packet.buildPacket` lays the body as:
///   0x00  locationCode            u64
///   0x08  sequenceId              u64
///   0x10  member_slot[8] @ 0x30   (empty slots stay zero-filled)
///   0x190 count                   u32  — number of members written
/// The client indexes the slot array by position (not by walking a
/// sequential stream), so we must respect the fixed 0x30 spacing even
/// for empty slots.
pub fn build_group_members_x08(
    source_actor_id: u32,
    location_code: u64,
    sequence_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
) -> SubPacket {
    build_group_members_n(
        source_actor_id,
        location_code,
        sequence_id,
        members,
        list_offset,
        8,
        OP_GROUP_MEMBERS_X08,
        0x1B8,
    )
}

pub fn build_group_members_x16(
    source_actor_id: u32,
    location_code: u64,
    sequence_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
) -> SubPacket {
    build_group_members_n(
        source_actor_id,
        location_code,
        sequence_id,
        members,
        list_offset,
        16,
        OP_GROUP_MEMBERS_X16,
        0x330,
    )
}

pub fn build_group_members_x32(
    source_actor_id: u32,
    location_code: u64,
    sequence_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
) -> SubPacket {
    build_group_members_n(
        source_actor_id,
        location_code,
        sequence_id,
        members,
        list_offset,
        32,
        OP_GROUP_MEMBERS_X32,
        0x630,
    )
}

pub fn build_group_members_x64(
    source_actor_id: u32,
    location_code: u64,
    sequence_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
) -> SubPacket {
    build_group_members_n(
        source_actor_id,
        location_code,
        sequence_id,
        members,
        list_offset,
        64,
        OP_GROUP_MEMBERS_X64,
        0xC30,
    )
}

fn build_group_members_n(
    source_actor_id: u32,
    location_code: u64,
    sequence_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
    cap: usize,
    opcode: u16,
    packet_size: usize,
) -> SubPacket {
    let mut data = body(packet_size);
    let max = members.len().saturating_sub(*list_offset).min(cap);
    // Header prefix.
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u64::<LittleEndian>(location_code).unwrap();
        c.write_u64::<LittleEndian>(sequence_id).unwrap();
    }
    // Member slots — fixed 0x30 spacing starting at body offset 0x10.
    for i in 0..max {
        encode_group_member_at(
            &mut data,
            0x10 + (GROUP_MEMBER_SLOT_BYTES * i),
            &members[*list_offset + i],
        );
    }
    // Count — Meteor writes it at body offset 0x10 + 0x30*cap (past the
    // last slot). For X08 that's 0x190; X16/X32/X64 scale.
    let count_offset = 0x10 + (GROUP_MEMBER_SLOT_BYTES * cap);
    if count_offset + 4 <= data.len() {
        data[count_offset..count_offset + 4].copy_from_slice(&(max as u32).to_le_bytes());
    }
    *list_offset += max;
    SubPacket::new(opcode, source_actor_id, data)
}

/// Content (instance/duty) member variants — shape identical, different opcodes.
pub fn build_content_members_x08(
    source_actor_id: u32,
    location_code: u64,
    sequence_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
) -> SubPacket {
    build_group_members_n(
        source_actor_id,
        location_code,
        sequence_id,
        members,
        list_offset,
        8,
        OP_CONTENT_MEMBERS_X08,
        0x1B8,
    )
}
pub fn build_content_members_x16(
    source_actor_id: u32,
    location_code: u64,
    sequence_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
) -> SubPacket {
    build_group_members_n(
        source_actor_id,
        location_code,
        sequence_id,
        members,
        list_offset,
        16,
        OP_CONTENT_MEMBERS_X16,
        0xF0,
    )
}
pub fn build_content_members_x32(
    source_actor_id: u32,
    location_code: u64,
    sequence_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
) -> SubPacket {
    build_group_members_n(
        source_actor_id,
        location_code,
        sequence_id,
        members,
        list_offset,
        32,
        OP_CONTENT_MEMBERS_X32,
        0x1B0,
    )
}
pub fn build_content_members_x64(
    source_actor_id: u32,
    location_code: u64,
    sequence_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
) -> SubPacket {
    build_group_members_n(
        source_actor_id,
        location_code,
        sequence_id,
        members,
        list_offset,
        64,
        OP_CONTENT_MEMBERS_X64,
        0x330,
    )
}

/// 0x0188 CreateNamedGroup — announce a new group by name.
pub fn build_create_named_group(
    source_actor_id: u32,
    group_id: u64,
    group_type: u16,
    name: &str,
    master: &str,
) -> SubPacket {
    let mut data = body(0x60);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u64::<LittleEndian>(group_id).unwrap();
    c.write_u16::<LittleEndian>(group_type).unwrap();
    write_padded_ascii(&mut c, name, 0x20);
    write_padded_ascii(&mut c, master, 0x20);
    SubPacket::new(OP_CREATE_NAMED_GROUP, source_actor_id, data)
}

/// 0x0189 CreateNamedGroupMultiple — batch LS list.
pub fn build_create_named_group_multiple(
    source_actor_id: u32,
    linkshells: &[(u64, String, u16)],
) -> SubPacket {
    let mut data = body(0x228);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u16::<LittleEndian>(linkshells.len() as u16)
        .unwrap();
    c.write_u16::<LittleEndian>(0).unwrap();
    for (gid, name, gtype) in linkshells {
        c.write_u64::<LittleEndian>(*gid).unwrap();
        c.write_u16::<LittleEndian>(*gtype).unwrap();
        write_padded_ascii(&mut c, name, 0x20);
    }
    SubPacket::new(OP_CREATE_NAMED_GROUP_MULTIPLE, source_actor_id, data)
}

/// 0x0143 DeleteGroupPacket.
pub fn build_delete_group(source_actor_id: u32, group_id: u64) -> SubPacket {
    let mut data = body(0x40);
    data[..8].copy_from_slice(&group_id.to_le_bytes());
    SubPacket::new(OP_DELETE_GROUP, source_actor_id, data)
}

/// 0x017A SynchGroupWorkValuesPacket — raw work-value blob (Phase-4 placeholder).
pub fn build_synch_group_work_values(
    source_actor_id: u32,
    group_id: u64,
    work_blob: &[u8],
) -> SubPacket {
    let mut data = body(0xB0);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u64::<LittleEndian>(group_id).unwrap();
    let n = work_blob.len().min(0xA0);
    for &b in work_blob.iter().take(n) {
        c.write_u8(b).unwrap();
    }
    SubPacket::new(OP_SYNCH_GROUP_WORK_VALUES, source_actor_id, data)
}

// ---------------------------------------------------------------------------
// 0x018D PartyMapMarkerUpdate — party-member icon overlay on the world map.
//
// Wire format (per wiki + retail bytes from `ffxiv_traces/chat_say.pcapng`
// record #1 of opcode 0x018D, decoded byte-by-byte):
//
//   body size = 0x298 (664 bytes)
//   0x00  u64 player_group_id          — solo retail uses 0x80000000_0077E9AC,
//                                         party uses Meteor's
//                                         `((leader_id as u64) << 32) | 0xB36F92`
//   0x08  u32 group_type               — 10001 (0x2711) = PlayerPartyGroup
//   0x0C  u32 zero/padding
//   0x10  marker[16] @ 40 bytes each   = 640 bytes
//   0x290 u32 num_entries              — count of populated marker slots
//   0x294 u32 zero/padding
//
// Per-marker layout (40 bytes), at marker-relative offsets:
//   0x00  u32 player_id (actor id)
//   0x04  u32 zero/padding
//   0x08  u32 unknown                  — wiki: "each player has a different
//                                         value" — likely a per-character
//                                         hash or session salt; client
//                                         appears not to validate
//   0x0C  u64 zero/padding
//   0x14  f32 x
//   0x18  f32 y
//   0x1C  f32 z
//   0x20  f32 orientation
//   0x24  u32 zero/padding
//
// Retail emits this on a regular interval (every position broadcast in
// our captures); see the wiki note: "Sent from the server at a regular
// interval, likely due to client not being programmed to send a request
// for such data when the player opens the map."
//
// Project Meteor never implemented this; with the retail-pcap audit
// (2026-05-02), garlemald becomes the first 1.x port to emit it.

/// One marker slot inside a 0x018D packet. `unknown` is opaque per the
/// wiki — pass 0 for a clean default; production code may want to seed
/// it from a per-character salt for full retail conformance.
#[derive(Debug, Clone, Copy, Default)]
pub struct PartyMapMarker {
    pub player_id: u32,
    pub unknown: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub orientation: f32,
}

/// 0x10001 / 10001 — `Group::PlayerPartyGroup` per Meteor
/// `World Server/DataObjects/Group/Group.cs`.
pub const PARTY_MAP_MARKER_GROUP_TYPE_PLAYER_PARTY: u32 = 10001;

/// playerGroupID retail uses for an unparty'd player. Magic constant
/// captured from `ffxiv_traces/chat_say.pcapng`; the high 0x80000000
/// bit looks like a "synthetic / solo group" flag, but we don't have
/// enough datapoints to confirm. Use this verbatim for solo emissions
/// until we capture another player's solo packet.
pub const PARTY_MAP_MARKER_SOLO_GROUP_ID: u64 = 0x8000_0000_0077_E9AC;

/// 0x018D PartyMapMarkerUpdate. Up to 16 markers per packet — extra
/// markers in `markers` are silently truncated.
pub fn build_party_map_marker_update(
    actor_id: u32,
    player_group_id: u64,
    group_type: u32,
    markers: &[PartyMapMarker],
) -> SubPacket {
    const PACKET_SIZE: usize = 0x2B8;
    const MARKER_SIZE: usize = 0x28;
    const MARKERS_OFFSET: usize = 0x10;
    const NUM_ENTRIES_OFFSET: usize = 0x290;
    const MAX_MARKERS: usize = 16;

    let mut data = body(PACKET_SIZE);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u64::<LittleEndian>(player_group_id).unwrap();
        c.write_u32::<LittleEndian>(group_type).unwrap();
        // 4-byte pad at 0x0C..0x10 already zero from `body()`.
    }
    let n = markers.len().min(MAX_MARKERS);
    for (i, m) in markers.iter().take(MAX_MARKERS).enumerate() {
        let off = MARKERS_OFFSET + i * MARKER_SIZE;
        let mut c = Cursor::new(&mut data[off..off + MARKER_SIZE]);
        c.write_u32::<LittleEndian>(m.player_id).unwrap();
        c.write_u32::<LittleEndian>(0).unwrap(); // 0x04 pad
        c.write_u32::<LittleEndian>(m.unknown).unwrap();
        c.write_u64::<LittleEndian>(0).unwrap(); // 0x0C..0x14 pad
        c.write_f32::<LittleEndian>(m.x).unwrap();
        c.write_f32::<LittleEndian>(m.y).unwrap();
        c.write_f32::<LittleEndian>(m.z).unwrap();
        c.write_f32::<LittleEndian>(m.orientation).unwrap();
        c.write_u32::<LittleEndian>(0).unwrap(); // 0x24 pad
    }
    data[NUM_ENTRIES_OFFSET..NUM_ENTRIES_OFFSET + 4]
        .copy_from_slice(&(n as u32).to_le_bytes());
    SubPacket::new(OP_PARTY_MAP_MARKER_UPDATE, actor_id, data)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reproduce the body bytes captured from
    /// `ffxiv_traces/chat_say.pcapng` record #1 of opcode 0x018D — solo
    /// player at `(1822.97, 149.47, 1728.025)`, orientation -2.354 rad,
    /// actor id 0x029B2941, with the captured solo group id and the
    /// per-marker `unknown` field 0x00C17909.
    #[test]
    fn party_map_marker_matches_retail_capture() {
        let marker = PartyMapMarker {
            player_id: 0x029B_2941,
            unknown: 0x00C1_7909,
            x: f32::from_le_bytes([0x80, 0xF3, 0xE3, 0x44]), // 1822.9688
            y: f32::from_le_bytes([0xFA, 0x78, 0x15, 0x43]), // 149.47256
            z: f32::from_le_bytes([0xCB, 0x00, 0xD8, 0x44]), // 1728.0247
            orientation: f32::from_le_bytes([0x17, 0xA8, 0x16, 0xC0]), // -2.3540878
        };
        let pkt = build_party_map_marker_update(
            0x029B_2941,
            PARTY_MAP_MARKER_SOLO_GROUP_ID,
            PARTY_MAP_MARKER_GROUP_TYPE_PLAYER_PARTY,
            &[marker],
        );
        let body = &pkt.data;
        assert_eq!(body.len(), 0x298);

        // Header: playerGroupID + groupType + pad.
        assert_eq!(
            &body[0x00..0x10],
            &[0xAC, 0xE9, 0x77, 0x00, 0x00, 0x00, 0x00, 0x80,
              0x11, 0x27, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        );

        // Marker 0 — full 40-byte slot.
        assert_eq!(
            &body[0x10..0x38],
            &[
                0x41, 0x29, 0x9B, 0x02, 0x00, 0x00, 0x00, 0x00, // playerID + pad
                0x09, 0x79, 0xC1, 0x00, 0x00, 0x00, 0x00, 0x00, // unknown + pad
                0x00, 0x00, 0x00, 0x00, 0x80, 0xF3, 0xE3, 0x44, // pad + X
                0xFA, 0x78, 0x15, 0x43, 0xCB, 0x00, 0xD8, 0x44, // Y + Z
                0x17, 0xA8, 0x16, 0xC0, 0x00, 0x00, 0x00, 0x00, // O + trailing pad
            ],
        );

        // Empty marker slots (15 of them, 600 bytes) must be all zero.
        assert!(body[0x38..0x290].iter().all(|b| *b == 0));

        // numEntries u32 then 4 bytes trailing pad.
        assert_eq!(&body[0x290..0x298], &[0x01, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn party_map_marker_truncates_above_16() {
        let m = PartyMapMarker {
            player_id: 1,
            ..Default::default()
        };
        let pkt = build_party_map_marker_update(0x029B_2941, 0, 10001, &vec![m; 20]);
        // numEntries clamps to 16 even when the caller hands in more.
        let n = u32::from_le_bytes(pkt.data[0x290..0x294].try_into().unwrap());
        assert_eq!(n, 16);
    }

    #[test]
    fn party_map_marker_empty_yields_zero_count() {
        let pkt = build_party_map_marker_update(0x029B_2941, 0, 10001, &[]);
        assert_eq!(pkt.data.len(), 0x298);
        let n = u32::from_le_bytes(pkt.data[0x290..0x294].try_into().unwrap());
        assert_eq!(n, 0);
        // Marker block stays all zero.
        assert!(pkt.data[0x10..0x290].iter().all(|b| *b == 0));
    }
}
