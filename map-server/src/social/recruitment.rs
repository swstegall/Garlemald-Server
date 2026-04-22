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

//! Stubbed recruitment responses. The C# `PacketProcessor` mostly
//! returns canned data; we mirror that here so the client's "party
//! finder" UI resolves without crashing.

#![allow(dead_code)]

use super::outbox::{SocialEvent, SocialOutbox};

pub struct CannedRecruitmentDetails {
    pub recruiter_name: &'static str,
    pub purpose_id: u8,
    pub location_id: u8,
    pub sub_task_id: u8,
    pub comment: &'static str,
}

/// Retail-equivalent static response for `GetRecruitmentDetails`.
pub const CANNED_RECRUITMENT_DETAILS: CannedRecruitmentDetails = CannedRecruitmentDetails {
    recruiter_name: "Localhost Character",
    purpose_id: 2,
    location_id: 1,
    sub_task_id: 1,
    comment: "This is a test details packet sent by the server. No implementation has been Created yet...",
};

/// Queue all four recruitment response helpers in sequence.
pub fn emit_canned_details(actor_id: u32, outbox: &mut SocialOutbox) {
    outbox.push(SocialEvent::RecruitmentDetailsSent {
        actor_id,
        recruiter_name: CANNED_RECRUITMENT_DETAILS.recruiter_name.to_string(),
        purpose_id: CANNED_RECRUITMENT_DETAILS.purpose_id,
        location_id: CANNED_RECRUITMENT_DETAILS.location_id,
        sub_task_id: CANNED_RECRUITMENT_DETAILS.sub_task_id,
        comment: CANNED_RECRUITMENT_DETAILS.comment.to_string(),
    });
}
