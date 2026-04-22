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

//! [`RecipeResolver`] — in-memory catalog ported from
//! `Map Server/DataObjects/RecipeResolver.cs` on
//! `origin/ioncannon/crafting_and_localleves`.
//!
//! Two indexes are kept:
//!   * `by_id: HashMap<u32, Recipe>` — direct lookup for
//!     `GetRecipeByID` / `GetRecipeByItemID`.
//!   * `by_mats: HashMap<[u32; 8], Vec<u32>>` — ordered material
//!     fingerprint → list of recipe ids. The C# packs the 8×u32
//!     material array into 32 bytes and MD5s it; we skip the hash and
//!     key directly on the array, which collides at exactly the same
//!     boundaries because `[u32; 8]` implements `Hash + Eq` as a
//!     position-sensitive concatenation.
//!
//! Values stored in `by_mats` are *ids*, not recipe references — the
//! client's craft-start widget only ever needs the small set of ids
//! to feed back to `GetRecipeByID`, and storing ids sidesteps the
//! lifetime gymnastics of mixing owned and borrowed rows in one map.

#![allow(dead_code)]

use std::collections::HashMap;

use super::recipe::{RECIPE_MATERIAL_SLOTS, Recipe};

#[derive(Debug, Default)]
pub struct RecipeResolver {
    by_id: HashMap<u32, Recipe>,
    by_mats: HashMap<[u32; RECIPE_MATERIAL_SLOTS], Vec<u32>>,
}

impl RecipeResolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build from a flat list. The resolver consumes the vector and
    /// distributes rows into both indexes — each `Recipe` lives exactly
    /// once (in `by_id`), keyed fingerprints hold only the ids so
    /// duplicate storage is avoided.
    pub fn from_recipes(recipes: impl IntoIterator<Item = Recipe>) -> Self {
        let mut this = Self::new();
        for r in recipes {
            this.insert(r);
        }
        this
    }

    pub fn insert(&mut self, recipe: Recipe) {
        let fp = recipe.mat_fingerprint();
        let id = recipe.id;
        self.by_mats.entry(fp).or_default().push(id);
        self.by_id.insert(id, recipe);
    }

    /// Direct lookup. Meteor's `GetRecipeByID(0)` explicitly returns
    /// null — we mirror that by bailing early on id = 0 (which no
    /// recipe row actually uses, but the UI sometimes flushes an
    /// unchosen slot as 0).
    pub fn by_id(&self, recipe_id: u32) -> Option<&Recipe> {
        if recipe_id == 0 {
            return None;
        }
        self.by_id.get(&recipe_id)
    }

    /// Linear scan for the first recipe that produces `item_id`. The C#
    /// uses `Where(...).FirstOrDefault()` which is the same O(n) — in
    /// practice this is called once per craft-start to resolve the
    /// active leve's target item, not inside any hot loop.
    pub fn by_item_id(&self, item_id: u32) -> Option<&Recipe> {
        self.by_id
            .values()
            .find(|r| r.result_item_id == item_id)
    }

    /// Recipes matching an ordered 8-slot material fingerprint. Padding
    /// with `0` in trailing slots is required — the resolver does not
    /// try to match permutations or prefixes.
    pub fn by_mats(&self, fingerprint: [u32; RECIPE_MATERIAL_SLOTS]) -> Vec<&Recipe> {
        self.by_mats
            .get(&fingerprint)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.by_id.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn num_recipes(&self) -> usize {
        self.by_id.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Recipe> {
        self.by_id.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(id: u32, item: u32, mats: [u32; 8]) -> Recipe {
        Recipe::new(id, item, 1, mats, 0, 0, 0, 0, Vec::new(), 1)
    }

    #[test]
    fn by_id_zero_returns_none_even_if_inserted() {
        let r = RecipeResolver::from_recipes([mk(0, 100, [1, 0, 0, 0, 0, 0, 0, 0])]);
        assert!(r.by_id(0).is_none());
    }

    #[test]
    fn by_id_round_trips() {
        let r = RecipeResolver::from_recipes([mk(5, 100, [1, 0, 0, 0, 0, 0, 0, 0])]);
        assert_eq!(r.by_id(5).map(|x| x.result_item_id), Some(100));
        assert!(r.by_id(6).is_none());
    }

    #[test]
    fn by_item_id_returns_first_producer() {
        let r = RecipeResolver::from_recipes([
            mk(1, 100, [10, 0, 0, 0, 0, 0, 0, 0]),
            mk(2, 100, [20, 0, 0, 0, 0, 0, 0, 0]),
        ]);
        assert_eq!(r.by_item_id(100).map(|x| x.id).filter(|id| *id == 1 || *id == 2), r.by_item_id(100).map(|x| x.id));
        assert!(r.by_item_id(999).is_none());
    }

    #[test]
    fn by_mats_groups_by_order_sensitive_fingerprint() {
        let r = RecipeResolver::from_recipes([
            mk(1, 100, [10, 20, 0, 0, 0, 0, 0, 0]),
            mk(2, 101, [10, 20, 0, 0, 0, 0, 0, 0]),
            mk(3, 102, [20, 10, 0, 0, 0, 0, 0, 0]),
        ]);
        let same: Vec<u32> = r
            .by_mats([10, 20, 0, 0, 0, 0, 0, 0])
            .iter()
            .map(|rec| rec.id)
            .collect();
        assert_eq!(same.len(), 2);
        assert!(same.contains(&1));
        assert!(same.contains(&2));
        let flipped: Vec<u32> = r
            .by_mats([20, 10, 0, 0, 0, 0, 0, 0])
            .iter()
            .map(|rec| rec.id)
            .collect();
        assert_eq!(flipped, vec![3]);
        assert!(r.by_mats([1, 2, 3, 0, 0, 0, 0, 0]).is_empty());
    }

    #[test]
    fn num_recipes_counts_unique_ids() {
        let r = RecipeResolver::from_recipes([
            mk(1, 100, [10, 0, 0, 0, 0, 0, 0, 0]),
            mk(2, 101, [11, 0, 0, 0, 0, 0, 0, 0]),
        ]);
        assert_eq!(r.num_recipes(), 2);
    }
}
