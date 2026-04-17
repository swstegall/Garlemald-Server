//! `ReferencedItemPackage` — the equipment view.
//!
//! Unlike `ItemPackage`, this doesn't own items. Each slot holds a reference
//! (by unique_id + a cloned snapshot of the row we last saw) to an item that
//! lives in the normal inventory package. Setting a slot emits
//! `DbEquip`/`DbUnequip` events; the in-memory source package is expected
//! to `mark_dirty` the referenced item so its resend reflects the new link.

#![allow(dead_code)]

use crate::data::InventoryItem;

use super::outbox::{InventoryEvent, InventoryOutbox};
use super::{ItemPackage, PKG_EQUIPMENT};

#[derive(Debug, Clone)]
pub struct ReferencedItemPackage {
    owner_actor_id: u32,
    capacity: u16,
    code: u16,
    write_to_db: bool,
    reference_list: Vec<Option<InventoryItem>>,
}

impl ReferencedItemPackage {
    pub fn new(owner_actor_id: u32, capacity: u16, code: u16) -> Self {
        let write_to_db = code == PKG_EQUIPMENT;
        Self {
            owner_actor_id,
            capacity,
            code,
            write_to_db,
            reference_list: vec![None; capacity as usize],
        }
    }

    pub fn toggle_db_write(&mut self, flag: bool) {
        self.write_to_db = flag;
    }

    pub fn code(&self) -> u16 {
        self.code
    }

    pub fn capacity(&self) -> u16 {
        self.capacity
    }

    pub fn get_at_slot(&self, slot: u16) -> Option<&InventoryItem> {
        self.reference_list.get(slot as usize).and_then(|o| o.as_ref())
    }

    /// Bulk-replace the whole equipment vector (used right after
    /// `Database.GetEquipment`).
    pub fn set_list(&mut self, items: &[Option<InventoryItem>]) {
        let n = items.len().min(self.reference_list.len());
        self.reference_list[..n].clone_from_slice(&items[..n]);
    }

    /// `Set(ushort[], ushort[], ushort)` batched variant.
    pub fn set_many(
        &mut self,
        assignments: &[(u16, InventoryItem)],
        outbox: &mut InventoryOutbox,
    ) {
        for (position, item) in assignments {
            self.set_internal(*position, item.clone(), outbox, /* send_single */ false);
        }
        outbox.push(InventoryEvent::PacketBeginChange { owner_actor_id: self.owner_actor_id });
        self.send_update(outbox);
        outbox.push(InventoryEvent::PacketEndChange { owner_actor_id: self.owner_actor_id });
    }

    /// `Set(ushort position, InventoryItem item)` — equip one item.
    /// Side packages' mutations must be forwarded by the caller: we emit
    /// a `PacketLinkedSingle` and a DbEquip, and mark the source slot in
    /// the caller-supplied `source_package` dirty so its next SendUpdate
    /// reflects the move.
    pub fn set_with_source(
        &mut self,
        position: u16,
        item: InventoryItem,
        source_package: &mut ItemPackage,
        outbox: &mut InventoryOutbox,
    ) {
        let old_source_code = self
            .reference_list
            .get(position as usize)
            .and_then(|o| o.as_ref())
            .map(|i| i.item_package);

        self.set_internal(position, item.clone(), outbox, /* send_single */ false);

        // Mark new source slot dirty so its resend reflects the link flag.
        source_package.mark_dirty(&item);

        outbox.push(InventoryEvent::PacketBeginChange { owner_actor_id: self.owner_actor_id });
        if old_source_code == Some(source_package.code()) {
            source_package.send_update(outbox);
        }
        self.send_single_update(position, outbox);
        outbox.push(InventoryEvent::PacketEndChange { owner_actor_id: self.owner_actor_id });
    }

    pub fn set(
        &mut self,
        position: u16,
        item: InventoryItem,
        outbox: &mut InventoryOutbox,
    ) {
        self.set_internal(position, item, outbox, /* send_single */ true);
    }

    fn set_internal(
        &mut self,
        position: u16,
        item: InventoryItem,
        outbox: &mut InventoryOutbox,
        send_single: bool,
    ) {
        if position as usize >= self.reference_list.len() {
            return;
        }
        if self.write_to_db {
            outbox.push(InventoryEvent::DbEquip {
                owner_actor_id: self.owner_actor_id,
                equip_slot: position,
                unique_item_id: item.unique_id,
            });
        }
        self.reference_list[position as usize] = Some(item);
        if send_single {
            outbox.push(InventoryEvent::PacketBeginChange { owner_actor_id: self.owner_actor_id });
            self.send_single_update(position, outbox);
            outbox.push(InventoryEvent::PacketEndChange { owner_actor_id: self.owner_actor_id });
        }
    }

    pub fn clear(&mut self, position: u16, outbox: &mut InventoryOutbox) {
        if position as usize >= self.reference_list.len() {
            return;
        }
        if self.write_to_db {
            outbox.push(InventoryEvent::DbUnequip {
                owner_actor_id: self.owner_actor_id,
                equip_slot: position,
            });
        }
        self.reference_list[position as usize] = None;
        outbox.push(InventoryEvent::PacketBeginChange { owner_actor_id: self.owner_actor_id });
        self.send_single_update(position, outbox);
        outbox.push(InventoryEvent::PacketEndChange { owner_actor_id: self.owner_actor_id });
    }

    pub fn clear_all(&mut self, outbox: &mut InventoryOutbox) {
        for i in 0..self.reference_list.len() {
            if self.reference_list[i].is_some() {
                if self.write_to_db {
                    outbox.push(InventoryEvent::DbUnequip {
                        owner_actor_id: self.owner_actor_id,
                        equip_slot: i as u16,
                    });
                }
                self.reference_list[i] = None;
            }
        }
        outbox.push(InventoryEvent::PacketBeginChange { owner_actor_id: self.owner_actor_id });
        self.send_update(outbox);
        outbox.push(InventoryEvent::PacketEndChange { owner_actor_id: self.owner_actor_id });
    }

    pub fn send_single_update(&self, position: u16, outbox: &mut InventoryOutbox) {
        outbox.push(InventoryEvent::PacketSetBegin {
            owner_actor_id: self.owner_actor_id,
            capacity: self.capacity,
            code: self.code,
        });
        let slot = position as usize;
        let item = self.reference_list.get(slot).and_then(|o| o.clone());
        outbox.push(InventoryEvent::PacketLinkedSingle {
            owner_actor_id: self.owner_actor_id,
            position,
            item,
        });
        outbox.push(InventoryEvent::PacketSetEnd { owner_actor_id: self.owner_actor_id });
    }

    pub fn send_update(&self, outbox: &mut InventoryOutbox) {
        let items: Vec<(u16, InventoryItem)> = self
            .reference_list
            .iter()
            .enumerate()
            .filter_map(|(i, o)| o.as_ref().map(|it| (i as u16, it.clone())))
            .collect();

        outbox.push(InventoryEvent::PacketSetBegin {
            owner_actor_id: self.owner_actor_id,
            capacity: self.capacity,
            code: self.code,
        });
        if !items.is_empty() {
            outbox.push(InventoryEvent::PacketLinkedMany {
                owner_actor_id: self.owner_actor_id,
                items,
            });
        }
        outbox.push(InventoryEvent::PacketSetEnd { owner_actor_id: self.owner_actor_id });
    }

    /// Mirror of `SendUpdateAsItemPackage` — used when examining a peer's
    /// equipment; we forward the refs as if they were a real item package.
    pub fn send_update_as_item_package(
        &self,
        destination_capacity: u16,
        destination_code: u16,
        outbox: &mut InventoryOutbox,
    ) {
        let mut items = Vec::new();
        for (i, slot) in self.reference_list.iter().enumerate() {
            if let Some(it) = slot {
                let mut it = it.clone();
                // The C# sets `linkSlot` to the reference position so the
                // target client knows where it came from; restore to 0xFFFF
                // after sending (here we never mutate the canonical list).
                it.link_slot = i as u16;
                items.push(it);
            }
        }

        outbox.push(InventoryEvent::PacketSetBegin {
            owner_actor_id: self.owner_actor_id,
            capacity: destination_capacity,
            code: destination_code,
        });
        if !items.is_empty() {
            outbox.push(InventoryEvent::PacketItems {
                owner_actor_id: self.owner_actor_id,
                items,
            });
        }
        outbox.push(InventoryEvent::PacketSetEnd { owner_actor_id: self.owner_actor_id });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_item(id: u32, unique_id: u64) -> InventoryItem {
        InventoryItem {
            unique_id,
            item_id: id,
            quantity: 1,
            quality: 1,
            slot: 0,
            link_slot: 0xFFFF,
            item_package: super::super::PKG_NORMAL,
            tag: Default::default(),
        }
    }

    #[test]
    fn set_writes_db_and_emits_single_update() {
        let mut eq = ReferencedItemPackage::new(1, 35, PKG_EQUIPMENT);
        let mut outbox = InventoryOutbox::new();

        eq.set(5, mk_item(1001, 42), &mut outbox);

        let kinds: Vec<&InventoryEvent> = outbox.events.iter().collect();
        assert!(matches!(kinds[0], InventoryEvent::DbEquip { equip_slot: 5, unique_item_id: 42, .. }));
        assert!(matches!(kinds.last(), Some(InventoryEvent::PacketEndChange { .. })));
        assert_eq!(eq.get_at_slot(5).unwrap().unique_id, 42);
    }

    #[test]
    fn clear_writes_unequip_and_removes_ref() {
        let mut eq = ReferencedItemPackage::new(1, 35, PKG_EQUIPMENT);
        let mut outbox = InventoryOutbox::new();
        eq.set(5, mk_item(1001, 42), &mut outbox);
        outbox.drain();

        eq.clear(5, &mut outbox);

        assert!(
            outbox.events.iter().any(|e| matches!(e, InventoryEvent::DbUnequip { equip_slot: 5, .. }))
        );
        assert!(eq.get_at_slot(5).is_none());
    }

    #[test]
    fn clear_all_emits_one_unequip_per_live_slot() {
        let mut eq = ReferencedItemPackage::new(1, 35, PKG_EQUIPMENT);
        let mut outbox = InventoryOutbox::new();
        eq.set(0, mk_item(1001, 1), &mut outbox);
        eq.set(1, mk_item(1002, 2), &mut outbox);
        eq.set(5, mk_item(1003, 3), &mut outbox);
        outbox.drain();

        eq.clear_all(&mut outbox);
        let unequips = outbox
            .events
            .iter()
            .filter(|e| matches!(e, InventoryEvent::DbUnequip { .. }))
            .count();
        assert_eq!(unequips, 3);
    }

    #[test]
    fn db_write_toggle_suppresses_db_events() {
        let mut eq = ReferencedItemPackage::new(1, 35, PKG_EQUIPMENT);
        eq.toggle_db_write(false);
        let mut outbox = InventoryOutbox::new();
        eq.set(0, mk_item(1001, 1), &mut outbox);
        assert!(!outbox.events.iter().any(|e| matches!(e, InventoryEvent::DbEquip { .. })));
    }
}
