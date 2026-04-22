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

//! [`Recipe`] — one crafting recipe. Ported verbatim from
//! `Map Server/DataObjects/Recipe.cs` on
//! `origin/ioncannon/crafting_and_localleves`.
//!
//! Field naming keeps Meteor's Lua-facing names (`resultItemID`,
//! `crystalId1`, `crystalQuantity1`, …) because `CraftCommand.lua`
//! reads them directly via the Lua userdata surface. The one deliberate
//! divergence is the DB: `allowedCrafters` is read from
//! `gamedata_recipes.job` (a single-char `'A'..'H'` code) rather than the
//! C# `new byte[] {}` placeholder, so the future "which classes can see
//! this recipe on the craft-start widget" filter has usable data even
//! though the branch itself never populated it.
//!
//! Materials are kept in the original DB order — order-sensitive, so the
//! resolver's `[u32; 8]` fingerprint lookup matches the C# MD5-over-
//! packed-bytes approach byte-for-byte.

#![allow(dead_code)]

use std::sync::Arc;

/// Number of material slots on the `gamedata_recipes` row. Lua pads
/// empties with `0` so the length is always 8.
pub const RECIPE_MATERIAL_SLOTS: usize = 8;

/// Immutable recipe record. Cheap to clone — 64-ish bytes plus one
/// `Arc<Vec<u8>>` for the allowed-crafters list (always very short).
///
/// `Debug` is preserved for logging/tests; `PartialEq + Eq` are useful
/// for test assertions but never used at runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipe {
    pub id: u32,
    pub result_item_id: u32,
    pub result_quantity: u32,
    pub materials: [u32; RECIPE_MATERIAL_SLOTS],
    pub crystal_id_1: u32,
    pub crystal_quantity_1: u32,
    pub crystal_id_2: u32,
    pub crystal_quantity_2: u32,
    /// Lowercase class short-codes (`crp`, `bsm`, …). Meteor's C# loader
    /// hardwired this to an empty `byte[]` and left recipe-filtering as
    /// future work — garlemald preserves the raw `job` column so scripts
    /// can route recipes to the right crafting job once the filter
    /// lands. A recipe row with `job = 'A'` maps to `["crp"]`.
    pub allowed_crafters: Arc<Vec<String>>,
    /// Meteor's `Recipe.tier` is always `1` today — reserved for the
    /// recipe-progression pass the branch notes as deferred work.
    pub tier: u8,
}

impl Recipe {
    pub fn new(
        id: u32,
        result_item_id: u32,
        result_quantity: u32,
        materials: [u32; RECIPE_MATERIAL_SLOTS],
        crystal_id_1: u32,
        crystal_quantity_1: u32,
        crystal_id_2: u32,
        crystal_quantity_2: u32,
        allowed_crafters: Vec<String>,
        tier: u8,
    ) -> Self {
        Self {
            id,
            result_item_id,
            result_quantity,
            materials,
            crystal_id_1,
            crystal_quantity_1,
            crystal_id_2,
            crystal_quantity_2,
            allowed_crafters: Arc::new(allowed_crafters),
            tier,
        }
    }

    /// Material fingerprint the resolver indexes on. The C# hashes the
    /// packed 32-byte big-endian representation; we skip the hashing and
    /// use the array directly, which collides at the same boundaries
    /// because the array is order-preserving and the mapping is total.
    pub fn mat_fingerprint(&self) -> [u32; RECIPE_MATERIAL_SLOTS] {
        self.materials
    }

    /// Convert the single-char `job` code from `gamedata_recipes.job`
    /// (`'A'..'H'`) to the lowercase class short-code Meteor's Lua uses.
    /// Unknown codes return `None`; the caller typically maps those to
    /// an empty allow-list.
    ///
    /// `A=CRP, B=BSM, C=ARM, D=GSM, E=LTW, F=WVR, G=ALC, H=CUL`.
    pub fn job_code_to_class(job: char) -> Option<&'static str> {
        match job {
            'A' | 'a' => Some("crp"),
            'B' | 'b' => Some("bsm"),
            'C' | 'c' => Some("arm"),
            'D' | 'd' => Some("gsm"),
            'E' | 'e' => Some("ltw"),
            'F' | 'f' => Some("wvr"),
            'G' | 'g' => Some("alc"),
            'H' | 'h' => Some("cul"),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_code_maps_to_lowercase_class() {
        assert_eq!(Recipe::job_code_to_class('A'), Some("crp"));
        assert_eq!(Recipe::job_code_to_class('H'), Some("cul"));
        assert_eq!(Recipe::job_code_to_class('h'), Some("cul"));
        assert_eq!(Recipe::job_code_to_class('Z'), None);
    }

    #[test]
    fn mat_fingerprint_is_order_sensitive() {
        let a = Recipe::new(
            1,
            100,
            1,
            [10, 20, 0, 0, 0, 0, 0, 0],
            0,
            0,
            0,
            0,
            vec!["crp".into()],
            1,
        );
        let b = Recipe::new(
            2,
            100,
            1,
            [20, 10, 0, 0, 0, 0, 0, 0],
            0,
            0,
            0,
            0,
            vec!["crp".into()],
            1,
        );
        assert_ne!(a.mat_fingerprint(), b.mat_fingerprint());
    }
}
