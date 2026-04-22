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

//! Inventory runtime. Faithful port of `Actors/Chara/ItemPackage.cs` +
//! `Actors/Chara/ReferencedItemPackage.cs`.
//!
//! Every mutation records side effects (DB writes, packet emissions) on an
//! `InventoryOutbox` passed by reference, rather than calling the real
//! Database / PacketProcessor inline. This keeps the package pure enough
//! to unit-test and lets the game loop batch everything per-tick.

#![allow(dead_code)]

pub mod outbox;
pub mod referenced;

use crate::data::{InventoryItem, ItemData};

pub use outbox::{InventoryEvent, InventoryOutbox};
pub use referenced::ReferencedItemPackage;

// ---------------------------------------------------------------------------
// Package codes + capacities (1:1 with the `ItemPackage.*` constants).
// ---------------------------------------------------------------------------

pub const PKG_NORMAL: u16 = 0;
pub const PKG_UNKNOWN: u16 = 1;
pub const PKG_LOOT: u16 = 4;
pub const PKG_MELDREQUEST: u16 = 5;
pub const PKG_BAZAAR: u16 = 7;
pub const PKG_CURRENCY_CRYSTALS: u16 = 99;
pub const PKG_KEYITEMS: u16 = 100;
pub const PKG_EQUIPMENT: u16 = 0x00FE;
pub const PKG_TRADE: u16 = 0x00FD;
pub const PKG_EQUIPMENT_OTHERPLAYER: u16 = 0x00F9;

pub const CAP_NORMAL: u16 = 200;
pub const CAP_CURRENCY: u16 = 320;
pub const CAP_KEYITEMS: u16 = 500;
pub const CAP_LOOT: u16 = 10;
pub const CAP_TRADE: u16 = 4;
pub const CAP_MELDREQUEST: u16 = 4;
pub const CAP_BAZAAR: u16 = 10;
pub const CAP_EQUIPMENT: u16 = 35;
pub const CAP_EQUIPMENT_OTHERPLAYER: u16 = 0x23;

/// Default capacity for `code`, matching the switch tables in C#.
pub fn default_capacity(code: u16) -> u16 {
    match code {
        PKG_NORMAL => CAP_NORMAL,
        PKG_CURRENCY_CRYSTALS => CAP_CURRENCY,
        PKG_KEYITEMS => CAP_KEYITEMS,
        PKG_LOOT => CAP_LOOT,
        PKG_TRADE => CAP_TRADE,
        PKG_MELDREQUEST => CAP_MELDREQUEST,
        PKG_BAZAAR => CAP_BAZAAR,
        PKG_EQUIPMENT => CAP_EQUIPMENT,
        PKG_EQUIPMENT_OTHERPLAYER => CAP_EQUIPMENT_OTHERPLAYER,
        _ => CAP_NORMAL,
    }
}

/// Error codes from the C# `ItemPackage.ERROR_*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryError {
    Success = 0,
    Full = 1,
    HasUnique = 2,
    System = 3,
}

pub type InventoryResult = Result<(), InventoryError>;

fn ok() -> InventoryResult {
    Ok(())
}

// ---------------------------------------------------------------------------
// ItemCatalog: caller-provided gamedata view (stack sizes, is_exclusive).
// ---------------------------------------------------------------------------

/// Minimal gamedata accessor the package needs. Real callers hand an
/// `Arc<Catalogs>` reader; tests hand a trivial HashMap or a hand-built
/// enum.
pub trait ItemCatalog {
    fn get(&self, item_id: u32) -> Option<ItemData>;
}

impl ItemCatalog for std::collections::HashMap<u32, ItemData> {
    fn get(&self, item_id: u32) -> Option<ItemData> {
        self.get(&item_id).cloned()
    }
}

// ---------------------------------------------------------------------------
// ItemPackage — the bag itself.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ItemPackage {
    owner_actor_id: u32,
    capacity: u16,
    code: u16,
    is_temporary: bool,

    list: Vec<Option<InventoryItem>>,
    is_dirty: Vec<bool>,
    end_of_list_index: usize,
    holding_updates: bool,
}

impl ItemPackage {
    pub fn new(owner_actor_id: u32, capacity: u16, code: u16) -> Self {
        Self::with_flags(owner_actor_id, capacity, code, false)
    }

    pub fn with_flags(owner_actor_id: u32, capacity: u16, code: u16, is_temporary: bool) -> Self {
        let cap = capacity as usize;
        Self {
            owner_actor_id,
            capacity,
            code,
            is_temporary,
            list: vec![None; cap],
            is_dirty: vec![false; cap],
            end_of_list_index: 0,
            holding_updates: false,
        }
    }

    pub fn owner_actor_id(&self) -> u32 {
        self.owner_actor_id
    }

    pub fn code(&self) -> u16 {
        self.code
    }

    pub fn capacity(&self) -> u16 {
        self.capacity
    }

    pub fn is_temporary(&self) -> bool {
        self.is_temporary
    }

    pub fn raw(&self) -> &[Option<InventoryItem>] {
        &self.list
    }

    pub fn count(&self) -> usize {
        self.end_of_list_index
    }

    pub fn is_full(&self) -> bool {
        self.end_of_list_index >= self.capacity as usize
    }

    pub fn free_slots(&self) -> usize {
        self.capacity as usize - self.end_of_list_index
    }

    pub fn next_empty_slot(&self) -> usize {
        self.end_of_list_index
    }

    // ----- Lookups --------------------------------------------------------

    pub fn get_at_slot(&self, slot: u16) -> Option<&InventoryItem> {
        self.list.get(slot as usize).and_then(|o| o.as_ref())
    }

    pub fn get_by_unique_id(&self, unique_id: u64) -> Option<&InventoryItem> {
        self.iter_items().find(|i| i.unique_id == unique_id)
    }

    pub fn get_by_catalog_id(&self, catalog_id: u32) -> Option<&InventoryItem> {
        self.iter_items().find(|i| i.item_id == catalog_id)
    }

    /// Iterate over live items only (skipping None holes and trailing nulls).
    pub fn iter_items(&self) -> impl Iterator<Item = &InventoryItem> + '_ {
        self.list
            .iter()
            .take(self.end_of_list_index)
            .filter_map(|o| o.as_ref())
    }

    /// Sum quantity of `item_id` at `quality` across all slots.
    fn total_quantity(&self, item_id: u32, quality: u8) -> i64 {
        self.iter_items()
            .filter(|i| i.item_id == item_id && i.quality == quality)
            .map(|i| i.quantity as i64)
            .sum()
    }

    pub fn has_item(&self, item_id: u32) -> bool {
        self.has_item_qty(item_id, 1)
    }

    pub fn has_item_qty(&self, item_id: u32, min_quantity: i32) -> bool {
        self.has_item_qq(item_id, min_quantity, 1)
    }

    pub fn has_item_qq(&self, item_id: u32, min_quantity: i32, quality: u8) -> bool {
        self.total_quantity(item_id, quality) >= min_quantity as i64
    }

    // ----- Capacity checks ------------------------------------------------

    /// Does at least `quantity` of `item_id @ quality` fit? Matches the C#
    /// stack-merging logic plus fallback "room for a new stack".
    pub fn is_space_for_add(
        &self,
        catalog: &impl ItemCatalog,
        item_id: u32,
        quantity: i32,
        quality: u8,
    ) -> bool {
        let Some(gd) = catalog.get(item_id) else {
            return !self.is_full();
        };
        let max_stack = gd.stack_size.max(1) as i32;

        let mut remaining = quantity;
        for item in self.iter_items() {
            if item.item_id == item_id && item.quality == quality && item.quantity < max_stack {
                remaining -= max_stack - item.quantity;
                if remaining <= 0 {
                    return true;
                }
            }
        }
        remaining <= 0 || !self.is_full()
    }

    /// CanAdd for a single existing item.
    pub fn can_add(&self, _item: &InventoryItem) -> bool {
        !self.is_full()
    }

    /// Batched CanAdd — needed by `AddItems` and the trade flow.
    pub fn can_add_many(
        &self,
        catalog: &impl ItemCatalog,
        item_ids: &[u32],
        quantity: Option<&[u32]>,
        quality: Option<&[u8]>,
    ) -> bool {
        let mut temp_size = self.count();
        for (i, &item_id) in item_ids.iter().enumerate() {
            let Some(gd) = catalog.get(item_id) else {
                return false;
            };
            let max_stack = gd.stack_size.max(1) as i32;
            let q_count = quantity.and_then(|q| q.get(i)).copied().unwrap_or(1) as i32;
            let q_quality = quality.and_then(|q| q.get(i)).copied().unwrap_or(1);

            let mut remaining = q_count;
            for item in self.iter_items() {
                if item.item_id == item_id && item.quality == q_quality && item.quantity < max_stack
                {
                    remaining -= max_stack - item.quantity;
                    if remaining <= 0 {
                        break;
                    }
                }
            }
            while remaining > 0 {
                remaining -= max_stack;
                temp_size += 1;
            }
            if temp_size > self.capacity as usize {
                return false;
            }
        }
        true
    }

    // ----- Dirty flags ---------------------------------------------------

    pub fn mark_dirty_slot(&mut self, slot: u16) {
        if let Some(flag) = self.is_dirty.get_mut(slot as usize) {
            *flag = true;
        }
    }

    pub fn mark_dirty(&mut self, item: &InventoryItem) {
        if item.item_package != self.code {
            return;
        }
        if self
            .list
            .get(item.slot as usize)
            .is_some_and(|o| o.is_some())
        {
            self.mark_dirty_slot(item.slot);
        }
    }

    pub fn start_send_update(&mut self) {
        self.holding_updates = true;
    }

    pub fn done_send_update(&mut self, outbox: &mut InventoryOutbox) {
        self.holding_updates = false;
        self.send_update(outbox);
        self.clear_dirty();
    }

    fn clear_dirty(&mut self) {
        for flag in self.is_dirty.iter_mut() {
            *flag = false;
        }
    }

    // ----- Population (from DB / trade) ----------------------------------

    /// One-shot init from a freshly-loaded DB row list. Matches `InitList`.
    pub fn init(&mut self, items: Vec<InventoryItem>) {
        let owner = self.owner_actor_id;
        let code = self.code;
        for (i, mut item) in items.into_iter().enumerate() {
            if i >= self.capacity as usize {
                break;
            }
            item.slot = i as u16;
            item.item_package = code;
            let _ = owner;
            self.list[i] = Some(item);
        }
        self.end_of_list_index = self
            .list
            .iter()
            .position(|o| o.is_none())
            .unwrap_or(self.capacity as usize);
    }

    // ----- Add -----------------------------------------------------------

    /// `AddItem(itemId, quantity, quality)` with full stack-merge.
    ///
    /// Emits:
    ///   - `DbQuantity` for each merged slot
    ///   - `DbAdd` for each spillover slot
    ///   - `PacketBeginChange` + `PacketEndChange` bracket (if owned)
    pub fn add(
        &mut self,
        catalog: &impl ItemCatalog,
        item_id: u32,
        mut quantity: i32,
        quality: u8,
        outbox: &mut InventoryOutbox,
    ) -> InventoryResult {
        if quantity <= 0 {
            return ok();
        }
        if !self.is_space_for_add(catalog, item_id, quantity, quality) {
            return Err(InventoryError::Full);
        }
        let Some(gd) = catalog.get(item_id) else {
            return Err(InventoryError::System);
        };
        if gd.is_exclusive && self.has_item(item_id) {
            return Err(InventoryError::HasUnique);
        }
        let max_stack = gd.stack_size.max(1) as i32;

        // Merge into existing partial stacks.
        for i in 0..self.end_of_list_index {
            if quantity <= 0 {
                break;
            }
            let Some(item) = self.list[i].as_mut() else {
                continue;
            };
            if item.item_id != item_id || item.quality != quality || item.quantity >= max_stack {
                continue;
            }
            let old_qty = item.quantity;
            item.quantity = (old_qty + quantity).min(max_stack);
            self.is_dirty[i] = true;
            quantity -= max_stack - old_qty;
            let unique_id = item.unique_id;
            let new_qty = item.quantity;
            if !self.is_temporary {
                outbox.push(InventoryEvent::DbQuantity {
                    server_item_id: unique_id,
                    quantity: new_qty,
                });
            }
        }

        // Spill into new slots.
        while quantity > 0 {
            if self.end_of_list_index >= self.capacity as usize {
                return Err(InventoryError::Full);
            }
            let stack_qty = quantity.min(max_stack);
            let slot = self.end_of_list_index as u16;
            // The DB insert in CreateItem happens elsewhere; here we record the
            // intent. The `unique_id` on the emitted item will be zero until
            // the game loop pairs the insert.
            let item = InventoryItem {
                unique_id: 0,
                item_id,
                quantity: stack_qty,
                quality,
                slot,
                link_slot: 0xFFFF,
                item_package: self.code,
                tag: Default::default(),
            };
            self.list[self.end_of_list_index] = Some(item.clone());
            self.is_dirty[self.end_of_list_index] = true;
            self.end_of_list_index += 1;
            quantity -= stack_qty;
            if !self.is_temporary {
                outbox.push(InventoryEvent::DbAdd {
                    owner_actor_id: self.owner_actor_id,
                    item,
                    item_package: self.code,
                    slot,
                });
            }
        }

        self.bracketed_send_update(outbox);
        ok()
    }

    /// `AddItem(InventoryItem)`. Used when the item already has a
    /// unique_id (trade, bazaar) — skips CreateItem.
    pub fn add_existing(
        &mut self,
        catalog: &impl ItemCatalog,
        mut item: InventoryItem,
        outbox: &mut InventoryOutbox,
    ) -> InventoryResult {
        let gd = catalog.get(item.item_id);
        let max_stack = gd.as_ref().map(|g| g.stack_size.max(1) as i32).unwrap_or(1);
        if self.code != PKG_BAZAAR && max_stack > 1 {
            return self.add(catalog, item.item_id, item.quantity, item.quality, outbox);
        }
        if !self.is_space_for_add(catalog, item.item_id, item.quantity, item.quality) {
            return Err(InventoryError::Full);
        }
        if gd.is_none() {
            return Err(InventoryError::System);
        }
        item.item_package = self.code;
        item.slot = self.end_of_list_index as u16;
        self.is_dirty[self.end_of_list_index] = true;
        let emitted = item.clone();
        self.list[self.end_of_list_index] = Some(item);
        self.end_of_list_index += 1;
        if !self.is_temporary {
            outbox.push(InventoryEvent::DbAdd {
                owner_actor_id: self.owner_actor_id,
                item: emitted.clone(),
                item_package: self.code,
                slot: emitted.slot,
            });
        }
        self.bracketed_send_update(outbox);
        ok()
    }

    // ----- Remove --------------------------------------------------------

    /// `RemoveItem(itemId, quantity, quality)`.
    pub fn remove(
        &mut self,
        item_id: u32,
        quantity: i32,
        quality: u8,
        outbox: &mut InventoryOutbox,
    ) {
        if !self.has_item_qq(item_id, quantity, quality) {
            return;
        }
        let mut remaining = quantity;
        // C# walks back-to-front so oldest stacks are left intact.
        for i in (0..self.end_of_list_index).rev() {
            if remaining <= 0 {
                break;
            }
            let Some(item) = self.list[i].as_mut() else {
                continue;
            };
            if item.item_id != item_id || item.quality != quality {
                continue;
            }
            let old_qty = item.quantity;
            if old_qty - remaining <= 0 {
                let unique_id = item.unique_id;
                self.list[i] = None;
                self.is_dirty[i] = true;
                if !self.is_temporary {
                    outbox.push(InventoryEvent::DbRemove {
                        owner_actor_id: self.owner_actor_id,
                        server_item_id: unique_id,
                    });
                }
            } else {
                item.quantity -= remaining;
                let (unique_id, new_qty) = (item.unique_id, item.quantity);
                self.is_dirty[i] = true;
                if !self.is_temporary {
                    outbox.push(InventoryEvent::DbQuantity {
                        server_item_id: unique_id,
                        quantity: new_qty,
                    });
                }
            }
            remaining -= old_qty;
        }

        self.realign(outbox);
        self.bracketed_send_update(outbox);
    }

    pub fn remove_item(&mut self, item: &InventoryItem, outbox: &mut InventoryOutbox) {
        if item.item_package == self.code {
            self.remove_at_slot(item.slot, outbox);
        }
    }

    pub fn remove_by_unique_id(
        &mut self,
        item_db_id: u64,
        quantity: i32,
        outbox: &mut InventoryOutbox,
    ) {
        let Some(slot) = self
            .list
            .iter()
            .take(self.end_of_list_index)
            .position(|o| o.as_ref().is_some_and(|i| i.unique_id == item_db_id))
        else {
            return;
        };
        let Some(item) = self.list[slot].as_mut() else {
            return;
        };
        if quantity >= item.quantity {
            let unique_id = item.unique_id;
            self.list[slot] = None;
            self.is_dirty[slot] = true;
            if !self.is_temporary {
                outbox.push(InventoryEvent::DbRemove {
                    owner_actor_id: self.owner_actor_id,
                    server_item_id: unique_id,
                });
            }
        } else {
            item.quantity -= quantity;
            let (unique_id, new_qty) = (item.unique_id, item.quantity);
            self.is_dirty[slot] = true;
            if !self.is_temporary {
                outbox.push(InventoryEvent::DbQuantity {
                    server_item_id: unique_id,
                    quantity: new_qty,
                });
            }
        }
        self.realign(outbox);
        self.bracketed_send_update(outbox);
    }

    pub fn remove_at_slot(&mut self, slot: u16, outbox: &mut InventoryOutbox) {
        let idx = slot as usize;
        if idx >= self.end_of_list_index {
            return;
        }
        if let Some(item) = self.list[idx].take() {
            self.is_dirty[idx] = true;
            if !self.is_temporary {
                outbox.push(InventoryEvent::DbRemove {
                    owner_actor_id: self.owner_actor_id,
                    server_item_id: item.unique_id,
                });
            }
        }
        self.realign(outbox);
        self.bracketed_send_update(outbox);
    }

    pub fn remove_at_slot_qty(&mut self, slot: u16, quantity: i32, outbox: &mut InventoryOutbox) {
        let idx = slot as usize;
        if idx >= self.end_of_list_index {
            return;
        }
        let Some(item) = self.list[idx].as_mut() else {
            return;
        };
        item.quantity -= quantity;
        let (unique_id, remaining) = (item.unique_id, item.quantity);
        if remaining <= 0 {
            self.list[idx] = None;
            self.is_dirty[idx] = true;
            if !self.is_temporary {
                outbox.push(InventoryEvent::DbRemove {
                    owner_actor_id: self.owner_actor_id,
                    server_item_id: unique_id,
                });
            }
            self.realign(outbox);
        } else {
            self.is_dirty[idx] = true;
            if !self.is_temporary {
                outbox.push(InventoryEvent::DbQuantity {
                    server_item_id: unique_id,
                    quantity: remaining,
                });
            }
        }
        self.bracketed_send_update(outbox);
    }

    pub fn clear(&mut self, outbox: &mut InventoryOutbox) {
        for i in 0..self.end_of_list_index {
            if self.list[i].take().is_some() {
                self.is_dirty[i] = true;
            }
        }
        self.end_of_list_index = 0;
        self.bracketed_send_update(outbox);
    }

    // ----- Move ----------------------------------------------------------

    pub fn move_to(
        &mut self,
        catalog: &impl ItemCatalog,
        slot: u16,
        destination: &mut ItemPackage,
        outbox: &mut InventoryOutbox,
    ) -> InventoryResult {
        let Some(item) = self.get_at_slot(slot).cloned() else {
            return Err(InventoryError::System);
        };
        if !destination.can_add(&item) {
            return Err(InventoryError::Full);
        }
        self.remove_at_slot(slot, outbox);
        destination.add_existing(catalog, item, outbox)
    }

    pub fn move_item(
        &mut self,
        catalog: &impl ItemCatalog,
        item: &InventoryItem,
        destination: &mut ItemPackage,
        outbox: &mut InventoryOutbox,
    ) -> InventoryResult {
        if !destination.can_add(item) {
            return Err(InventoryError::Full);
        }
        let snapshot = item.clone();
        self.remove_item(item, outbox);
        destination.add_existing(catalog, snapshot, outbox)
    }

    // ----- Realign (compact holes) ---------------------------------------

    /// Shift live items down to fill any `None` holes inside the packed
    /// region. Matches the C# `DoRealign` + emits `DbPositions` once at end.
    fn realign(&mut self, outbox: &mut InventoryOutbox) {
        let mut position_updates: Vec<InventoryItem> = Vec::new();
        let mut last_null: Option<usize> = None;
        let mut i = 0;
        while i < self.end_of_list_index {
            match (&self.list[i], last_null) {
                (None, None) => {
                    last_null = Some(i);
                }
                (Some(_), Some(dst)) => {
                    let mut moved = self.list[i].take().expect("live slot");
                    moved.slot = dst as u16;
                    self.list[dst] = Some(moved.clone());
                    self.is_dirty[dst] = true;
                    self.is_dirty[i] = true;
                    position_updates.push(moved);
                    last_null = Some(dst + 1);
                }
                _ => {}
            }
            i += 1;
        }
        if let Some(new_end) = last_null {
            self.end_of_list_index = new_end;
        }
        if !position_updates.is_empty() && !self.is_temporary {
            outbox.push(InventoryEvent::DbPositions {
                updates: position_updates,
            });
        }
    }

    // ----- Send-update emission -----------------------------------------

    fn bracketed_send_update(&mut self, outbox: &mut InventoryOutbox) {
        if self.holding_updates {
            return;
        }
        outbox.push(InventoryEvent::PacketBeginChange {
            owner_actor_id: self.owner_actor_id,
        });
        self.send_update(outbox);
        outbox.push(InventoryEvent::PacketEndChange {
            owner_actor_id: self.owner_actor_id,
        });
    }

    /// Build the `InventorySetBegin` → items → remove-tail → `InventorySetEnd`
    /// sequence.  Mirrors the C# `SendUpdate` with identical batching.
    pub fn send_update(&mut self, outbox: &mut InventoryOutbox) {
        if self.holding_updates {
            return;
        }

        let mut dirty_items: Vec<InventoryItem> = Vec::new();
        let mut dirty_tail_slots: Vec<u16> = Vec::new();

        for i in 0..self.end_of_list_index {
            if self.is_dirty[i]
                && let Some(item) = &self.list[i]
            {
                dirty_items.push(item.clone());
            }
        }
        for i in self.end_of_list_index..self.capacity as usize {
            if self.is_dirty[i] {
                dirty_tail_slots.push(i as u16);
            }
        }

        outbox.push(InventoryEvent::PacketSetBegin {
            owner_actor_id: self.owner_actor_id,
            capacity: self.capacity,
            code: self.code,
        });
        if !dirty_items.is_empty() {
            outbox.push(InventoryEvent::PacketItems {
                owner_actor_id: self.owner_actor_id,
                items: dirty_items,
            });
        }
        if !dirty_tail_slots.is_empty() {
            outbox.push(InventoryEvent::PacketRemoveSlots {
                owner_actor_id: self.owner_actor_id,
                slots: dirty_tail_slots,
            });
        }
        outbox.push(InventoryEvent::PacketSetEnd {
            owner_actor_id: self.owner_actor_id,
        });

        self.clear_dirty();
    }

    /// Emit a full snapshot of the package; used on login + zone change.
    pub fn send_full(&self, outbox: &mut InventoryOutbox) {
        outbox.push(InventoryEvent::PacketSetBegin {
            owner_actor_id: self.owner_actor_id,
            capacity: self.capacity,
            code: self.code,
        });
        let items: Vec<InventoryItem> = self.iter_items().cloned().collect();
        if !items.is_empty() {
            outbox.push(InventoryEvent::PacketItems {
                owner_actor_id: self.owner_actor_id,
                items,
            });
        }
        outbox.push(InventoryEvent::PacketSetEnd {
            owner_actor_id: self.owner_actor_id,
        });
    }
}

impl std::fmt::Display for ItemPackage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self.code {
            PKG_NORMAL => "Inventory",
            PKG_LOOT => "Loot",
            PKG_MELDREQUEST => "Meld Request",
            PKG_BAZAAR => "Bazaar",
            PKG_CURRENCY_CRYSTALS => "Currency",
            PKG_KEYITEMS => "KeyItems",
            PKG_EQUIPMENT => "Equipment",
            PKG_TRADE => "Trade",
            PKG_EQUIPMENT_OTHERPLAYER => "CheckEquip",
            _ => "Unknown",
        };
        write!(f, "{name} Package")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn catalog_with(id: u32, stack: u32, exclusive: bool) -> HashMap<u32, ItemData> {
        let mut m = HashMap::new();
        m.insert(
            id,
            ItemData {
                id,
                stack_size: stack,
                is_exclusive: exclusive,
                ..Default::default()
            },
        );
        m
    }

    #[test]
    fn add_merges_into_existing_stack() {
        let mut bag = ItemPackage::new(1, 10, PKG_NORMAL);
        let catalog = catalog_with(42, 99, false);
        let mut outbox = InventoryOutbox::new();

        bag.add(&catalog, 42, 30, 1, &mut outbox).unwrap();
        bag.add(&catalog, 42, 40, 1, &mut outbox).unwrap();

        assert_eq!(bag.count(), 1);
        assert_eq!(bag.get_at_slot(0).unwrap().quantity, 70);
    }

    #[test]
    fn add_spills_when_stack_full() {
        let mut bag = ItemPackage::new(1, 10, PKG_NORMAL);
        let catalog = catalog_with(42, 99, false);
        let mut outbox = InventoryOutbox::new();

        bag.add(&catalog, 42, 150, 1, &mut outbox).unwrap();

        assert_eq!(bag.count(), 2);
        assert_eq!(bag.get_at_slot(0).unwrap().quantity, 99);
        assert_eq!(bag.get_at_slot(1).unwrap().quantity, 51);
    }

    #[test]
    fn add_returns_full_when_over_capacity() {
        let mut bag = ItemPackage::new(1, 1, PKG_NORMAL);
        let catalog = catalog_with(42, 10, false);
        let mut outbox = InventoryOutbox::new();

        bag.add(&catalog, 42, 10, 1, &mut outbox).unwrap();
        assert_eq!(
            bag.add(&catalog, 43, 1, 1, &mut outbox),
            Err(InventoryError::Full)
        );
    }

    #[test]
    fn unique_items_reject_duplicate() {
        let mut bag = ItemPackage::new(1, 10, PKG_NORMAL);
        let catalog = catalog_with(42, 1, true);
        let mut outbox = InventoryOutbox::new();

        bag.add(&catalog, 42, 1, 1, &mut outbox).unwrap();
        assert_eq!(
            bag.add(&catalog, 42, 1, 1, &mut outbox),
            Err(InventoryError::HasUnique)
        );
    }

    #[test]
    fn remove_walks_back_to_front() {
        let mut bag = ItemPackage::new(1, 10, PKG_NORMAL);
        let catalog = catalog_with(42, 50, false);
        let mut outbox = InventoryOutbox::new();

        bag.add(&catalog, 42, 120, 1, &mut outbox).unwrap();
        // Stacks: 50, 50, 20
        assert_eq!(bag.count(), 3);
        bag.remove(42, 25, 1, &mut outbox);
        // Last stack should be gone (was 20) and next-to-last dropped by 5.
        assert_eq!(bag.get_at_slot(0).unwrap().quantity, 50);
        assert_eq!(bag.get_at_slot(1).unwrap().quantity, 45);
        assert_eq!(bag.count(), 2);
    }

    #[test]
    fn realign_compacts_holes() {
        let mut bag = ItemPackage::new(1, 4, PKG_NORMAL);
        let catalog = catalog_with(42, 10, false);
        let mut outbox = InventoryOutbox::new();

        bag.add(&catalog, 42, 10, 1, &mut outbox).unwrap();
        bag.add(&catalog, 42, 10, 1, &mut outbox).unwrap();
        bag.add(&catalog, 42, 10, 1, &mut outbox).unwrap();
        assert_eq!(bag.count(), 3);

        bag.remove_at_slot(1, &mut outbox);
        // Slot 2 should realign into slot 1; count should still report 2.
        assert_eq!(bag.count(), 2);
        assert!(bag.get_at_slot(0).is_some());
        assert!(bag.get_at_slot(1).is_some());
        assert_eq!(bag.get_at_slot(1).unwrap().slot, 1);
    }

    #[test]
    fn has_item_respects_quality() {
        let mut bag = ItemPackage::new(1, 4, PKG_NORMAL);
        let catalog = catalog_with(42, 10, false);
        let mut outbox = InventoryOutbox::new();

        bag.add(&catalog, 42, 5, 1, &mut outbox).unwrap();
        assert!(bag.has_item_qq(42, 5, 1));
        assert!(!bag.has_item_qq(42, 5, 2));
    }
}
