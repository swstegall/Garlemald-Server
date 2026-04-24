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

//! Regional (fieldcraft + battlecraft) leve scaffolding. Tier 3 #13.
//!
//! This module is to fieldcraft/battlecraft what `crafting::` is to
//! tradecraft — a read-only catalog plus a per-quest runtime view that
//! packs progress into the quest counter/flag budget. Unlike crafting
//! leves (which have an upstream reference at
//! `origin/ioncannon/crafting_and_localleves`), fieldcraft and
//! battlecraft have **no** upstream port; the design here is
//! garlemald-native, deliberately mirroring the [`PassiveGuildleveData`]
//! shape so future C#-side discoveries can fold in with minimal
//! churn.
//!
//! Progress semantics diverge by [`LeveType`]:
//!
//! * [`LeveType::Fieldcraft`] — `objectiveTargetId` is an item catalog
//!   id. Progress is advanced by every successful gather of a matching
//!   item (hook point: the same drain path `HarvestReward` already
//!   uses). Quantity target caps the leve's crafted count.
//! * [`LeveType::Battlecraft`] — `objectiveTargetId` is a BattleNpc
//!   actor-class id. Progress is advanced by `onKillBNpc` firing with
//!   a matching class id.
//!
//! Reserved quest id ranges (above the 112_048 `Bitstream2048` cap
//! because leves are repeatable and don't occupy the completed-quest
//! bitstream):
//!
//! * fieldcraft  : [`FIELDCRAFT_LEVE_ID_MIN`]  = 130_001 …
//!   [`FIELDCRAFT_LEVE_ID_MAX`]  = 130_450
//! * battlecraft : [`BATTLECRAFT_LEVE_ID_MIN`] = 140_001 …
//!   [`BATTLECRAFT_LEVE_ID_MAX`] = 140_450
//!
//! [`PassiveGuildleveData`]: crate::crafting::PassiveGuildleveData

#![allow(dead_code)]

pub mod data;
pub mod resolver;
pub mod view;

pub use data::{LeveType, RegionalLeveData};
pub use resolver::RegionalLeveResolver;
pub use view::RegionalLeveView;

// Re-exports used by tests + future Lua bindings. Kept behind
// `#[allow(unused_imports)]` so fresh builds that only reach the
// progress-hook entry points don't trip the lint.
#[allow(unused_imports)]
pub use view::{
    ACCEPTED_FLAG_BIT, BATTLECRAFT_LEVE_ID_MAX, BATTLECRAFT_LEVE_ID_MIN, COMPLETED_FLAG_BIT,
    FIELDCRAFT_LEVE_ID_MAX, FIELDCRAFT_LEVE_ID_MIN, is_battlecraft_leve_quest_id,
    is_fieldcraft_leve_quest_id, is_regional_leve_quest_id, leve_type_from_quest_id,
};
