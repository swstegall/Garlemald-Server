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

//! Actor hierarchy.
//!
//! The C# source uses `Actor ← Chara ← Player | Npc | BattleNpc` inheritance.
//! Rust doesn't inherit, so we model the shared fields as a `BaseActor`
//! struct that specialized kinds compose. Specialized behavior (inventory
//! math, AI, battle helpers) lives in separate modules (`inventory`, `ai`,
//! `battle`) that Phase 4 stubs out as they bottom out in the deep C# logic
//! we haven't ported yet.

#![allow(dead_code)]

pub mod chara;
pub mod event_conditions;
pub mod modifier;
pub mod player;
pub mod quest;

#[allow(unused_imports)]
pub use modifier::{Modifier, ModifierMap};

use common::Vector3;

pub const INVALID_ACTORID: u32 = 0xC0000000;

pub const MAIN_STATE_PASSIVE: u16 = 0x00;
pub const MAIN_STATE_ACTIVE: u16 = 0x01;
pub const MAIN_STATE_DEAD: u16 = 0x02;

#[derive(Debug, Clone, Default)]
pub struct BaseActor {
    pub actor_id: u32,
    pub actor_name: String,
    pub display_name_id: u32,
    pub custom_display_name: String,
    pub current_main_state: u16,

    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
    pub rotation: f32,

    pub old_position_x: f32,
    pub old_position_y: f32,
    pub old_position_z: f32,
    pub old_rotation: f32,

    pub move_state: u16,
    pub old_move_state: u16,

    pub zone_id: u32,
    pub zone_id2: u32,
    pub private_area: String,
    pub private_area_type: u32,

    pub is_zoning: bool,
    pub spawned_first_time: bool,
    pub class_path: String,
    pub class_name: String,
    pub is_at_spawn: bool,
    /// Parsed event conditions (talk / notice / emote / push). NPC-only;
    /// players leave this empty. Populated in `Npc::new` from
    /// `ActorClass::event_conditions`. Consumed by `push_npc_spawn` to
    /// fan the matching `SetTalkEventCondition` / `SetNoticeEventCondition`
    /// / `SetEmoteEventCondition` / push-variant packets so the client
    /// lights up each condition's trigger before the `ScriptBind`.
    pub event_conditions: event_conditions::EventConditionList,
}

impl BaseActor {
    pub fn new(actor_id: u32) -> Self {
        Self {
            actor_id,
            display_name_id: 0xFFFFFFFF,
            current_main_state: MAIN_STATE_PASSIVE,
            is_at_spawn: true,
            ..Default::default()
        }
    }

    pub fn position(&self) -> Vector3 {
        Vector3::new(self.position_x, self.position_y, self.position_z)
    }

    pub fn set_position(&mut self, pos: Vector3, rotation: f32) {
        self.old_position_x = self.position_x;
        self.old_position_y = self.position_y;
        self.old_position_z = self.position_z;
        self.old_rotation = self.rotation;
        self.position_x = pos.x;
        self.position_y = pos.y;
        self.position_z = pos.z;
        self.rotation = rotation;
    }

    pub fn display_name(&self) -> &str {
        if !self.custom_display_name.is_empty() {
            &self.custom_display_name
        } else {
            &self.actor_name
        }
    }

    pub fn is_facing(&self, target: &BaseActor, half_angle_deg: f32) -> bool {
        let half_angle_rad = half_angle_deg.to_radians();
        let angle_to = common::Vector3::angle_xz(
            self.position_x,
            self.position_z,
            target.position_x,
            target.position_z,
        );
        let diff = (angle_to - self.rotation).rem_euclid(std::f32::consts::TAU);
        diff.min(std::f32::consts::TAU - diff) <= half_angle_rad
    }
}

// ---------------------------------------------------------------------------
// Character — base for both Player and Npc. Adds combat/stat state.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CharaState {
    pub is_static: bool,
    pub is_moving_to_spawn: bool,
    pub is_auto_attack_enabled: bool,

    pub model_id: u32,
    /// 28-slot appearance/equipment id table sent to the client via
    /// `SetActorAppearancePacket` (opcode 0x00D6). Indices mirror the C#
    /// `SetActorAppearancePacket` constants — SIZE=0, COLORINFO=1,
    /// FACEINFO=2, HIGHLIGHT_HAIR=3, VOICE=4, weapons 5..11, gear 12..27.
    pub appearance_ids: [u32; 28],
    /// Gamedata actor-class id — used by `Npc::CreateScriptBindPacket`
    /// as the 7th LuaParam on the wire (Meteor
    /// `Actors/Chara/Npc/Npc.cs:197`). Zero for Players.
    pub actor_class_id: u32,
    /// Bit field mirroring `actorClass.propertyFlags` (Meteor
    /// `Actors/Chara/Npc/Npc.cs:98`). Each set bit becomes a
    /// `charaWork.property[i] = 1` entry in `GetInitPackets()`,
    /// which the 1.x client reads to flip per-actor capabilities
    /// such as nameplate visibility, collision, and targetability.
    /// Zero for Players (Player.cs hardcodes its own bit set).
    pub property_flags: u32,
    /// `playerWork.*` profile fields emitted in the `/_init` property dump
    /// mirrored from C# `Player.GetInitPackets()`. These exist on
    /// CharaState so the zone-in bundle can read them without a second DB
    /// round-trip; only populated for players, zero for NPCs.
    pub tribe: u8,
    pub guardian: u8,
    pub birthday_day: u8,
    pub birthday_month: u8,
    pub initial_town: u8,
    pub rest_bonus_exp_rate: i32,
    /// Set when `player.lua:onBeginLogin` invokes `player:SetLoginDirector(...)`.
    /// Non-zero → the ScriptBind LuaParam layout switches to the
    /// "tutorial with init director" variant C# `Player.CreateScriptBindPacket`
    /// uses when `loginInitDirector != null`, and the zone-in bundle emits
    /// the director's 7-packet spawn sequence so the `Actor(…)` reference
    /// resolves on the client side.
    pub login_director_actor_id: u32,
    pub animation_id: u32,
    pub current_target: u32,
    pub current_locked_target: u32,
    pub current_actor_icon: u32,
    pub current_job: u16,
    pub new_main_state: u16,

    pub spawn_x: f32,
    pub spawn_y: f32,
    pub spawn_z: f32,

    pub hp: i16,
    pub max_hp: i16,
    pub mp: i16,
    pub max_mp: i16,
    pub tp: u16,

    pub class: i16,
    pub level: i16,
    pub extra_int: i32,
    pub extra_uint: u32,
    pub extra_float: f32,

    /// Modifier map (stat bonuses, statuses, trait-derived buffs). Matches
    /// the C# `Dictionary<uint, double>` on Character.
    pub mods: ModifierMap,
    /// Indexed by the `STAT_*` constants from scripts/global.lua.
    pub stats: [i16; chara::STAT_COUNT],
}

// `[i16; 36]` is past the stdlib auto-derived `Default` window.
#[allow(clippy::derivable_impls)]
impl Default for CharaState {
    fn default() -> Self {
        Self {
            is_static: false,
            is_moving_to_spawn: false,
            is_auto_attack_enabled: false,
            model_id: 0,
            appearance_ids: [0u32; 28],
            actor_class_id: 0,
            property_flags: 0,
            tribe: 0,
            guardian: 0,
            birthday_day: 0,
            birthday_month: 0,
            initial_town: 0,
            rest_bonus_exp_rate: 0,
            login_director_actor_id: 0,
            animation_id: 0,
            current_target: 0,
            current_locked_target: 0,
            current_actor_icon: 0,
            current_job: 0,
            new_main_state: 0,
            spawn_x: 0.0,
            spawn_y: 0.0,
            spawn_z: 0.0,
            hp: 0,
            max_hp: 0,
            mp: 0,
            max_mp: 0,
            tp: 0,
            class: 0,
            level: 0,
            extra_int: 0,
            extra_uint: 0,
            extra_float: 0.0,
            mods: ModifierMap::default(),
            stats: [0; chara::STAT_COUNT],
        }
    }
}

impl CharaState {
    pub fn is_dead(&self) -> bool {
        self.hp <= 0
    }

    pub fn is_alive(&self) -> bool {
        self.hp > 0
    }

    pub fn hpp(&self) -> u8 {
        if self.max_hp == 0 {
            0
        } else {
            ((self.hp as i32 * 100) / self.max_hp as i32).clamp(0, 100) as u8
        }
    }

    pub fn mpp(&self) -> u8 {
        if self.max_mp == 0 {
            0
        } else {
            ((self.mp as i32 * 100) / self.max_mp as i32).clamp(0, 100) as u8
        }
    }

    pub fn tpp(&self) -> u8 {
        ((self.tp as u32).min(3000) * 100 / 3000) as u8
    }
}

#[derive(Debug, Clone, Default)]
pub struct Character {
    pub base: BaseActor,
    pub chara: CharaState,
    /// Status-effect bookkeeping (effect map + the client-visible
    /// `charaWork.status[20]` / `charaWork.statusShownTime[20]` arrays).
    pub status_effects: crate::status::StatusEffectContainer,
    /// Top-level AI / state-machine orchestrator. Populated for actors
    /// that actually fight; stubbed for the static/NPC cases.
    pub ai_container: crate::battle::AIContainer,
    /// Enmity tracker — populated for BattleNpcs; empty for Players.
    pub hate: crate::battle::HateContainer,
    /// Transient battle state (cast-gauge speed, timing flags).
    pub battle_temp: crate::battle::BattleTemp,
    /// Persistent battle state — skill levels + physical level.
    pub battle_save: crate::battle::BattleSave,
    /// Currently-running scripted event (matches the C#
    /// `currentEventOwner` / `currentEventName` / `currentEventType`
    /// fields on `PlayerWork`). Kept on Character so NPCs can also
    /// drive kicks, though in practice only Player rows populate it.
    pub event_session: crate::event::EventSession,
}

impl Character {
    pub fn new(actor_id: u32) -> Self {
        Self {
            base: BaseActor::new(actor_id),
            chara: CharaState {
                is_auto_attack_enabled: true,
                ..Default::default()
            },
            status_effects: crate::status::StatusEffectContainer::new(actor_id),
            ai_container: crate::battle::AIContainer::new(actor_id, None, None),
            hate: crate::battle::HateContainer::new(actor_id),
            battle_temp: crate::battle::BattleTemp::default(),
            battle_save: crate::battle::BattleSave::default(),
            event_session: crate::event::EventSession::default(),
        }
    }

}

// ---------------------------------------------------------------------------
// Player — owned by a logged-in human session.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct PlayerState {
    pub current_event_owner: u32,
    pub current_event_name: String,
    pub current_event_type: u8,

    pub destination_zone: u32,
    pub destination_spawn_type: u16,

    pub current_title: u32,
    pub play_time: u32,
    pub last_play_time_update: u32,

    pub is_gm: bool,
    pub is_zone_changing: bool,

    pub gc_current: u8,
    pub gc_rank_limsa: u8,
    pub gc_rank_gridania: u8,
    pub gc_rank_uldah: u8,

    pub has_chocobo: bool,
    pub has_goobbue: bool,
    pub chocobo_name: String,
    pub mount_state: u8,
    pub chocobo_appearance: u8,
    pub rental_expire_time: u32,
    pub rental_min_left: u8,

    pub achievement_points: u32,
    pub homepoint: u32,
    pub homepoint_inn: u8,
    pub current_ls_plate: u32,
    pub repair_type: u8,
    pub sent_retainer_spawn: bool,
    pub rest_bonus_exp_rate: i32,
    /// Max class level across all jobs; refreshed when the DB save loads or
    /// a level changes. Mirrors the C# `GetHighestLevel()` scan result.
    pub highest_level_cache: i32,
}

#[derive(Debug, Clone, Default)]
pub struct Player {
    pub character: Character,
    pub player: PlayerState,
    pub helpers: player::PlayerHelperState,
    /// Every item bag the player owns. Keyed by the `PKG_*` code so lookups
    /// match `Player.GetItemPackage(code)` from C#. Populated on login by
    /// `Database::load_player_character` + the game loop.
    pub item_packages: std::collections::HashMap<u16, crate::inventory::ItemPackage>,
    /// Equipment view (ReferencedItemPackage wrapping the NORMAL bag).
    pub equipment: Option<crate::inventory::ReferencedItemPackage>,
}

impl Player {
    pub fn new(actor_id: u32) -> Self {
        let mut me = Self {
            character: Character::new(actor_id),
            player: PlayerState {
                is_zone_changing: true,
                ..Default::default()
            },
            helpers: player::PlayerHelperState::default(),
            item_packages: std::collections::HashMap::new(),
            equipment: None,
        };
        me.install_default_packages();
        me
    }

    fn install_default_packages(&mut self) {
        use crate::inventory::{
            ItemPackage, PKG_BAZAAR, PKG_CURRENCY_CRYSTALS, PKG_EQUIPMENT, PKG_KEYITEMS, PKG_LOOT,
            PKG_MELDREQUEST, PKG_NORMAL, PKG_TRADE, ReferencedItemPackage, default_capacity,
        };
        let aid = self.character.base.actor_id;
        for code in [
            PKG_NORMAL,
            PKG_LOOT,
            PKG_MELDREQUEST,
            PKG_BAZAAR,
            PKG_CURRENCY_CRYSTALS,
            PKG_KEYITEMS,
            PKG_TRADE,
        ] {
            self.item_packages
                .insert(code, ItemPackage::new(aid, default_capacity(code), code));
        }
        self.equipment = Some(ReferencedItemPackage::new(
            aid,
            default_capacity(PKG_EQUIPMENT),
            PKG_EQUIPMENT,
        ));
    }

    /// Convenience: short-hand for `player.item_packages.get(&code)`.
    pub fn get_item_package(&self, code: u16) -> Option<&crate::inventory::ItemPackage> {
        self.item_packages.get(&code)
    }

    pub fn get_item_package_mut(
        &mut self,
        code: u16,
    ) -> Option<&mut crate::inventory::ItemPackage> {
        self.item_packages.get_mut(&code)
    }
}

// ---------------------------------------------------------------------------
// NPC variants. `Npc` is a decorative/talking NPC; `BattleNpc` has AI.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct Npc {
    pub character: Character,
    pub actor_class_id: u32,
}

impl Npc {
    pub fn new(actor_id: u32, actor_class_id: u32) -> Self {
        Self {
            character: Character::new(actor_id),
            actor_class_id,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BattleNpc {
    pub character: Character,
    pub actor_class_id: u32,
    pub spawn_type: u8,
    pub level: u8,
    pub respawn_time: u32,
    pub despawn_time: u32,
    /// Behaviour controller id; wired up to the AI controller in the full
    /// port. Phase 4 leaves this as an opaque identifier.
    pub controller_id: u32,
}

impl BattleNpc {
    pub fn new(actor_id: u32, actor_class_id: u32) -> Self {
        Self {
            character: Character::new(actor_id),
            actor_class_id,
            ..Default::default()
        }
    }
}

/// Canonical list of static world/command/debug/event actors, corresponding
/// to StaticActors.cs. These are spawned once per zone server and assigned
/// fixed actor ids.
pub mod r#static {
    pub const WORLD_MANAGER_ACTOR_ID: u32 = 0x4000_0001;
    pub const DEBUG_ACTOR_ID: u32 = 0x4000_0002;
    pub const LOG_MANAGER_ACTOR_ID: u32 = 0x4000_0003;
    pub const CHAT_MANAGER_ACTOR_ID: u32 = 0x4000_0004;
    pub const ITEM_PACKAGE_ACTOR_ID: u32 = 0x4000_0005;
    pub const SCHEDULER_ACTOR_ID: u32 = 0x4000_0006;
    pub const DIRECTOR_ACTOR_ID: u32 = 0x4000_0007;
    pub const PARTY_ACTOR_ID: u32 = 0x4000_0008;
    pub const LAYOUT_ACTOR_ID: u32 = 0x4000_0009;
    pub const EVENT_ACTOR_ID: u32 = 0x4000_000A;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hpp_math_is_clamped() {
        let mut state = CharaState {
            hp: 50,
            max_hp: 100,
            ..Default::default()
        };
        assert_eq!(state.hpp(), 50);
        state.hp = 200;
        assert_eq!(state.hpp(), 100);
        state.hp = -10;
        assert_eq!(state.hpp(), 0);
    }

    #[test]
    fn set_position_preserves_old_values() {
        let mut actor = BaseActor::new(1);
        actor.position_x = 10.0;
        actor.set_position(Vector3::new(20.0, 0.0, 0.0), 0.5);
        assert_eq!(actor.old_position_x, 10.0);
        assert_eq!(actor.position_x, 20.0);
    }
}
