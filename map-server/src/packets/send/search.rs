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

//! Market-board, retainer-search, and player-search result packets.

use std::io::Cursor;

use byteorder::{LittleEndian, WriteBytesExt};
use common::subpacket::SubPacket;

use super::super::opcodes::*;
use super::{body, write_padded_ascii};

fn ack(actor_id: u32, opcode: u16, is_success: bool, name: &str, size: usize) -> SubPacket {
    let mut data = body(size);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u8(is_success as u8).unwrap();
    write_padded_ascii(&mut c, name, 0x20);
    SubPacket::new(opcode, actor_id, data)
}

/// 0x01D7 ItemSearchResultsBegin.
pub fn build_item_search_results_begin(actor_id: u32, is_success: bool, name: &str) -> SubPacket {
    ack(
        actor_id,
        OP_ITEM_SEARCH_RESULTS_BEGIN,
        is_success,
        name,
        0x28,
    )
}

/// 0x01D9 ItemSearchResultsEnd.
pub fn build_item_search_results_end(actor_id: u32, is_success: bool, name: &str) -> SubPacket {
    ack(actor_id, OP_ITEM_SEARCH_RESULTS_END, is_success, name, 0x28)
}

/// 0x01E1 ItemSearchClose.
pub fn build_item_search_close(actor_id: u32, is_success: bool, name: &str) -> SubPacket {
    ack(actor_id, OP_ITEM_SEARCH_CLOSE, is_success, name, 0x28)
}

/// A single result row — the C# `ItemSearchResult` has name/price/quality/…
#[derive(Debug, Clone, Default)]
pub struct ItemSearchResult {
    pub unique_id: u64,
    pub item_id: u32,
    pub quantity: u32,
    pub price: u32,
    pub quality: u8,
    pub seller_name: String,
    pub retainer_name: String,
    pub location: String,
}

/// 0x01D8 ItemSearchResultsBody — up to 8 entries per packet.
pub fn build_item_search_results_body(
    actor_id: u32,
    results: &[ItemSearchResult],
    list_offset: &mut usize,
) -> SubPacket {
    let mut data = body(0x228);
    let max = results.len().saturating_sub(*list_offset).min(8);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u8(max as u8).unwrap();
        c.write_u8(0).unwrap();
        c.write_u16::<LittleEndian>(0).unwrap();
        for i in 0..max {
            let r = &results[*list_offset + i];
            c.write_u64::<LittleEndian>(r.unique_id).unwrap();
            c.write_u32::<LittleEndian>(r.item_id).unwrap();
            c.write_u32::<LittleEndian>(r.quantity).unwrap();
            c.write_u32::<LittleEndian>(r.price).unwrap();
            c.write_u8(r.quality).unwrap();
            c.write_u8(0).unwrap();
            c.write_u16::<LittleEndian>(0).unwrap();
            write_padded_ascii(&mut c, &r.seller_name, 0x20);
            write_padded_ascii(&mut c, &r.retainer_name, 0x20);
            write_padded_ascii(&mut c, &r.location, 0x20);
        }
    }
    *list_offset += max;
    SubPacket::new(OP_ITEM_SEARCH_RESULTS_BODY, actor_id, data)
}

/// Retainer-search session management (ack-only packets).
pub fn build_retainer_result_body(actor_id: u32, is_success: bool, name: &str) -> SubPacket {
    ack(actor_id, OP_RETAINER_RESULT_BODY, is_success, name, 0x28)
}

pub fn build_retainer_result_update(actor_id: u32, is_success: bool, name: &str) -> SubPacket {
    ack(actor_id, OP_RETAINER_RESULT_UPDATE, is_success, name, 0x28)
}

pub fn build_retainer_result_end(actor_id: u32, is_success: bool) -> SubPacket {
    let mut data = body(0x38);
    data[0] = is_success as u8;
    SubPacket::new(OP_RETAINER_RESULT_END, actor_id, data)
}

pub fn build_retainer_search_history(actor_id: u32, count: u8, has_ended: bool) -> SubPacket {
    let mut data = body(0x120);
    data[0] = count;
    data[1] = has_ended as u8;
    SubPacket::new(OP_RETAINER_SEARCH_HISTORY, actor_id, data)
}

/// A single player-search hit.
#[derive(Debug, Clone, Default)]
pub struct PlayerSearchResult {
    pub player_id: u64,
    pub name: String,
    pub comment: String,
    pub current_class: u8,
    pub current_level: u8,
    pub zone_id: u32,
    pub online: bool,
}

/// 0x01DF PlayerSearchInfoResult — up to N matches per packet.
pub fn build_player_search_info_result(
    actor_id: u32,
    search_session_id: u32,
    result_code: u8,
    results: &[PlayerSearchResult],
    offset: &mut usize,
) -> SubPacket {
    let mut data = body(0x3C8);
    let max = results.len().saturating_sub(*offset).min(8);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u32::<LittleEndian>(search_session_id).unwrap();
        c.write_u8(result_code).unwrap();
        c.write_u8(max as u8).unwrap();
        c.write_u16::<LittleEndian>(0).unwrap();
        for i in 0..max {
            let r = &results[*offset + i];
            c.write_u64::<LittleEndian>(r.player_id).unwrap();
            write_padded_ascii(&mut c, &r.name, 0x20);
            c.write_u8(r.current_class).unwrap();
            c.write_u8(r.current_level).unwrap();
            c.write_u8(r.online as u8).unwrap();
            c.write_u8(0).unwrap();
            c.write_u32::<LittleEndian>(r.zone_id).unwrap();
        }
    }
    *offset += max;
    SubPacket::new(OP_PLAYER_SEARCH_INFO_RESULT, actor_id, data)
}

/// 0x01E0 PlayerSearchCommentResult.
pub fn build_player_search_comment_result(
    actor_id: u32,
    search_session_id: u32,
    result_code: u8,
    results: &[PlayerSearchResult],
    offset: &mut usize,
) -> SubPacket {
    let mut data = body(0x288);
    let max = results.len().saturating_sub(*offset).min(4);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u32::<LittleEndian>(search_session_id).unwrap();
        c.write_u8(result_code).unwrap();
        c.write_u8(max as u8).unwrap();
        c.write_u16::<LittleEndian>(0).unwrap();
        for i in 0..max {
            let r = &results[*offset + i];
            c.write_u64::<LittleEndian>(r.player_id).unwrap();
            write_padded_ascii(&mut c, &r.name, 0x20);
            write_padded_ascii(&mut c, &r.comment, 0x80);
        }
    }
    *offset += max;
    SubPacket::new(OP_PLAYER_SEARCH_COMMENT_RESULT, actor_id, data)
}
