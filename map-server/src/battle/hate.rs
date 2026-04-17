//! Enmity tracker. 1:1 port of `Actors/Chara/Ai/HateContainer.cs`.
//!
//! The C# retains a reference to each `Character` in the hate list. We key
//! by actor id instead since that matches how our outbox events identify
//! actors and avoids interior-mutability headaches.

#![allow(dead_code)]

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HateEntry {
    pub actor_id: u32,
    pub cumulative_enmity: u32,
    pub volatile_enmity: u32,
    pub is_active: bool,
}

impl HateEntry {
    pub fn new(actor_id: u32) -> Self {
        Self {
            actor_id,
            cumulative_enmity: 1,
            volatile_enmity: 0,
            is_active: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct HateContainer {
    pub owner_actor_id: u32,
    entries: HashMap<u32, HateEntry>,
}

impl HateContainer {
    pub fn new(owner_actor_id: u32) -> Self {
        Self {
            owner_actor_id,
            entries: HashMap::new(),
        }
    }

    /// Insert a base-hate entry if the target isn't already tracked.
    pub fn add_base_hate(&mut self, target_actor_id: u32) {
        self.entries
            .entry(target_actor_id)
            .or_insert_with(|| HateEntry::new(target_actor_id));
    }

    /// Add damage-derived enmity to `target`. Negative `damage` values are
    /// clamped to 0 (matches the C# `(uint)damage` cast semantics).
    pub fn update_hate(&mut self, target_actor_id: u32, damage: i32) {
        self.add_base_hate(target_actor_id);
        if let Some(entry) = self.entries.get_mut(&target_actor_id) {
            let delta = damage.max(0) as u32;
            entry.cumulative_enmity = entry.cumulative_enmity.saturating_add(delta);
        }
    }

    pub fn clear_hate(&mut self, target_actor_id: Option<u32>) {
        match target_actor_id {
            Some(id) => {
                self.entries.remove(&id);
            }
            None => self.entries.clear(),
        }
    }

    pub fn has_hate_for(&self, target_actor_id: u32) -> bool {
        self.entries.contains_key(&target_actor_id)
    }

    pub fn get(&self, target_actor_id: u32) -> Option<&HateEntry> {
        self.entries.get(&target_actor_id)
    }

    pub fn get_mut(&mut self, target_actor_id: u32) -> Option<&mut HateEntry> {
        self.entries.get_mut(&target_actor_id)
    }

    pub fn entries(&self) -> impl Iterator<Item = &HateEntry> {
        self.entries.values()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return the active entry with the highest cumulative enmity. Ties
    /// break by lowest actor id to stay deterministic.
    pub fn most_hated(&self) -> Option<u32> {
        self.entries
            .values()
            .filter(|e| e.is_active)
            .max_by(|a, b| {
                a.cumulative_enmity
                    .cmp(&b.cumulative_enmity)
                    .then_with(|| b.actor_id.cmp(&a.actor_id))
            })
            .map(|e| e.actor_id)
    }

    /// Deactivate a target without removing its history (used when a target
    /// zones away or goes out of range). A later `UpdateHate` re-activates.
    pub fn set_active(&mut self, target_actor_id: u32, active: bool) {
        if let Some(e) = self.entries.get_mut(&target_actor_id) {
            e.is_active = active;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_update_hate() {
        let mut h = HateContainer::new(1);
        h.update_hate(100, 50);
        h.update_hate(100, 25);
        h.update_hate(200, 10);

        assert_eq!(h.most_hated(), Some(100));
        assert_eq!(h.get(100).unwrap().cumulative_enmity, 50 + 25 + 1);
        // add_base_hate seeded with 1 before the damage updates land.
    }

    #[test]
    fn clear_hate_targeted_vs_all() {
        let mut h = HateContainer::new(1);
        h.update_hate(100, 50);
        h.update_hate(200, 10);
        h.clear_hate(Some(100));
        assert!(!h.has_hate_for(100));
        assert!(h.has_hate_for(200));
        h.clear_hate(None);
        assert!(h.is_empty());
    }

    #[test]
    fn inactive_entries_excluded_from_most_hated() {
        let mut h = HateContainer::new(1);
        h.update_hate(100, 100);
        h.update_hate(200, 10);
        h.set_active(100, false);
        assert_eq!(h.most_hated(), Some(200));
    }

    #[test]
    fn most_hated_stable_on_tie() {
        let mut h = HateContainer::new(1);
        h.update_hate(200, 10);
        h.update_hate(100, 10);
        // Both have cumulative_enmity = 1 (base) + 10 (damage) = 11.
        // Tie-break prefers the lowest actor id (100) for determinism.
        assert_eq!(h.most_hated(), Some(100));
    }
}
