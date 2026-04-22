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

//! `Retainer` — a player-owned NPC merchant. Port of
//! `Actors/Chara/Npc/Retainer.cs`. A Retainer is an `Npc` (not a
//! BattleNpc — no combat) with three preinstalled item packages.

#![allow(dead_code)]

use std::collections::HashMap;

use crate::inventory::{ItemPackage, PKG_BAZAAR, PKG_CURRENCY_CRYSTALS, PKG_NORMAL};

use super::actor_class::ActorClass;
use super::npc::Npc;

pub const MAX_INVENTORY_NORMAL: u16 = 150;
pub const MAX_INVENTORY_CURRENCY: u16 = 320;
pub const MAX_INVENTORY_BAZAAR: u16 = 10;

#[derive(Debug, Clone)]
pub struct Retainer {
    pub npc: Npc,
    pub retainer_id: u32,
    pub owner_actor_id: u32,
    pub item_packages: HashMap<u16, ItemPackage>,
}

impl Retainer {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        retainer_id: u32,
        actor_class: &ActorClass,
        owner_actor_id: u32,
        area_id: u32,
        x: f32,
        y: f32,
        z: f32,
        rotation: f32,
    ) -> Self {
        let mut npc = Npc::new(
            0,
            actor_class,
            "myretainer",
            area_id,
            x,
            y,
            z,
            rotation,
            0,
            0,
            None,
        );
        // Retail uses `_rtnre{actorId:x7}` as the actor name.
        npc.character.base.actor_name = format!("_rtnre{:07x}", npc.actor_id());

        let actor_id = npc.actor_id();
        let mut item_packages = HashMap::new();
        item_packages.insert(
            PKG_NORMAL,
            ItemPackage::new(actor_id, MAX_INVENTORY_NORMAL, PKG_NORMAL),
        );
        item_packages.insert(
            PKG_CURRENCY_CRYSTALS,
            ItemPackage::new(actor_id, MAX_INVENTORY_CURRENCY, PKG_CURRENCY_CRYSTALS),
        );
        item_packages.insert(
            PKG_BAZAAR,
            ItemPackage::new(actor_id, MAX_INVENTORY_BAZAAR, PKG_BAZAAR),
        );

        Self {
            npc,
            retainer_id,
            owner_actor_id,
            item_packages,
        }
    }

    pub fn actor_id(&self) -> u32 {
        self.npc.actor_id()
    }

    pub fn retainer_id(&self) -> u32 {
        self.retainer_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retainer_has_three_item_packages() {
        let class = ActorClass::new(
            9_999_999,
            "/Chara/Npc/Retainer/RetainerDefault",
            0,
            0,
            "",
            0,
            0,
            0,
        );
        let r = Retainer::new(100, &class, 42, 200, 0.0, 0.0, 0.0, 0.0);
        assert_eq!(r.item_packages.len(), 3);
        assert!(r.item_packages.contains_key(&PKG_NORMAL));
        assert!(r.item_packages.contains_key(&PKG_CURRENCY_CRYSTALS));
        assert!(r.item_packages.contains_key(&PKG_BAZAAR));
        assert!(r.npc.character.base.actor_name.starts_with("_rtnre"));
    }
}
