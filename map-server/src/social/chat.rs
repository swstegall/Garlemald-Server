//! Chat-channel constants + routing helpers. Matches the retail
//! `MESSAGE_TYPE_*` ids on `SendMessagePacket`.

#![allow(dead_code)]

pub const CHAT_SAY: u8 = 0x01;
pub const CHAT_SHOUT: u8 = 0x02;
pub const CHAT_TELL: u8 = 0x03;
pub const CHAT_PARTY: u8 = 0x04;
pub const CHAT_LS: u8 = 0x05;
pub const CHAT_YELL: u8 = 0x1D;
pub const CHAT_SYSTEM: u8 = 0x20;
pub const CHAT_SYSTEM_ERROR: u8 = 0x21;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatKind {
    Say = 0x01,
    Shout = 0x02,
    Tell = 0x03,
    Party = 0x04,
    Linkshell = 0x05,
    Yell = 0x1D,
    System = 0x20,
    SystemError = 0x21,
    Unknown,
}

/// Convert a wire-format log type (comes in as a u32 in
/// `ChatMessagePacket`) into a typed `ChatKind`. Unknown values fall
/// through to `ChatKind::Unknown`.
pub fn message_type_from_u32(log_type: u32) -> ChatKind {
    match log_type as u8 {
        CHAT_SAY => ChatKind::Say,
        CHAT_SHOUT => ChatKind::Shout,
        CHAT_TELL => ChatKind::Tell,
        CHAT_PARTY => ChatKind::Party,
        CHAT_LS => ChatKind::Linkshell,
        CHAT_YELL => ChatKind::Yell,
        CHAT_SYSTEM => ChatKind::System,
        CHAT_SYSTEM_ERROR => ChatKind::SystemError,
        _ => ChatKind::Unknown,
    }
}

impl ChatKind {
    pub fn as_u8(self) -> u8 {
        match self {
            ChatKind::Say => CHAT_SAY,
            ChatKind::Shout => CHAT_SHOUT,
            ChatKind::Tell => CHAT_TELL,
            ChatKind::Party => CHAT_PARTY,
            ChatKind::Linkshell => CHAT_LS,
            ChatKind::Yell => CHAT_YELL,
            ChatKind::System => CHAT_SYSTEM,
            ChatKind::SystemError => CHAT_SYSTEM_ERROR,
            ChatKind::Unknown => 0,
        }
    }

    /// Does this kind broadcast to spatial neighbours (Say/Shout/Yell)?
    pub fn is_spatial(self) -> bool {
        matches!(self, ChatKind::Say | ChatKind::Shout | ChatKind::Yell)
    }

    /// Does this kind route through a group (Party/Linkshell)?
    pub fn is_group(self) -> bool {
        matches!(self, ChatKind::Party | ChatKind::Linkshell)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_type_roundtrip() {
        for lt in [
            CHAT_SAY,
            CHAT_SHOUT,
            CHAT_TELL,
            CHAT_PARTY,
            CHAT_LS,
            CHAT_YELL,
            CHAT_SYSTEM,
            CHAT_SYSTEM_ERROR,
        ] {
            let k = message_type_from_u32(lt as u32);
            assert_eq!(k.as_u8(), lt);
        }
    }

    #[test]
    fn unknown_log_type() {
        assert_eq!(message_type_from_u32(0xFF), ChatKind::Unknown);
    }

    #[test]
    fn routing_predicates() {
        assert!(ChatKind::Say.is_spatial());
        assert!(!ChatKind::Say.is_group());
        assert!(ChatKind::Party.is_group());
        assert!(!ChatKind::Tell.is_spatial());
        assert!(!ChatKind::Tell.is_group());
    }
}
