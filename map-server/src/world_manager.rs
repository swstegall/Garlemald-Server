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

//! Zone + session registry and transition orchestrator. Port of the
//! `WorldManager.cs` surface that deals with zones + boundaries +
//! session handoff. Heavy sub-systems (Director, Group, Party) live in
//! their own modules.
//!
//! Character state (Players, Npcs, BattleNpcs) lives in
//! `runtime::actor_registry::ActorRegistry`. WorldManager only owns:
//!
//! * `zones` — canonical `zone::Zone` instances keyed by zone id.
//! * `zone_entrances` — named warp points keyed by entrance id.
//! * `seamless_boundaries` — per-region boundary boxes for seamless
//!   zone transitions.
//! * `sessions` / `clients` — per-socket state.
//!
//! All `RwLock<HashMap>` so independent zones / sessions don't contend.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;

use common::Vector3;

use crate::data::{ClientHandle, SeamlessBoundary, Session, ZoneEntrance, check_pos_in_bounds};
use crate::database::{Database, PrivateAreaRow, ZoneRow};
use crate::packets::send as tx;
use crate::runtime::actor_registry::ActorRegistry;
use crate::zone::navmesh::StubNavmeshLoader;
use crate::zone::private_area::PrivateArea;
use crate::zone::zone::Zone;

/// Empty `/_init` SetActorProperty for a director. Mirrors C#
/// `Director.GetInitPackets` which builds a `SetActorPropetyPacket`
/// with only an `AddTarget()` call — no actual properties. On the wire
/// this is the 0x0137 SetActorProperty body with:
///   - byte 0: runningByteTotal = 1 + target.len()
///   - byte 1: target marker = 0x82 + target.len()
///   - byte 2..: target path ("/_init")
/// Body is zero-filled to the 0xA8 packet size and wrapped as a
/// game-message subpacket (opcode 0x0137).
fn build_director_init_packet(actor_id: u32) -> common::subpacket::SubPacket {
    use std::io::Write as _;
    let mut data = vec![0u8; 0xA8 - 0x20];
    let target = b"/_init";
    let running_total = (1 + target.len()) as u8;
    data[0] = running_total;
    // Write at offset 1: marker byte, then target bytes.
    let mut c = std::io::Cursor::new(&mut data[..]);
    c.set_position(1);
    c.write_all(&[0x82u8 + target.len() as u8]).unwrap();
    c.write_all(target).unwrap();
    common::subpacket::SubPacket::new(
        crate::packets::opcodes::OP_SET_ACTOR_PROPERTY,
        actor_id,
        data,
    )
}

/// Mirror of C# `Director.GenerateActorName` zone-name abbreviation:
/// `Field→Fld, Dungeon→Dgn, Town→Twn, Battle→Btl, Test→Tes,
/// Event→Evt, Ship→Shp, Office→Ofc`. Used when building the director's
/// actor-name field so it matches the format the client expects (e.g.
/// `ocn0Battle02` → `ocn0Btl02`).
fn shorten_zone_name(zone_name: &str) -> String {
    zone_name
        .replace("Field", "Fld")
        .replace("Dungeon", "Dgn")
        .replace("Town", "Twn")
        .replace("Battle", "Btl")
        .replace("Test", "Tes")
        .replace("Event", "Evt")
        .replace("Ship", "Shp")
        .replace("Office", "Ofc")
}

/// Append the full 7-packet spawn sequence for a zone-resident "master"
/// actor (area master, debug, or world master) — matches C# `Actor.
/// GetSpawnPackets`: AddActor(0), Speed, SpawnPosition(spawnType=1),
/// Name, State(passive), IsZoning(false), ScriptBind. All three master
/// actors share this shape; the only thing that varies is the actor id,
/// name, class name, and the LuaParam list that goes into the
/// `ActorInstantiate` script-bind packet.
///
/// Re-enabled after rebuilding ScriptBind LuaParams to match Project
/// Meteor's `Zone.CreateScriptBindPacket` / `DebugProg.
/// CreateScriptBindPacket` / `WorldMaster.CreateScriptBindPacket`
/// verbatim. The earlier STATUS_INVALID_PARAMETER crash traced to a
/// param list the client couldn't resolve; the current call sites in
/// `send_zone_in_bundle` build the full 15/9/7-param lists the C#
/// reference ships.
fn push_master_spawn(
    subpackets: &mut Vec<common::subpacket::SubPacket>,
    actor_id: u32,
    actor_name: String,
    class_name: String,
    script_bind_params: Vec<common::luaparam::LuaParam>,
) {
    subpackets.push(tx::actor::build_add_actor(actor_id, 0));
    subpackets.push(tx::actor::build_set_actor_speed_default(actor_id));
    // C# `Actor.CreateSpawnPositonPacket` passes `actorId` as the second
    // (target) arg for plain actors. The `-1` sentinel is player-self only
    // — using it for NPCs trips STATUS_INVALID_PARAMETER inside the client's
    // actor-resolve path and raises 0xc000000d a couple seconds after the
    // zone-in packets are consumed.
    subpackets.push(tx::actor::build_set_actor_position(
        actor_id, actor_id as i32, 0.0, 0.0, 0.0, 0.0, 0x1, false,
    ));
    // C# `CreateNamePacket` uses displayNameId=0 when a customDisplayName
    // is set; all three masters ship with names ("debug", "worldMaster",
    // "_areaMaster@…"). The area master's displayNameId is technically
    // 0xFFFFFFFF in C# but the packet skips that branch when a custom
    // name is present, so 0 here is fine.
    subpackets.push(tx::actor::build_set_actor_name(actor_id, 0, &actor_name));
    subpackets.push(tx::actor::build_set_actor_state(actor_id, 0, 0));
    subpackets.push(tx::actor::build_set_actor_is_zoning(actor_id, false));
    subpackets.push(tx::actor::build_actor_instantiate(
        actor_id,
        0,
        0x3040,
        &actor_name,
        &class_name,
        &script_bind_params,
    ));
}

/// Emit the 11-packet NPC spawn bundle a single visible actor needs on
/// a client's zone-in. Mirrors C# `Npc.GetSpawnPackets`:
///   AddActor + Speed + SpawnPosition + Appearance + Name + State +
///   SubState + InitStatus + Icon + IsZoning + ScriptBind(0x00CC).
///
/// `GetEventConditionPackets` (0x016B) / `GetSetEventStatusPackets`
/// (0x0136) are still omitted — Meteor only emits them when the NPC
/// has parsed event-condition entries, and we'll wire that once the
/// event-table parser lands. The 11-packet bundle alone is enough to
/// give the client a renderable nameplate.
/// Lowercase every path segment except the final (class) component.
/// Turns `/Chara/Npc/Populace/PopulaceStandard` (what our gamedata
/// stores) into `/chara/npc/populace/PopulaceStandard` (what Meteor
/// sends on the wire and the 1.x client's script loader expects).
fn lowercase_class_path(path: &str) -> String {
    if let Some(last_slash) = path.rfind('/') {
        let prefix = &path[..last_slash];
        let tail = &path[last_slash..];
        format!("{}{}", prefix.to_lowercase(), tail)
    } else {
        path.to_string()
    }
}

/// Format an NPC's actor name the way Meteor's
/// `Actor.GenerateActorName` (Map Server/Actors/Actor.cs:501) does:
///   "<classAbbrev>_<zoneAbbrev>_<numBase63>@<zoneId:X3><privLevel:X2>"
/// Example for tribe Miqo'te populace #1 in zone 193 ocn0Battle02:
///   "pplStd_ocn0Btl02_01@0C100"
fn generate_npc_actor_name(
    class_name: &str,
    zone_name: &str,
    actor_number: u32,
    zone_id: u32,
    priv_level: u32,
) -> String {
    fn lowercase_first(s: &str) -> String {
        let mut c = s.chars();
        match c.next() {
            Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
            None => String::new(),
        }
    }
    fn replace_all(s: &str, subs: &[(&str, &str)]) -> String {
        let mut out = s.to_string();
        for (a, b) in subs {
            out = out.replace(a, b);
        }
        out
    }
    let class_short = replace_all(
        class_name,
        &[
            ("Populace", "Ppl"),
            ("Monster", "Mon"),
            ("Crowd", "Crd"),
            ("MapObj", "Map"),
            ("Object", "Obj"),
            ("Retainer", "Rtn"),
            ("Standard", "Std"),
        ],
    );
    let zone_short = replace_all(
        zone_name,
        &[
            ("Field", "Fld"),
            ("Dungeon", "Dgn"),
            ("Town", "Twn"),
            ("Battle", "Btl"),
            ("Test", "Tes"),
            ("Event", "Evt"),
            ("Ship", "Shp"),
            ("Office", "Ofc"),
        ],
    );
    let class_lower = lowercase_first(&class_short);
    let zone_lower = lowercase_first(&zone_short);
    // Truncate class to fit under 20 chars combined; mirrors Meteor's
    // `className.Substring(0, 20 - zoneName.Length)`.
    let max_class_len = 20usize.saturating_sub(zone_lower.len());
    let class_trunc: String = class_lower.chars().take(max_class_len).collect();
    // Base-63 is Meteor's custom alphabet. For actor numbers <= 62 we
    // emit a 2-char zero-padded decimal string, which matches what
    // Meteor's `pplStd_ocn0Btl02_01@0C100` capture shows for actor #1.
    // Above 62 we fall back to decimal — the server doesn't spawn enough
    // NPCs per zone today for that path to matter.
    let num_str = if actor_number < 100 {
        format!("{:02}", actor_number)
    } else {
        format!("{}", actor_number)
    };
    format!(
        "{}_{}_{}@{:03X}{:02X}",
        class_trunc, zone_lower, num_str, zone_id, priv_level
    )
}

/// Public entry for the retainer-spawn path
/// (`processor::apply_spawn_my_retainer`). Wraps `push_npc_spawn` so
/// the processor can build a session-targeted spawn bundle for a
/// session-private retainer without taking a hard dep on the private
/// helper. `priv_level=0` matches a root-zone spawn (retainers always
/// spawn in the player's current zone, never in a private area
/// instance).
pub fn build_retainer_spawn_bundle(
    character: &crate::actor::Character,
    zone_name: &str,
) -> Vec<common::subpacket::SubPacket> {
    let mut out = Vec::new();
    push_npc_spawn(&mut out, character, zone_name, 0);
    out
}

fn push_npc_spawn(
    subpackets: &mut Vec<common::subpacket::SubPacket>,
    character: &crate::actor::Character,
    zone_name: &str,
    priv_level: u32,
) {
    let actor_id = character.base.actor_id;
    // Meteor's `Actor.CreateNamePacket` (Map Server/Actors/Actor.cs:153)
    // branches on `customDisplayName`:
    //   * customDisplayName != null     → emit (displayNameId=0, name=custom)
    //   * displayNameId in {0, 0xFFFFFFFF} or customDisplayName != null
    //                                   → emit custom (possibly "")
    //   * otherwise                     → emit (displayNameId, "") so the
    //                                     client looks the name up in
    //                                     its localized string table.
    // Our `display_name()` helper falls back to `actor_name` when there is
    // no custom name, which sent garbage like "_npc@00001" as the custom
    // override and made the client ignore the valid displayNameId. For
    // NPCs with a valid displayNameId and no custom name we must emit an
    // empty custom string so the client pulls the name from its table.
    let custom_display_name = character.base.custom_display_name.clone();
    let display_name_id = character.base.display_name_id;
    let (packet_name_id, packet_name) = if !custom_display_name.is_empty() {
        (0u32, custom_display_name.clone())
    } else if display_name_id == 0 || display_name_id == 0xFFFF_FFFF {
        (display_name_id, String::new())
    } else {
        (display_name_id, String::new())
    };
    let position = character.base.position();
    let rotation = character.base.rotation;
    let state = character.base.current_main_state as u8;
    let model_id = character.chara.model_id;
    let appearance_ids = character.chara.appearance_ids;
    // Meteor stashes the seed's animationId into `currentSubState.motionPack`
    // (Npc.cs:79), which CreateSubStatePacket then writes at offset 0x06.
    // We don't have a separate SubState struct; the animation_id lives on
    // CharaState directly. Truncate to u16 to match the wire (the Meteor
    // table values 1000..1109 all fit). Missing this was the reason NPCs
    // rendered in their T-pose instead of their populace motion loop.
    let motion_pack = character.chara.animation_id as u16;
    // Meteor lowercases the class-path parent segments before sending
    // (`/chara/npc/populace/PopulaceStandard` — only the final class
    // name keeps its CamelCase). The 1.x `require` path is case-
    // sensitive, so sending `/Chara/Npc/...` makes the client's script
    // loader fail and the NPC never renders.
    let class_path_lower = lowercase_class_path(&character.base.class_path);
    let class_name = character.base.class_name.clone();
    let actor_class_id = character.chara.actor_class_id;
    // Derive the actor_number from the composite id
    // `(4<<28 | zone<<19 | num&0x7FFFF)` set by `Npc::new`.
    let actor_number = actor_id & 0x7FFFF;
    let zone_id = character.base.zone_id;
    let actor_name =
        generate_npc_actor_name(&class_name, zone_name, actor_number, zone_id, priv_level);

    let script_bind_params = vec![
        common::luaparam::LuaParam::String(class_path_lower.clone()),
        common::luaparam::LuaParam::False,
        common::luaparam::LuaParam::False,
        common::luaparam::LuaParam::False,
        common::luaparam::LuaParam::False,
        common::luaparam::LuaParam::False,
        common::luaparam::LuaParam::Int32(actor_class_id as i32),
        common::luaparam::LuaParam::False,
        common::luaparam::LuaParam::False,
        common::luaparam::LuaParam::Int32(0),
        common::luaparam::LuaParam::Int32(0),
    ];

    let is_monster = class_path_lower.contains("/monster/");

    // Meteor's `Npc.CreateAddActorPacket()` hardcodes flag=8 (see
    // Map Server/Actors/Chara/Npc/Npc.cs:162). Passing 0 here — as
    // Area/Director/WorldMaster do — made the client route the
    // AddActor through the non-actor slot path and mis-bucket every
    // following NPC subpacket. The previous zero-flag port is the
    // last-known crash source after Now-Loading with a single
    // populace spawn bundle: strip just the ActorInstantiate and
    // the client survived, add it back with flag=0 and Wine dies.
    subpackets.push(tx::actor::build_add_actor(actor_id, 8));
    // Meteor's `Actor.GetSpawnPackets` (Actor.cs:321-333) emits
    // `GetEventConditionPackets()` immediately after `AddActor`, before
    // `Speed`/`Position`. Without these writes, a later
    // `KickEventPacket(conditionName)` is ignored by the 1.23b client —
    // for the 3 Limsa-opening monsters that means their
    // `noticeEvent` trigger fires unbound and the client tunnels an
    // error EventStart that we have no handler for, surfacing as
    // "An error has occurred" popups on zone-in.
    subpackets.extend(tx::actor_events::build_event_condition_packets(
        actor_id,
        &character.base.event_conditions,
    ));
    subpackets.push(tx::actor::build_set_actor_speed_default(actor_id));
    // Meteor's `Npc.GetSpawnPackets` (Actors/Chara/Npc/Npc.cs:210)
    // passes `CreateSpawnPositonPacket(0x0)` — spawn type `FADEIN`,
    // meaning the actor has always been in the zone and should
    // fade in place with full capsule collider attached. The prior
    // port used `0x1` (PLAYERWAKE), which is a warp-in animation
    // intended for the player's own avatar; under that spawn type
    // the client skips the collider init path for the actor so the
    // player walks right through the NPC. SPAWNTYPE constants live
    // in `SetActorPositionPacket.cs:34-41`.
    subpackets.push(tx::actor::build_set_actor_position(
        actor_id,
        actor_id as i32,
        position.x,
        position.y,
        position.z,
        rotation,
        0x0,
        false,
    ));
    subpackets.push(tx::actor::build_set_actor_appearance(
        actor_id,
        model_id,
        &appearance_ids,
    ));
    let _ = display_name_id;
    subpackets.push(tx::actor::build_set_actor_name(
        actor_id,
        packet_name_id,
        &packet_name,
    ));
    subpackets.push(tx::actor::build_set_actor_state(actor_id, state, 0));
    subpackets.push(tx::actor::build_set_actor_sub_state(
        actor_id, 0, 0, 0, 0, 0, motion_pack,
    ));
    subpackets.push(tx::actor::build_set_actor_status_all(actor_id, &[0u16; 20]));
    subpackets.push(tx::actor::build_set_actor_icon(actor_id, 0));
    subpackets.push(tx::actor::build_set_actor_is_zoning(actor_id, false));
    subpackets.push(tx::actor::build_actor_instantiate(
        actor_id,
        0,
        0x3040,
        &actor_name,
        &class_name,
        &script_bind_params,
    ));
    // Meteor's `Session.UpdateInstance` fires `GetInitPackets()` for each
    // new actor immediately after the spawn bundle (DataObjects/
    // Session.cs:161). For populace NPCs that init dump is a series of
    // `SetActorProperty` (0x0137) writes to `charaWork.property[i]` for
    // every non-zero bit of `actorClass.propertyFlags`, plus baseline
    // HP/MP/TP and state_mainSkill. Without those writes the 1.x client
    // keeps the actor's nameplate hidden and treats it as non-collidable
    // regardless of what SetActorName carries. Populace class 1000438
    // has propertyFlags = 0x13 (bits 0/1/4) — bit 1 is the nameplate
    // flag; without it the server's valid displayNameId never surfaces.
    //
    // Monster-path actors (class 2205403 jellyfish / 2290001/2 fighter
    // openings etc.) ship `propertyFlags = 0x17` — populace bits plus
    // bit 2 (PROPERTY_TARGETABLE / solid-collision). When we spawn them
    // through the populace pipeline (no BattleNpc stat backing), the
    // client's `DepictionJudge:judgeNameplate()` takes its BattleNpc-
    // aware branch on bit 2 and then tries to compare numbers against
    // fields that a real BattleNpc spawn would populate — but we only
    // sent populace init state. Observed error dump (2026-04-21):
    //   attempt to compare number with nil
    //   01>DepictionJudge:judgeNameplate() [?:900]
    //     program => CharaBaseClass:_onUpdateWork() [?:5685]
    // Masking bit 2 off for `/monster/` actors routes the client
    // through the populace nameplate path and suppresses the three
    // "An error has occurred" popups. Bit 2 needs to come back the
    // moment these actors spawn through the real BattleNpc pipeline.
    let property_flags = if is_monster {
        character.chara.property_flags & !(1u32 << 2)
    } else {
        character.chara.property_flags
    };
    let hp = character.chara.hp.max(0) as u16;
    let hp_max = character.chara.max_hp.max(0) as u16;
    let mp = character.chara.mp.max(0) as u16;
    let mp_max = character.chara.max_mp.max(0) as u16;
    let tp = character.chara.tp;
    let npc_init = tx::actor::build_npc_property_init(
        actor_id,
        property_flags,
        hp,
        hp_max,
        mp,
        mp_max,
        tp,
    );
    subpackets.extend(npc_init);

    // SetEventStatus enable-flags for every condition the spawn bundle
    // just registered. Mirrors Meteor's `Session.UpdateInstance` order:
    // `GetSpawnPackets()` (which is where EventConditionPackets lived) →
    // `GetInitPackets()` → `GetSetEventStatusPackets()` (Session.cs:159-161).
    // The condition packets alone register the trigger client-side (radius,
    // condition name, etc.) but it stays *disabled* until a SetEventStatus
    // says otherwise — so without these the 1.x client never fires
    // `EventStart(eventType=2, owner=npc, eventName="pushDefault")` when the
    // player walks into the circle, and proximity-driven cinematics like
    // `man0g0::onPush(YDA)` never reach the player.
    //
    // Defaults match Meteor's signature `(talkEnabled=true, emoteEnabled=true,
    // pushEnabled=null → unwrap_or(true), noticeEnabled=true)`. A subsequent
    // `quest:SetENpc(...)` broadcast can flip `pushEnabled=false` if the
    // current sequence wants the trigger silent.
    subpackets.extend(crate::packets::send::build_actor_event_status_packets(
        actor_id,
        &character.base.event_conditions,
        true,
        true,
        None,
        true,
    ));

    // BattleNpc `/npcWork/hate` tail is load-bearing — omitting it
    // for monster-path actors Wine-hard-crashes on zone-in (confirmed
    // 2026-04-21). Keep the emission; it stays at `hateType = 0` so
    // the client routes the nameplate judge through the passive path.
    if is_monster {
        subpackets.push(tx::actor::build_npc_hate_type_packet(actor_id));
    }
}

/// Outcome of a single `seamless_check` call. Describes what, if anything,
/// the player's position change triggered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeamlessResult {
    /// Inside the main zone-1 box, not near a boundary — nothing to do.
    InsideZoneOne,
    /// Inside the main zone-2 box, not near a boundary — nothing to do.
    InsideZoneTwo,
    /// The player crossed into a zone they weren't tracked in — we fired
    /// a seamless zone change. The `u32` is the new primary zone id.
    ZoneChanged(u32),
    /// Player entered the merge strip; a secondary zone was merged in.
    /// The `u32` is the merged (secondary) zone id.
    ZoneMerged(u32),
    /// Position isn't inside any boundary — fully inside a single zone.
    None,
}

/// Metadata carried alongside a live gather-node actor. Keyed by
/// `(zone_id, unique_id)` on [`WorldManager::gather_node_metadata`]
/// so `DummyCommand.lua` can resolve "the node the player clicked"
/// back to its template id + harvest action without baking those
/// fields onto the generic [`SpawnLocation`] / [`ActorClass`] shape.
///
/// The key mirrors how the spawner threads `unique_id` through from
/// seed → live actor (see `Npc::new(… seed.unique_id …)` in
/// `npc/spawner.rs`), so lookups from a targeted actor's
/// `(zone_id, unique_id)` resolve in one map read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GatherNodeMetadata {
    pub harvest_node_id: u32,
    pub harvest_type: u32,
}

/// Top-level zone + session registry.
pub struct WorldManager {
    zones: RwLock<HashMap<u32, Arc<RwLock<Zone>>>>,

    /// Named entrance points (`server_zones_spawnlocations`) keyed by id.
    zone_entrances: RwLock<HashMap<u32, ZoneEntrance>>,

    /// Seamless boundary boxes keyed by region id.
    seamless_boundaries: RwLock<HashMap<u32, Vec<SeamlessBoundary>>>,

    /// Per-gather-spawn metadata keyed by `(zone_id, unique_id)`. Lets
    /// runtime code resolve a targeted actor (via its own
    /// `unique_id`) to the `(harvest_node_id, harvest_type)` pair it
    /// was seeded with without plumbing those fields through the
    /// generic spawn pipeline.
    gather_node_metadata: RwLock<HashMap<(u32, String), GatherNodeMetadata>>,

    /// Player state indexed by session id — zone membership, player
    /// position snapshot, etc. Updated by movement handlers.
    sessions: RwLock<HashMap<u32, Session>>,

    /// Live socket handles. Used by packet dispatchers to fan outbound
    /// SubPackets to the right clients.
    clients: RwLock<HashMap<u32, ClientHandle>>,
}

impl WorldManager {
    pub fn new() -> Self {
        Self {
            zones: RwLock::new(HashMap::new()),
            zone_entrances: RwLock::new(HashMap::new()),
            seamless_boundaries: RwLock::new(HashMap::new()),
            gather_node_metadata: RwLock::new(HashMap::new()),
            sessions: RwLock::new(HashMap::new()),
            clients: RwLock::new(HashMap::new()),
        }
    }

    /// Resolve a gather node's `(harvest_node_id, harvest_type)` from
    /// the live actor's `(zone_id, unique_id)` key. Returns `None` if
    /// the targeted actor is not a gather-node spawn (or gather spawns
    /// weren't loaded at boot).
    pub async fn gather_metadata(&self, zone_id: u32, unique_id: &str) -> Option<GatherNodeMetadata> {
        self.gather_node_metadata
            .read()
            .await
            .get(&(zone_id, unique_id.to_string()))
            .copied()
    }

    #[cfg(test)]
    pub(crate) async fn gather_metadata_count(&self) -> usize {
        self.gather_node_metadata.read().await.len()
    }

    // -----------------------------------------------------------------
    // Boot-time loaders — pull from DB, populate in-memory registries.
    // -----------------------------------------------------------------

    /// Full boot-time zone load — port of
    /// `WorldManager.LoadZoneList + LoadZoneEntranceList + LoadSeamlessBoundryList`.
    pub async fn load_from_database(
        &self,
        db: &Database,
        server_ip: &str,
        server_port: u16,
    ) -> Result<()> {
        tracing::info!(server_ip, server_port, "world boot-load starting");
        // 1. Zones
        let zone_rows = db.load_zones(server_ip, server_port).await?;
        tracing::info!(count = zone_rows.len(), "zones fetched from DB");
        for row in zone_rows {
            self.install_zone(row).await;
        }
        // 2. Private areas — attach to already-loaded zones.
        let private_area_rows = db.load_private_areas().await?;
        tracing::info!(count = private_area_rows.len(), "private areas fetched");
        for row in private_area_rows {
            self.install_private_area(row).await;
        }
        // 3. Zone entrances.
        let entrances = db.load_zone_entrances().await?;
        tracing::info!(count = entrances.len(), "zone entrances loaded");
        *self.zone_entrances.write().await = entrances;
        // 4. Seamless boundaries.
        let seamless = db.load_seamless_boundaries().await?;
        let total: usize = seamless.values().map(|v| v.len()).sum();
        tracing::info!(
            regions = seamless.len(),
            boundaries = total,
            "seamless boundaries loaded"
        );
        *self.seamless_boundaries.write().await = seamless;
        // 5. NPC spawn locations — one row per static actor seeded into
        // a zone (or a private area inside that zone). Without this the
        // phase-3 `spawn_all_actors` pass sees an empty seed list on
        // every zone and no NPCs are ever instantiated, which is what
        // we were hitting on Asdf-shape logins (`npc spawn pass
        // complete count=0` in map-server.log even though
        // `SELECT COUNT(*) FROM server_spawn_locations` returned 999).
        let spawn_rows = db.load_npc_spawn_locations().await?;
        let spawn_total = spawn_rows.len();
        let mut attached = 0usize;
        let mut missing_zone = 0usize;
        for row in spawn_rows {
            let Some(zone_arc) = self.zone(row.zone_id).await else {
                missing_zone += 1;
                continue;
            };
            let mut z = zone_arc.write().await;
            if z.add_spawn_location(row).is_ok() {
                attached += 1;
            }
        }
        tracing::info!(
            fetched = spawn_total,
            attached,
            missing_zone,
            "npc spawn locations loaded"
        );
        // 6. Gather-node spawns. Treated as regular NPC seeds so the
        // existing phase-3 spawner materialises them with no extra
        // code path; the `(harvest_node_id, harvest_type)` pair is
        // stashed on `WorldManager` keyed by `(zone_id, unique_id)`
        // for DummyCommand.lua to look up later.
        let gather_rows = db.load_gather_node_spawns().await?;
        let gather_total = gather_rows.len();
        let mut gather_attached = 0usize;
        let mut gather_missing_zone = 0usize;
        let mut gather_invalid_type = 0usize;
        {
            let mut meta = self.gather_node_metadata.write().await;
            meta.clear();
            for row in gather_rows {
                if !crate::gathering::is_valid_harvest_type(row.harvest_type) {
                    gather_invalid_type += 1;
                    continue;
                }
                let Some(zone_arc) = self.zone(row.zone_id).await else {
                    gather_missing_zone += 1;
                    continue;
                };
                let seed = crate::zone::SpawnLocation::new(
                    row.actor_class_id,
                    row.unique_id.clone(),
                    row.zone_id,
                    row.private_area_name.clone(),
                    row.private_area_level.max(0) as u32,
                    row.position.0,
                    row.position.1,
                    row.position.2,
                    row.rotation,
                    0,
                    0,
                );
                let mut z = zone_arc.write().await;
                if z.add_spawn_location(seed).is_ok() {
                    meta.insert(
                        (row.zone_id, row.unique_id),
                        GatherNodeMetadata {
                            harvest_node_id: row.harvest_node_id,
                            harvest_type: row.harvest_type,
                        },
                    );
                    gather_attached += 1;
                }
            }
        }
        tracing::info!(
            fetched = gather_total,
            attached = gather_attached,
            missing_zone = gather_missing_zone,
            invalid_harvest_type = gather_invalid_type,
            "gather-node spawns loaded"
        );
        tracing::info!("world boot-load complete");
        Ok(())
    }

    async fn install_zone(&self, row: ZoneRow) {
        tracing::debug!(
            id = row.id,
            name = %row.zone_name,
            region = row.region_id,
            navmesh = row.load_nav_mesh,
            "installing zone"
        );
        let navmesh_loader = if row.load_nav_mesh {
            Some(&StubNavmeshLoader as &dyn crate::zone::navmesh::NavmeshLoader)
        } else {
            None
        };
        let zone = Zone::new(
            row.id,
            row.zone_name,
            row.region_id,
            row.class_path,
            row.bgm_day,
            row.bgm_night,
            row.bgm_battle,
            row.is_isolated,
            row.is_inn,
            row.can_ride_chocobo,
            row.can_stealth,
            row.is_instance_raid,
            navmesh_loader,
        );
        self.register_zone(zone).await;
    }

    async fn install_private_area(&self, row: PrivateAreaRow) {
        let Some(parent_arc) = self.zone(row.parent_zone_id).await else {
            tracing::warn!(
                parent = row.parent_zone_id,
                name = %row.private_area_name,
                "private area references missing parent zone"
            );
            return;
        };
        let (zone_name, region_id, is_isolated, is_inn, can_ride_chocobo, can_stealth) = {
            let parent = parent_arc.read().await;
            (
                parent.core.zone_name.clone(),
                parent.core.region_id,
                parent.core.is_isolated,
                parent.core.is_inn,
                parent.core.can_ride_chocobo,
                parent.core.can_stealth,
            )
        };
        let pa = PrivateArea::new(
            row.parent_zone_id,
            zone_name,
            region_id,
            row.id,
            row.class_name,
            row.private_area_name,
            row.private_area_type,
            row.bgm_day,
            row.bgm_night,
            row.bgm_battle,
            is_isolated,
            is_inn,
            can_ride_chocobo,
            can_stealth,
        );
        let mut parent = parent_arc.write().await;
        parent.add_private_area(pa);
    }

    // -----------------------------------------------------------------
    // Zone registry
    // -----------------------------------------------------------------

    /// Register (or replace) a zone. Called once per zone during startup.
    pub async fn register_zone(&self, zone: Zone) {
        let id = zone.core.actor_id;
        self.zones
            .write()
            .await
            .insert(id, Arc::new(RwLock::new(zone)));
    }

    pub async fn zone(&self, zone_id: u32) -> Option<Arc<RwLock<Zone>>> {
        self.zones.read().await.get(&zone_id).cloned()
    }

    /// Snapshot of all zone ids — used by the game ticker.
    pub async fn zone_ids(&self) -> Vec<u32> {
        self.zones.read().await.keys().copied().collect()
    }

    pub async fn zone_count(&self) -> usize {
        self.zones.read().await.len()
    }

    // -----------------------------------------------------------------
    // Zone entrances + seamless boundaries
    // -----------------------------------------------------------------

    pub async fn zone_entrance(&self, id: u32) -> Option<ZoneEntrance> {
        self.zone_entrances.read().await.get(&id).cloned()
    }

    /// Returns every boundary for `region_id`. Empty if the region has
    /// none.
    pub async fn seamless_boundaries_for(&self, region_id: u32) -> Vec<SeamlessBoundary> {
        self.seamless_boundaries
            .read()
            .await
            .get(&region_id)
            .cloned()
            .unwrap_or_default()
    }

    // -----------------------------------------------------------------
    // Session + client registries
    // -----------------------------------------------------------------

    pub async fn upsert_session(&self, session: Session) {
        self.sessions.write().await.insert(session.id, session);
    }

    pub async fn session(&self, id: u32) -> Option<Session> {
        self.sessions.read().await.get(&id).cloned()
    }

    pub async fn remove_session(&self, id: u32) -> Option<Session> {
        self.clients.write().await.remove(&id);
        self.sessions.write().await.remove(&id)
    }

    pub async fn register_client(&self, id: u32, handle: ClientHandle) {
        self.clients.write().await.insert(id, handle);
    }

    pub async fn client(&self, id: u32) -> Option<ClientHandle> {
        self.clients.read().await.get(&id).cloned()
    }

    pub async fn all_clients(&self) -> Vec<ClientHandle> {
        self.clients.read().await.values().cloned().collect()
    }

    // -----------------------------------------------------------------
    // Zone-change orchestration — port of WorldManager.DoZoneChange /
    // DoSeamlessZoneChange / MergeZones / SeamlessCheck.
    // -----------------------------------------------------------------

    /// Whole-cloth zone transition — removes the player from their old
    /// zone (if any), places them in the new one, updates their session
    /// state. Callers must follow this with `send_zone_in_bundle` once
    /// they are ready to fan the first-render packets to the client;
    /// `do_zone_change` only settles registries.
    pub async fn do_zone_change(
        &self,
        actor_id: u32,
        session_id: u32,
        destination_zone_id: u32,
        spawn: Vector3,
        rotation: f32,
    ) -> Result<()> {
        // 1. Look up the destination zone.
        let Some(dest_zone) = self.zone(destination_zone_id).await else {
            tracing::warn!(
                zone = destination_zone_id,
                "do_zone_change: destination not on this map server"
            );
            return Ok(());
        };

        // 2. Lock the session for updates + update its zone/dest fields.
        {
            let mut sessions = self.sessions.write().await;
            let session = sessions
                .entry(session_id)
                .or_insert_with(|| Session::new(session_id));
            session.is_updates_locked = true;
            let old_zone_id = session.current_zone_id;
            session.current_zone_id = destination_zone_id;
            session.destination_zone_id = destination_zone_id;
            session.destination_x = spawn.x;
            session.destination_y = spawn.y;
            session.destination_z = spawn.z;
            session.destination_rot = rotation;

            // 3. Detach from old zone's spatial grid if different.
            if old_zone_id != 0
                && old_zone_id != destination_zone_id
                && let Some(old_zone) = self.zones.read().await.get(&old_zone_id).cloned()
            {
                let mut old = old_zone.write().await;
                let mut ob = crate::zone::outbox::AreaOutbox::new();
                old.core.remove_actor(actor_id, &mut ob);
            }

            // 4. Attach to new zone.
            let mut dest = dest_zone.write().await;
            let mut ob = crate::zone::outbox::AreaOutbox::new();
            dest.core.add_actor(
                crate::zone::area::StoredActor {
                    actor_id,
                    kind: crate::zone::area::ActorKind::Player,
                    position: spawn,
                    grid: (0, 0),
                    is_alive: true,
                },
                &mut ob,
            );

            // Unlock updates.
            session.is_updates_locked = false;
        }
        Ok(())
    }

    /// Port of `Player.SendZoneInPackets(world, spawnType)`. This is the
    /// bundle the client waits on before leaving "Now loading…": zoning
    /// clear, music/weather/map, the player's self-spawn, an empty
    /// inventory bracket, and the `/_init` property flags. Without it the
    /// client has no way to know the server is done placing the actor.
    ///
    /// Inventory dump and area-master/director spawns are intentionally
    /// stubbed — the minimum viable login flow doesn't need them and they
    /// depend on plumbing that's still in progress (item_packages live on
    /// the `Player` shape, the registry only holds `Character`).
    pub async fn send_zone_in_bundle(
        &self,
        registry: &ActorRegistry,
        session_id: u32,
        spawn_type: u16,
    ) {
        let Some(session) = self.session(session_id).await else {
            tracing::warn!(session = session_id, "send_zone_in_bundle: no session");
            return;
        };
        let Some(client) = self.client(session_id).await else {
            tracing::warn!(session = session_id, "send_zone_in_bundle: no client");
            return;
        };
        let Some(actor_handle) = registry.by_session(session_id).await else {
            tracing::warn!(session = session_id, "send_zone_in_bundle: no actor");
            return;
        };
        let Some(zone_arc) = self.zone(session.current_zone_id).await else {
            tracing::warn!(
                session = session_id,
                zone = session.current_zone_id,
                "send_zone_in_bundle: no zone",
            );
            return;
        };

        let actor_id = actor_handle.actor_id;
        let (
            actor_name,
            display_name_id,
            main_state,
            position,
            rotation,
            model_id,
            appearance_ids,
            hp,
            hp_max,
            mp,
            mp_max,
            tp,
            class_slot,
            tribe,
            guardian,
            birthday_day,
            birthday_month,
            initial_town,
            rest_bonus_exp_rate,
            current_job,
            login_director_actor_id,
            active_quests,
        ) = {
            let c = actor_handle.character.read().await;
            // (slot, quest_actor_id) pairs for `playerWork.questScenario[N]`
            // emission inside the `/_init` bundle. Collected here so the
            // character lock can drop before the long packet-build below.
            let aq: Vec<(u32, u32)> = c
                .quest_journal
                .slots
                .iter()
                .enumerate()
                .filter_map(|(slot, q)| {
                    q.as_ref().map(|q| (slot as u32, q.actor_id))
                })
                .collect();
            (
                c.base.display_name().to_string(),
                c.base.display_name_id,
                c.base.current_main_state as u8,
                c.base.position(),
                c.base.rotation,
                c.chara.model_id,
                c.chara.appearance_ids,
                c.chara.hp.max(0) as u16,
                c.chara.max_hp.max(0) as u16,
                c.chara.mp.max(0) as u16,
                c.chara.max_mp.max(0) as u16,
                c.chara.tp,
                c.chara.class.max(0) as u8,
                c.chara.tribe,
                c.chara.guardian,
                c.chara.birthday_day,
                c.chara.birthday_month,
                c.chara.initial_town,
                c.chara.rest_bonus_exp_rate,
                c.chara.current_job,
                c.chara.login_director_actor_id,
                aq,
            )
        };
        let has_login_director = login_director_actor_id != 0;
        let login_director_spec = session.login_director.clone();
        let pending_kick_event = session.pending_kick_event.clone();
        // Grand Company tuple — read cheap from CharaState after
        // the main destructure block, used downstream to conditionally
        // emit `SetGrandCompanyPacket` in the zone-in bundle.
        let (gc_current, gc_rank_limsa, gc_rank_gridania, gc_rank_uldah) = {
            let c = actor_handle.character.read().await;
            (
                c.chara.gc_current,
                c.chara.gc_rank_limsa,
                c.chara.gc_rank_gridania,
                c.chara.gc_rank_uldah,
            )
        };
        let (zone_actor_id, region_id, bgm_day, zone_name, zone_class_path, zone_class_name) = {
            let z = zone_arc.read().await;
            (
                z.core.actor_id,
                z.core.region_id as u32,
                z.core.bgm_day,
                z.core.zone_name.clone(),
                z.core.class_path.clone(),
                z.core.class_name.clone(),
            )
        };

        // The "script-bind" for the player — mirrors
        // `Map Server/Actors/Chara/Player/Player.cs` `CreateScriptBindPacket`
        // for the self-view. Two variants depending on whether the
        // `player.lua:onBeginLogin` hook attached a login director (via
        // `player:SetLoginDirector(director)` — fires on the tutorial
        // path in zones 193/166/184).
        //
        // - No director (default): `[classPath, true, false, false, true,
        //   Int(0), false, timers[20], true]` — 28 params.
        // - With director: `[classPath, false, false, true, Actor(dirId),
        //   true, Int(0), false, timers[20], true]` — 29 params, one
        //   extra `Actor` reference for the director.
        //
        // Director spawning is still stubbed, so we send `Actor(0)` as a
        // placeholder; the client's Lua binder receives it as a null
        // actor reference, and the tutorial script-bind path still sees
        // the "Is Init Director = true" flag.
        let player_actor_name = format!("_pc{:08}", actor_id);
        let player_class_name = "Player";
        let mut script_bind_params: Vec<common::luaparam::LuaParam> = if has_login_director {
            vec![
                common::luaparam::LuaParam::String("/Chara/Player/Player_work".to_string()),
                common::luaparam::LuaParam::False,
                common::luaparam::LuaParam::False,
                common::luaparam::LuaParam::True, // "Is Init Director"
                common::luaparam::LuaParam::Actor(login_director_actor_id),
                common::luaparam::LuaParam::True,
                common::luaparam::LuaParam::Int32(0),
                common::luaparam::LuaParam::False,
            ]
        } else {
            vec![
                common::luaparam::LuaParam::String("/Chara/Player/Player_work".to_string()),
                common::luaparam::LuaParam::True,
                common::luaparam::LuaParam::False,
                common::luaparam::LuaParam::False,
                common::luaparam::LuaParam::True,
                common::luaparam::LuaParam::Int32(0),
                common::luaparam::LuaParam::False,
            ]
        };
        // Timers slice — Meteor's `int[20] timers = new int[20]` passes
        // signed Int32 zeroes via `LuaUtils.CreateLuaParamList`'s array
        // expansion, not UInt32. Garlemald was emitting `UInt32(0)` which
        // produces LuaParam type byte 0x1 instead of the expected 0x0;
        // the client's Player_work script reads the timer list by type
        // to distinguish it from other fields.
        script_bind_params.extend((0..20).map(|_| common::luaparam::LuaParam::Int32(0)));
        script_bind_params.push(common::luaparam::LuaParam::True);

        // Every subpacket crosses the world-server proxy; its reader
        // (`world-server/src/server.rs`) drops subpackets whose
        // `target_id == 0`, so tag each one with the session id before
        // serialising.
        //
        // The 8 `_0x132` packets register the client's command / widget /
        // macro system. They fire only for the self-view — mirrors the C#
        // `Player.Create0x132Packets()`. The client needs `widgetCreate`
        // in particular to instantiate the in-game UI; without these the
        // player sits on "Now Loading" indefinitely after an otherwise
        // clean zone-in bundle.
        //
        // The login director (if any) is spawned **first** — C#
        // `Director.StartDirector(spawnImmediate=true)` emits the
        // director's 7-packet spawn sequence during `onBeginLogin`
        // BEFORE `DoZoneIn` runs `SendZoneInPackets`. That ordering
        // matters: the player's `ActorInstantiate` references the
        // director via `Actor(login_director_actor_id)` inside its
        // ScriptBind LuaParams, and the client needs to have seen the
        // director's `AddActor` before it can resolve that reference.
        let mut subpackets: Vec<common::subpacket::SubPacket> = Vec::new();
        if let Some(spec) = &login_director_spec {
            let zone_short = shorten_zone_name(&zone_name);
            let mut class_lower = spec.class_name.clone();
            if let Some(first) = class_lower.chars().next() {
                let mut lowered = first.to_lowercase().to_string();
                lowered.push_str(&class_lower[first.len_utf8()..]);
                class_lower = lowered;
            }
            let max_class_len = 20usize.saturating_sub(zone_short.len());
            if class_lower.len() > max_class_len {
                class_lower.truncate(max_class_len);
            }
            let director_actor_name = format!(
                "{class_lower}_{zone_short}_0@{zone_actor_id:03X}00",
                zone_actor_id = spec.zone_actor_id
            );
            let director_bind_params = vec![
                common::luaparam::LuaParam::String(spec.class_path.clone()),
                common::luaparam::LuaParam::False,
                common::luaparam::LuaParam::False,
                common::luaparam::LuaParam::False,
                common::luaparam::LuaParam::False,
                common::luaparam::LuaParam::False,
            ];
            subpackets.push(tx::actor::build_add_actor(spec.actor_id, 0));
            // C# `Director` ctor registers three notice-event conditions:
            //   ("noticeEvent",   0xE, 0x0)   ← event the login director fires
            //   ("noticeRequest", 0x0, 0x1)
            //   ("reqForChild",   0x0, 0x1)
            // `Director.GetSpawnPackets` emits them right after
            // `AddActor` via `SetNoticeEventCondition` (opcode 0x016B).
            // Without these, the `KickEventPacket("noticeEvent")` a few
            // packets later can't resolve to any registered condition
            // on the director and the client silently drops it — which
            // is what we were seeing.
            subpackets.push(tx::actor_events::build_set_notice_event_condition_raw(
                spec.actor_id,
                0x0E,
                0x00,
                "noticeEvent",
            ));
            subpackets.push(tx::actor_events::build_set_notice_event_condition_raw(
                spec.actor_id,
                0x00,
                0x01,
                "noticeRequest",
            ));
            subpackets.push(tx::actor_events::build_set_notice_event_condition_raw(
                spec.actor_id,
                0x00,
                0x01,
                "reqForChild",
            ));
            subpackets.push(tx::actor::build_set_actor_speed_default(spec.actor_id));
            subpackets.push(tx::actor::build_set_actor_position(
                spec.actor_id,
                spec.actor_id as i32,
                0.0,
                0.0,
                0.0,
                0.0,
                0x0,
                false,
            ));
            subpackets.push(tx::actor::build_set_actor_name(
                spec.actor_id,
                0,
                &director_actor_name,
            ));
            subpackets.push(tx::actor::build_set_actor_state(spec.actor_id, 0, 0));
            subpackets.push(tx::actor::build_set_actor_is_zoning(spec.actor_id, false));
            subpackets.push(tx::actor::build_actor_instantiate(
                spec.actor_id,
                0,
                0x3040,
                &director_actor_name,
                &spec.class_name,
                &director_bind_params,
            ));
            // C# `Director.GetInitPackets` emits a single empty
            // `SetActorProperty` with `/_init` target after the spawn —
            // signals to the client that the director is initialised
            // and safe to fire events against. Empty body (just the
            // target marker); our existing `build_actor_property_init`
            // emits three flag entries which is fine for a player but
            // C# emits zero for a director. We build one directly.
            subpackets.push(build_director_init_packet(spec.actor_id));
            tracing::info!(
                director = spec.actor_id,
                class_path = %spec.class_path,
                name = %director_actor_name,
                "login director spawn packets prepended"
            );
            // C# `onBeginLogin` calls `player:KickEvent(director,
            // "noticeEvent", true)` right after `StartDirector(true)` —
            // this is the packet that actually fires the intro cutscene
            // on the client. Without it the director exists in the
            // client's actor table but nothing tells it to play the
            // opening event. Emit the KickEventPacket here (eventType=5,
            // which matches `Player.KickEvent` vs KickEventSpecial).
            if let Some(kick) = &pending_kick_event {
                subpackets.push(tx::events::build_kick_event(
                    kick.trigger_actor_id,
                    kick.owner_actor_id,
                    &kick.event_name,
                    5,
                    &kick.args,
                ));
                tracing::info!(
                    trigger = kick.trigger_actor_id,
                    owner = kick.owner_actor_id,
                    event = %kick.event_name,
                    args = kick.args.len(),
                    "KickEventPacket appended after director spawn"
                );
            }
        }
        // Mount-music persistence — port of Meteor commit `ea7bf4b8`.
        // When the player zones while mounted, the normal zone BGM is
        // overridden with the mount theme (64 = rental chocobo, 83 =
        // owned chocobo or goobbue). Non-mounted zones use the day BGM.
        let (mount_state_char, rental_expire) = {
            let c = actor_handle.character.read().await;
            (c.chara.mount_state, c.chara.rental_expire_time)
        };
        let music_id = if main_state == crate::actor::MAIN_STATE_MOUNTED as u8
            && mount_state_char != 0
        {
            if rental_expire != 0 { 64 } else { 83 }
        } else {
            bgm_day
        };
        subpackets.extend(vec![
            tx::actor::build_set_actor_is_zoning(actor_id, false),
            tx::misc::build_set_dalamud(actor_id, 0),
            tx::misc::build_set_music(actor_id, music_id, 0x01),
            tx::misc::build_set_weather(actor_id, 1, 1),
            tx::misc::build_set_map(actor_id, region_id, zone_actor_id),
            tx::actor::build_add_actor(actor_id, 8),
            tx::actor::build_0x132(actor_id, 0x0B, "commandForced"),
            tx::actor::build_0x132(actor_id, 0x0A, "commandDefault"),
            tx::actor::build_0x132(actor_id, 0x06, "commandWeak"),
            tx::actor::build_0x132(actor_id, 0x04, "commandContent"),
            tx::actor::build_0x132(actor_id, 0x06, "commandJudgeMode"),
            tx::actor::build_0x132(actor_id, 0x100, "commandRequest"),
            tx::actor::build_0x132(actor_id, 0x100, "widgetCreate"),
            tx::actor::build_0x132(actor_id, 0x100, "macroRequest"),
            tx::actor::build_set_actor_speed_default(actor_id),
            tx::actor::build_set_actor_position(
                actor_id, -1, position.x, position.y, position.z, rotation, spawn_type, true,
            ),
            tx::actor::build_set_actor_appearance(actor_id, model_id, &appearance_ids),
            tx::actor::build_set_actor_name(actor_id, display_name_id, &actor_name),
            tx::handshake::build_0xf(actor_id),
            tx::actor::build_set_actor_state(actor_id, main_state, 0),
            tx::actor::build_set_actor_sub_state(actor_id, 0, 0, 0, 0, 0, 0),
            tx::actor::build_set_actor_status_all(actor_id, &[0u16; 20]),
            tx::actor::build_set_actor_icon(actor_id, 0),
            tx::actor::build_set_actor_is_zoning(actor_id, false),
        ]);
        // C# `Player.GetSpawnPackets` order:
        //   AddActor + 0x132×N + Speed + SpawnPosition + Appearance + Name +
        //   0xF + State + SubState + InitStatus + Icon + IsZoning +
        //   **CreatePlayerRelatedPackets** + **ScriptBind (0x00CC)**
        // where CreatePlayerRelatedPackets emits SetSpecialEventWork +
        // 3× achievement packets *before* the ActorInstantiate. We were
        // doing it in the opposite order, which — for the Asdf login —
        // made `DepictionJudge:judgeNameplate` index the achievement
        // tables during the first `_onUpdateWork` after ScriptBind and
        // find them still nil.
        if current_job != 0 {
            subpackets.push(tx::player::build_set_current_job(
                actor_id,
                current_job as u32,
            ));
        }
        subpackets.push(tx::player::build_set_special_event_work(actor_id));
        subpackets.push(tx::player::build_set_achievement_points(actor_id, 0));
        subpackets.push(tx::player::build_set_latest_achievements(
            actor_id,
            &[0u32; 5],
        ));
        subpackets.push(tx::player::build_set_completed_achievements(
            actor_id,
            &[],
        ));
        subpackets.push(tx::actor::build_actor_instantiate(
            actor_id,
            0,
            0x3040,
            &player_actor_name,
            player_class_name,
            &script_bind_params,
        ));
        subpackets.extend([
            tx::actor_inventory::build_inventory_begin_change(actor_id, true),
            // Empty-package brackets for the 6 item packages + equipment,
            // matching the C# `Player.SendZoneInPackets` sequence that
            // calls `itemPackages[...].SendFullPackage()` for each of
            // NORMAL/CURRENCY_CRYSTALS/KEYITEMS/BAZAAR/MELDREQUEST/LOOT
            // followed by `equipment.SendUpdate()`. For a fresh
            // character each package is empty — the client still needs
            // to see the (SetBegin, SetEnd) pair to know the package
            // exists and is empty.
            //
            // `code` values from C# `ItemPackage.cs`, `size` from the
            // `MAXSIZE_*` constants: NORMAL=0/200, CURRENCY_CRYSTALS=99/320,
            // KEYITEMS=100/500, BAZAAR=7/10, MELDREQUEST=5/4, LOOT=4/10,
            // EQUIPMENT=0x00FE/35.
            tx::actor_inventory::build_inventory_set_begin(actor_id, 200, 0),
            tx::actor_inventory::build_inventory_set_end(actor_id),
            tx::actor_inventory::build_inventory_set_begin(actor_id, 320, 99),
            tx::actor_inventory::build_inventory_set_end(actor_id),
            tx::actor_inventory::build_inventory_set_begin(actor_id, 500, 100),
            tx::actor_inventory::build_inventory_set_end(actor_id),
            tx::actor_inventory::build_inventory_set_begin(actor_id, 10, 7),
            tx::actor_inventory::build_inventory_set_end(actor_id),
            tx::actor_inventory::build_inventory_set_begin(actor_id, 4, 5),
            tx::actor_inventory::build_inventory_set_end(actor_id),
            tx::actor_inventory::build_inventory_set_begin(actor_id, 10, 4),
            tx::actor_inventory::build_inventory_set_end(actor_id),
            // `equipment.SendUpdate` — ReferencedItemPackage shape with
            // code=0x00FE, size=35. Empty for a fresh character.
            tx::actor_inventory::build_inventory_set_begin(actor_id, 35, 0x00FE),
        ]);
        // Meteor's `equipment.SendUpdate` calls SetInitialEquipmentPacket
        // (0x014E) between the set-begin/set-end brackets, even for a
        // fully-empty equipment set — the client's DepictionJudge Lua
        // indexes into the equipment table during nameplate rendering,
        // and without this packet the table stays nil, which produces
        // the `DepictionJudge:judgeNameplate [?:900] attempt to index a
        // nil value` crash ~10s after zone-in. Emit one empty packet
        // (count=0) for the Asdf-shape login; real populated equipment
        // lands once we wire `characters_parametersave.weaponX`/gear
        // slots into this bundle.
        subpackets.extend(tx::actor_inventory::build_set_initial_equipment(
            actor_id,
            &[],
        ));
        subpackets.extend([
            tx::actor_inventory::build_inventory_set_end(actor_id),
            tx::actor_inventory::build_inventory_end_change(actor_id),
        ]);
        // `Player.GetInitPackets` can span multiple `SetActorProperty`
        // subpackets when the byte budget is exceeded; the builder emits
        // them in order with the right continuation markers (0x60+len on
        // every packet except the last, which gets 0x82+len). Extend the
        // zone-in bundle with whatever the builder returned.
        // `playerWork/journal`-targeted companion to the `/_init` bundle.
        // EMITTED FIRST (before `/_init`) — pmeteor's
        // `Player.AddQuest` → `SendQuestClientUpdate` fires during
        // `onBeginLogin` lua execution, which lands the journal packet
        // ahead of the player's main zone-in init in the captured
        // outbound stream. The 1.x client appears to require this
        // ordering for the journal pane to render the quest's
        // description/summary text — sending it after `/_init` (as we
        // did originally) leaves the journal entry name-only.
        subpackets.extend(tx::actor::build_player_journal_property(
            actor_id,
            &active_quests,
        ));
        subpackets.extend(tx::actor::build_player_property_init(
            actor_id,
            hp,
            hp_max,
            mp,
            mp_max,
            tp,
            class_slot,
            1,
            0x20, // commandBorder: C# CharaWork default is 0x20
            tribe,
            guardian,
            birthday_day,
            birthday_month,
            initial_town,
            rest_bonus_exp_rate,
            &active_quests,
        ));
        // Post-init property emission — C# `PostUpdate` drives these on
        // the first tick after spawn, but the client's
        // `DepictionJudge:judgeNameplate` runs BEFORE that tick lands
        // and reads both /stateAtQuicklyForAll (for nameplate HP/level
        // bars) and /battleParameter (for nameplate-visibility flags).
        // Emit them inside the zone-in bundle so those tables are live
        // before the first `_onUpdateWork` frame.
        //
        // Meteor's Asdf [OUT] trace shows exactly one /battleParameter
        // (15 bytes of properties) + two /stateAtQuicklyForAll packets
        // — one from `Character.PostUpdate` (hp, hpMax, mp, mpMax, tp)
        // and one from `Player.PostUpdate` (hp, hpMax, mainSkill,
        // mainSkillLevel).
        subpackets.extend(tx::actor::build_chara_state_at_quickly_for_all(
            actor_id, hp, hp_max, mp, mp_max, tp,
        ));
        subpackets.extend(tx::actor::build_player_state_at_quickly_for_all(
            actor_id,
            hp,
            hp_max,
            class_slot,
            1,
        ));
        // `battleTemp.generalParameter[0..3] = 1` matches C# defaults for
        // NAMEPLATE_SHOWN (0), TARGETABLE (1), NAMEPLATE_SHOWN2 (2), and
        // STR (3). Leaving them 0 would still emit an empty
        // /battleParameter packet (Meteor does that too), but seeding
        // the nameplate flags lines up with the explicit
        // `generalParameter[0..3]` setters C# stamps during spawn.
        let mut general_parameter = [0i16; 35];
        general_parameter[0] = 1;
        general_parameter[1] = 1;
        general_parameter[2] = 1;
        general_parameter[3] = 1;
        subpackets.extend(tx::actor::build_battle_parameter(
            actor_id,
            &general_parameter,
        ));
        // Master-actor spawns — C# `Player.SendZoneInPackets` queues
        // `zone.GetSpawnPackets()` + `debugActor.GetSpawnPackets()` +
        // `worldMaster.GetSpawnPackets()` after the player's own init
        // packets. Omitting them leaves the 1.23b client's login state
        // machine waiting on fixed-id actors it expects to resolve
        // before the zone is considered "live". The earlier removal
        // was due to a STATUS_INVALID_PARAMETER crash traced to a bad
        // ScriptBind LuaParam list; we now rebuild those LuaParam sets
        // directly from `Zone.CreateScriptBindPacket` /
        // `DebugProg.CreateScriptBindPacket` /
        // `WorldMaster.CreateScriptBindPacket` in Project Meteor.
        //
        // Actor ids are fixed constants in the C# reference:
        //   WorldMaster = 0x5FF80001   (`/World/WorldMaster_event`)
        //   Debug       = 0x5FF80002   (`/System/Debug.prog`)
        //   AreaMaster  = zone_actor_id (runtime, from `AreaCore`)
        const WORLD_MASTER_ACTOR_ID: u32 = 0x5FF8_0001;
        const DEBUG_ACTOR_ID: u32 = 0x5FF8_0002;

        // AreaMaster (Zone). 15 LuaParams per `Zone.CreateScriptBindPacket`:
        //   classPath, false, true, zoneName, "", -1,
        //   canRideChocobo?1:0 (byte), canStealth, isInn,
        //   false, false, false, true, isInstanceRaid, isEntranceDesion
        // We don't track `isEntranceDesion` per-session so pass false (the
        // C# default — the flag only flips during seamless boundary crossings).
        let (can_ride_chocobo, can_stealth, is_inn, is_instance_raid) = {
            let z = zone_arc.read().await;
            (
                z.core.can_ride_chocobo,
                z.core.can_stealth,
                z.core.is_inn,
                z.core.is_instance_raid,
            )
        };
        let area_master_params: Vec<common::luaparam::LuaParam> = vec![
            common::luaparam::LuaParam::String(zone_class_path.clone()),
            common::luaparam::LuaParam::False,
            common::luaparam::LuaParam::True,
            common::luaparam::LuaParam::String(zone_name.clone()),
            common::luaparam::LuaParam::String(String::new()),
            common::luaparam::LuaParam::Int32(-1),
            // C# `Zone.CreateScriptBindPacket` passes
            // `canRideChocobo ? (byte)1 : (byte)0` — explicit byte cast,
            // LuaParam type 0xC (1 payload byte) on the wire. Emitting
            // this as UInt32 would inject three extra zero bytes into
            // the param stream and shift every following param out of
            // alignment. The 1.23b client's Lua reads the parsed params
            // positionally; a misaligned stream is read as `nil` where
            // a value was expected, which surfaces as the Client Script
            // ERROR "attempt to index a nil value" the client reports
            // back to us wrapped in an EventStart packet.
            common::luaparam::LuaParam::Byte(if can_ride_chocobo { 1 } else { 0 }),
            if can_stealth {
                common::luaparam::LuaParam::True
            } else {
                common::luaparam::LuaParam::False
            },
            if is_inn {
                common::luaparam::LuaParam::True
            } else {
                common::luaparam::LuaParam::False
            },
            common::luaparam::LuaParam::False,
            common::luaparam::LuaParam::False,
            common::luaparam::LuaParam::False,
            common::luaparam::LuaParam::True,
            if is_instance_raid {
                common::luaparam::LuaParam::True
            } else {
                common::luaparam::LuaParam::False
            },
            common::luaparam::LuaParam::False,
        ];
        let area_master_name = format!("_areaMaster@{:05X}", zone_actor_id << 8);
        push_master_spawn(
            &mut subpackets,
            zone_actor_id,
            area_master_name,
            zone_class_name.clone(),
            area_master_params,
        );

        // Debug. 9 LuaParams per `DebugProg.CreateScriptBindPacket`:
        //   "/System/Debug.prog", false, false, false, false, true,
        //   0xC51F, true, true
        push_master_spawn(
            &mut subpackets,
            DEBUG_ACTOR_ID,
            "debug".to_string(),
            "Debug".to_string(),
            vec![
                common::luaparam::LuaParam::String("/System/Debug.prog".to_string()),
                common::luaparam::LuaParam::False,
                common::luaparam::LuaParam::False,
                common::luaparam::LuaParam::False,
                common::luaparam::LuaParam::False,
                common::luaparam::LuaParam::True,
                common::luaparam::LuaParam::Int32(0xC51F),
                common::luaparam::LuaParam::True,
                common::luaparam::LuaParam::True,
            ],
        );

        // WorldMaster. 7 LuaParams per `WorldMaster.CreateScriptBindPacket`:
        //   "/World/WorldMaster_event", false, false, false, false, false, nil
        push_master_spawn(
            &mut subpackets,
            WORLD_MASTER_ACTOR_ID,
            "worldMaster".to_string(),
            "WorldMaster".to_string(),
            vec![
                common::luaparam::LuaParam::String("/World/WorldMaster_event".to_string()),
                common::luaparam::LuaParam::False,
                common::luaparam::LuaParam::False,
                common::luaparam::LuaParam::False,
                common::luaparam::LuaParam::False,
                common::luaparam::LuaParam::False,
                common::luaparam::LuaParam::Nil,
            ],
        );

        // Grand Company packet — port of `Player.Create0x132Packets`
        // GC arm. Emitted only when the player has enlisted
        // (`gc_current != 0`); sets the client-side allegiance +
        // per-GC rank cache the UI reads for nameplate + menu.
        if gc_current != 0 {
            subpackets.push(crate::packets::send::player::build_set_grand_company(
                actor_id,
                gc_current,
                gc_rank_limsa,
                gc_rank_gridania,
                gc_rank_uldah,
            ));
        }

        // Inn packets — port of `Player.Create0x132Packets` inn arm
        // (Meteor commit `42f0046e`). When the player lands in an inn
        // zone, the client expects:
        //   SetCutsceneBook — unlocks all 2048 "rewatched cutscene"
        //   flags on the Path Companion sNPC so every past cutscene
        //   shows up as rewatchable.
        //   SetPlayerDream  — a pair of bytes (dreamId, innId). `0x16`
        //   is the standard "player asleep" id used during login to
        //   an inn; innId is derived from position via
        //   `inn_code_from_position`.
        if is_inn {
            let inn_code = crate::actor::inn::inn_code_from_position(
                (position.x, position.y, position.z),
                true,
            );
            let all_flags = [true; 2048];
            subpackets.push(
                crate::packets::send::player::build_set_cutscene_book(
                    actor_id,
                    "<Path Companion>",
                    11,
                    1,
                    1,
                    Some(&all_flags[..]),
                ),
            );
            // Meteor's Create0x132 passes dreamId = 0x16 as the
            // "just logged into an inn" scene and innId = inn code.
            subpackets.push(crate::packets::send::player::build_set_player_dream(
                actor_id, 0x16, inn_code,
            ));
        }

        let _ = &main_state;
        let _ = &login_director_spec;

        // Populace NPC spawns. Mirrors C# `Session.UpdateInstance` which
        // iterates `zone.GetActorsAroundActor(player, 50)` and queues a
        // full `Npc::GetSpawnPackets` bundle for each neighbour. Without
        // these, zone 193 (Ocean Battle) is empty and the client's
        // DepictionJudge iterates an unpopulated nameplate table on its
        // first `_onUpdateWork` tick.
        //
        // We run through the zone's spatial grid, pull each neighbour's
        // Character via the registry, and emit the 10-packet actor
        // bundle (AddActor + Speed + Position + Appearance + Name +
        // State + SubState + StatusAll + Icon + IsZoning) followed by
        // the ScriptBind (0x00CC) ActorInstantiate. Event-condition
        // registration (0x016B / 0x0136) is still deferred — Meteor
        // only emits those for NPCs with parsed event tables, which
        // we'll wire when Lua event-condition parsing lands.
        let neighbours: Vec<(u32, crate::zone::area::ActorKind)> = {
            let z = zone_arc.read().await;
            z.core
                .actors_around(actor_id, 50.0)
                .into_iter()
                .filter(|a| a.actor_id != actor_id)
                .map(|a| (a.actor_id, a.kind))
                .collect()
        };
        // Send the main bundle (masters + player packets + inventory +
        // achievements + ActorInstantiate + property_init) FIRST, then
        // the per-neighbour NPC spawns, then the empty group sync.
        for mut sub in std::mem::take(&mut subpackets) {
            sub.set_target_id(session_id);
            client.send_bytes(sub.to_bytes()).await;
        }

        // Diagnostic cap on the number of NPC spawns emitted this
        // fanout. Raised to usize::MAX now that the AddActor flag=8
        // fix unblocked per-NPC rendering.
        let mut emitted: usize = 0;
        const NPC_SPAWN_EMIT_CAP: usize = usize::MAX;

        // NPC spawn fanout.
        //
        // Monsters (`/Chara/Npc/Monster/...`) previously gated here. Re-
        // enabled 2026-04-21 after character-select crash resolution
        // invalidated the 2026-04-20 bisect. In Meteor `server_spawn_
        // locations` rows (even monster-class ones) instantiate as
        // plain `Npc`, not `BattleNpc` — only `server_battlenpc_spawn_
        // locations` routes through `BattleNpc::new` (Area.cs:485 vs
        // WorldManager.cs:622). So the 3 Limsa-opening monster actors
        // (opening_jelly, opening_yshtola, opening_stahlmann) go
        // through the same populace pipeline.
        for (neighbour_id, kind) in neighbours {
            use crate::zone::area::ActorKind;
            if !matches!(kind, ActorKind::Npc | ActorKind::BattleNpc | ActorKind::Ally) {
                continue;
            }
            let Some(handle) = registry.get(neighbour_id).await else {
                continue;
            };
            let character = handle.character.read().await;
            if emitted >= NPC_SPAWN_EMIT_CAP {
                continue;
            }
            emitted += 1;
            let mut npc_bundle = Vec::new();
            push_npc_spawn(
                &mut npc_bundle,
                &*character,
                &zone_name,
                // Priv-level is 0 for the root Zone (non-PrivateArea).
                // PrivateArea spawns route through a different fanout
                // and will need their own priv-level threading later.
                0,
            );
            for mut sub in npc_bundle {
                sub.set_target_id(session_id);
                client.send_bytes(sub.to_bytes()).await;
            }
        }

        // Solo-party group sync. Decompiled
        // `CharaBaseClass:getPlayerParty` (proto[2] of
        // `script/729s9/729s989r57y9rr.le.lpb`) is literally
        //     return self:_getExtendedTemporaryGroup(10001)
        // and 10001 == 0x2711 == TYPEID_PARTY — so the party object
        // the client's `DepictionJudge:judgeNameplate` dereferences
        // is a 0x017C-registered group whose extended-temp key is
        // the party type id. The nameplate renderer assumes
        // getPlayerParty() is non-nil and immediately SELFs
        // `_getOccupancyGroup` on it (proto[0] #7 at line ~907 of
        // `script/0p635/…`).
        //
        // Byte-diff against Meteor's Asdf 0x017C/D/E/F surfaced
        // three field-level misses from the first stab:
        //   1. 0x017C localizedNameId = -1, not 0 (wiki: "-1 if
        //      custom name used", and our custom name is "" so it
        //      still counts).
        //   2. 0x017D numMembers = 1, not 0 (the solo party has
        //      ONE member — the player themselves).
        //   3. 0x017F member[0] populated with actor_id + is_online
        //      + player's name. Empty X08 is what the client treats
        //      as malformed and hard-crashes on.
        {
            const PARTY_SOLO_SELF_FLAG: u64 = 0x8000_0000_0000_0000;
            const GROUP_TYPE_PARTY: u32 = 0x2711;
            let group_index: u64 = PARTY_SOLO_SELF_FLAG | (actor_id as u64);
            let location_code = zone_actor_id as u64;
            let sequence_id = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or_default();
            // Single-member roster: the player themselves. Matches
            // `GroupMember` struct shape (localized_name=-1 ⇒ use
            // custom name; flag1=false = not leader flag; is_online=
            // true since they're obviously logged in).
            let self_member = tx::groups::GroupMember {
                actor_id,
                localized_name: -1,
                unknown2: 0,
                flag1: false,
                is_online: true,
                name: actor_name.clone(),
            };
            let members = [self_member];
            let mut offset = 0usize;
            let group_pkts = vec![
                tx::groups::build_group_header(
                    actor_id,
                    location_code,
                    sequence_id,
                    group_index,
                    GROUP_TYPE_PARTY,
                    -1,
                    "",
                    members.len() as u32,
                ),
                tx::groups::build_group_members_begin(
                    actor_id,
                    location_code,
                    sequence_id,
                    group_index,
                    members.len() as u32,
                ),
                tx::groups::build_group_members_x08(
                    actor_id,
                    location_code,
                    sequence_id,
                    &members,
                    &mut offset,
                ),
                tx::groups::build_group_members_end(
                    actor_id,
                    location_code,
                    sequence_id,
                    group_index,
                ),
            ];
            for mut sub in group_pkts {
                sub.set_target_id(session_id);
                client.send_bytes(sub.to_bytes()).await;
            }

            // 0x018D PartyMapMarkerUpdate — the "where is each party
            // member on the world map" overlay. Retail emits this on
            // a periodic interval (see wiki note); for now we send a
            // single solo-marker emission as part of the zone-in
            // bundle so the world map starts populated. A periodic
            // resend can be added later via a ticker hook.
            //
            // NOTE: `unknown` per-marker field is seeded as 0; wiki
            // describes it as opaque ("each player has a different
            // value"), and the client doesn't appear to validate it.
            // If a future capture lights up an actual constraint,
            // refine via a per-character salt.
            let marker = tx::groups::PartyMapMarker {
                player_id: actor_id,
                unknown: 0,
                x: position.x,
                y: position.y,
                z: position.z,
                orientation: rotation,
            };
            let mut sub = tx::groups::build_party_map_marker_update(
                actor_id,
                tx::groups::PARTY_MAP_MARKER_SOLO_GROUP_ID,
                tx::groups::PARTY_MAP_MARKER_GROUP_TYPE_PLAYER_PARTY,
                &[marker],
            );
            sub.set_target_id(session_id);
            client.send_bytes(sub.to_bytes()).await;

            // 0x018B SetGroupLayoutID — the row-level companion to the
            // 0x018D map-marker overlay (same playerGroupID space).
            // Tells the client what name + layout-id to draw in the
            // party-list row for this actor, even when the actor isn't
            // in render range. Retail emits one of these per party
            // member (we have one — the player themselves — for solo).
            //
            // `layout_id = 0` is the safe default for a player not
            // bound to a specific map object; retail captures show
            // this field can be nonzero (combat_skills.pcapng had
            // 0x131) but the exact derivation from zone state is
            // unclear and 0 is what Meteor's `mapObjLayoutId` defaults
            // to for un-bound actors.
            //
            // `unknown1 = 1` because the player is online (wiki
            // suggests 0/1 maps to offline/online — confirmed by
            // captures where the same player flips from 0 → 1 when
            // rolling between captures, but solo-self captures
            // observed value 0 too; pick 1 here since we know the
            // player is logged in).
            let mut sub = tx::groups::build_set_group_layout_id(
                actor_id,
                tx::groups::PARTY_MAP_MARKER_SOLO_GROUP_ID,
                actor_id,
                tx::groups::SET_GROUP_LAYOUT_ID_PLAYER_DISPLAY_NAME,
                0,
                1,
                &actor_name,
            );
            sub.set_target_id(session_id);
            client.send_bytes(sub.to_bytes()).await;
        }
        tracing::info!(
            session = session_id,
            actor = actor_id,
            zone = zone_actor_id,
            "zone-in bundle dispatched",
        );
    }

    /// Lightweight port of `DoSeamlessZoneChange`. Used when the player
    /// crosses a seamless boundary into an adjacent zone — the client
    /// doesn't see a full zone-in cutscene; we just move their projection.
    pub async fn do_seamless_zone_change(
        &self,
        actor_id: u32,
        session_id: u32,
        destination_zone_id: u32,
        position: Vector3,
    ) -> Result<()> {
        let Some(_dest_zone) = self.zone(destination_zone_id).await else {
            return Ok(());
        };

        // Pop the actor projection out of whatever zone held it, add it
        // to the new one, clear any merged-secondary-zone reference.
        let old_zone_id = {
            let sessions = self.sessions.read().await;
            sessions
                .get(&session_id)
                .map(|s| s.current_zone_id)
                .unwrap_or(0)
        };
        if let Some(old) = self.zone(old_zone_id).await {
            let mut ob = crate::zone::outbox::AreaOutbox::new();
            old.write().await.core.remove_actor(actor_id, &mut ob);
        }
        if let Some(dest) = self.zone(destination_zone_id).await {
            let mut ob = crate::zone::outbox::AreaOutbox::new();
            dest.write().await.core.add_actor(
                crate::zone::area::StoredActor {
                    actor_id,
                    kind: crate::zone::area::ActorKind::Player,
                    position,
                    grid: (0, 0),
                    is_alive: true,
                },
                &mut ob,
            );
        }

        // Update session bookkeeping.
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            session.current_zone_id = destination_zone_id;
        }
        Ok(())
    }

    /// Lightweight port of `MergeZones`. Pulls actors from `mergedZoneId`
    /// into the player's view (logically — the session carries `zoneId2`
    /// so range queries expand to include the secondary zone). No
    /// primary-zone change happens.
    pub async fn merge_zones(
        &self,
        actor_id: u32,
        _session_id: u32,
        merged_zone_id: u32,
        position: Vector3,
    ) -> Result<()> {
        let Some(merged) = self.zone(merged_zone_id).await else {
            return Ok(());
        };
        // Add a projection of the player into the merged zone too. The
        // game loop then broadcasts to the merged zone's grid as well.
        let mut ob = crate::zone::outbox::AreaOutbox::new();
        merged.write().await.core.add_actor(
            crate::zone::area::StoredActor {
                actor_id,
                kind: crate::zone::area::ActorKind::Player,
                position,
                grid: (0, 0),
                is_alive: true,
            },
            &mut ob,
        );
        Ok(())
    }

    /// `SeamlessCheck(player)` port. Drives all three possible outcomes:
    ///
    /// * Inside zone-1 box but primary zone isn't zone 1 → fire
    ///   `do_seamless_zone_change` to zone 1.
    /// * Inside zone-2 box but primary zone isn't zone 2 → fire to zone 2.
    /// * Inside the merge strip → fire `merge_zones` with whichever zone
    ///   isn't already primary.
    pub async fn seamless_check(
        &self,
        actor_id: u32,
        session_id: u32,
        position: Vector3,
    ) -> SeamlessResult {
        // Which region is this player in?
        let (region_id, current_zone_id) = match self.session(session_id).await {
            Some(s) => {
                let zone = self.zone(s.current_zone_id).await;
                let region = match zone {
                    Some(z) => z.read().await.core.region_id as u32,
                    None => return SeamlessResult::None,
                };
                (region, s.current_zone_id)
            }
            None => return SeamlessResult::None,
        };

        let bounds = self.seamless_boundaries_for(region_id).await;
        for b in &bounds {
            if check_pos_in_bounds(
                position.x, position.z, b.zone1_x1, b.zone1_y1, b.zone1_x2, b.zone1_y2,
            ) {
                if current_zone_id == b.zone_id_1 {
                    return SeamlessResult::InsideZoneOne;
                }
                let _ = self
                    .do_seamless_zone_change(actor_id, session_id, b.zone_id_1, position)
                    .await;
                return SeamlessResult::ZoneChanged(b.zone_id_1);
            }
            if check_pos_in_bounds(
                position.x, position.z, b.zone2_x1, b.zone2_y1, b.zone2_x2, b.zone2_y2,
            ) {
                if current_zone_id == b.zone_id_2 {
                    return SeamlessResult::InsideZoneTwo;
                }
                let _ = self
                    .do_seamless_zone_change(actor_id, session_id, b.zone_id_2, position)
                    .await;
                return SeamlessResult::ZoneChanged(b.zone_id_2);
            }
            if check_pos_in_bounds(
                position.x, position.z, b.merge_x1, b.merge_y1, b.merge_x2, b.merge_y2,
            ) {
                let merged = if current_zone_id == b.zone_id_1 {
                    b.zone_id_2
                } else {
                    b.zone_id_1
                };
                let _ = self
                    .merge_zones(actor_id, session_id, merged, position)
                    .await;
                return SeamlessResult::ZoneMerged(merged);
            }
        }
        SeamlessResult::None
    }

    /// Move an actor *within* its current zone — updates the spatial
    /// grid so broadcast fan-out stays accurate. Called from the
    /// packet-processor's `UpdatePlayerPosition` handler.
    pub async fn update_actor_position(
        &self,
        actor_id: u32,
        session_id: u32,
        new_position: Vector3,
    ) {
        let zone_id = match self.session(session_id).await {
            Some(s) => s.current_zone_id,
            None => return,
        };
        let Some(zone_arc) = self.zone(zone_id).await else {
            return;
        };
        let mut zone = zone_arc.write().await;
        let mut ob = crate::zone::outbox::AreaOutbox::new();
        zone.core
            .update_actor_position(actor_id, new_position, &mut ob);
    }
}

impl Default for WorldManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zone::navmesh::StubNavmeshLoader;

    fn mk_zone(id: u32, name: &str, region: u16) -> Zone {
        Zone::new(
            id,
            name,
            region,
            "/Area/Zone/Test",
            0,
            0,
            0,
            false,
            false,
            false,
            false,
            false,
            Some(&StubNavmeshLoader),
        )
    }

    #[tokio::test]
    async fn seamless_check_zone_1_no_change_when_already_primary() {
        let wm = WorldManager::new();
        wm.register_zone(mk_zone(1, "east_thanalan", 103)).await;
        wm.register_zone(mk_zone(2, "central_thanalan", 103)).await;

        // Install a session that's already primary to zone 1.
        let mut s = Session::new(42);
        s.current_zone_id = 1;
        wm.upsert_session(s).await;

        // Install a boundary that wraps (−10..10, −10..10) around origin for
        // zone 1; zone 2 box is elsewhere; merge box is a tiny strip.
        let boundary = SeamlessBoundary {
            id: 1,
            region_id: 103,
            zone_id_1: 1,
            zone_id_2: 2,
            zone1_x1: -10.0,
            zone1_y1: -10.0,
            zone1_x2: 10.0,
            zone1_y2: 10.0,
            zone2_x1: 100.0,
            zone2_y1: 100.0,
            zone2_x2: 110.0,
            zone2_y2: 110.0,
            merge_x1: 20.0,
            merge_y1: 20.0,
            merge_x2: 30.0,
            merge_y2: 30.0,
        };
        wm.seamless_boundaries
            .write()
            .await
            .entry(103)
            .or_default()
            .push(boundary);

        let result = wm
            .seamless_check(100, 42, Vector3::new(0.0, 0.0, 0.0))
            .await;
        assert_eq!(result, SeamlessResult::InsideZoneOne);
    }

    #[tokio::test]
    async fn seamless_check_fires_zone_change_when_entering_zone_2_box() {
        let wm = WorldManager::new();
        wm.register_zone(mk_zone(1, "east", 103)).await;
        wm.register_zone(mk_zone(2, "central", 103)).await;

        let mut s = Session::new(42);
        s.current_zone_id = 1;
        wm.upsert_session(s).await;

        let boundary = SeamlessBoundary {
            id: 1,
            region_id: 103,
            zone_id_1: 1,
            zone_id_2: 2,
            zone1_x1: -10.0,
            zone1_y1: -10.0,
            zone1_x2: 10.0,
            zone1_y2: 10.0,
            zone2_x1: 100.0,
            zone2_y1: 100.0,
            zone2_x2: 110.0,
            zone2_y2: 110.0,
            merge_x1: 20.0,
            merge_y1: 20.0,
            merge_x2: 30.0,
            merge_y2: 30.0,
        };
        wm.seamless_boundaries
            .write()
            .await
            .entry(103)
            .or_default()
            .push(boundary);

        let result = wm
            .seamless_check(100, 42, Vector3::new(105.0, 0.0, 105.0))
            .await;
        assert_eq!(result, SeamlessResult::ZoneChanged(2));
        // And the session now reflects the new primary zone.
        let updated = wm.session(42).await.unwrap();
        assert_eq!(updated.current_zone_id, 2);
    }

    #[tokio::test]
    async fn seamless_check_merges_in_merge_strip() {
        let wm = WorldManager::new();
        wm.register_zone(mk_zone(1, "east", 103)).await;
        wm.register_zone(mk_zone(2, "central", 103)).await;

        let mut s = Session::new(42);
        s.current_zone_id = 1;
        wm.upsert_session(s).await;

        let boundary = SeamlessBoundary {
            id: 1,
            region_id: 103,
            zone_id_1: 1,
            zone_id_2: 2,
            zone1_x1: -10.0,
            zone1_y1: -10.0,
            zone1_x2: 10.0,
            zone1_y2: 10.0,
            zone2_x1: 100.0,
            zone2_y1: 100.0,
            zone2_x2: 110.0,
            zone2_y2: 110.0,
            merge_x1: 20.0,
            merge_y1: 20.0,
            merge_x2: 30.0,
            merge_y2: 30.0,
        };
        wm.seamless_boundaries
            .write()
            .await
            .entry(103)
            .or_default()
            .push(boundary);

        let result = wm
            .seamless_check(100, 42, Vector3::new(25.0, 0.0, 25.0))
            .await;
        assert_eq!(result, SeamlessResult::ZoneMerged(2));
        // Session's primary zone is unchanged; the secondary is merged.
        assert_eq!(wm.session(42).await.unwrap().current_zone_id, 1);
    }

    #[tokio::test]
    async fn do_zone_change_moves_actor_between_zones() {
        let wm = WorldManager::new();
        wm.register_zone(mk_zone(1, "east", 103)).await;
        wm.register_zone(mk_zone(2, "central", 103)).await;
        let mut s = Session::new(42);
        s.current_zone_id = 1;
        wm.upsert_session(s).await;

        // Pre-populate zone 1 with the player projection.
        {
            let z = wm.zone(1).await.unwrap();
            let mut z = z.write().await;
            let mut ob = crate::zone::outbox::AreaOutbox::new();
            z.core.add_actor(
                crate::zone::area::StoredActor {
                    actor_id: 100,
                    kind: crate::zone::area::ActorKind::Player,
                    position: Vector3::ZERO,
                    grid: (0, 0),
                    is_alive: true,
                },
                &mut ob,
            );
        }

        wm.do_zone_change(100, 42, 2, Vector3::new(50.0, 0.0, 50.0), 0.0)
            .await
            .unwrap();

        assert!(!wm.zone(1).await.unwrap().read().await.core.contains(100));
        assert!(wm.zone(2).await.unwrap().read().await.core.contains(100));
    }
}
