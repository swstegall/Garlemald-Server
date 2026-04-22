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

//! Stubbed support-desk responses. Matches the C# `PacketProcessor`
//! canned replies so the "help" UI resolves.

#![allow(dead_code)]

use super::outbox::{SocialEvent, SocialOutbox};

pub const CANNED_FAQ_TITLES: &[&str] = &["Testing FAQ1", "Coded style!"];
pub const CANNED_FAQ_BODY: &str = "HERE IS A GIANT BODY. Nothing else to say!";
pub const CANNED_ISSUES: &[&str] = &["Test1", "Test2", "Test3", "Test4", "Test5"];
pub const CANNED_GM_TITLE: &str = "This is a GM Ticket Title";
pub const CANNED_GM_BODY: &str = "This is a GM Ticket Body.";

pub fn emit_faq_list(actor_id: u32, outbox: &mut SocialOutbox) {
    outbox.push(SocialEvent::FaqListRequested {
        actor_id,
        faqs: CANNED_FAQ_TITLES.iter().map(|s| s.to_string()).collect(),
    });
}

pub fn emit_faq_body(actor_id: u32, outbox: &mut SocialOutbox) {
    outbox.push(SocialEvent::FaqBodyRequested {
        actor_id,
        body: CANNED_FAQ_BODY.to_string(),
    });
}

pub fn emit_issue_list(actor_id: u32, outbox: &mut SocialOutbox) {
    outbox.push(SocialEvent::SupportIssueListRequested {
        actor_id,
        issues: CANNED_ISSUES.iter().map(|s| s.to_string()).collect(),
    });
}

pub fn emit_gm_ticket_state(actor_id: u32, is_active: bool, outbox: &mut SocialOutbox) {
    outbox.push(SocialEvent::GmTicketStartQueried {
        actor_id,
        is_active,
    });
}

pub fn emit_gm_ticket_response(actor_id: u32, outbox: &mut SocialOutbox) {
    outbox.push(SocialEvent::GmTicketResponseQueried {
        actor_id,
        title: CANNED_GM_TITLE.to_string(),
        body: CANNED_GM_BODY.to_string(),
    });
}

pub fn emit_gm_ticket_sent(actor_id: u32, accepted: bool, outbox: &mut SocialOutbox) {
    outbox.push(SocialEvent::GmTicketSent { actor_id, accepted });
}

pub fn emit_gm_ticket_ended(actor_id: u32, outbox: &mut SocialOutbox) {
    outbox.push(SocialEvent::GmTicketEnded { actor_id });
}
