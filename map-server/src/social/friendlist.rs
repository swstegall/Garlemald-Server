//! Per-player friendlist + blacklist state. Retail keeps these as
//! small lists with name + id + online flag.

#![allow(dead_code)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendlistEntry {
    pub character_id: u64,
    pub name: String,
    pub is_online: bool,
}

impl FriendlistEntry {
    pub fn new(character_id: u64, name: impl Into<String>, is_online: bool) -> Self {
        Self {
            character_id,
            name: name.into(),
            is_online,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlacklistEntry {
    pub name: String,
}

impl BlacklistEntry {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}
