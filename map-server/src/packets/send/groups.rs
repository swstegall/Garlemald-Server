//! Group/party/linkshell sync packets. Similar chunked shape to inventory:
//! `GroupMembersBegin` → one or more `GroupMembers{X08,X16,X32,X64}` →
//! `GroupMembersEnd`, plus the optional named/content/sync variants.

use std::io::Cursor;

use byteorder::{LittleEndian, WriteBytesExt};
use common::subpacket::SubPacket;

use super::super::opcodes::*;
use super::{body, write_padded_ascii};

/// One per-member row serialized into the X08/X16/X32/X64 containers.
#[derive(Debug, Clone, Default)]
pub struct GroupMember {
    pub actor_id: u32,
    pub ally_actor_id: u32,
    pub name: String,
    pub class_or_job: u8,
    pub level: u8,
    pub hp: u16,
    pub hp_max: u16,
    pub mp: u16,
    pub mp_max: u16,
    pub current_zone_id: u32,
    pub leader_flags: u16,
}

fn encode_group_member(c: &mut Cursor<&mut [u8]>, m: &GroupMember) {
    c.write_u32::<LittleEndian>(m.actor_id).unwrap();
    c.write_u32::<LittleEndian>(m.ally_actor_id).unwrap();
    c.write_u16::<LittleEndian>(m.hp).unwrap();
    c.write_u16::<LittleEndian>(m.hp_max).unwrap();
    c.write_u16::<LittleEndian>(m.mp).unwrap();
    c.write_u16::<LittleEndian>(m.mp_max).unwrap();
    c.write_u32::<LittleEndian>(m.current_zone_id).unwrap();
    c.write_u8(m.class_or_job).unwrap();
    c.write_u8(m.level).unwrap();
    c.write_u16::<LittleEndian>(m.leader_flags).unwrap();
    write_padded_ascii(c, &m.name, 0x20);
}

/// 0x017C GroupHeaderPacket — first packet in a group sync.
pub fn build_group_header(
    source_actor_id: u32,
    group_id: u64,
    group_type: u16,
    member_count: u16,
) -> SubPacket {
    let mut data = body(0x98);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u64::<LittleEndian>(group_id).unwrap();
    c.write_u16::<LittleEndian>(group_type).unwrap();
    c.write_u16::<LittleEndian>(member_count).unwrap();
    SubPacket::new(OP_GROUP_HEADER, source_actor_id, data)
}

/// 0x017D GroupMembersBeginPacket.
pub fn build_group_members_begin(source_actor_id: u32, group_id: u64) -> SubPacket {
    let mut data = body(0x40);
    data[..8].copy_from_slice(&group_id.to_le_bytes());
    SubPacket::new(OP_GROUP_MEMBERS_BEGIN, source_actor_id, data)
}

/// 0x017E GroupMembersEndPacket.
pub fn build_group_members_end(source_actor_id: u32, group_id: u64) -> SubPacket {
    let mut data = body(0x38);
    data[..8].copy_from_slice(&group_id.to_le_bytes());
    SubPacket::new(OP_GROUP_MEMBERS_END, source_actor_id, data)
}

/// 0x017F GroupMembersX08Packet — up to 8 members.
pub fn build_group_members_x08(
    source_actor_id: u32,
    group_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
) -> SubPacket {
    build_group_members_n(source_actor_id, group_id, members, list_offset, 8, OP_GROUP_MEMBERS_X08, 0x1B8)
}

pub fn build_group_members_x16(
    source_actor_id: u32,
    group_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
) -> SubPacket {
    build_group_members_n(source_actor_id, group_id, members, list_offset, 16, OP_GROUP_MEMBERS_X16, 0x330)
}

pub fn build_group_members_x32(
    source_actor_id: u32,
    group_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
) -> SubPacket {
    build_group_members_n(source_actor_id, group_id, members, list_offset, 32, OP_GROUP_MEMBERS_X32, 0x630)
}

pub fn build_group_members_x64(
    source_actor_id: u32,
    group_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
) -> SubPacket {
    build_group_members_n(source_actor_id, group_id, members, list_offset, 64, OP_GROUP_MEMBERS_X64, 0xC30)
}

fn build_group_members_n(
    source_actor_id: u32,
    group_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
    cap: usize,
    opcode: u16,
    packet_size: usize,
) -> SubPacket {
    let mut data = body(packet_size);
    let max = members.len().saturating_sub(*list_offset).min(cap);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u64::<LittleEndian>(group_id).unwrap();
        c.write_u32::<LittleEndian>(max as u32).unwrap();
        c.write_u32::<LittleEndian>(0).unwrap();
        for i in 0..max {
            encode_group_member(&mut c, &members[*list_offset + i]);
        }
    }
    *list_offset += max;
    SubPacket::new(opcode, source_actor_id, data)
}

/// Content (instance/duty) member variants — shape identical, different opcodes.
pub fn build_content_members_x08(
    source_actor_id: u32,
    group_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
) -> SubPacket {
    build_group_members_n(source_actor_id, group_id, members, list_offset, 8, OP_CONTENT_MEMBERS_X08, 0x1B8)
}
pub fn build_content_members_x16(
    source_actor_id: u32,
    group_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
) -> SubPacket {
    build_group_members_n(source_actor_id, group_id, members, list_offset, 16, OP_CONTENT_MEMBERS_X16, 0xF0)
}
pub fn build_content_members_x32(
    source_actor_id: u32,
    group_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
) -> SubPacket {
    build_group_members_n(source_actor_id, group_id, members, list_offset, 32, OP_CONTENT_MEMBERS_X32, 0x1B0)
}
pub fn build_content_members_x64(
    source_actor_id: u32,
    group_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
) -> SubPacket {
    build_group_members_n(source_actor_id, group_id, members, list_offset, 64, OP_CONTENT_MEMBERS_X64, 0x330)
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
    c.write_u16::<LittleEndian>(linkshells.len() as u16).unwrap();
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
pub fn build_synch_group_work_values(source_actor_id: u32, group_id: u64, work_blob: &[u8]) -> SubPacket {
    let mut data = body(0xB0);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u64::<LittleEndian>(group_id).unwrap();
    let n = work_blob.len().min(0xA0);
    for &b in work_blob.iter().take(n) {
        c.write_u8(b).unwrap();
    }
    SubPacket::new(OP_SYNCH_GROUP_WORK_VALUES, source_actor_id, data)
}
