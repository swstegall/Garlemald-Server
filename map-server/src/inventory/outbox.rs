//! `InventoryOutbox` — the event sink ItemPackage writes into instead of
//! calling Database / PacketProcessor directly.
//!
//! In the C# server, every mutation on `ItemPackage` calls back into the
//! packet queue (`QueuePacket(...)`) and `Database.CreateItem/AddItem/...`
//! inline. Porting that verbatim would either (a) require an
//! `Arc<Database>` + outbound `mpsc::Sender` on every package instance, or
//! (b) drag async methods into what should be pure state mutation.
//!
//! Instead, we mutate the package synchronously and append typed events to
//! an `InventoryOutbox`. The map-server game loop drains that at the end of
//! each tick and fans the events out to DB writes + packet sends. Pure and
//! unit-testable.

#![allow(dead_code)]

use crate::data::InventoryItem;

/// One side effect produced by an `ItemPackage` mutation.
#[derive(Debug, Clone)]
pub enum InventoryEvent {
    /// `Database.CreateItem` already happened on the DB side; this records
    /// the row-level `characters_inventory` insert.
    DbAdd { owner_actor_id: u32, item: InventoryItem, item_package: u16, slot: u16 },
    /// `Database.RemoveItem`.
    DbRemove { owner_actor_id: u32, server_item_id: u64 },
    /// `Database.SetQuantity`.
    DbQuantity { server_item_id: u64, quantity: i32 },
    /// `Database.UpdateItemPositions` — batched after a realign.
    DbPositions { updates: Vec<InventoryItem> },
    /// `Database.EquipItem`.
    DbEquip { owner_actor_id: u32, equip_slot: u16, unique_item_id: u64 },
    /// `Database.UnequipItem`.
    DbUnequip { owner_actor_id: u32, equip_slot: u16 },

    /// `InventoryBeginChangePacket`.
    PacketBeginChange { owner_actor_id: u32 },
    /// `InventoryEndChangePacket`.
    PacketEndChange { owner_actor_id: u32 },
    /// `InventorySetBeginPacket`.
    PacketSetBegin { owner_actor_id: u32, capacity: u16, code: u16 },
    /// `InventorySetEndPacket`.
    PacketSetEnd { owner_actor_id: u32 },
    /// Batched items → one of InventoryListX01/08/16/32/64 (the game loop
    /// picks the right opcode by length).
    PacketItems { owner_actor_id: u32, items: Vec<InventoryItem> },
    /// Batched slot indices → one of InventoryRemoveX01/08/16/32/64.
    PacketRemoveSlots { owner_actor_id: u32, slots: Vec<u16> },
    /// Equipment update. Caller chooses between full resend and single-slot.
    PacketLinkedSingle { owner_actor_id: u32, position: u16, item: Option<InventoryItem> },
    /// Full linked-item resend; the sink splits into LinkedItemListX01/08/16/32/64.
    PacketLinkedMany { owner_actor_id: u32, items: Vec<(u16, InventoryItem)> },
}

/// Collector. Mutation methods on ItemPackage take `&mut InventoryOutbox` and
/// append to `events`. Callers drain it once per tick.
#[derive(Debug, Default)]
pub struct InventoryOutbox {
    pub events: Vec<InventoryEvent>,
}

impl InventoryOutbox {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, event: InventoryEvent) {
        self.events.push(event);
    }

    pub fn drain(&mut self) -> Vec<InventoryEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Wrap a closure in the client's inventory-change bracket
    /// (`PacketBeginChange` / `PacketEndChange`). Mirrors the C# sugar
    /// around `SendUpdate`.
    pub fn with_change_bracket<F: FnOnce(&mut Self)>(&mut self, owner: u32, f: F) {
        self.push(InventoryEvent::PacketBeginChange { owner_actor_id: owner });
        f(self);
        self.push(InventoryEvent::PacketEndChange { owner_actor_id: owner });
    }
}
