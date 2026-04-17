//! `NpcWork` — transient per-NPC state. Port of
//! `Actors/Chara/Npc/NpcWork.cs`.

#![allow(dead_code)]

pub const HATE_TYPE_NONE: u8 = 0;
pub const HATE_TYPE_ENGAGED: u8 = 2;
pub const HATE_TYPE_ENGAGED_PARTY: u8 = 3;

#[derive(Debug, Clone)]
pub struct NpcWork {
    pub push_command: u16,
    pub push_command_sub: i32,
    pub push_command_priority: u8,
    /// Defaults to `1` — matches the C# initializer. Actors flip to
    /// `HATE_TYPE_ENGAGED` / `HATE_TYPE_ENGAGED_PARTY` when they grab a
    /// target.
    pub hate_type: u8,
}

impl Default for NpcWork {
    fn default() -> Self {
        Self {
            push_command: 0,
            push_command_sub: 0,
            push_command_priority: 0,
            hate_type: 1,
        }
    }
}

impl NpcWork {
    pub fn new_from_class(push: u16, push_sub: u16, priority: u8) -> Self {
        Self {
            push_command: push,
            push_command_sub: push_sub as i32,
            push_command_priority: priority,
            hate_type: 1,
        }
    }
}
