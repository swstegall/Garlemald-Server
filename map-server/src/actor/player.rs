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

//! Player helper methods, ported from `Actors/Chara/Player/Player.cs`.
//!
//! Pure / mostly-pure methods only. Anything that would broadcast a packet or
//! mutate shared zone state is left to the game-loop integration.

#![allow(dead_code)]

use std::collections::{HashMap, HashSet};

use super::chara::TraitRef;
use super::quest::{GuildleveJournal, Quest};
use super::{Player, PlayerState};

// Class-id constants (match `scripts/global.lua` and `Player.cs` constants).
pub const CLASSID_PUG: u8 = 2;
pub const CLASSID_GLA: u8 = 3;
pub const CLASSID_MRD: u8 = 4;
pub const CLASSID_ARC: u8 = 7;
pub const CLASSID_LNC: u8 = 8;
pub const CLASSID_THM: u8 = 22;
pub const CLASSID_CNJ: u8 = 23;

// Equipment slot constants used by `HasItemEquippedInSlot`.
pub const SLOT_MAINHAND: u16 = 0;
pub const SLOT_OFFHAND: u16 = 1;
pub const SLOT_HEAD: u16 = 4;
pub const SLOT_BODY: u16 = 5;
pub const SLOT_HANDS: u16 = 6;
pub const SLOT_WAIST: u16 = 7;
pub const SLOT_LEGS: u16 = 8;
pub const SLOT_FEET: u16 = 9;
pub const SLOT_UNDERSHIRT: u16 = 11;
pub const SLOT_UNDERGARMENT: u16 = 12;
pub const SLOT_NECK: u16 = 13;
pub const SLOT_EAR_RIGHT: u16 = 14;
pub const SLOT_EAR_LEFT: u16 = 15;
pub const SLOT_FINGER_RIGHT: u16 = 16;
pub const SLOT_FINGER_LEFT: u16 = 17;

/// Extra per-player state needed by the helpers that doesn't live on the
/// packet-shape `PlayerState`.
///
/// `quest_journal` *used* to live here but moved onto [`Character`] so the
/// packet processor (which only has `ActorRegistry` / `Arc<RwLock<Character>>`
/// handles) can mutate it. Everything else here stays player-scoped.
#[derive(Debug, Clone, Default)]
pub struct PlayerHelperState {
    pub guildleve_journal: GuildleveJournal,
    pub unlocked_aetherytes: HashSet<u32>,
    pub traits: Vec<TraitRef>,
    /// Equipment by slot id → catalog id. Populated by the game loop when
    /// inventory changes.
    pub equipment_by_slot: HashMap<u16, u32>,
    /// Flat (item_id, quantity) view used by `has_item` / `get_current_gil`.
    pub inventory_summary: HashMap<u32, i32>,
    /// True when a trade invite is live.
    pub in_trade: bool,
    pub trade_accepted: bool,
    pub in_party: bool,
    pub party_leader: bool,
    /// UNIX timestamp at which the chocobo rental expires (0 = no rental).
    pub chocobo_rental_until: u64,

    /// Directors (composite actor ids, 6-prefixed) this player is a
    /// member of. Populated by `add_director` / `remove_director`.
    pub owned_directors: Vec<u32>,
    /// If set, the next zone-in bundle spawns this director's actor on
    /// the client — matches the C# `loginInitDirector`.
    pub login_init_director: Option<u32>,
    /// Fast-path cache: which of `owned_directors` is the active
    /// guildleve (at most one at a time).
    pub current_guildleve_director: Option<u32>,

    /// Group id of the party this player is in, if any. Matches the C#
    /// `currentParty` nullable reference.
    pub current_party_id: Option<u64>,
    /// Cached roster of the current party (actor ids). Kept in sync by
    /// the world server on party state changes.
    pub current_party_members: Vec<u32>,
    /// Is this player the party leader?
    pub current_party_is_leader: bool,
    /// Group id of the content group this player is in (guildleve /
    /// duty instance), if any.
    pub current_content_group_id: Option<u64>,
    /// Linkshell group ids this player has joined (retail cap = 8).
    pub current_linkshell_ids: Vec<u64>,

    // ---- Phase 8 — achievements / titles / retainer / bazaar ---------
    /// Ids of earned achievements. Serialised into the completed-bits
    /// array on zone-in.
    pub earned_achievements: std::collections::HashSet<u32>,
    pub achievement_points: u32,
    /// The 5 most-recently-earned ids, FIFO. Retail shows these in the
    /// profile card.
    pub latest_achievements: [u32; 5],
    /// Currently-equipped title id. 0 = none.
    pub current_title_id: u32,
    /// Non-zero when the player has a retainer spawned in-zone — lets
    /// the item-package-request dispatch tell retainer from player.
    pub current_spawned_retainer_id: u32,
}

impl PlayerHelperState {
    /// Port of `Player::AddDirector(director, spawnImmediately=false)`.
    /// The outbox event moves member-bookkeeping into the director's
    /// side; this just tracks the fact on the player.
    pub fn add_director(
        &mut self,
        director_actor_id: u32,
        is_guildleve: bool,
        outbox: &mut crate::director::DirectorOutbox,
        player_actor_id: u32,
    ) {
        if !self.owned_directors.contains(&director_actor_id) {
            self.owned_directors.push(director_actor_id);
        }
        if is_guildleve {
            self.current_guildleve_director = Some(director_actor_id);
        }
        outbox.push(crate::director::DirectorEvent::MemberAdded {
            director_id: director_actor_id,
            actor_id: player_actor_id,
            is_player: true,
        });
    }

    /// Port of `Player::RemoveDirector(director)`.
    pub fn remove_director(
        &mut self,
        director_actor_id: u32,
        outbox: &mut crate::director::DirectorOutbox,
        player_actor_id: u32,
    ) {
        self.owned_directors.retain(|id| *id != director_actor_id);
        if self.current_guildleve_director == Some(director_actor_id) {
            self.current_guildleve_director = None;
        }
        if self.login_init_director == Some(director_actor_id) {
            self.login_init_director = None;
        }
        outbox.push(crate::director::DirectorEvent::MemberRemoved {
            director_id: director_actor_id,
            actor_id: player_actor_id,
            is_player: true,
        });
    }

    /// Port of `Player::GetGuildleveDirector()`.
    pub fn guildleve_director(&self) -> Option<u32> {
        self.current_guildleve_director
    }

    // ----- Group bookkeeping -----------------------------------------------

    /// World server pushed down the authoritative party roster. Updates
    /// local caches so the next UI refresh / group-sync packet bundle
    /// reflects reality.
    pub fn set_party(&mut self, party_id: u64, members: Vec<u32>, is_leader: bool) {
        self.current_party_id = Some(party_id);
        self.current_party_members = members;
        self.current_party_is_leader = is_leader;
        self.in_party = true;
        self.party_leader = is_leader;
    }

    /// Clear party state — player left or the party disbanded.
    pub fn clear_party(&mut self) {
        self.current_party_id = None;
        self.current_party_members.clear();
        self.current_party_is_leader = false;
        self.in_party = false;
        self.party_leader = false;
    }

    /// Player entered / left a guildleve or duty instance.
    pub fn set_content_group(&mut self, content_group_id: Option<u64>) {
        self.current_content_group_id = content_group_id;
    }

    pub fn add_linkshell(&mut self, ls_id: u64) {
        if !self.current_linkshell_ids.contains(&ls_id) {
            self.current_linkshell_ids.push(ls_id);
        }
    }

    pub fn remove_linkshell(&mut self, ls_id: u64) {
        self.current_linkshell_ids.retain(|id| *id != ls_id);
    }

    pub fn is_in_linkshell(&self, ls_id: u64) -> bool {
        self.current_linkshell_ids.contains(&ls_id)
    }

    // ----- Phase 8 — achievements / titles / retainer -----------------

    /// Port of `Player.EarnAchievement(id, points)`. Idempotent: the
    /// same id can't be earned twice. Returns `true` if this was a
    /// first-time earn (the only case that emits an `Earned` toast in
    /// retail).
    pub fn earn_achievement(
        &mut self,
        player_actor_id: u32,
        achievement_id: u32,
        points: u32,
        outbox: &mut crate::achievement::AchievementOutbox,
    ) -> bool {
        if !self.earned_achievements.insert(achievement_id) {
            return false;
        }
        self.achievement_points = self.achievement_points.saturating_add(points);
        // Shift latest_achievements FIFO: new entry lands at index 0.
        self.latest_achievements.rotate_right(1);
        self.latest_achievements[0] = achievement_id;

        outbox.push(crate::achievement::AchievementEvent::Earned {
            player_actor_id,
            achievement_id,
        });
        outbox.push(crate::achievement::AchievementEvent::SetPoints {
            player_actor_id,
            points: self.achievement_points,
        });
        outbox.push(crate::achievement::AchievementEvent::SetLatest {
            player_actor_id,
            latest_ids: self.latest_achievements,
        });
        true
    }

    pub fn has_achievement(&self, id: u32) -> bool {
        self.earned_achievements.contains(&id)
    }

    /// Build the wire-format bit array. Indices up to the retail cap
    /// (`COMPLETED_ACHIEVEMENTS_BITS`) get a `true` if earned.
    pub fn completed_achievement_bits(&self) -> Vec<bool> {
        let mut bits = vec![false; crate::achievement::COMPLETED_ACHIEVEMENTS_BITS];
        for &id in &self.earned_achievements {
            if (id as usize) < bits.len() {
                bits[id as usize] = true;
            }
        }
        bits
    }

    /// Port of `Player.SetTitle(id)`.
    pub fn set_title(
        &mut self,
        player_actor_id: u32,
        title_id: u32,
        outbox: &mut crate::achievement::AchievementOutbox,
    ) {
        self.current_title_id = title_id;
        outbox.push(crate::achievement::AchievementEvent::SetPlayerTitle {
            player_actor_id,
            title_id,
        });
    }

    pub fn set_spawned_retainer(&mut self, actor_id: u32) {
        self.current_spawned_retainer_id = actor_id;
    }

    pub fn clear_spawned_retainer(&mut self) {
        self.current_spawned_retainer_id = 0;
    }

    pub fn has_spawned_retainer(&self) -> bool {
        self.current_spawned_retainer_id != 0
    }
}

impl Player {
    /// Create a Player with fresh helper state attached.
    pub fn with_helpers(actor_id: u32) -> Self {
        let mut p = Self::new(actor_id);
        p.helpers = PlayerHelperState::default();
        p
    }

    // ----- Identity --------------------------------------------------------

    pub fn is_my_player(&self, other_actor_id: u32) -> bool {
        self.character.base.actor_id == other_actor_id
    }

    // ----- Play time -------------------------------------------------------

    /// Port of `Player.GetPlayTime(bool doUpdate)`. When `do_update` is
    /// true, accumulate elapsed seconds since `last_play_time_update`.
    pub fn get_play_time(&mut self, do_update: bool) -> u32 {
        if do_update {
            let now = common::utils::unix_timestamp();
            self.player.play_time += now.saturating_sub(self.player.last_play_time_update);
            self.player.last_play_time_update = now;
        }
        self.player.play_time
    }

    // ----- Mount / Chocobo -------------------------------------------------

    pub fn get_mount_state(&self) -> u8 {
        self.player.mount_state
    }

    pub fn set_mount_state(&mut self, state: u8) {
        self.player.mount_state = state;
    }

    pub fn is_chocobo_rental_active(&self) -> bool {
        self.helpers.chocobo_rental_until > common::utils::unix_timestamp() as u64
    }

    pub fn start_chocobo_rental(&mut self, minutes: u8) {
        let now = common::utils::unix_timestamp() as u64;
        self.helpers.chocobo_rental_until = now + (minutes as u64 * 60);
    }

    // ----- Home points / initial town --------------------------------------

    pub fn get_initial_town(&self) -> u8 {
        // Not held on Player directly in the C# shape; mirror that by
        // deferring to the inventory-summary-less CharaState helpers. For
        // now return 0 when no slot is set — the real value is loaded in
        // by `Database::load_player_character`.
        0
    }

    pub fn get_home_point(&self) -> u32 {
        self.player.homepoint
    }

    pub fn get_home_point_inn(&self) -> u8 {
        self.player.homepoint_inn
    }

    pub fn set_home_point(&mut self, aetheryte_id: u32) {
        self.player.homepoint = aetheryte_id;
        self.helpers.unlocked_aetherytes.insert(aetheryte_id);
    }

    pub fn set_home_point_inn(&mut self, town_id: u8) {
        self.player.homepoint_inn = town_id;
    }

    pub fn has_aetheryte_node_unlocked(&self, aetheryte_id: u32) -> bool {
        self.helpers.unlocked_aetherytes.contains(&aetheryte_id)
    }

    // ----- Traits ----------------------------------------------------------

    pub fn has_trait(&self, id: u16) -> bool {
        self.helpers
            .traits
            .iter()
            .any(|t| t.id == id && self.character.meets_trait(*t))
    }

    pub fn has_trait_ref(&self, trait_ref: TraitRef) -> bool {
        self.character.meets_trait(trait_ref)
    }

    // ----- Quests ----------------------------------------------------------

    pub fn get_free_quest_slot(&self) -> i32 {
        self.character
            .quest_journal
            .get_free_slot()
            .map(|s| s as i32)
            .unwrap_or(-1)
    }

    pub fn has_quest(&self, id: u32) -> bool {
        self.character.quest_journal.has(id)
    }

    pub fn has_quest_by_name(&self, name: &str) -> bool {
        self.character.quest_journal.has_by_name(name)
    }

    pub fn is_quest_completed(&self, id: u32) -> bool {
        self.character.quest_journal.is_completed(id)
    }

    pub fn can_accept_quest(&self, id: u32) -> bool {
        self.character.quest_journal.can_accept(id)
    }

    pub fn get_quest(&self, id: u32) -> Option<&Quest> {
        self.character.quest_journal.get(id)
    }

    pub fn get_quest_mut(&mut self, id: u32) -> Option<&mut Quest> {
        self.character.quest_journal.get_mut(id)
    }

    pub fn get_quest_slot(&self, id: u32) -> Option<usize> {
        self.character.quest_journal.slot_of(id)
    }

    pub fn add_quest(&mut self, quest: Quest) -> Option<usize> {
        self.character.quest_journal.add(quest)
    }

    pub fn complete_quest(&mut self, id: u32) {
        self.character.quest_journal.complete(id);
    }

    pub fn abandon_quest(&mut self, id: u32) -> Option<Quest> {
        self.character.quest_journal.remove(id)
    }

    // ----- Guildleves ------------------------------------------------------

    pub fn get_free_guildleve_slot(&self) -> i32 {
        self.helpers
            .guildleve_journal
            .get_free_slot()
            .map(|s| s as i32)
            .unwrap_or(-1)
    }

    pub fn has_guildleve(&self, id: u32) -> bool {
        self.helpers.guildleve_journal.has(id)
    }

    pub fn add_guildleve(&mut self, id: u16) -> Option<usize> {
        self.helpers.guildleve_journal.add(id)
    }

    pub fn remove_guildleve(&mut self, id: u32) -> bool {
        self.helpers.guildleve_journal.remove(id)
    }

    // ----- Class / job ----------------------------------------------------

    /// Convert `classId` → matching `jobId`, matching the C# lookup.
    pub fn convert_class_id_to_job_id(class_id: u8) -> u8 {
        match class_id {
            CLASSID_PUG | CLASSID_GLA | CLASSID_MRD => class_id + 13,
            CLASSID_ARC | CLASSID_LNC => class_id + 11,
            CLASSID_THM | CLASSID_CNJ => class_id + 4,
            other => other,
        }
    }

    /// Returns the job id when the player has one, otherwise the main-skill
    /// class id (from the C# `PlayerCharacterUpdateClassLevel` flow).
    pub fn get_current_class_or_job(&self) -> u8 {
        if self.character.chara.current_job != 0 {
            self.character.chara.current_job as u8
        } else {
            self.character.chara.class as u8
        }
    }

    /// Highest class level across the 42-slot skill table on the snapshot.
    /// The C# iterates `charaWork.battleSave.skillLevel`; we use the mirror
    /// populated by `Database::load_player_character`.
    pub fn get_highest_level(&self) -> i32 {
        self.player.highest_level_cache.max(0)
    }

    pub fn set_highest_level(&mut self, level: i32) {
        self.player.highest_level_cache = level;
    }

    // ----- Trading --------------------------------------------------------

    pub fn is_trading(&self) -> bool {
        self.helpers.in_trade
    }

    pub fn is_trade_accepted(&self) -> bool {
        self.helpers.in_trade && self.helpers.trade_accepted
    }

    // ----- Party ---------------------------------------------------------

    pub fn is_in_party(&self) -> bool {
        self.helpers.in_party
    }

    pub fn is_party_leader(&self) -> bool {
        self.helpers.in_party && self.helpers.party_leader
    }

    // ----- Equipment / inventory -----------------------------------------

    pub fn has_item_equipped_in_slot(&self, item_id: u32, slot: u16) -> bool {
        self.helpers
            .equipment_by_slot
            .get(&slot)
            .copied()
            .is_some_and(|id| id == item_id)
    }

    pub fn get_equipped_item(&self, slot: u16) -> Option<u32> {
        self.helpers.equipment_by_slot.get(&slot).copied()
    }

    /// Does the player carry at least `min_quantity` of `catalog_id`?
    pub fn has_item(&self, catalog_id: u32, min_quantity: i32) -> bool {
        self.helpers
            .inventory_summary
            .get(&catalog_id)
            .copied()
            .unwrap_or(0)
            >= min_quantity
    }

    /// Gil catalog id is `1000001` per the C# constant.
    pub fn get_current_gil(&self) -> i32 {
        self.helpers
            .inventory_summary
            .get(&1_000_001)
            .copied()
            .unwrap_or(0)
    }

    // ----- Zone change ---------------------------------------------------

    pub fn is_in_zone_change(&self) -> bool {
        self.player.is_zone_changing
    }

    pub fn set_zone_changing(&mut self, flag: bool) {
        self.player.is_zone_changing = flag;
    }

    // ----- Discipline-of passthroughs ------------------------------------

    pub fn is_disciple_of_war(&self) -> bool {
        self.character.is_disciple_of_war()
    }
    pub fn is_disciple_of_magic(&self) -> bool {
        self.character.is_disciple_of_magic()
    }
    pub fn is_disciple_of_hand(&self) -> bool {
        self.character.is_disciple_of_hand()
    }
    pub fn is_disciple_of_land(&self) -> bool {
        self.character.is_disciple_of_land()
    }
}

impl PlayerState {
    /// Convenience — record the rest-bonus exp rate without touching the
    /// wider PlayerWork save.
    pub fn set_rest_bonus(&mut self, rate: i32) {
        self.rest_bonus_exp_rate = rate;
    }
}

#[cfg(test)]
mod tests {
    use super::super::quest::{Quest, quest_actor_id};
    use super::*;
    use crate::actor::Player;

    fn fresh_player() -> Player {
        Player::with_helpers(1)
    }

    #[test]
    fn quest_flow() {
        let mut p = fresh_player();
        assert_eq!(p.get_free_quest_slot(), 0);
        let q = Quest::new(quest_actor_id(110_001), "man0l0");
        p.add_quest(q);
        assert!(p.has_quest(110_001));
        assert_eq!(p.get_quest_slot(110_001), Some(0));
        assert!(!p.can_accept_quest(110_001));
        p.complete_quest(110_001);
        assert!(!p.has_quest(110_001));
        assert!(p.is_quest_completed(110_001));
    }

    #[test]
    fn aetheryte_unlocked_only_when_set() {
        let mut p = fresh_player();
        assert!(!p.has_aetheryte_node_unlocked(1_280_001));
        p.set_home_point(1_280_001);
        assert!(p.has_aetheryte_node_unlocked(1_280_001));
    }

    #[test]
    fn convert_class_to_job() {
        assert_eq!(Player::convert_class_id_to_job_id(CLASSID_GLA), 16); // PLD
        assert_eq!(Player::convert_class_id_to_job_id(CLASSID_THM), 26); // BLM
        assert_eq!(Player::convert_class_id_to_job_id(200), 200); // unknown passthrough
    }

    #[test]
    fn current_class_or_job_prefers_job() {
        let mut p = fresh_player();
        p.character.chara.class = 3;
        p.character.chara.current_job = 0;
        assert_eq!(p.get_current_class_or_job(), 3);
        p.character.chara.current_job = 16;
        assert_eq!(p.get_current_class_or_job(), 16);
    }

    #[test]
    fn gil_lookup_falls_through_inventory_summary() {
        let mut p = fresh_player();
        assert_eq!(p.get_current_gil(), 0);
        p.helpers.inventory_summary.insert(1_000_001, 42);
        assert_eq!(p.get_current_gil(), 42);
    }
}
