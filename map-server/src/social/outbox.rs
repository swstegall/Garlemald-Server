//! Events emitted by chat + social + recruitment + support-desk
//! mutations. The dispatcher drains the outbox and turns each into
//! the right packet send.

#![allow(dead_code)]

use super::chat::ChatKind;

#[derive(Debug, Clone)]
pub enum SocialEvent {
    // ---- Chat ----------------------------------------------------------
    /// Broadcast chat to neighbours (say/shout/yell). Fan-out uses the
    /// spatial grid, not the group registry.
    ChatBroadcast {
        source_actor_id: u32,
        kind: ChatKind,
        sender_name: String,
        message: String,
    },
    /// Private message — one target receives it. `target_actor_id` is
    /// the resolved recipient; `target_name` is what the client shows.
    ChatTell {
        source_actor_id: u32,
        target_actor_id: u32,
        sender_name: String,
        message: String,
    },
    /// Party chat — fan to all party members.
    ChatParty {
        source_actor_id: u32,
        party_id: u64,
        sender_name: String,
        message: String,
    },
    /// Linkshell chat — fan to all members of a specific linkshell id.
    ChatLinkshell {
        source_actor_id: u32,
        linkshell_id: u64,
        sender_name: String,
        message: String,
    },
    /// System notice — one player sees it (white or red per `kind`).
    ChatSystemToPlayer {
        target_actor_id: u32,
        kind: ChatKind,
        message: String,
    },

    // ---- Friendlist / blacklist ---------------------------------------
    FriendlistAdded {
        actor_id: u32,
        friend_character_id: u64,
        name: String,
        success: bool,
        is_online: bool,
    },
    FriendlistRemoved {
        actor_id: u32,
        name: String,
        success: bool,
    },
    FriendlistSend {
        actor_id: u32,
        entries: Vec<(i64, String)>,
    },
    FriendStatus {
        actor_id: u32,
        entries: Vec<(i64, bool)>,
    },
    BlacklistAdded {
        actor_id: u32,
        name: String,
        success: bool,
    },
    BlacklistRemoved {
        actor_id: u32,
        name: String,
        success: bool,
    },
    BlacklistSend {
        actor_id: u32,
        names: Vec<String>,
    },

    // ---- Recruitment --------------------------------------------------
    RecruitingStarted {
        actor_id: u32,
        success: bool,
    },
    RecruitingEnded {
        actor_id: u32,
    },
    RecruiterStateQueried {
        actor_id: u32,
        is_recruiter: bool,
        is_recruiting: bool,
        total_recruiters: u32,
    },
    RecruitmentDetailsSent {
        actor_id: u32,
        recruiter_name: String,
        purpose_id: u8,
        location_id: u8,
        sub_task_id: u8,
        comment: String,
    },

    // ---- Support desk --------------------------------------------------
    FaqListRequested {
        actor_id: u32,
        faqs: Vec<String>,
    },
    FaqBodyRequested {
        actor_id: u32,
        body: String,
    },
    SupportIssueListRequested {
        actor_id: u32,
        issues: Vec<String>,
    },
    GmTicketStartQueried {
        actor_id: u32,
        is_active: bool,
    },
    GmTicketResponseQueried {
        actor_id: u32,
        title: String,
        body: String,
    },
    GmTicketSent {
        actor_id: u32,
        accepted: bool,
    },
    GmTicketEnded {
        actor_id: u32,
    },
}

#[derive(Debug, Default)]
pub struct SocialOutbox {
    pub events: Vec<SocialEvent>,
}

impl SocialOutbox {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, event: SocialEvent) {
        self.events.push(event);
    }

    pub fn drain(&mut self) -> Vec<SocialEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }
}
