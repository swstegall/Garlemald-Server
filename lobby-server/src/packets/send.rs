#![allow(clippy::explicit_counter_loop)]

//! Outgoing lobby packets. Byte-for-byte compatible with the C# builders in
//! Lobby Server/Packets/Send.
//!
//! Each list-style packet is chunked into subpackets with at most
//! `MAX_PER_PACKET` entries. The chunk header is:
//!
//! | offset | field         | type   |
//! |--------|---------------|--------|
//! | 0x00   | sequence      | u64 LE |
//! | 0x08   | list tracker  | u8     |
//! | 0x09   | count         | u32 LE |
//! | 0x0D   | padding       | u8     |
//! | 0x0E   | padding       | u16 LE |
//!
//! `list tracker = (MAX_PER_PACKET * 2 * chunk_index)` with the low bit set on
//! the final chunk.

use std::io::{Cursor, Seek, SeekFrom, Write};

use byteorder::{LittleEndian, WriteBytesExt};
use common::subpacket::SubPacket;

use crate::data::chara_info;
use crate::data::{Account, Appearance, Character, Retainer, World};

pub const LOBBY_TARGET_ID: u32 = 0xe0006868;

fn write_padded_ascii<W: Write>(w: &mut W, s: &str, width: usize) {
    let bytes = s.as_bytes();
    let n = bytes.len().min(width);
    w.write_all(&bytes[..n]).unwrap();
    for _ in n..width {
        w.write_u8(0).unwrap();
    }
}

fn write_list_header(
    c: &mut Cursor<&mut [u8]>,
    sequence: u64,
    chunk_index: usize,
    max_per_packet: u16,
    remaining: usize,
) {
    c.write_u64::<LittleEndian>(sequence).unwrap();
    let list_tracker = (max_per_packet as usize * 2 * chunk_index) as u8;
    if remaining <= max_per_packet as usize {
        c.write_u8(list_tracker + 1).unwrap();
        c.write_u32::<LittleEndian>(remaining as u32).unwrap();
    } else {
        c.write_u8(list_tracker).unwrap();
        c.write_u32::<LittleEndian>(max_per_packet as u32).unwrap();
    }
    c.write_u8(0).unwrap();
    c.write_u16::<LittleEndian>(0).unwrap();
}

fn subpacket_for(opcode: u16, data: Vec<u8>) -> SubPacket {
    let mut sub = SubPacket::new(opcode, LOBBY_TARGET_ID, data);
    sub.set_target_id(LOBBY_TARGET_ID);
    sub
}

// ---------------------------------------------------------------------------
// Error (opcode 0x02)
// ---------------------------------------------------------------------------

pub fn error_packet(
    sequence: u64,
    error_code: u32,
    status_code: u32,
    text_id: u32,
    message: &str,
) -> SubPacket {
    const CAPACITY: usize = 0x210;
    let mut buf = vec![0u8; CAPACITY];
    {
        let mut c = Cursor::new(&mut buf[..]);
        c.write_u64::<LittleEndian>(sequence).unwrap();
        c.write_u32::<LittleEndian>(error_code).unwrap();
        c.write_u32::<LittleEndian>(status_code).unwrap();
        c.write_u32::<LittleEndian>(text_id).unwrap();
        let msg = message.as_bytes();
        let n = msg.len().min(CAPACITY - (c.position() as usize));
        c.write_all(&msg[..n]).unwrap();
    }
    subpacket_for(0x02, buf)
}

// ---------------------------------------------------------------------------
// WorldList (opcode 0x15)
// ---------------------------------------------------------------------------

pub fn world_list_packets(sequence: u64, worlds: &[World]) -> Vec<SubPacket> {
    const OPCODE: u16 = 0x15;
    const MAX: u16 = 6;
    const CAPACITY: usize = 0x210;

    let mut packets: Vec<SubPacket> = Vec::new();
    let mut current: Option<(Vec<u8>, usize)> = None;
    let mut server_count = 0usize;
    let mut total = 0usize;

    for world in worlds {
        if total == 0 || server_count.is_multiple_of(MAX as usize) {
            let mut buf = vec![0u8; CAPACITY];
            {
                let mut c = Cursor::new(&mut buf[..]);
                write_list_header(
                    &mut c,
                    sequence,
                    packets.len(),
                    MAX,
                    worlds.len() - total,
                );
            }
            current = Some((buf, 0x10));
        }

        if let Some((buf, pos)) = current.as_mut() {
            let mut c = Cursor::new(&mut buf[..]);
            c.set_position(*pos as u64);
            c.write_u16::<LittleEndian>(world.id).unwrap();
            c.write_u16::<LittleEndian>(world.list_position).unwrap();
            c.write_u32::<LittleEndian>(world.population as u32).unwrap();
            c.write_u64::<LittleEndian>(0).unwrap();
            let mut name = vec![0u8; 64];
            let name_bytes = world.name.as_bytes();
            let n = name_bytes.len().min(64);
            name[..n].copy_from_slice(&name_bytes[..n]);
            c.write_all(&name).unwrap();
            *pos = c.position() as usize;
        }

        server_count += 1;
        total += 1;

        if server_count >= MAX as usize {
            if let Some((buf, _)) = current.take() {
                packets.push(subpacket_for(OPCODE, buf));
            }
            server_count = 0;
        }
    }

    if server_count > 0 {
        if let Some((buf, _)) = current.take() {
            packets.push(subpacket_for(OPCODE, buf));
        }
    } else if worlds.is_empty() {
        let mut buf = vec![0u8; CAPACITY];
        {
            let mut c = Cursor::new(&mut buf[..]);
            write_list_header(&mut c, sequence, packets.len(), MAX, 0);
        }
        packets.push(subpacket_for(OPCODE, buf));
    }

    packets
}

// ---------------------------------------------------------------------------
// AccountList (opcode 0x0C)
// ---------------------------------------------------------------------------

pub fn account_list_packets(sequence: u64, accounts: &[Account]) -> Vec<SubPacket> {
    const OPCODE: u16 = 0x0C;
    const MAX: u16 = 8;
    const CAPACITY: usize = 0x280;

    let mut packets: Vec<SubPacket> = Vec::new();
    let mut current: Option<(Vec<u8>, usize)> = None;
    let mut acc_count = 0usize;
    let mut total = 0usize;

    for account in accounts {
        if total == 0 || acc_count.is_multiple_of(MAX as usize) {
            let mut buf = vec![0u8; CAPACITY];
            {
                let mut c = Cursor::new(&mut buf[..]);
                write_list_header(
                    &mut c,
                    sequence,
                    packets.len(),
                    MAX,
                    accounts.len() - total,
                );
            }
            current = Some((buf, 0x10));
        }

        if let Some((buf, pos)) = current.as_mut() {
            let mut c = Cursor::new(&mut buf[..]);
            c.set_position(*pos as u64);
            c.write_u32::<LittleEndian>(account.id).unwrap();
            c.write_u32::<LittleEndian>(0).unwrap();
            write_padded_ascii(&mut c, &account.name, 0x40);
            *pos = c.position() as usize;
        }

        acc_count += 1;
        total += 1;

        if acc_count >= MAX as usize {
            if let Some((buf, _)) = current.take() {
                packets.push(subpacket_for(OPCODE, buf));
            }
            acc_count = 0;
        }
    }

    if acc_count > 0 {
        if let Some((buf, _)) = current.take() {
            packets.push(subpacket_for(OPCODE, buf));
        }
    } else if accounts.is_empty() {
        let mut buf = vec![0u8; 0x210];
        {
            let mut c = Cursor::new(&mut buf[..]);
            write_list_header(&mut c, sequence, packets.len(), MAX, 0);
        }
        packets.push(subpacket_for(OPCODE, buf));
    }

    packets
}

// ---------------------------------------------------------------------------
// CharacterList (opcode 0x0D)
// ---------------------------------------------------------------------------

pub fn character_list_packets(
    sequence: u64,
    characters: &[Character],
    world_lookup: impl Fn(u16) -> Option<World>,
    appearance_lookup: impl Fn(u32) -> Appearance,
) -> Vec<SubPacket> {
    const OPCODE: u16 = 0x0D;
    const MAX: u16 = 2;
    const CAPACITY: usize = 0x3B0;
    const ENTRY_STRIDE: usize = 0x1D0;

    let mut packets: Vec<SubPacket> = Vec::new();
    let mut current: Option<(Vec<u8>, usize)> = None;
    let mut char_count = 0usize;
    let mut total = 0usize;

    // The 'NEW' placeholder slot bumps the apparent count by one when the
    // roster has room for another character. Matches the C# math.
    let num_characters = if characters.len() >= 8 { 8 } else { characters.len() + 1 };

    for chara in characters.iter().take(8) {
        let appearance = appearance_lookup(chara.id);

        if total == 0 || char_count.is_multiple_of(MAX as usize) {
            let mut buf = vec![0u8; CAPACITY];
            {
                let mut c = Cursor::new(&mut buf[..]);
                write_list_header(
                    &mut c,
                    sequence,
                    packets.len(),
                    MAX,
                    num_characters - total,
                );
            }
            current = Some((buf, 0x10));
        }

        if let Some((buf, pos)) = current.as_mut() {
            let entry_start = 0x10 + ENTRY_STRIDE * char_count;
            *pos = entry_start;
            let mut c = Cursor::new(&mut buf[..]);
            c.set_position(entry_start as u64);

            let world = world_lookup(chara.server_id);
            let world_name = world.map(|w| w.name).unwrap_or_else(|| "Unknown".to_string());

            c.write_u32::<LittleEndian>(0).unwrap();
            c.write_u32::<LittleEndian>(chara.id).unwrap();
            c.write_u8(total as u8).unwrap();

            let mut options: u8 = 0;
            if chara.state == 1 {
                options |= 0x01;
            }
            if chara.do_rename {
                options |= 0x02;
            }
            if chara.is_legacy {
                options |= 0x08;
            }
            c.write_u8(options).unwrap();
            c.write_u16::<LittleEndian>(0).unwrap();
            c.write_u32::<LittleEndian>(chara.current_zone_id).unwrap();
            write_padded_ascii(&mut c, &chara.name, 0x20);
            write_padded_ascii(&mut c, &world_name, 0x0E);

            let appearance_blob = chara_info::build_for_chara_list(chara, &appearance);
            c.write_all(appearance_blob.as_bytes()).unwrap();
        }

        char_count += 1;
        total += 1;

        if char_count >= MAX as usize {
            if let Some((buf, _)) = current.take() {
                packets.push(subpacket_for(OPCODE, buf));
            }
            char_count = 0;
        }
    }

    // 'NEW' placeholder if there's still a slot.
    if characters.len() < 8 {
        if char_count.is_multiple_of(MAX as usize) {
            let mut buf = vec![0u8; CAPACITY];
            {
                let mut c = Cursor::new(&mut buf[..]);
                write_list_header(
                    &mut c,
                    sequence,
                    packets.len(),
                    MAX,
                    num_characters - total,
                );
            }
            current = Some((buf, 0x10));
        }

        if let Some((buf, _)) = current.as_mut() {
            let entry_start = 0x10 + ENTRY_STRIDE * char_count;
            let mut c = Cursor::new(&mut buf[..]);
            c.set_position(entry_start as u64);
            c.write_u32::<LittleEndian>(0).unwrap();
            c.write_u32::<LittleEndian>(0).unwrap();
            c.write_u8(total as u8).unwrap();
            c.write_u8(0).unwrap();
            c.write_u16::<LittleEndian>(0).unwrap();
            c.write_u32::<LittleEndian>(0).unwrap();
        }

        char_count += 1;
        let _ = total; // logical totals bumped by the 'NEW' placeholder; no writes depend on it.

        if char_count >= MAX as usize {
            if let Some((buf, _)) = current.take() {
                packets.push(subpacket_for(OPCODE, buf));
            }
            char_count = 0;
        }
    }

    if (char_count > 0 || num_characters == 0)
        && let Some((buf, _)) = current.take() {
            packets.push(subpacket_for(OPCODE, buf));
        }

    packets
}

// ---------------------------------------------------------------------------
// RetainerList (opcode 0x17)
// ---------------------------------------------------------------------------

pub fn retainer_list_packets(sequence: u64, retainers: &[Retainer]) -> Vec<SubPacket> {
    const OPCODE: u16 = 0x17;
    const MAX: u16 = 9;
    const CAPACITY: usize = 0x210;

    let mut packets: Vec<SubPacket> = Vec::new();
    let mut current: Option<(Vec<u8>, usize)> = None;
    let mut count = 0usize;
    let mut total = 0usize;

    for retainer in retainers {
        if total == 0 || count.is_multiple_of(MAX as usize) {
            let mut buf = vec![0u8; CAPACITY];
            {
                let mut c = Cursor::new(&mut buf[..]);
                write_list_header(
                    &mut c,
                    sequence,
                    packets.len(),
                    MAX,
                    retainers.len() - total,
                );
                c.write_u64::<LittleEndian>(0).unwrap();
                c.write_u32::<LittleEndian>(0).unwrap();
            }
            current = Some((buf, 0x1C));
        }

        if let Some((buf, pos)) = current.as_mut() {
            let mut c = Cursor::new(&mut buf[..]);
            c.set_position(*pos as u64);
            c.write_u32::<LittleEndian>(retainer.id).unwrap();
            c.write_u32::<LittleEndian>(retainer.character_id).unwrap();
            c.write_u16::<LittleEndian>(total as u16).unwrap();
            c.write_u16::<LittleEndian>(if retainer.do_rename { 0x04 } else { 0x00 }).unwrap();
            c.write_u32::<LittleEndian>(0).unwrap();
            write_padded_ascii(&mut c, &retainer.name, 0x20);
            *pos = c.position() as usize;
        }

        count += 1;
        total += 1;

        if count >= MAX as usize {
            if let Some((buf, _)) = current.take() {
                packets.push(subpacket_for(OPCODE, buf));
            }
            count = 0;
        }
    }

    if count > 0 {
        if let Some((buf, _)) = current.take() {
            packets.push(subpacket_for(OPCODE, buf));
        }
    } else if retainers.is_empty() {
        let mut buf = vec![0u8; CAPACITY];
        {
            let mut c = Cursor::new(&mut buf[..]);
            write_list_header(&mut c, sequence, packets.len(), MAX, 0);
        }
        packets.push(subpacket_for(OPCODE, buf));
    }

    packets
}

// ---------------------------------------------------------------------------
// ImportList (opcode 0x16)
// ---------------------------------------------------------------------------

pub fn import_list_packets(sequence: u64, names: &[String]) -> Vec<SubPacket> {
    const OPCODE: u16 = 0x16;
    const MAX: u16 = 12;
    const CAPACITY: usize = 0x210;

    let mut packets: Vec<SubPacket> = Vec::new();
    let mut current: Option<(Vec<u8>, usize)> = None;
    let mut count = 0usize;
    let mut total = 0usize;

    for name in names {
        if total == 0 || count.is_multiple_of(MAX as usize) {
            let mut buf = vec![0u8; CAPACITY];
            {
                let mut c = Cursor::new(&mut buf[..]);
                write_list_header(&mut c, sequence, packets.len(), MAX, names.len() - total);
            }
            current = Some((buf, 0x10));
        }

        if let Some((buf, pos)) = current.as_mut() {
            let mut c = Cursor::new(&mut buf[..]);
            c.set_position(*pos as u64);
            c.write_u32::<LittleEndian>(0).unwrap();
            c.write_u32::<LittleEndian>(total as u32).unwrap();
            let full = if !name.contains(' ') { format!("{name} Last") } else { name.clone() };
            write_padded_ascii(&mut c, &full, 0x20);
            *pos = c.position() as usize;
        }

        count += 1;
        total += 1;

        if count >= MAX as usize {
            if let Some((buf, _)) = current.take() {
                packets.push(subpacket_for(OPCODE, buf));
            }
            count = 0;
        }
    }

    if count > 0 {
        if let Some((buf, _)) = current.take() {
            packets.push(subpacket_for(OPCODE, buf));
        }
    } else if names.is_empty() {
        let mut buf = vec![0u8; CAPACITY];
        {
            let mut c = Cursor::new(&mut buf[..]);
            write_list_header(&mut c, sequence, packets.len(), MAX, 0);
        }
        packets.push(subpacket_for(OPCODE, buf));
    }

    packets
}

// ---------------------------------------------------------------------------
// SelectCharacterConfirm (opcode 0x0F)
// ---------------------------------------------------------------------------

pub fn select_character_confirm_packet(
    sequence: u64,
    character_id: u32,
    session_token: &str,
    world_ip: &str,
    world_port: u16,
    select_ticket: u64,
) -> SubPacket {
    const CAPACITY: usize = 0x98;
    let mut buf = vec![0u8; CAPACITY];
    {
        let mut c = Cursor::new(&mut buf[..]);
        c.write_u64::<LittleEndian>(sequence).unwrap();
        c.write_u32::<LittleEndian>(character_id).unwrap();
        c.write_u32::<LittleEndian>(character_id).unwrap();
        c.write_u32::<LittleEndian>(0).unwrap();
        write_padded_ascii(&mut c, session_token, 0x42);
        c.write_u16::<LittleEndian>(world_port).unwrap();
        write_padded_ascii(&mut c, world_ip, 0x38);
        c.write_u64::<LittleEndian>(select_ticket).unwrap();
    }
    subpacket_for(0x0F, buf)
}

// ---------------------------------------------------------------------------
// CharaCreator (opcode 0x0E)
// ---------------------------------------------------------------------------

pub fn chara_creator_packet(
    sequence: u64,
    command: u16,
    pid: u32,
    cid: u32,
    ticket: u32,
    chara_name: &str,
    world_name: &str,
) -> SubPacket {
    const CAPACITY: usize = 0x1F0;
    let mut buf = vec![0u8; CAPACITY];
    {
        let mut c = Cursor::new(&mut buf[..]);
        c.write_u64::<LittleEndian>(sequence).unwrap();
        c.write_u8(1).unwrap();
        c.write_u8(1).unwrap();
        c.write_u16::<LittleEndian>(command).unwrap();
        c.write_u32::<LittleEndian>(0).unwrap();
        c.write_u32::<LittleEndian>(pid).unwrap();
        c.write_u32::<LittleEndian>(cid).unwrap();
        c.write_u32::<LittleEndian>(0x400017).unwrap();
        c.write_u32::<LittleEndian>(ticket).unwrap();
        write_padded_ascii(&mut c, chara_name, 0x20);
        write_padded_ascii(&mut c, world_name, 0x20);
    }
    subpacket_for(0x0E, buf)
}

// Silence dead_code warnings for SeekFrom when unused.
#[allow(dead_code)]
fn _touch_seek(c: &mut Cursor<&mut [u8]>) {
    let _ = c.seek(SeekFrom::Start(0));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_character_confirm_is_exact_size() {
        let sub = select_character_confirm_packet(0x1, 0x2, "tok", "127.0.0.1", 54991, 0x3);
        assert_eq!(sub.data.len(), 0x98);
    }

    #[test]
    fn world_list_empty_emits_one_chunk() {
        let pkts = world_list_packets(0, &[]);
        assert_eq!(pkts.len(), 1);
        assert_eq!(pkts[0].data.len(), 0x210);
    }

    #[test]
    fn world_list_fits_within_a_chunk() {
        let mut worlds = Vec::new();
        for i in 0..3u16 {
            worlds.push(World {
                id: i,
                address: "127.0.0.1".into(),
                port: 54991,
                list_position: i,
                population: 0,
                name: format!("w{i}"),
                is_active: true,
            });
        }
        let pkts = world_list_packets(0, &worlds);
        assert_eq!(pkts.len(), 1);
    }

    #[test]
    fn world_list_splits_across_chunks() {
        let mut worlds = Vec::new();
        for i in 0..7u16 {
            worlds.push(World {
                id: i,
                address: "127.0.0.1".into(),
                port: 54991,
                list_position: i,
                population: 0,
                name: format!("w{i}"),
                is_active: true,
            });
        }
        let pkts = world_list_packets(0, &worlds);
        assert_eq!(pkts.len(), 2);
    }

    #[test]
    fn error_packet_ok() {
        let sub = error_packet(0xFF, 1003, 0, 13005, "hi");
        assert_eq!(sub.data.len(), 0x210);
    }
}
