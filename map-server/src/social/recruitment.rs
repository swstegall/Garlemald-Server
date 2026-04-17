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
