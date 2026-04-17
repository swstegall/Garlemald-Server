//! Support desk — GM tickets, FAQ, issues.

use std::io::Cursor;

use byteorder::{LittleEndian, WriteBytesExt};
use common::subpacket::SubPacket;

use super::super::opcodes::*;
use super::{body, write_padded_ascii};

/// 0x01D0 FaqListResponse — N titles, 0x20 padded each.
pub fn build_faq_list_response(actor_id: u32, faqs: &[String]) -> SubPacket {
    let mut data = body(0x2B8);
    let n = faqs.len().min(20);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u16::<LittleEndian>(n as u16).unwrap();
        c.write_u16::<LittleEndian>(0).unwrap();
        for f in faqs.iter().take(n) {
            write_padded_ascii(&mut c, f, 0x20);
        }
    }
    SubPacket::new(OP_FAQ_LIST_RESPONSE, actor_id, data)
}

/// 0x01D1 FaqBodyResponse — single large body.
pub fn build_faq_body_response(actor_id: u32, body_text: &str) -> SubPacket {
    let mut data = body(0x587);
    let bytes = body_text.as_bytes();
    let n = bytes.len().min(data.len());
    data[..n].copy_from_slice(&bytes[..n]);
    SubPacket::new(OP_FAQ_BODY_RESPONSE, actor_id, data)
}

/// 0x01D2 IssueListResponse.
pub fn build_issue_list_response(actor_id: u32, issues: &[String]) -> SubPacket {
    let mut data = body(0x160);
    let n = issues.len().min(10);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u16::<LittleEndian>(n as u16).unwrap();
        c.write_u16::<LittleEndian>(0).unwrap();
        for issue in issues.iter().take(n) {
            write_padded_ascii(&mut c, issue, 0x20);
        }
    }
    SubPacket::new(OP_ISSUE_LIST_RESPONSE, actor_id, data)
}

/// 0x01D3 StartGMTicket.
pub fn build_start_gm_ticket(actor_id: u32, start_gm: bool) -> SubPacket {
    let mut data = body(0x28);
    data[0] = start_gm as u8;
    SubPacket::new(OP_START_GM_TICKET, actor_id, data)
}

/// 0x01D4 GMTicket — title + body.
pub fn build_gm_ticket(actor_id: u32, title: &str, body_text: &str) -> SubPacket {
    let mut data = body(0x2B8);
    let mut c = Cursor::new(&mut data[..]);
    write_padded_ascii(&mut c, title, 0x40);
    write_padded_ascii(&mut c, body_text, 0x230);
    SubPacket::new(OP_GM_TICKET, actor_id, data)
}

/// 0x01D5 GMTicketSentResponse.
pub fn build_gm_ticket_sent_response(actor_id: u32, was_sent: bool) -> SubPacket {
    let mut data = body(0x28);
    data[0] = was_sent as u8;
    SubPacket::new(OP_GM_TICKET_SENT_RESPONSE, actor_id, data)
}

/// 0x01D6 EndGMTicket.
pub fn build_end_gm_ticket(actor_id: u32) -> SubPacket {
    SubPacket::new(OP_END_GM_TICKET, actor_id, body(0x28))
}
