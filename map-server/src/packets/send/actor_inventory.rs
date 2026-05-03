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

//! Inventory flow packets.
//!
//! The client expects inventory changes to come in a strict bracket:
//! `InventoryBeginChange` → one or more `InventorySetBegin` +
//! `InventoryListX*` / `InventoryRemoveX*` + `InventorySetEnd` → finally
//! `InventoryEndChange`.
//!
//! Each `InventoryListX01/08/16/32/64` variant serializes N items at a time;
//! callers batch through their inventory via a `list_offset` cursor to match
//! the C# `ref int listOffset` pattern.

use std::io::Cursor;
use std::io::Write as _;

use byteorder::{LittleEndian, WriteBytesExt};
use common::subpacket::SubPacket;

use super::super::opcodes::*;
use super::body;
use crate::data::InventoryItem;

/// 0x0116D — "about to push inventory updates; optionally wipe first".
pub fn build_inventory_begin_change(actor_id: u32, clear_item_package: bool) -> SubPacket {
    let mut data = body(0x28);
    data[0] = clear_item_package as u8;
    SubPacket::new(OP_INVENTORY_BEGIN_CHANGE, actor_id, data)
}

/// 0x016E — end of inventory update stream.
pub fn build_inventory_end_change(actor_id: u32) -> SubPacket {
    SubPacket::new(OP_INVENTORY_END_CHANGE, actor_id, body(0x28))
}

/// 0x0146 InventorySetBeginPacket: prefix for a package update.
/// `code` is the package id (0=NORMAL, 0x04=LOOT, 0x05=MELDREQUEST, …).
pub fn build_inventory_set_begin(actor_id: u32, size: u16, code: u16) -> SubPacket {
    let mut data = vec![0u8; 8];
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(actor_id).unwrap();
    c.write_u16::<LittleEndian>(size).unwrap();
    c.write_u16::<LittleEndian>(code).unwrap();
    SubPacket::new(OP_INVENTORY_SET_BEGIN, actor_id, data)
}

/// 0x0147 InventorySetEndPacket.
pub fn build_inventory_set_end(actor_id: u32) -> SubPacket {
    SubPacket::new(OP_INVENTORY_SET_END, actor_id, body(0x28))
}

/// 0x0149 InventoryItemEndPacket — matches InventoryListX08 shape when there's
/// 1-2 items left at end of stream.
pub fn build_inventory_item_end(
    actor_id: u32,
    items: &[InventoryItem],
    list_offset: &mut usize,
) -> SubPacket {
    build_inventory_list_x08(actor_id, items, list_offset, OP_INVENTORY_LIST_X08, 0x90)
}

/// 0x014A InventoryItemPacket — single-item inline.
pub fn build_inventory_item(
    actor_id: u32,
    items: &[InventoryItem],
    list_offset: &mut usize,
) -> SubPacket {
    build_inventory_list_x08(actor_id, items, list_offset, OP_INVENTORY_LIST_X16, 0x90)
}

/// 0x0148 InventoryListX01Packet — single item. C# packet holds exactly one
/// 0x70-byte item record and nothing else; there is no trailing count field.
pub fn build_inventory_list_x01(actor_id: u32, item: &InventoryItem) -> SubPacket {
    let mut data = body(0x90);
    let mut c = Cursor::new(&mut data[..]);
    c.write_all(&encode_item(item)).unwrap();
    SubPacket::new(OP_INVENTORY_LIST_X01, actor_id, data)
}

/// 0x0149 InventoryListX08Packet — up to 8 items.
pub fn build_inventory_list_x08_n(
    actor_id: u32,
    items: &[InventoryItem],
    list_offset: &mut usize,
) -> SubPacket {
    build_inventory_list_x08(actor_id, items, list_offset, OP_INVENTORY_LIST_X08, 0x3A8)
}

/// 0x014A InventoryListX16 — up to 16 items.
pub fn build_inventory_list_x16(
    actor_id: u32,
    items: &[InventoryItem],
    list_offset: &mut usize,
) -> SubPacket {
    build_inventory_list_n(
        actor_id,
        items,
        list_offset,
        16,
        OP_INVENTORY_LIST_X16,
        0x720,
    )
}

/// 0x014B InventoryListX32 — up to 32 items.
pub fn build_inventory_list_x32(
    actor_id: u32,
    items: &[InventoryItem],
    list_offset: &mut usize,
) -> SubPacket {
    build_inventory_list_n(
        actor_id,
        items,
        list_offset,
        32,
        OP_INVENTORY_LIST_X32,
        0xE20,
    )
}

/// 0x014C InventoryListX64 — up to 64 items.
pub fn build_inventory_list_x64(
    actor_id: u32,
    items: &[InventoryItem],
    list_offset: &mut usize,
) -> SubPacket {
    build_inventory_list_n(
        actor_id,
        items,
        list_offset,
        64,
        OP_INVENTORY_LIST_X64,
        0x1C20,
    )
}

fn build_inventory_list_x08(
    actor_id: u32,
    items: &[InventoryItem],
    list_offset: &mut usize,
    opcode: u16,
    packet_size: usize,
) -> SubPacket {
    build_inventory_list_n(actor_id, items, list_offset, 8, opcode, packet_size)
}

fn build_inventory_list_n(
    actor_id: u32,
    items: &[InventoryItem],
    list_offset: &mut usize,
    cap: usize,
    opcode: u16,
    packet_size: usize,
) -> SubPacket {
    let mut data = body(packet_size);
    let max = items.len().saturating_sub(*list_offset).min(cap);
    let mut c = Cursor::new(&mut data[..]);
    for i in 0..max {
        c.write_all(&encode_item(&items[*list_offset + i])).unwrap();
    }
    *list_offset += max;
    // C# writes (UInt32)max at `cap * 0x70` (i.e. end of per-item block).
    let tail = cap * 0x70;
    if tail + 4 <= data.len() {
        data[tail..tail + 4].copy_from_slice(&(max as u32).to_le_bytes());
    }
    SubPacket::new(opcode, actor_id, data)
}

/// 0x0152 InventoryRemoveX01Packet — one slot.
pub fn build_inventory_remove_x01(actor_id: u32, slot: u16) -> SubPacket {
    let mut data = body(0x28);
    data[..2].copy_from_slice(&slot.to_le_bytes());
    // C# writes max=1 at offset 0x10.
    data[0x10] = 1;
    SubPacket::new(OP_INVENTORY_REMOVE_X01, actor_id, data)
}

/// 0x0153 InventoryRemoveX08Packet — up to 8 slots.
pub fn build_inventory_remove_x08(
    actor_id: u32,
    slots: &[u16],
    list_offset: &mut usize,
) -> SubPacket {
    build_inventory_remove_n(
        actor_id,
        slots,
        list_offset,
        8,
        OP_INVENTORY_REMOVE_X08,
        0x38,
        0x10,
    )
}

pub fn build_inventory_remove_x16(
    actor_id: u32,
    slots: &[u16],
    list_offset: &mut usize,
) -> SubPacket {
    build_inventory_remove_n(
        actor_id,
        slots,
        list_offset,
        16,
        OP_INVENTORY_REMOVE_X16,
        0x40,
        0x20,
    )
}

pub fn build_inventory_remove_x32(
    actor_id: u32,
    slots: &[u16],
    list_offset: &mut usize,
) -> SubPacket {
    build_inventory_remove_n(
        actor_id,
        slots,
        list_offset,
        32,
        OP_INVENTORY_REMOVE_X32,
        0x60,
        0x40,
    )
}

pub fn build_inventory_remove_x64(
    actor_id: u32,
    slots: &[u16],
    list_offset: &mut usize,
) -> SubPacket {
    build_inventory_remove_n(
        actor_id,
        slots,
        list_offset,
        64,
        OP_INVENTORY_REMOVE_X64,
        0xA0,
        0x80,
    )
}

fn build_inventory_remove_n(
    actor_id: u32,
    slots: &[u16],
    list_offset: &mut usize,
    cap: usize,
    opcode: u16,
    packet_size: usize,
    count_offset: usize,
) -> SubPacket {
    let mut data = body(packet_size);
    let max = slots.len().saturating_sub(*list_offset).min(cap);
    let mut c = Cursor::new(&mut data[..]);
    for i in 0..max {
        c.write_u16::<LittleEndian>(slots[*list_offset + i])
            .unwrap();
    }
    *list_offset += max;
    if count_offset < data.len() {
        data[count_offset] = max as u8;
    }
    SubPacket::new(opcode, actor_id, data)
}

/// 0x014D LinkedItemListX01 — equip slot linking for one item.
/// Each entry is 3× u16: (linked equip slot, source item slot, source item
/// package). `item=None` writes zeros, matching the "clear this slot" path
/// Meteor uses after an unequip.
pub fn build_linked_item_list_x01(
    actor_id: u32,
    position: u16,
    item: Option<&InventoryItem>,
) -> SubPacket {
    let mut data = body(0x28);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u16::<LittleEndian>(position).unwrap();
    let (src_slot, src_pkg) = item.map(|i| (i.slot, i.item_package)).unwrap_or((0, 0));
    c.write_u16::<LittleEndian>(src_slot).unwrap();
    c.write_u16::<LittleEndian>(src_pkg).unwrap();
    SubPacket::new(OP_LINKED_ITEM_LIST_X01, actor_id, data)
}

/// 0x014E LinkedItemListX08 — up to 8 equip-slot/item-ref triplets + a
/// trailing `count: u32` at offset 0x30.
pub fn build_linked_item_list_x08(
    actor_id: u32,
    entries: &[(u16, InventoryItem)],
    list_offset: &mut usize,
) -> SubPacket {
    build_linked_item_list_n(
        actor_id,
        entries,
        list_offset,
        8,
        OP_LINKED_ITEM_LIST_X08,
        0x58,
        Some(0x30),
    )
}

/// 0x014F LinkedItemListX16 — up to 16 triplets, no count field.
pub fn build_linked_item_list_x16(
    actor_id: u32,
    entries: &[(u16, InventoryItem)],
    list_offset: &mut usize,
) -> SubPacket {
    build_linked_item_list_n(
        actor_id,
        entries,
        list_offset,
        16,
        OP_LINKED_ITEM_LIST_X16,
        0x80,
        None,
    )
}

/// 0x0150 LinkedItemListX32 — up to 32 triplets.
pub fn build_linked_item_list_x32(
    actor_id: u32,
    entries: &[(u16, InventoryItem)],
    list_offset: &mut usize,
) -> SubPacket {
    build_linked_item_list_n(
        actor_id,
        entries,
        list_offset,
        32,
        OP_LINKED_ITEM_LIST_X32,
        0xE0,
        None,
    )
}

/// 0x0151 LinkedItemListX64 — up to 64 triplets.
///
/// Note: Meteor's `LinkedItemListX64Packet.cs` declares `PACKET_SIZE = 0x194`
/// (body = 0x174 / 372 bytes), which only fits 62 of the advertised 64
/// entries and throws on a full batch. We size the body to fit 64 × 6 bytes
/// + the 0x20 header (0x1A0) so a full batch doesn't truncate. Client-side
/// compat with 1.23b here is untested because this path effectively never
/// fires — equipment tops out at ~35 slots.
pub fn build_linked_item_list_x64(
    actor_id: u32,
    entries: &[(u16, InventoryItem)],
    list_offset: &mut usize,
) -> SubPacket {
    build_linked_item_list_n(
        actor_id,
        entries,
        list_offset,
        64,
        OP_LINKED_ITEM_LIST_X64,
        0x1A0,
        None,
    )
}

fn build_linked_item_list_n(
    actor_id: u32,
    entries: &[(u16, InventoryItem)],
    list_offset: &mut usize,
    cap: usize,
    opcode: u16,
    packet_size: usize,
    count_offset: Option<usize>,
) -> SubPacket {
    let mut data = body(packet_size);
    let max = entries.len().saturating_sub(*list_offset).min(cap);
    {
        let mut c = Cursor::new(&mut data[..]);
        for i in 0..max {
            let (equip_slot, item) = &entries[*list_offset + i];
            c.write_u16::<LittleEndian>(*equip_slot).unwrap();
            c.write_u16::<LittleEndian>(item.slot).unwrap();
            c.write_u16::<LittleEndian>(item.item_package).unwrap();
        }
    }
    *list_offset += max;
    if let Some(off) = count_offset
        && off + 4 <= data.len()
    {
        data[off..off + 4].copy_from_slice(&(max as u32).to_le_bytes());
    }
    SubPacket::new(opcode, actor_id, data)
}

/// 0x014E SetInitialEquipmentPacket — mirrors Meteor's
/// `SetInitialEquipmentPacket.BuildPackets`. The wire layout is NOT a
/// 35-slot dense array (the previous port assumed that, using opcode
/// 0x0178 — a ghost opcode — with 35 u32 slots, which blew past the
/// 56-byte body and the client silently dropped the packet). Actual
/// layout per Meteor's C#:
///
///   body size = 0x38 (packet 0x58 - 0x20 header)
///   for each *equipped* slot index `i` in 0..0x17:
///     u16 slot_index
///     u32 item_id
///   seek(0x30) — write u32 count at end of the body
///
/// Each packet holds up to 8 (slot, item) pairs + the trailing count.
/// Emitting one empty packet (count=0, all zero body) is what Meteor
/// sends for a character with no equipped items, which is what we need
/// for Asdf-shape logins. Callers that want to populate slots can pass
/// `(u16, u32)` pairs; we chunk in 8s to match Meteor.
pub fn build_set_initial_equipment(actor_id: u32, slots: &[(u16, u32)]) -> Vec<SubPacket> {
    const SLOT_LIMIT: usize = 8;
    const COUNT_OFFSET: usize = 0x30;
    let mut packets = Vec::new();
    let emit_one = |chunk: &[(u16, u32)]| {
        let mut data = body(0x58);
        {
            let mut c = Cursor::new(&mut data[..]);
            for (slot, item_id) in chunk {
                c.write_u16::<LittleEndian>(*slot).unwrap();
                c.write_u32::<LittleEndian>(*item_id).unwrap();
            }
        }
        data[COUNT_OFFSET..COUNT_OFFSET + 4]
            .copy_from_slice(&(chunk.len() as u32).to_le_bytes());
        SubPacket::new(0x014E, actor_id, data)
    };

    if slots.is_empty() {
        packets.push(emit_one(&[]));
    } else {
        for chunk in slots.chunks(SLOT_LIMIT) {
            packets.push(emit_one(chunk));
        }
    }
    packets
}

// ---------------------------------------------------------------------------
// Mass Set Item Modifier (0x018F begin / 0x0190 body / 0x0191 end).
//
// Frame structure:
//   0x018F  body 8 bytes of zeros — "begin" marker
//   0x0190  one per item; body 0x68 bytes:
//     u64 unique_id              (offset 0x00)
//     ItemModifier (0x2E bytes)  (offset 0x08, see encode_item_modifier)
//     50 bytes of zero padding   (offset 0x36..0x68)
//   0x0191  body 8 bytes of zeros — "end" marker
//
// Format anchors:
//   • Wiki page `Game_Opcodes:Mass_Set_Item_Modifier` (data size 0x68)
//   • Wiki page `Item_Modifier` (says 0x2A but the actual layout per Meteor
//     C# `ItemModifier::WriteBytes` and retail captures is 0x2E)
//   • Retail bytes captured under `ffxiv_traces/repair_items.pcapng`,
//     `ffxiv_traces/combat_skills.pcapng`, etc., decoded via
//     `packet-diff/cargo run --bin pcap-survey -- … --dump-opcode 0x0190`
//
// Project Meteor never figured this opcode out (no builder exists), so
// this is a net-new emit garlemald gets to add.
// ---------------------------------------------------------------------------

/// 0x018F MassSetItemModifierBegin — opens a 0x0190 burst.
pub fn build_mass_set_item_modifier_begin(actor_id: u32) -> SubPacket {
    SubPacket::new(OP_MASS_SET_ITEM_MODIFIER_BEGIN, actor_id, body(0x28))
}

/// 0x0191 MassSetItemModifierEnd — closes a 0x0190 burst. The client
/// applies the per-item modifier deltas to the bag UI on this packet.
pub fn build_mass_set_item_modifier_end(actor_id: u32) -> SubPacket {
    SubPacket::new(OP_MASS_SET_ITEM_MODIFIER_END, actor_id, body(0x28))
}

/// 0x0190 MassSetItemModifier — one per item.
///
/// `unique_id` is the server-side per-item-instance id (Meteor's
/// `serverItemId`, an auto-increment INT pool); the client's bag UI
/// keys modifier state by this id, so it must match the `unique_id`
/// used in the corresponding `0x0148/0x014A InventoryListX*` emission
/// for the same slot.
pub fn build_mass_set_item_modifier(actor_id: u32, item: &InventoryItem) -> SubPacket {
    let mut data = body(0x88);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u64::<LittleEndian>(item.unique_id).unwrap();
    encode_item_modifier(&mut c, &item.tag);
    SubPacket::new(OP_MASS_SET_ITEM_MODIFIER, actor_id, data)
}

/// Write the 46-byte ItemModifier struct at the cursor's current position.
/// Layout matches Meteor's `ItemModifier::WriteBytes` and decodes 1:1 with
/// the bytes captured in `ffxiv_traces/*.pcapng`.
fn encode_item_modifier(c: &mut Cursor<&mut [u8]>, tag: &crate::data::ItemTag) {
    c.write_u32::<LittleEndian>(tag.durability).unwrap();
    c.write_u16::<LittleEndian>(tag.use_count).unwrap();
    c.write_u32::<LittleEndian>(tag.materia_id).unwrap();
    c.write_u32::<LittleEndian>(tag.materia_life).unwrap();
    c.write_u8(tag.main_quality).unwrap();
    c.write_u8(tag.sub_quality[0]).unwrap();
    c.write_u8(tag.sub_quality[1]).unwrap();
    c.write_u8(tag.sub_quality[2]).unwrap();
    c.write_u32::<LittleEndian>(tag.polish).unwrap();
    c.write_u32::<LittleEndian>(tag.param1).unwrap();
    c.write_u32::<LittleEndian>(tag.param2).unwrap();
    c.write_u32::<LittleEndian>(tag.param3).unwrap();
    c.write_u16::<LittleEndian>(tag.spiritbind).unwrap();
    for b in tag.materia_type {
        c.write_u8(b).unwrap();
    }
    for b in tag.materia_grade {
        c.write_u8(b).unwrap();
    }
}

/// Convenience: emit a full begin/body*/end frame for `items`. Yields
/// one Vec containing 2 + items.len() SubPackets so the caller can
/// extend its outbox in one shot.
pub fn build_mass_set_item_modifier_frame(
    actor_id: u32,
    items: &[InventoryItem],
) -> Vec<SubPacket> {
    let mut out = Vec::with_capacity(2 + items.len());
    out.push(build_mass_set_item_modifier_begin(actor_id));
    for item in items {
        out.push(build_mass_set_item_modifier(actor_id, item));
    }
    out.push(build_mass_set_item_modifier_end(actor_id));
    out
}

fn encode_item(item: &InventoryItem) -> Vec<u8> {
    let mut out = vec![0u8; 0x70];
    let mut c = Cursor::new(&mut out[..]);
    c.write_u64::<LittleEndian>(item.unique_id).unwrap();
    c.write_i32::<LittleEndian>(item.quantity).unwrap();
    c.write_u32::<LittleEndian>(item.item_id).unwrap();
    let slot = if item.link_slot == 0xFFFF {
        item.slot
    } else {
        item.link_slot
    };
    c.write_u16::<LittleEndian>(slot).unwrap();
    // dealingVal + dealingMode bytes + three dealingAttached u32 — left zero
    // until trade/bazaar plumbing lights up.
    for _ in 0..(1 + 1 + 4 + 4 + 4) {
        c.write_u8(0).unwrap();
    }
    // tags[] + tagValues[] — 16 bytes total of defaults.
    for _ in 0..16 {
        c.write_u8(0).unwrap();
    }
    c.write_u8(item.quality).unwrap();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{InventoryItem, ItemTag};

    /// Reproduce the 0x0190 body bytes captured from
    /// `ffxiv_traces/repair_items.pcapng` record #1 (and confirmed
    /// identical in `harvest.pcapng`, `combat_skills.pcapng`, etc. for
    /// the same item unique-id 0x07460059). The retail item is in its
    /// NQ-default state: durability 0, use_count 1, mainQuality 1,
    /// subQuality [1,1,1], everything else zero.
    #[test]
    fn mass_set_item_modifier_matches_retail_capture() {
        let item = InventoryItem {
            unique_id: 0x0746_0059,
            tag: ItemTag {
                durability: 0,
                use_count: 1,
                materia_id: 0,
                materia_life: 0,
                main_quality: 1,
                sub_quality: [1, 1, 1],
                polish: 0,
                param1: 0,
                param2: 0,
                param3: 0,
                spiritbind: 0,
                materia_type: [0; 5],
                materia_grade: [0; 5],
            },
            ..InventoryItem::default()
        };
        let pkt = build_mass_set_item_modifier(0x029B_2941, &item);
        // The body slice is everything after SubPacketHeader+GameMessageHeader,
        // i.e. exactly the 0x68 bytes the wiki documents.
        let body = &pkt.data;
        assert_eq!(body.len(), 0x68);

        // Captured ground truth — see
        // `captures/retail_pcap_gap_analysis.md` for derivation.
        // Byte 0x00..0x08 = u64 unique_id (LE).
        // Byte 0x08..0x36 = ItemModifier struct (46 bytes).
        // Byte 0x36..0x68 = 50 bytes of zero padding.
        #[rustfmt::skip]
        let expected: [u8; 0x68] = [
            0x59, 0x00, 0x46, 0x07, 0x00, 0x00, 0x00, 0x00, // unique_id
            0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, // life=0, use=1, materiaId(lo)=0
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x01, // materiaId(hi)=0, materiaLife=0, mainQuality=1, subQ[0]=1
            0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // subQ[1]=1, subQ[2]=1, polish=0, param1(lo)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // param1(hi), param2
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // param3, fitness, materiaType[0..2]
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // padding starts here (50 bytes)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        assert_eq!(body, &expected[..]);
    }

    /// Begin and end markers are both 0x28-byte SubPackets with empty
    /// (8-byte zero) bodies — confirmed against
    /// `ffxiv_traces/repair_items.pcapng` records #1 of opcode 0x018F
    /// and 0x0191.
    #[test]
    fn mass_set_item_modifier_brackets_have_zero_bodies() {
        let begin = build_mass_set_item_modifier_begin(0x029B_2941);
        let end = build_mass_set_item_modifier_end(0x029B_2941);
        assert_eq!(begin.data.len(), 8);
        assert!(begin.data.iter().all(|b| *b == 0));
        assert_eq!(end.data.len(), 8);
        assert!(end.data.iter().all(|b| *b == 0));
        assert_eq!(begin.game_message.opcode, OP_MASS_SET_ITEM_MODIFIER_BEGIN);
        assert_eq!(end.game_message.opcode, OP_MASS_SET_ITEM_MODIFIER_END);
    }

    #[test]
    fn mass_set_item_modifier_frame_emits_begin_body_end() {
        let items = vec![
            InventoryItem { unique_id: 1, ..Default::default() },
            InventoryItem { unique_id: 2, ..Default::default() },
            InventoryItem { unique_id: 3, ..Default::default() },
        ];
        let frame = build_mass_set_item_modifier_frame(0x029B_2941, &items);
        assert_eq!(frame.len(), 5);
        assert_eq!(
            frame[0].game_message.opcode,
            OP_MASS_SET_ITEM_MODIFIER_BEGIN
        );
        assert!(frame[1..4]
            .iter()
            .all(|p| p.game_message.opcode == OP_MASS_SET_ITEM_MODIFIER));
        assert_eq!(
            frame[4].game_message.opcode,
            OP_MASS_SET_ITEM_MODIFIER_END
        );
    }
}
