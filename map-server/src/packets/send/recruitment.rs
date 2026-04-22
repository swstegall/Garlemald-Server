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

//! Party-finder / recruitment packets.

use std::io::Cursor;

use byteorder::{LittleEndian, WriteBytesExt};
use common::subpacket::SubPacket;

use super::super::opcodes::*;
use super::{body, write_padded_ascii};

#[derive(Debug, Clone, Default)]
pub struct RecruitmentDetails {
    pub recruiter_id: u64,
    pub purpose: u16,
    pub location: u16,
    pub min_level: u8,
    pub max_level: u8,
    pub description: String,
    pub recruiter_name: String,
}

/// 0x01C3 StartRecruitingResponse.
pub fn build_start_recruiting_response(actor_id: u32, success: bool) -> SubPacket {
    let mut data = body(0x28);
    data[0] = success as u8;
    SubPacket::new(OP_START_RECRUITING_RESPONSE, actor_id, data)
}

/// 0x01C4 EndRecruitment.
pub fn build_end_recruitment(actor_id: u32) -> SubPacket {
    SubPacket::new(OP_END_RECRUITMENT, actor_id, body(0x28))
}

/// 0x01C5 RecruiterState.
pub fn build_recruiter_state(
    actor_id: u32,
    is_recruiting: bool,
    is_recruiter: bool,
    recruitment_id: i64,
) -> SubPacket {
    let mut data = body(0x38);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u8(is_recruiting as u8).unwrap();
    c.write_u8(is_recruiter as u8).unwrap();
    c.write_u16::<LittleEndian>(0).unwrap();
    c.write_i64::<LittleEndian>(recruitment_id).unwrap();
    SubPacket::new(OP_RECRUITER_STATE, actor_id, data)
}

/// 0x01C8 CurrentRecruitmentDetails.
pub fn build_current_recruitment_details(actor_id: u32, details: &RecruitmentDetails) -> SubPacket {
    let mut data = body(0x218);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u64::<LittleEndian>(details.recruiter_id).unwrap();
    c.write_u16::<LittleEndian>(details.purpose).unwrap();
    c.write_u16::<LittleEndian>(details.location).unwrap();
    c.write_u8(details.min_level).unwrap();
    c.write_u8(details.max_level).unwrap();
    c.write_u16::<LittleEndian>(0).unwrap();
    write_padded_ascii(&mut c, &details.recruiter_name, 0x20);
    write_padded_ascii(&mut c, &details.description, 0x1A0);
    SubPacket::new(OP_CURRENT_RECRUITMENT_DETAILS, actor_id, data)
}
