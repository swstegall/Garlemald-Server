//! Group type ids + shared value-types. Matches the constants in the
//! base `Group.cs`.

#![allow(dead_code)]

/// Retail group type-id constants. These are the values the client
/// expects in the group-header packet's `typeId` field — they're
/// load-bearing for UI dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GroupTypeId(pub u32);

impl GroupTypeId {
    pub const NONE: Self = Self(0);

    // Party / company.
    pub const PARTY: Self = Self(10001);
    pub const COMPANY: Self = Self(20002);
    pub const MONSTER_PARTY: Self = Self(10002);

    // Relation groups.
    pub const GROUP_INVITATION: Self = Self(50001);
    pub const TRADE_RELATION: Self = Self(50002);
    pub const BAZAAR_BUY_ITEM: Self = Self(50009);

    // Retainers.
    pub const RETAINER: Self = Self(80001);

    // Content groups.
    pub const CONTENT_GUILDLEVE: Self = Self(30001);
    pub const CONTENT_PUBLIC_POP: Self = Self(30002);
    pub const CONTENT_SIMPLE_24A: Self = Self(30003);
    pub const CONTENT_SIMPLE_32A: Self = Self(30004);
    pub const CONTENT_SIMPLE_128: Self = Self(30005);
    pub const CONTENT_SIMPLE_24B: Self = Self(30006);
    pub const CONTENT_SIMPLE_32B: Self = Self(30007);
    pub const CONTENT_RETAINER_ACCESS: Self = Self(30008);
    pub const CONTENT_SIMPLE_99999: Self = Self(30009);
    pub const CONTENT_SIMPLE_512: Self = Self(30010);
    pub const CONTENT_SIMPLE_64A: Self = Self(30011);

    pub const fn bits(self) -> u32 {
        self.0
    }
}

/// Broad categorisation matching the C# class hierarchy.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupKind {
    Party,
    Content,
    Relation,
    Trade,
    Retainer,
    Monster,
}

/// One row of the group-member list that goes out in the wire packets.
/// `name` is empty for content groups (the retail server omits names
/// for instance rosters).
#[derive(Debug, Clone, Default)]
pub struct GroupMemberRef {
    pub actor_id: u32,
    pub class_id: i16,
    pub level: u16,
    pub is_leader: bool,
    pub is_online: bool,
    pub name: String,
}

impl GroupMemberRef {
    pub fn new(actor_id: u32, is_online: bool, name: impl Into<String>) -> Self {
        Self {
            actor_id,
            class_id: -1,
            level: 0,
            is_leader: false,
            is_online,
            name: name.into(),
        }
    }
}

/// Max players in a `Party` — matches retail.
pub const PARTY_MAX_MEMBERS: usize = 8;

/// `variableCommand` value the `TradeGroup` ctor stamps on its
/// RelationWork — used by the client to distinguish trade from a
/// generic relation.
pub const RELATION_COMMAND_TRADE: u32 = 30_001;

/// Which chunk packet to emit at a given fill level. Mirrors the
/// `if >=64 … else if >=32 …` ladder in the C# `SendGroupPackets`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkBucket {
    None,
    X08,
    X16,
    X32,
    X64,
}

/// Route `remaining` members into the right chunking packet. `None`
/// means "stop sending".
pub fn chunk_bucket(remaining: usize) -> ChunkBucket {
    if remaining >= 64 {
        ChunkBucket::X64
    } else if remaining >= 32 {
        ChunkBucket::X32
    } else if remaining >= 16 {
        ChunkBucket::X16
    } else if remaining > 0 {
        ChunkBucket::X08
    } else {
        ChunkBucket::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_bucket_routing() {
        assert_eq!(chunk_bucket(0), ChunkBucket::None);
        assert_eq!(chunk_bucket(1), ChunkBucket::X08);
        assert_eq!(chunk_bucket(8), ChunkBucket::X08);
        assert_eq!(chunk_bucket(15), ChunkBucket::X08);
        assert_eq!(chunk_bucket(16), ChunkBucket::X16);
        assert_eq!(chunk_bucket(31), ChunkBucket::X16);
        assert_eq!(chunk_bucket(32), ChunkBucket::X32);
        assert_eq!(chunk_bucket(63), ChunkBucket::X32);
        assert_eq!(chunk_bucket(64), ChunkBucket::X64);
        assert_eq!(chunk_bucket(128), ChunkBucket::X64);
    }

    #[test]
    fn group_type_id_constants() {
        assert_eq!(GroupTypeId::PARTY.bits(), 10_001);
        assert_eq!(GroupTypeId::CONTENT_GUILDLEVE.bits(), 30_001);
        assert_eq!(GroupTypeId::TRADE_RELATION.bits(), 50_002);
    }
}
