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

//! Map-server data objects. Ported from `DataObjects/` with pragmatic
//! trimming: this Phase-4 scaffold keeps the shape (fields) the rest of the
//! code needs; full logic on each struct lives in dedicated modules where
//! it's non-trivial (inventory, events, trades, search).

#![allow(dead_code)]

use tokio::sync::mpsc;

/// Per-client connection state.
#[derive(Clone)]
pub struct ClientHandle {
    pub session_id: u32,
    pub out: mpsc::Sender<Vec<u8>>,
}

impl ClientHandle {
    pub fn new(session_id: u32, out: mpsc::Sender<Vec<u8>>) -> Self {
        Self { session_id, out }
    }

    pub async fn send_bytes(&self, bytes: Vec<u8>) {
        let _ = self.out.send(bytes).await;
    }
}

/// Ported from DataObjects/Session.cs. Mutable player-session scratchpad
/// used by the packet processor.
#[derive(Debug, Clone, Default)]
pub struct Session {
    pub id: u32,
    pub language_code: u32,
    pub is_updates_locked: bool,
    pub error_message: String,
    pub current_zone_id: u32,
    pub destination_zone_id: u32,
    pub destination_spawn_type: u8,
    pub destination_x: f32,
    pub destination_y: f32,
    pub destination_z: f32,
    pub destination_rot: f32,
    /// Pending login director captured during `onBeginLogin`'s
    /// `zone:CreateDirector(...)` call. `zone_in_bundle` reads this to
    /// decide whether to emit the director's 7-packet spawn sequence
    /// and which actor id to reference in the player's ScriptBind
    /// LuaParam list. `None` → no director (non-tutorial login).
    pub login_director: Option<LoginDirectorSpec>,
    /// Pending `KickEvent` captured during `onBeginLogin`. The tutorial
    /// flow calls `player:KickEvent(director, "noticeEvent", true)`
    /// AFTER the director is spawned — `zone_in_bundle` emits the
    /// corresponding `KickEventPacket` as the final step so the client
    /// dispatches the event on the freshly-spawned director actor.
    pub pending_kick_event: Option<PendingKickEvent>,
    /// Retainer currently summoned by this session. Mirrors
    /// `Player.currentSpawnedRetainer` in C#. `None` means no
    /// retainer is in the world for this player right now; on
    /// `SpawnMyRetainer` the processor writes a snapshot here that
    /// Lua can inspect via `player:GetSpawnedRetainer()` / the
    /// bell-NPC scripts can address directly.
    pub spawned_retainer: Option<SpawnedRetainer>,
    /// Set by `ObjectBed.lua`'s `player:SetSleeping()` binding right
    /// before the script calls `QuitGame`/`Logout`. The zone-in
    /// packet bundle reads this back on the next session to emit the
    /// right `SetPlayerDreamPacket` arm, and the processor's logout
    /// path persists the snap position so the re-login puts the
    /// player at the bed rather than at the last free-movement
    /// position. Cleared by the next zone-out / wake event.
    pub is_sleeping: bool,
    /// Scripted dream state — `player:StartDream(dreamId)` sets this
    /// to `Some(dreamId)` and emits the fade-in packet; `EndDream()`
    /// clears it. `IsDreaming()` reads this via the snapshot.
    pub current_dream_id: Option<u8>,
}

/// Runtime-only snapshot of an in-world retainer. Holds the minimal
/// fields the bell NPC scripts + item-trade path need; the full
/// [`crate::npc::Retainer`] object (with `ItemPackage` map) is built
/// on demand when the client wants to exchange items.
#[derive(Debug, Clone, PartialEq)]
pub struct SpawnedRetainer {
    /// `server_retainers.id` — primary key into the retainer catalog.
    pub retainer_id: u32,
    /// `server_retainers.actorClassId` — drives the 3D model / race.
    pub actor_class_id: u32,
    /// `gamedata_actor_class.classPath` for the spawned retainer.
    /// Cached on the spawn snapshot so the despawn-side code (and
    /// any future event-route lookup) doesn't need a second DB round-
    /// trip per summon.
    pub class_path: String,
    /// `server_retainers.name` — display name over the retainer's head.
    pub name: String,
    /// Composite actor id allocated by `apply_spawn_my_retainer` —
    /// `(4 << 28) | (zone << 19) | local_id` where `local_id` is
    /// derived from the owning player's actor id (top bit set so the
    /// boot spawn pass's sequential ids never collide). Routes the
    /// despawn `RemoveActor` and lets `EventStart` handlers detect
    /// "this is my retainer" by id-matching against the session
    /// snapshot, mirroring Meteor's `playerActor.currentSpawnedRetainer.actorId`
    /// check at `Map Server/PacketProcessor.cs:205`.
    pub actor_id: u32,
    /// Position in the zone, relative to the retainer bell the player
    /// clicked. Matches the `SpawnMyRetainer(bell, idx)` math in C#.
    pub position: (f32, f32, f32),
    pub rotation: f32,
    /// Mirror of `Player.sentRetainerSpawn` — flip true once the
    /// zone's tick loop sends the spawn/init/event-status packet
    /// trio to the owning player. Allocated here so the processor
    /// can keep the flag alongside the other summon state without a
    /// second HashMap.
    pub sent_spawn_packets: bool,
}

#[derive(Debug, Clone)]
pub struct LoginDirectorSpec {
    pub actor_id: u32,
    pub zone_actor_id: u32,
    pub class_path: String,
    pub class_name: String,
}

#[derive(Debug, Clone)]
pub struct PendingKickEvent {
    pub trigger_actor_id: u32,
    pub owner_actor_id: u32,
    pub event_name: String,
    /// LuaParams passed from the script's `player:KickEvent(actor,
    /// "eventName", …args)` call. C# propagates these verbatim into
    /// the `KickEventPacket` body at offset 0x30; without them the
    /// client's event dispatcher doesn't have the arguments the
    /// tutorial opening event expects and silently no-ops.
    pub args: Vec<common::luaparam::LuaParam>,
}

impl Session {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            language_code: 1,
            is_updates_locked: true,
            ..Default::default()
        }
    }
}

/// Inventory row. Fields mirror `DataObjects/InventoryItem.cs` so the packet
/// builders can dump a contiguous blob.
#[derive(Debug, Clone, Default)]
pub struct InventoryItem {
    pub unique_id: u64,
    pub item_id: u32,
    pub quantity: i32,
    pub quality: u8,
    pub slot: u16,
    pub link_slot: u16,
    pub item_package: u16,

    pub tag: ItemTag,
}

#[derive(Debug, Clone, Default)]
pub struct ItemTag {
    pub durability: u32,
    pub use_count: u16,
    pub materia_id: u32,
    pub materia_life: u32,
    pub main_quality: u8,
    pub polish: u32,
    pub param1: u32,
    pub param2: u32,
    pub param3: u32,
    pub spiritbind: u16,
}

/// Mainhand/offhand weapon stats ported from `gamedata_items_weapon`.
/// `None` on non-weapon items (the JOIN yields NULLs, parsed as None).
///
/// Loaded once at boot and read by `apply_player_weapon_stats` — unlike
/// `gear_bonuses` which sums across all slots, weapon attributes come
/// from the main-hand only (+ offhand for dual-wield / shield), and
/// they're `set` into modifiers (replacing the previous weapon's
/// values) rather than added.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WeaponAttributes {
    /// Auto-attack delay in milliseconds. DB stores `damageInterval`
    /// as REAL seconds (typical: 2.5–4.0) — loader multiplies by 1000
    /// and rounds to u32.
    pub delay_ms: u32,
    /// `damageAttributeType1` — hit type (Slashing / Piercing / Blunt /
    /// etc.). Raw enum value as stored; feeds
    /// [`Modifier::AttackType`].
    pub attack_type: u16,
    /// `frequency` — number of hits per swing. 1 = single-hit melee,
    /// 2 = dual-wield / paired daggers, etc. Baseline is 1; populated
    /// by the equipped weapon.
    pub hit_count: u16,
    /// `damagePower` — the raw weapon-damage component of auto-attacks.
    /// Not a Modifier because nothing on the wire cares; stored on
    /// ItemData and read by `attack_calculate_base_damage` directly
    /// off the equipped item.
    pub damage_power: u16,
    /// `attack` — flat Attack stat bonus the weapon confers. Added to
    /// [`Modifier::Attack`] on equip alongside paramBonus deltas.
    pub attack: u16,
    /// `parry` — flat Parry stat bonus. Added to [`Modifier::Parry`]
    /// on equip.
    pub parry: u16,
}

/// Reference data for an item, keyed by `item_id`. Populated from the
/// `server_items` table on startup.
#[derive(Debug, Clone, Default)]
pub struct ItemData {
    pub id: u32,
    pub name: String,
    pub singular: String,
    pub plural: String,
    pub start_with_vowel: bool,
    pub kana: String,
    pub description: String,
    pub icon: u16,
    pub rarity: u16,
    pub item_ui_category: u16,
    pub stack_size: u32,
    pub item_level: u16,
    pub equip_level: u16,
    pub price: u32,
    pub buy_price: u32,
    pub sell_price: u32,
    pub bazaar_category: u8,
    pub unknown1: u8,
    pub unknown2: u8,
    pub is_exclusive: bool,
    pub is_rare: bool,
    pub is_ex: bool,
    pub is_dyeable: bool,
    pub is_tradable: bool,
    pub is_untradable: bool,
    pub is_soldable: bool,
    /// Pre-parsed stat bonuses from `gamedata_items_equipment.paramBonus`.
    /// Each pair is `(modifier_id, delta)` — add `delta` to
    /// `Modifier::from_u32(modifier_id)` when this item is equipped.
    ///
    /// Decoding rule: `modifier_id = paramBonusType - 15001` for types
    /// in `15001..=15100`. Other ranges are ignored — `16xxx` are item
    /// class-kind flags (what job can equip), `20xxx` sit in an
    /// unmapped Meteor-side namespace (possibly `ParamGrow`-class
    /// effects), `1015xxx` is a `1_000_000`-offset conditional/HQ
    /// variant of `15xxx` that never landed in Meteor's C# loader
    /// either, and `-1` is the empty-slot sentinel. Bogus rows (e.g.
    /// non-zero value at `-1` type) get dropped silently.
    ///
    /// Empty on non-equipment items (consumables, crafting materials,
    /// crystal stacks) because `gamedata_items_equipment` has no row —
    /// the loader's LEFT JOIN fills with `NULL` and we skip.
    pub gear_bonuses: Vec<(u32, i32)>,
    /// `Some` for weapons (main-hand / offhand / shields), `None`
    /// otherwise. Read by `apply_player_weapon_stats` during recalc —
    /// the main-hand entry drives Delay / HitCount / AttackType; both
    /// hands contribute their `attack`/`parry` flats.
    pub weapon: Option<WeaponAttributes>,
}

/// Seamless zone boundary. Port of `DataObjects/SeamlessBoundry.cs`. Each
/// row represents a pair of zones that share a border:
///
/// * `zone1_*` — bounding box inside which the player is in `zone_id_1`.
/// * `zone2_*` — bounding box inside which the player is in `zone_id_2`.
/// * `merge_*` — the narrow "both zones are visible" strip where the
///   WorldManager calls `MergeZones` to pull in the adjacent zone's actors.
#[derive(Debug, Clone, Copy)]
pub struct SeamlessBoundary {
    pub id: u32,
    pub region_id: u32,
    pub zone_id_1: u32,
    pub zone_id_2: u32,

    pub zone1_x1: f32,
    pub zone1_y1: f32,
    pub zone1_x2: f32,
    pub zone1_y2: f32,

    pub zone2_x1: f32,
    pub zone2_y1: f32,
    pub zone2_x2: f32,
    pub zone2_y2: f32,

    pub merge_x1: f32,
    pub merge_y1: f32,
    pub merge_x2: f32,
    pub merge_y2: f32,
}

/// `CheckPosInBounds(x, y, x1, y1, x2, y2)` — matches the C# axis-order-
/// agnostic bounding-box check (either `x1 < x < x2` or `x1 > x > x2`).
pub fn check_pos_in_bounds(x: f32, y: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> bool {
    let x_ok = (x1 < x && x < x2) || (x1 > x && x > x2);
    let y_ok = (y1 < y && y < y2) || (y1 > y && y > y2);
    x_ok && y_ok
}

/// One row in `server_zones_spawnlocations` — a named entry point that
/// `DoZoneChange(player, zoneEntrance)` warps to.
#[derive(Debug, Clone)]
pub struct ZoneEntrance {
    pub id: u32,
    pub zone_id: u32,
    pub private_area_name: Option<String>,
    pub private_area_level: i32,
    pub spawn_type: u8,
    pub spawn_x: f32,
    pub spawn_y: f32,
    pub spawn_z: f32,
    pub spawn_rotation: f32,
}

/// Zone-to-zone teleport row (aetheryte destinations, cutscene-driven
/// transitions, `DataObjects/ZoneConnection.cs`).
#[derive(Debug, Clone)]
pub struct ZoneConnection {
    pub id: u32,
    pub zone_id: u32,
    pub destination_zone_id: u32,
    pub destination_spawn_type: u8,
    pub destination_x: f32,
    pub destination_y: f32,
    pub destination_z: f32,
    pub destination_rot: f32,
}

/// Pending trade transaction (ported from TradeTransaction.cs). Both sides of
/// a trade reference the same struct via an `Arc<Mutex<_>>` in the real
/// implementation; Phase 4 keeps just the data shape.
#[derive(Debug, Clone, Default)]
pub struct TradeTransaction {
    pub initiator_id: u32,
    pub target_id: u32,
    pub initiator_items: Vec<InventoryItem>,
    pub target_items: Vec<InventoryItem>,
    pub initiator_gil: u32,
    pub target_gil: u32,
    pub initiator_accepted: bool,
    pub target_accepted: bool,
}

/// Represents a player's search-board entry (`DataObjects/SearchEntry.cs`).
#[derive(Debug, Clone, Default)]
pub struct SearchEntry {
    pub actor_id: u32,
    pub name: String,
    pub message: String,
    pub current_class: u8,
    pub current_level: u8,
    pub zone_id: u32,
}

/// Represents one guildleve (`DataObjects/GuildleveData.cs`).
#[derive(Debug, Clone, Default)]
pub struct GuildleveData {
    pub id: u32,
    pub zone_id: u32,
    pub name: String,
    pub difficulty: u8,
    pub leve_type: u8,
    pub reward_exp: u32,
    pub reward_gil: u32,
}
