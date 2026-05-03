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

//! mlua `UserData` wrappers for every game object a script can touch.
//!
//! **Design.** Actor state isn't shared live with Lua — each script call
//! copies read-only fields into these userdata structs up front. Mutations
//! happen by pushing `LuaCommand`s onto a queue; the map-server game loop
//! drains the queue after the script returns. This keeps scripts off any
//! async lock.
//!
//! The `Arc<Mutex<CommandQueue>>` inside each userdata is shared so *all*
//! userdata created for one script invocation write into the same bucket.

#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use mlua::{AnyUserData, UserData, UserDataFields, UserDataMethods, Value};

use super::command::{CommandQueue, LuaCommand};
use crate::crafting::{Recipe, RecipeResolver};
use crate::gathering::{GatherNode, GatherNodeItem, GatherResolver};
use crate::leve::{LeveType, RegionalLeveData, RegionalLeveResolver};

fn push(queue: &Arc<Mutex<CommandQueue>>, cmd: LuaCommand) {
    CommandQueue::push(queue, cmd);
}

// ---------------------------------------------------------------------------
// LuaActor — base type common to Player, Npc, BattleNpc.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LuaActor {
    pub actor_id: u32,
    pub name: String,
    pub class_name: String,
    pub class_path: String,
    pub unique_id: String,
    pub zone_id: u32,
    pub zone_name: String,
    pub state: u16,
    pub pos: (f32, f32, f32),
    pub rotation: f32,
    pub queue: Arc<Mutex<CommandQueue>>,
}

impl UserData for LuaActor {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("GetName", |_, this, _: ()| Ok(this.name.clone()));
        methods.add_method("GetClassName", |_, this, _: ()| Ok(this.class_name.clone()));
        methods.add_method("GetUniqueId", |_, this, _: ()| Ok(this.unique_id.clone()));
        methods.add_method("GetActorClassId", |_, this, _: ()| Ok(this.actor_id));
        methods.add_method("GetZoneID", |_, this, _: ()| Ok(this.zone_id));
        methods.add_method("GetState", |_, this, _: ()| Ok(this.state));
        methods.add_method("GetPos", |_, this, _: ()| {
            Ok((
                this.pos.0,
                this.pos.1,
                this.pos.2,
                this.rotation,
                this.zone_id,
            ))
        });
        methods.add_method("ChangeState", |_, this, state: u16| {
            push(
                &this.queue,
                LuaCommand::ChangeState {
                    actor_id: this.actor_id,
                    main_state: state,
                },
            );
            Ok(())
        });
        methods.add_method("PlayAnimation", |_, this, animation_id: u32| {
            push(
                &this.queue,
                LuaCommand::PlayAnimation {
                    actor_id: this.actor_id,
                    animation_id,
                },
            );
            Ok(())
        });
        methods.add_method(
            "SendMessage",
            |_, this, (message_type, sender, text): (u8, String, String)| {
                push(
                    &this.queue,
                    LuaCommand::SendMessage {
                        actor_id: this.actor_id,
                        message_type,
                        sender,
                        text,
                    },
                );
                Ok(())
            },
        );
        methods.add_method("GraphicChange", |_, this, (slot, graphic): (u8, u32)| {
            push(
                &this.queue,
                LuaCommand::GraphicChange {
                    actor_id: this.actor_id,
                    slot,
                    graphic_id: graphic,
                },
            );
            Ok(())
        });

        // `actor:SetMod(modifier_key, value)` — same B3 plumbing as
        // `LuaPlayer:SetMod`. Lets scripts apply modifiers to any
        // actor type (BNpc, NPC, populace) returned from
        // `SpawnBattleNpcById` / `SpawnActor` / etc.
        methods.add_method("SetMod", |_, this, (modifier_key, value): (i64, i64)| {
            push(
                &this.queue,
                LuaCommand::SetActorMod {
                    actor_id: this.actor_id,
                    modifier_key: modifier_key as u32,
                    value,
                },
            );
            Ok(())
        });

        // Field-style accessors (scripts do `actor.positionX = ...` too).
        methods.add_meta_method(mlua::MetaMethod::Index, |_, this, key: String| {
            let out: Value = match key.as_str() {
                "positionX" => Value::Number(this.pos.0 as f64),
                "positionY" => Value::Number(this.pos.1 as f64),
                "positionZ" => Value::Number(this.pos.2 as f64),
                "rotation" => Value::Number(this.rotation as f64),
                "actorId" => Value::Integer(this.actor_id as i64),
                "zoneId" => Value::Integer(this.zone_id as i64),
                _ => Value::Nil,
            };
            Ok(out)
        });
    }
}

// ---------------------------------------------------------------------------
// LuaNpc — adds a couple of NPC-specific helpers.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LuaNpc {
    pub base: LuaActor,
    pub actor_class_id: u32,
    pub quest_graphic: u8,
}

impl UserData for LuaNpc {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("GetName", |_, this, _: ()| Ok(this.base.name.clone()));
        methods.add_method("GetUniqueId", |_, this, _: ()| {
            Ok(this.base.unique_id.clone())
        });
        methods.add_method("GetActorClassId", |_, this, _: ()| Ok(this.actor_class_id));
        methods.add_method("GetZoneID", |_, this, _: ()| Ok(this.base.zone_id));
        methods.add_method("SetQuestGraphic", |_, this, graphic: u8| {
            push(
                &this.base.queue,
                LuaCommand::GraphicChange {
                    actor_id: this.base.actor_id,
                    slot: graphic,
                    graphic_id: 0,
                },
            );
            Ok(())
        });
        methods.add_method("GetPos", |_, this, _: ()| {
            Ok((
                this.base.pos.0,
                this.base.pos.1,
                this.base.pos.2,
                this.base.rotation,
                this.base.zone_id,
            ))
        });
    }
}

/// One row of `PlayerSnapshot::active_quest_states` — a frozen view of a
/// [`crate::actor::quest::Quest`] so Lua handles can answer getters
/// without going back to the Rust side. Mutations still flow through
/// the command queue.
#[derive(Debug, Clone, Copy, Default)]
pub struct QuestStateSnapshot {
    pub quest_id: u32,
    pub sequence: u32,
    pub flags: u32,
    pub counters: [u16; 3],
}

// ---------------------------------------------------------------------------
// LuaPlayer — the big one. Stores a rich snapshot so scripts can read every
// field they previously asked the C# Player for.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct PlayerSnapshot {
    pub actor_id: u32,
    pub name: String,
    pub zone_id: u32,
    pub pos: (f32, f32, f32),
    pub rotation: f32,
    pub state: u16,

    pub hp: i16,
    pub max_hp: i16,
    pub mp: i16,
    pub max_mp: i16,
    pub tp: u16,

    pub play_time: u32,
    pub current_class: u8,
    pub current_level: i16,
    pub current_job: u8,
    pub current_gil: u32,
    pub initial_town: u8,
    pub tribe: u8,

    pub guardian: u8,
    pub birth_month: u8,
    pub birth_day: u8,

    pub homepoint: u32,
    pub homepoint_inn: u8,

    pub mount_state: u8,
    pub has_chocobo: bool,
    pub chocobo_appearance: u8,
    pub chocobo_name: String,
    pub rental_expire_time: u32,
    pub rental_min_left: u8,
    /// Grand Company allegiance — 0 = not enlisted, 1 = Maelstrom,
    /// 2 = Twin Adder, 3 = Immortal Flames.
    pub gc_current: u8,
    pub gc_rank_limsa: u8,
    pub gc_rank_gridania: u8,
    pub gc_rank_uldah: u8,
    pub is_gm: bool,

    pub is_engaged: bool,
    pub is_trading: bool,
    pub is_trade_accepted: bool,
    pub is_party_leader: bool,

    pub current_event_owner: u32,
    pub current_event_name: String,
    pub current_event_type: u8,

    /// Completed quest ids — keeps `IsQuestCompleted` cheap.
    pub completed_quests: Vec<u32>,
    /// Active quest ids.
    pub active_quests: Vec<u32>,
    /// Per-active-quest state — `(quest_id, sequence, flags, counters)`
    /// snapshotted at build time so [`LuaQuestHandle`] / [`LuaQuestDataHandle`]
    /// can answer getters without a round-trip back to the Rust side.
    /// Kept in the same order as [`active_quests`], so binary or linear
    /// lookup by id is cheap.
    pub active_quest_states: Vec<QuestStateSnapshot>,
    /// Unlocked aetheryte node ids.
    pub unlocked_aetherytes: Vec<u32>,
    /// Trait ids the player has learned.
    pub traits: Vec<u16>,
    /// (item_id, quantity) tuples in inventory.
    pub inventory: Vec<(u32, i32)>,
    /// Set by `player:SetLoginDirector(director)` during `onBeginLogin`
    /// — zero means no director attached. `player:GetDirector()` reads
    /// this to hand the `zone.lua:onZoneIn` hook a real director handle
    /// so its `player:KickEvent(player:GetDirector(), "noticeEvent")`
    /// call lands on the right actor id.
    pub login_director_actor_id: u32,
    /// Mirror of `Session.spawned_retainer` at the time the snapshot
    /// was built. Lets scripts read `player:HasSpawnedRetainer()` +
    /// `player:GetSpawnedRetainer()` without a round-trip.
    /// Populated by `PlayerSnapshot::populate_retainer` after the
    /// From impl runs — the session isn't reachable from a plain
    /// `&Player`, so the processor fills it in before handing the
    /// snapshot to Lua.
    pub spawned_retainer: Option<SpawnedRetainerSnapshot>,
    /// Mirror of `Session.current_dream_id` — `Some(id)` while the
    /// client is showing the dream-overlay view, `None` otherwise.
    /// Read by `player:IsDreaming()` / `player:GetDreamId()`.
    pub current_dream_id: Option<u8>,
    /// Mirror of `Session.is_sleeping` — true between a
    /// `SetSleeping` call and the next wake (zone-out / explicit
    /// unset). Read by `player:IsSleeping()`.
    pub is_sleeping: bool,
}

/// Script-visible view of [`crate::data::SpawnedRetainer`]. Keeps
/// only the fields `retainer.lua` actually reads — if a Lua call
/// site needs richer data we can extend without touching the wire.
#[derive(Debug, Clone, Default)]
pub struct SpawnedRetainerSnapshot {
    pub retainer_id: u32,
    pub actor_class_id: u32,
    pub name: String,
    pub position: (f32, f32, f32),
    pub rotation: f32,
}

impl From<&crate::actor::Player> for PlayerSnapshot {
    fn from(p: &crate::actor::Player) -> Self {
        let active_quests: Vec<u32> = p
            .character
            .quest_journal
            .slots
            .iter()
            .flatten()
            .map(|q| q.quest_id())
            .collect();
        let active_quest_states: Vec<QuestStateSnapshot> = p
            .character
            .quest_journal
            .slots
            .iter()
            .flatten()
            .map(|q| QuestStateSnapshot {
                quest_id: q.quest_id(),
                sequence: q.get_sequence(),
                flags: q.get_flags(),
                counters: [
                    q.get_counter(0),
                    q.get_counter(1),
                    q.get_counter(2),
                ],
            })
            .collect();
        let completed_quests: Vec<u32> = p.character.quest_journal.iter_completed().collect();
        let unlocked_aetherytes: Vec<u32> = p.helpers.unlocked_aetherytes.iter().copied().collect();
        let traits: Vec<u16> = p.helpers.traits.iter().map(|t| t.id).collect();
        let inventory: Vec<(u32, i32)> = p
            .helpers
            .inventory_summary
            .iter()
            .map(|(id, qty)| (*id, *qty))
            .collect();

        Self {
            actor_id: p.character.base.actor_id,
            name: p.character.base.actor_name.clone(),
            zone_id: p.character.base.zone_id,
            pos: (
                p.character.base.position_x,
                p.character.base.position_y,
                p.character.base.position_z,
            ),
            rotation: p.character.base.rotation,
            state: p.character.base.current_main_state,
            hp: p.character.chara.hp,
            max_hp: p.character.chara.max_hp,
            mp: p.character.chara.mp,
            max_mp: p.character.chara.max_mp,
            tp: p.character.chara.tp,
            play_time: p.player.play_time,
            current_class: p.character.chara.class as u8,
            current_level: p.character.chara.level,
            current_job: p.character.chara.current_job as u8,
            current_gil: p.get_current_gil().max(0) as u32,
            initial_town: p.get_initial_town(),
            tribe: 0,
            guardian: 0,
            birth_month: 0,
            birth_day: 0,
            homepoint: p.player.homepoint,
            homepoint_inn: p.player.homepoint_inn,
            // Mount/chocobo state moved to CharaState so the processor
            // can mutate it via the registry. Read from CharaState here
            // — PlayerState stays as the login DTO shape and is copied
            // into CharaState at login (see `processor::handle_session_begin`).
            mount_state: p.character.chara.mount_state,
            has_chocobo: p.character.chara.has_chocobo,
            chocobo_appearance: p.character.chara.chocobo_appearance,
            chocobo_name: p.character.chara.chocobo_name.clone(),
            rental_expire_time: p.character.chara.rental_expire_time,
            rental_min_left: p.character.chara.rental_min_left,
            gc_current: p.character.chara.gc_current,
            gc_rank_limsa: p.character.chara.gc_rank_limsa,
            gc_rank_gridania: p.character.chara.gc_rank_gridania,
            gc_rank_uldah: p.character.chara.gc_rank_uldah,
            is_gm: p.player.is_gm,
            is_engaged: p.character.is_engaged(),
            is_trading: p.is_trading(),
            is_trade_accepted: p.is_trade_accepted(),
            is_party_leader: p.is_party_leader(),
            current_event_owner: p.player.current_event_owner,
            current_event_name: p.player.current_event_name.clone(),
            current_event_type: p.player.current_event_type,
            completed_quests,
            active_quests,
            active_quest_states,
            unlocked_aetherytes,
            traits,
            inventory,
            login_director_actor_id: p.character.chara.login_director_actor_id,
            // The From<&Player> impl can't reach the session store
            // (Player is in the registry, Session lives on
            // WorldManager). Default to None here and let the
            // processor overlay the real snapshot after the fact
            // via `PlayerSnapshot::set_spawned_retainer`.
            spawned_retainer: None,
            current_dream_id: None,
            is_sleeping: false,
        }
    }
}

impl PlayerSnapshot {
    /// Overlay the retainer snapshot sourced from
    /// [`crate::data::Session::spawned_retainer`]. Called right
    /// after `From<&Player>` when the session is available.
    pub fn set_spawned_retainer(&mut self, r: Option<&crate::data::SpawnedRetainer>) {
        self.spawned_retainer = r.map(|r| SpawnedRetainerSnapshot {
            retainer_id: r.retainer_id,
            actor_class_id: r.actor_class_id,
            name: r.name.clone(),
            position: r.position,
            rotation: r.rotation,
        });
    }

    /// Overlay the inn/dream snapshot sourced from
    /// [`crate::data::Session::current_dream_id`] +
    /// [`crate::data::Session::is_sleeping`]. Called alongside
    /// `set_spawned_retainer` when the session is available.
    pub fn set_inn_state(&mut self, current_dream_id: Option<u8>, is_sleeping: bool) {
        self.current_dream_id = current_dream_id;
        self.is_sleeping = is_sleeping;
    }
}

#[derive(Debug, Clone)]
pub struct LuaPlayer {
    pub snapshot: PlayerSnapshot,
    pub queue: Arc<Mutex<CommandQueue>>,
}

impl LuaPlayer {
    fn is_class_range(class: u8, range: std::ops::RangeInclusive<u8>) -> bool {
        range.contains(&class)
    }
}

impl UserData for LuaPlayer {
    // `PopulaceChocoboLender.lua` and a handful of other scripts
    // address a few read-only fields via dot syntax (`player.hasChocobo`,
    // `player.mountState`, `player.currentGil`, …) rather than the
    // colon-call form (`player:HasChocobo()`). mlua only exposes them
    // through the __index metamethod if they're registered as
    // `add_field_method_get`, so we mirror the most-used scalar getters
    // here. New scripts should prefer the method form when adding a
    // binding.
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("hasChocobo", |_, this| {
            Ok(this.snapshot.has_chocobo)
        });
        fields.add_field_method_get("mountState", |_, this| {
            Ok(this.snapshot.mount_state)
        });
        fields.add_field_method_get("chocoboAppearance", |_, this| {
            Ok(this.snapshot.chocobo_appearance)
        });
        fields.add_field_method_get("chocoboName", |_, this| {
            Ok(this.snapshot.chocobo_name.clone())
        });
        // Grand Company dot-syntax fields — `gcseals.lua` and
        // `PopulaceCompanyOfficer.lua` read all four directly as
        // `player.gcCurrent` / `player.gcRankLimsa` / `.gcRankGridania`
        // / `.gcRankUldah`.
        fields.add_field_method_get("gcCurrent", |_, this| Ok(this.snapshot.gc_current));
        fields.add_field_method_get("gcRankLimsa", |_, this| Ok(this.snapshot.gc_rank_limsa));
        fields.add_field_method_get("gcRankGridania", |_, this| {
            Ok(this.snapshot.gc_rank_gridania)
        });
        fields.add_field_method_get("gcRankUldah", |_, this| {
            Ok(this.snapshot.gc_rank_uldah)
        });
        // `player.CurrentArea` — pmeteor exposes Player.CurrentArea as a
        // public Area field, which Lua reaches through dot syntax in
        // `man0g0::doContentArea`:
        //   `contentArea = player.CurrentArea:CreateContentArea(...)`.
        // We return a `LuaZone` userdata whose `CreateContentArea`
        // method picks up the parent zone and routes the rest. Same
        // queue handle as `player:GetZone()` so commands flow through
        // a single command pipeline.
        fields.add_field_method_get("CurrentArea", |lua, this| {
            let zone = LuaZone {
                snapshot: ZoneSnapshot {
                    zone_id: this.snapshot.zone_id,
                    zone_name: String::new(),
                    player_ids: Vec::new(),
                    npc_ids: Vec::new(),
                    monster_ids: Vec::new(),
                },
                queue: this.queue.clone(),
            };
            lua.create_userdata(zone)
        });
    }

    #[allow(clippy::too_many_lines)]
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // --- Identity --------------------------------------------------------
        methods.add_method("GetName", |_, this, _: ()| Ok(this.snapshot.name.clone()));
        methods.add_method("GetZoneID", |_, this, _: ()| Ok(this.snapshot.zone_id));
        methods.add_method("GetState", |_, this, _: ()| Ok(this.snapshot.state));
        methods.add_method("GetPos", |_, this, _: ()| {
            let s = &this.snapshot;
            Ok((s.pos.0, s.pos.1, s.pos.2, s.rotation, s.zone_id))
        });

        // --- Class/level -----------------------------------------------------
        methods.add_method("GetCurrentClassOrJob", |_, this, _: ()| {
            Ok(if this.snapshot.current_job != 0 {
                this.snapshot.current_job
            } else {
                this.snapshot.current_class
            })
        });
        methods.add_method("GetHighestLevel", |_, this, _: ()| {
            Ok(this.snapshot.current_level)
        });
        methods.add_method("ConvertClassIdToJobId", |_, _, class_id: u8| {
            // Direct port of Player.ConvertClassIdToJobId — classes get their
            // corresponding jobs; everything else is the identity.
            let job = match class_id {
                2 => 19,  // PUG → MNK
                3 => 20,  // GLA → PLD
                4 => 21,  // MRD → WAR
                7 => 23,  // ARC → BRD
                8 => 22,  // LNC → DRG
                22 => 25, // THM → BLM
                23 => 24, // CNJ → WHM
                other => other,
            };
            Ok(job)
        });

        // --- Stats -----------------------------------------------------------
        methods.add_method("GetHP", |_, this, _: ()| Ok(this.snapshot.hp));
        methods.add_method("GetMaxHP", |_, this, _: ()| Ok(this.snapshot.max_hp));
        methods.add_method("GetMP", |_, this, _: ()| Ok(this.snapshot.mp));
        methods.add_method("GetMaxMP", |_, this, _: ()| Ok(this.snapshot.max_mp));
        methods.add_method("GetTP", |_, this, _: ()| Ok(this.snapshot.tp));

        methods.add_method("IsDiscipleOfWar", |_, this, _: ()| {
            Ok(LuaPlayer::is_class_range(
                this.snapshot.current_class,
                2..=8,
            ))
        });
        methods.add_method("IsDiscipleOfMagic", |_, this, _: ()| {
            Ok(LuaPlayer::is_class_range(
                this.snapshot.current_class,
                22..=23,
            ))
        });
        methods.add_method("IsDiscipleOfHand", |_, this, _: ()| {
            Ok(LuaPlayer::is_class_range(
                this.snapshot.current_class,
                29..=36,
            ))
        });
        methods.add_method("IsDiscipleOfLand", |_, this, _: ()| {
            Ok(LuaPlayer::is_class_range(
                this.snapshot.current_class,
                39..=41,
            ))
        });

        // --- Location / money ------------------------------------------------
        methods.add_method("GetCurrentGil", |_, this, _: ()| {
            Ok(this.snapshot.current_gil)
        });
        methods.add_method("GetInitialTown", |_, this, _: ()| {
            Ok(this.snapshot.initial_town)
        });
        methods.add_method("GetHomePoint", |_, this, _: ()| Ok(this.snapshot.homepoint));
        methods.add_method("GetHomePointInn", |_, this, _: ()| {
            Ok(this.snapshot.homepoint_inn)
        });
        methods.add_method("SetHomePoint", |_, this, homepoint: u32| {
            push(
                &this.queue,
                LuaCommand::SetHomePoint {
                    player_id: this.snapshot.actor_id,
                    homepoint,
                },
            );
            Ok(())
        });
        methods.add_method("GetMountState", |_, this, _: ()| {
            Ok(this.snapshot.mount_state)
        });

        // --- Retainer --------------------------------------------------------
        // `player:SpawnMyRetainer(bell, retainerIndex)` — the bell
        // argument can be any Npc/Actor userdata (we only read its
        // position); `retainerIndex` is 1-based per Meteor's caller
        // at `Player.SpawnMyRetainer(bell, retainerIndex)`.
        methods.add_method(
            "SpawnMyRetainer",
            |_, this, (bell, retainer_index): (AnyUserData, Option<i32>)| {
                let idx = retainer_index.unwrap_or(1);
                let (bell_actor_id, bell_pos) = if let Ok(npc) = bell.borrow::<LuaNpc>() {
                    (npc.base.actor_id, npc.base.pos)
                } else if let Ok(actor) = bell.borrow::<LuaActor>() {
                    (actor.actor_id, actor.pos)
                } else {
                    (0, (0.0, 0.0, 0.0))
                };
                push(
                    &this.queue,
                    LuaCommand::SpawnMyRetainer {
                        player_id: this.snapshot.actor_id,
                        bell_actor_id,
                        bell_position: bell_pos,
                        retainer_index: idx,
                    },
                );
                Ok(())
            },
        );
        methods.add_method("DespawnMyRetainer", |_, this, _: ()| {
            push(
                &this.queue,
                LuaCommand::DespawnMyRetainer {
                    player_id: this.snapshot.actor_id,
                },
            );
            Ok(())
        });
        methods.add_method("HireRetainer", |_, this, retainer_id: u32| {
            push(
                &this.queue,
                LuaCommand::HireRetainer {
                    player_id: this.snapshot.actor_id,
                    retainer_id,
                },
            );
            Ok(())
        });
        methods.add_method("DismissMyRetainer", |_, this, retainer_id: u32| {
            push(
                &this.queue,
                LuaCommand::DismissMyRetainer {
                    player_id: this.snapshot.actor_id,
                    retainer_id,
                },
            );
            Ok(())
        });
        // `player:HandInRegionalLeve(leveId)` — drain-side trigger
        // for the levemete hand-in flow. Pays `reward_gil` +
        // optional `reward_item_id x reward_quantity`; for an
        // enlisted player on a battlecraft leve, also accrues GC
        // seals at `reward_gil / 2`. Silently no-ops when the leve
        // isn't completed or isn't in the journal. Tier 3 #13
        // reward-payout + Tier 4 #16 C seal accrual.
        methods.add_method("HandInRegionalLeve", |_, this, leve_id: u32| {
            push(
                &this.queue,
                LuaCommand::HandInRegionalLeve {
                    player_id: this.snapshot.actor_id,
                    leve_id,
                },
            );
            Ok(())
        });
        // `player:AcceptRegionalLeve(leveId, difficulty)` — levemete
        // accept counterpart to `HandInRegionalLeve`. Adds the leve
        // to the journal with ACCEPTED_FLAG_BIT set + difficulty
        // band stamped. Idempotent on already-accepted leves.
        // `difficulty` defaults to 0 when omitted for ergonomic
        // call-sites that only ever offer the easiest band.
        methods.add_method(
            "AcceptRegionalLeve",
            |_, this, (leve_id, difficulty): (u32, Option<u8>)| {
                push(
                    &this.queue,
                    LuaCommand::AcceptRegionalLeve {
                        player_id: this.snapshot.actor_id,
                        leve_id,
                        difficulty: difficulty.unwrap_or(0),
                    },
                );
                Ok(())
            },
        );
        // `player:BuyFromRetainer(retainerId, serverItemId)` —
        // Tier 4 #14 D bazaar purchase. Drains through
        // `apply_purchase_retainer_bazaar_item` which handles gil
        // transfer + item move + listing cleanup in a single DB
        // transaction. Silently no-ops on an already-gone listing
        // (idempotent), insufficient gil, or self-purchase — the
        // outcome is logged but not propagated back to Lua since
        // the retail BazaarDeal flow handles rejection via
        // subsequent network messages rather than return codes.
        methods.add_method(
            "BuyFromRetainer",
            |_, this, (retainer_id, server_item_id): (u32, u64)| {
                push(
                    &this.queue,
                    LuaCommand::PurchaseRetainerBazaarItem {
                        buyer_id: this.snapshot.actor_id,
                        retainer_id,
                        server_item_id,
                    },
                );
                Ok(())
            },
        );
        methods.add_method("HasSpawnedRetainer", |_, this, _: ()| {
            Ok(this.snapshot.spawned_retainer.is_some())
        });

        // --- Chocobo / mount ------------------------------------------------
        // `PopulaceChocoboLender.lua` drives all of these on hire /
        // rent / summon flows. `IssueChocobo(appearance, name)`
        // records a fresh license; `StartChocoboRental(minutes)`
        // starts the rental timer; `SetMountState(state)` flips the
        // mount flag and triggers an appearance broadcast;
        // `SendMountAppearance()` re-emits the packet (used on
        // zone-in + appearance change).
        methods.add_method(
            "IssueChocobo",
            |_, this, (appearance, name): (u8, String)| {
                push(
                    &this.queue,
                    LuaCommand::IssueChocobo {
                        player_id: this.snapshot.actor_id,
                        appearance_id: appearance,
                        name,
                    },
                );
                Ok(())
            },
        );
        methods.add_method("StartChocoboRental", |_, this, minutes: u8| {
            push(
                &this.queue,
                LuaCommand::StartChocoboRental {
                    player_id: this.snapshot.actor_id,
                    minutes,
                },
            );
            Ok(())
        });
        methods.add_method("SetMountState", |_, this, state: u8| {
            push(
                &this.queue,
                LuaCommand::SetMountState {
                    player_id: this.snapshot.actor_id,
                    state,
                },
            );
            Ok(())
        });
        methods.add_method("SendMountAppearance", |_, this, _: ()| {
            push(
                &this.queue,
                LuaCommand::SendMountAppearance {
                    player_id: this.snapshot.actor_id,
                },
            );
            Ok(())
        });
        methods.add_method("SetChocoboName", |_, this, name: String| {
            push(
                &this.queue,
                LuaCommand::SetChocoboName {
                    player_id: this.snapshot.actor_id,
                    name,
                },
            );
            Ok(())
        });
        methods.add_method("HasChocobo", |_, this, _: ()| {
            Ok(this.snapshot.has_chocobo)
        });
        methods.add_method("IsChocoboRentalActive", |_, this, _: ()| {
            Ok(this.snapshot.rental_expire_time != 0)
        });
        methods.add_method("GetChocoboAppearance", |_, this, _: ()| {
            Ok(this.snapshot.chocobo_appearance)
        });
        methods.add_method("GetChocoboName", |_, this, _: ()| {
            Ok(this.snapshot.chocobo_name.clone())
        });

        // --- Grand Company -------------------------------------------------
        // `gcseals.lua::AddGCSeals` + `PopulaceCompanyOfficer.lua`'s
        // promotion flow drive these. `JoinGC(gc)` enlists; `SetGCRank`
        // promotes; `AddSeals(gc, amount)` grants the per-GC currency;
        // the `GetGC*` readers return the snapshot fields.
        methods.add_method("JoinGC", |_, this, gc: u8| {
            push(
                &this.queue,
                LuaCommand::JoinGC {
                    player_id: this.snapshot.actor_id,
                    gc,
                },
            );
            Ok(())
        });
        methods.add_method("SetGCRank", |_, this, (gc, rank): (u8, u8)| {
            push(
                &this.queue,
                LuaCommand::SetGCRank {
                    player_id: this.snapshot.actor_id,
                    gc,
                    rank,
                },
            );
            Ok(())
        });
        methods.add_method("AddSeals", |_, this, (gc, amount): (u8, i32)| {
            push(
                &this.queue,
                LuaCommand::AddSeals {
                    player_id: this.snapshot.actor_id,
                    gc,
                    amount,
                },
            );
            Ok(())
        });
        // `player:PromoteGC(gc)` — atomic seal-spend + rank-bump.
        // Called from `PopulaceCompanyOfficer.lua` after the
        // `eventDoRankUp` confirm fires (choice == 1). The processor
        // re-validates every precondition (enrollment, rank cap, seal
        // balance) so a script that pushes this command without
        // checking can't desync the player's state.
        methods.add_method("PromoteGC", |_, this, gc: u8| {
            push(
                &this.queue,
                LuaCommand::PromoteGC {
                    player_id: this.snapshot.actor_id,
                    gc,
                },
            );
            Ok(())
        });
        methods.add_method("GetGC", |_, this, _: ()| Ok(this.snapshot.gc_current));
        methods.add_method("GetGCRank", |_, this, gc: u8| {
            Ok(match gc {
                1 => this.snapshot.gc_rank_limsa,
                2 => this.snapshot.gc_rank_gridania,
                3 => this.snapshot.gc_rank_uldah,
                _ => 0,
            })
        });
        methods.add_method("GetSealCap", |_, this, gc: u8| {
            let rank = match gc {
                1 => this.snapshot.gc_rank_limsa,
                2 => this.snapshot.gc_rank_gridania,
                3 => this.snapshot.gc_rank_uldah,
                _ => return Ok(0),
            };
            Ok(crate::actor::gc::rank_seal_cap(rank))
        });

        // --- Inn / dream -----------------------------------------------------
        // `ObjectBed.lua::onEventStarted` calls `player:SetSleeping()`
        // right before `QuitGame` / `Logout`; the processor snaps the
        // character's transform to the bed coord of whichever inn
        // room they're in (1/2/3). No-ops outside an inn zone.
        methods.add_method("SetSleeping", |_, this, _: ()| {
            push(
                &this.queue,
                LuaCommand::SetSleeping {
                    player_id: this.snapshot.actor_id,
                },
            );
            Ok(())
        });
        // `player:Logout()` — soft logout (returns to character
        // select). Called from `ObjectBed.lua` (sleep-and-stay-logged-
        // in choice) and `LogoutCommand.lua`. Drains to
        // `LuaCommand::Logout` → processor emits LogoutPacket
        // (0x000E) to the owning session.
        methods.add_method("Logout", |_, this, _: ()| {
            push(
                &this.queue,
                LuaCommand::Logout {
                    player_id: this.snapshot.actor_id,
                },
            );
            Ok(())
        });
        // `player:QuitGame()` — hard exit (closes the client). Same
        // call sites as `Logout`; drains to `LuaCommand::QuitGame` →
        // processor emits QuitPacket (0x0011) to the owning session.
        methods.add_method("QuitGame", |_, this, _: ()| {
            push(
                &this.queue,
                LuaCommand::QuitGame {
                    player_id: this.snapshot.actor_id,
                },
            );
            Ok(())
        });
        // Hildibrand `etc5*` quest scripts drive dream cutscenes via
        // these. `dreamId` is the visual-effect id (retail 1.x
        // known codes: 0x16 for the standard inn fade).
        methods.add_method("StartDream", |_, this, dream_id: u8| {
            push(
                &this.queue,
                LuaCommand::StartDream {
                    player_id: this.snapshot.actor_id,
                    dream_id,
                },
            );
            Ok(())
        });
        methods.add_method("EndDream", |_, this, _: ()| {
            push(
                &this.queue,
                LuaCommand::EndDream {
                    player_id: this.snapshot.actor_id,
                },
            );
            Ok(())
        });
        methods.add_method("IsDreaming", |_, this, _: ()| {
            Ok(this.snapshot.current_dream_id.is_some())
        });
        methods.add_method("GetDreamId", |_, this, _: ()| {
            Ok(this.snapshot.current_dream_id.unwrap_or(0))
        });
        methods.add_method("IsSleeping", |_, this, _: ()| {
            Ok(this.snapshot.is_sleeping)
        });
        // Returns `LuaRetainer | nil`. The snapshot already has the
        // retainer fields; we copy them into a userdata with a no-op
        // `GetItemPackage` binding (matches the existing
        // `LuaItemPackage` surface — the retainer inventory live path
        // isn't wired yet, so the chain resolves but emits AddItem
        // commands that currently log-only for retainer-owned bags).
        methods.add_method("GetSpawnedRetainer", |_, this, _: ()| {
            Ok(this
                .snapshot
                .spawned_retainer
                .clone()
                .map(|r| LuaRetainer {
                    retainer_id: r.retainer_id,
                    actor_class_id: r.actor_class_id,
                    name: r.name,
                    position: r.position,
                    rotation: r.rotation,
                    queue: this.queue.clone(),
                    player_actor_id: this.snapshot.actor_id,
                }))
        });

        // --- Play time -------------------------------------------------------
        methods.add_method("GetPlayTime", |_, this, _do_update: Option<bool>| {
            Ok(this.snapshot.play_time)
        });

        // --- Status flags ----------------------------------------------------
        methods.add_method("IsEngaged", |_, this, _: ()| Ok(this.snapshot.is_engaged));
        methods.add_method("IsTrading", |_, this, _: ()| Ok(this.snapshot.is_trading));
        methods.add_method("IsTradeAccepted", |_, this, _: ()| {
            Ok(this.snapshot.is_trade_accepted)
        });
        methods.add_method("IsPartyLeader", |_, this, _: ()| {
            Ok(this.snapshot.is_party_leader)
        });
        methods.add_method("IsGM", |_, this, _: ()| Ok(this.snapshot.is_gm));

        // --- Identity helpers (aetheryte, traits, items) --------------------
        methods.add_method("HasAetheryteNodeUnlocked", |_, this, id: u32| {
            Ok(this.snapshot.unlocked_aetherytes.contains(&id))
        });
        methods.add_method("HasTrait", |_, this, id: u16| {
            Ok(this.snapshot.traits.contains(&id))
        });
        methods.add_method(
            "HasItem",
            |_, this, (catalog_id, min_quantity): (u32, Option<i32>)| {
                let min = min_quantity.unwrap_or(1);
                Ok(this
                    .snapshot
                    .inventory
                    .iter()
                    .any(|(id, q)| *id == catalog_id && *q >= min))
            },
        );

        // --- Quests ----------------------------------------------------------
        methods.add_method("HasQuest", |_, this, id: u32| {
            Ok(this.snapshot.active_quests.contains(&id))
        });
        methods.add_method("IsQuestCompleted", |_, this, id: u32| {
            Ok(this.snapshot.completed_quests.contains(&id))
        });
        methods.add_method("CanAcceptQuest", |_, this, id: u32| {
            Ok(!this.snapshot.completed_quests.contains(&id)
                && !this.snapshot.active_quests.contains(&id)
                && this.snapshot.active_quests.len() < 16)
        });
        methods.add_method("GetFreeQuestSlot", |_, this, _: ()| {
            Ok(16i32 - this.snapshot.active_quests.len() as i32)
        });
        methods.add_method("AddQuest", |_, this, id: u32| {
            push(
                &this.queue,
                LuaCommand::AddQuest {
                    player_id: this.snapshot.actor_id,
                    quest_id: id,
                },
            );
            Ok(())
        });
        methods.add_method("CompleteQuest", |_, this, id: u32| {
            push(
                &this.queue,
                LuaCommand::CompleteQuest {
                    player_id: this.snapshot.actor_id,
                    quest_id: id,
                },
            );
            Ok(())
        });
        methods.add_method("AbandonQuest", |_, this, id: u32| {
            push(
                &this.queue,
                LuaCommand::AbandonQuest {
                    player_id: this.snapshot.actor_id,
                    quest_id: id,
                },
            );
            Ok(())
        });

        // --- Event control --------------------------------------------------
        methods.add_method("EndEvent", |_, this, _: ()| {
            push(
                &this.queue,
                LuaCommand::EndEvent {
                    player_id: this.snapshot.actor_id,
                    event_owner: this.snapshot.current_event_owner,
                    event_name: this.snapshot.current_event_name.clone(),
                },
            );
            Ok(())
        });
        methods.add_method(
            "RunEventFunction",
            |_, this, (name, varargs): (String, mlua::MultiValue)| {
                // Marshal the trailing varargs through the same
                // value_to_command_arg pipeline KickEvent uses. These end up
                // in the wire packet's Lua-param region (offset 0x49) and
                // are *load-bearing* for cutscene RPCs: the opening flow
                // calls `callClientFunction(player, "delegateEvent",
                // player, quest, "processTtrNomal001withHQ")` from
                // `quests/man/man0l0.lua`, and the inner string param is
                // the cutscene routine the 1.x client dispatches.
                // Dropping it (the previous behaviour) shipped a packet
                // with `function_name=delegateEvent` and an empty
                // params block, and the client silently no-op'd it.
                let args: Vec<super::command::LuaCommandArg> = varargs
                    .iter()
                    .map(super::scheduler::value_to_command_arg)
                    .collect();
                push(
                    &this.queue,
                    LuaCommand::RunEventFunction {
                        player_id: this.snapshot.actor_id,
                        event_name: this.snapshot.current_event_name.clone(),
                        function_name: name,
                        args,
                    },
                );
                Ok(())
            },
        );
        methods.add_method(
            "KickEvent",
            |_, this, (target, trigger, varargs): (Value, String, mlua::MultiValue)| {
                // Extract the target actor id from the userdata the
                // script passed in (usually a `LuaDirectorHandle`). On
                // the tutorial path this is the `OpeningDirector`, and
                // the resulting `KickEventPacket` is what tells the
                // client to start the intro cutscene on that actor.
                let target_actor_id = match &target {
                    Value::UserData(ud) => ud
                        .borrow::<LuaDirectorHandle>()
                        .ok()
                        .map(|h| h.actor_id)
                        .or_else(|| {
                            ud.borrow::<LuaActor>().ok().map(|a| a.actor_id)
                        })
                        .unwrap_or(this.snapshot.current_event_owner),
                    _ => this.snapshot.current_event_owner,
                };
                // Convert the Lua varargs to `LuaCommandArg`s. Scripts
                // commonly pass `true`/`false`/integers here — for
                // `player:KickEvent(director, "noticeEvent", true)`
                // that becomes `[Bool(true)]`, which the C# server
                // propagates into the KickEventPacket's lua-param
                // stream via `LuaUtils.CreateLuaParamList`.
                let args: Vec<super::command::LuaCommandArg> = varargs
                    .iter()
                    .map(super::scheduler::value_to_command_arg)
                    .collect();
                push(
                    &this.queue,
                    LuaCommand::KickEvent {
                        player_id: this.snapshot.actor_id,
                        actor_id: target_actor_id,
                        trigger,
                        args,
                    },
                );
                Ok(())
            },
        );

        // --- Economy / progression ------------------------------------------
        methods.add_method("AddExp", |_, this, (class_id, exp): (u8, i32)| {
            push(
                &this.queue,
                LuaCommand::AddExp {
                    actor_id: this.snapshot.actor_id,
                    class_id,
                    exp,
                },
            );
            Ok(())
        });

        // Convenience over Meteor's
        // `player:GetItemPackage(INVENTORY_CURRENCY):AddItem(1000001, qty, 1)`.
        // The Rust side special-cases gil so reward scripts don't need to
        // know the item id / package code.
        methods.add_method("AddGil", |_, this, amount: i32| {
            push(
                &this.queue,
                LuaCommand::AddGil {
                    actor_id: this.snapshot.actor_id,
                    amount,
                },
            );
            Ok(())
        });

        // --- Lifecycle ------------------------------------------------------
        methods.add_method("Die", |_, this, _: ()| {
            push(
                &this.queue,
                LuaCommand::Die {
                    actor_id: this.snapshot.actor_id,
                },
            );
            Ok(())
        });
        methods.add_method("Revive", |_, this, _: ()| {
            push(
                &this.queue,
                LuaCommand::Revive {
                    actor_id: this.snapshot.actor_id,
                },
            );
            Ok(())
        });

        // --- Chat -----------------------------------------------------------
        methods.add_method(
            "SendMessage",
            |_, this, (message_type, sender, text): (u8, String, String)| {
                push(
                    &this.queue,
                    LuaCommand::SendMessage {
                        actor_id: this.snapshot.actor_id,
                        message_type,
                        sender,
                        text,
                    },
                );
                Ok(())
            },
        );

        // --- Phase 8c — top-called Lua host stubs ---------------------------
        // These reach scripts on every quest/NPC tick. Most emit a
        // LuaCommand that the game loop translates into a real packet
        // send later; a few are pure snapshot getters.

        methods.add_method(
            "SendGameMessage",
            |_,
             this,
             (_sender_actor, text_id, log_type, _rest): (
                Value,
                u32,
                Option<u8>,
                mlua::MultiValue,
            )| {
                // Matches the C# signature `SendGameMessage(worldmaster,
                // textId, logType, params…)`. We flatten the dynamic
                // params into the chat text field so scripts still see
                // their message reach the client.
                push(
                    &this.queue,
                    LuaCommand::SendMessage {
                        actor_id: this.snapshot.actor_id,
                        message_type: log_type.unwrap_or(0x20),
                        sender: String::new(),
                        text: format!("text:{text_id}"),
                    },
                );
                Ok(())
            },
        );
        methods.add_method(
            "SendDataPacket",
            |_,
             this,
             (_attention, _sender, _param, _text_id, _rest): (
                Value,
                Value,
                Value,
                Option<u32>,
                mlua::MultiValue,
            )| {
                // `player:SendDataPacket("attention", worldmaster, "", 25225, …)`
                // in retail. We log only — the real packet builder lives
                // on the cross-cutting sprint's TODO list.
                let _ = &this.snapshot.actor_id;
                Ok(())
            },
        );
        methods.add_method("ChangeMusic", |_, this, music_id: u16| {
            push(
                &this.queue,
                LuaCommand::ChangeMusic {
                    player_id: this.snapshot.actor_id,
                    music_id,
                },
            );
            Ok(())
        });
        methods.add_method(
            "ChangeSpeed",
            |_, this, (_idle, _walk, _run, _active): (f32, f32, f32, f32)| {
                // Speed changes flow through ActorProperty packets in
                // retail; Phase 8c records the intent so scripts don't
                // error. Full packet emission rides with the dispatcher
                // depth fills.
                let _ = this.snapshot.actor_id;
                Ok(())
            },
        );
        methods.add_method("GetZone", |lua, this, _: ()| {
            // Return a `LuaZone` userdata so scripts can chain
            // `player:GetZone():CreateDirector(...)`. `battlenpc.lua`
            // `onBeginLogin` needs this chain for the tutorial zone 193
            // opening director. An integer handle here would error on
            // the `:CreateDirector` call and the Lua frame would abort
            // before `SetLoginDirector` ran.
            let zone = LuaZone {
                snapshot: ZoneSnapshot {
                    zone_id: this.snapshot.zone_id,
                    zone_name: String::new(),
                    player_ids: Vec::new(),
                    npc_ids: Vec::new(),
                    monster_ids: Vec::new(),
                },
                queue: this.queue.clone(),
            };
            lua.create_userdata(zone)
        });
        methods.add_method("GetItemPackage", |lua, this, pkg_code: u16| {
            // Returning nil here made `onLogin`'s `initClassItems` /
            // `initRaceItems` path immediately abort on the first
            // `GetItemPackage(0):AddItems(...)` call (nil is not
            // indexable). Return a real `LuaItemPackage` userdata that
            // routes `AddItem`/`AddItems` into the command queue so the
            // hook traverses its full class/race branches and the
            // subsequent `SavePlayTime` etc. run to completion.
            //
            // Inventory snapshot is cloned through so `HasItem` /
            // `GetItemQuantity` calls on the returned package can
            // answer locally — `gcseals.lua` and
            // `PopulaceCompanyOfficer.lua` both chain
            // `:GetItemPackage(99):HasItem(seal, cost)`.
            let pkg = LuaItemPackage {
                owner_actor_id: this.snapshot.actor_id,
                package_code: pkg_code,
                queue: this.queue.clone(),
                inventory_snapshot: this.snapshot.inventory.clone(),
                is_retainer: false,
            };
            lua.create_userdata(pkg)
        });
        methods.add_method("GetQuest", |lua, this, id: u32| {
            // Scripts chain `GetQuest(id):SetQuestFlag(...)` /
            // `:GetData():IncCounter(...)` etc. If the player doesn't have
            // the quest we still return a handle — Meteor's behaviour is
            // similarly lenient (the mutations no-op on missing quest in
            // the processor). Populate the snapshot fields from the live
            // per-quest state so getters give real answers.
            let state = this
                .snapshot
                .active_quest_states
                .iter()
                .find(|s| s.quest_id == id)
                .copied()
                .unwrap_or(QuestStateSnapshot {
                    quest_id: id,
                    sequence: 0,
                    flags: 0,
                    counters: [0; 3],
                });
            let handle = LuaQuestHandle {
                player_id: this.snapshot.actor_id,
                quest_id: id,
                has_quest: this.snapshot.active_quests.contains(&id),
                sequence: state.sequence,
                flags: state.flags,
                counters: state.counters,
                queue: this.queue.clone(),
            };
            lua.create_userdata(handle)
        });
        methods.add_method("RemoveQuest", |_, this, id: u32| {
            push(
                &this.queue,
                LuaCommand::AbandonQuest {
                    player_id: this.snapshot.actor_id,
                    quest_id: id,
                },
            );
            Ok(())
        });
        methods.add_method("GetQuestSlot", |_, this, id: u32| {
            // Returns 0-based slot index, or -1 if the quest isn't
            // active. Matches Meteor's `GetQuestSlot(questId)` behaviour.
            let slot = this
                .snapshot
                .active_quests
                .iter()
                .position(|&q| q == id)
                .map(|i| i as i32)
                .unwrap_or(-1);
            Ok(slot)
        });
        methods.add_method("SetQuestComplete", |_, this, args: mlua::MultiValue| {
            // `SetQuestComplete(id, flag=true)` — flag omitted means
            // "mark complete". Meteor scripts call this form on prereq
            // cross-references and GM debug commands.
            let mut iter = args.into_iter();
            let Some(Value::Integer(id)) = iter.next() else {
                return Ok(());
            };
            let flag = match iter.next() {
                Some(Value::Boolean(b)) => b,
                Some(Value::Nil) | None => true,
                Some(Value::Integer(i)) => i != 0,
                _ => true,
            };
            push(
                &this.queue,
                LuaCommand::SetQuestComplete {
                    player_id: this.snapshot.actor_id,
                    quest_id: id as u32,
                    flag,
                },
            );
            Ok(())
        });
        methods.add_method("GetQuestsForNpc", |lua, this, _npc: Value| {
            // Meteor's implementation filters active quests by whether
            // the NPC is in the quest's `QuestState.current` — the map
            // populated by `quest:SetENpc(...)` during `onStateChange`.
            // We don't surface ENPC membership in the snapshot (see
            // `QuestStateSnapshot` — only flags/counters travel across),
            // so the safe-and-permissive port returns ALL active
            // quests. Scripts typically use this to iterate for
            // onTalk-style dispatch, and our `onTalk` already fires per
            // active quest — the filtering happens inside each script's
            // sequence/classId check. Returns a Lua array.
            let table = lua.create_table()?;
            for (i, qid) in this.snapshot.active_quests.iter().enumerate() {
                let state = this
                    .snapshot
                    .active_quest_states
                    .iter()
                    .find(|s| s.quest_id == *qid)
                    .copied()
                    .unwrap_or(QuestStateSnapshot {
                        quest_id: *qid,
                        sequence: 0,
                        flags: 0,
                        counters: [0; 3],
                    });
                let handle = LuaQuestHandle {
                    player_id: this.snapshot.actor_id,
                    quest_id: *qid,
                    has_quest: true,
                    sequence: state.sequence,
                    flags: state.flags,
                    counters: state.counters,
                    queue: this.queue.clone(),
                };
                table.set(i + 1, lua.create_userdata(handle)?)?;
            }
            Ok(table)
        });
        methods.add_method("GetDefaultTalkQuest", |lua, this, _npc: Value| {
            // Meteor: "return the first active quest that has this NPC
            // in its state list." Same lenient approach as
            // `GetQuestsForNpc`: return the first active quest if any,
            // otherwise nil. onTalk already fans out to all quests so
            // scripts using this for talk-dispatch still hit the
            // right handler through their internal filter.
            match this.snapshot.active_quests.first().copied() {
                Some(qid) => {
                    let state = this
                        .snapshot
                        .active_quest_states
                        .iter()
                        .find(|s| s.quest_id == qid)
                        .copied()
                        .unwrap_or(QuestStateSnapshot {
                            quest_id: qid,
                            sequence: 0,
                            flags: 0,
                            counters: [0; 3],
                        });
                    let handle = LuaQuestHandle {
                        player_id: this.snapshot.actor_id,
                        quest_id: qid,
                        has_quest: true,
                        sequence: state.sequence,
                        flags: state.flags,
                        counters: state.counters,
                        queue: this.queue.clone(),
                    };
                    Ok(Value::UserData(lua.create_userdata(handle)?))
                }
                None => Ok(Value::Nil),
            }
        });
        methods.add_method("RemoveQuestByQuestId", |_, this, id: u32| {
            push(
                &this.queue,
                LuaCommand::AbandonQuest {
                    player_id: this.snapshot.actor_id,
                    quest_id: id,
                },
            );
            Ok(())
        });
        methods.add_method("ReplaceQuest", |_, this, (old_id, new_id): (u32, u32)| {
            push(
                &this.queue,
                LuaCommand::AbandonQuest {
                    player_id: this.snapshot.actor_id,
                    quest_id: old_id,
                },
            );
            push(
                &this.queue,
                LuaCommand::AddQuest {
                    player_id: this.snapshot.actor_id,
                    quest_id: new_id,
                },
            );
            Ok(())
        });

        // --- Director hooks (scripts call these from guildleve flows) -------
        methods.add_method("AddDirector", |_, _this, _director: Value| {
            // Director userdata isn't yet exposed to Lua; the real
            // member-add fires via DirectorOutbox on the Rust side. This
            // stub prevents nil-call errors from scripts.
            Ok(())
        });
        methods.add_method("RemoveDirector", |_, _this, _director: Value| Ok(()));
        methods.add_method("GetDirector", |lua, this, _id: Option<u32>| {
            // `zone.lua:onZoneIn` calls `player:GetDirector()` to recover
            // the login director attached earlier by `onBeginLogin`'s
            // `player:SetLoginDirector(director)`. Returning nil here
            // caused the followup `player:KickEvent(player:GetDirector(),
            // "noticeEvent")` to fall through to `current_event_owner`
            // (0 on fresh login), emitting a malformed KickEventPacket
            // that the client silently drops — which is precisely why
            // "Now Loading" never dismissed after the opening director
            // spawn. Hand back a real `LuaDirectorHandle` so the 2-arg
            // `KickEvent` resolves to the director's actor id.
            if this.snapshot.login_director_actor_id == 0 {
                return Ok(Value::Nil);
            }
            let handle = LuaDirectorHandle {
                name: String::new(),
                actor_id: this.snapshot.login_director_actor_id,
                class_path: String::new(),
                queue: this.queue.clone(),
            };
            let ud = lua.create_userdata(handle)?;
            Ok(Value::UserData(ud))
        });
        methods.add_method("GetGuildleveDirector", |_, _this, _: ()| Ok(Value::Nil));
        methods.add_method("SetLoginDirector", |_, this, director: Value| {
            // Extract the director's actor_id from the userdata so we can
            // reference the spawned actor in the player's ScriptBind
            // LuaParams. If the script somehow passes a non-director
            // value, fall back to 0 (client will see a null actor ref).
            let director_actor_id = match &director {
                Value::UserData(ud) => ud
                    .borrow::<LuaDirectorHandle>()
                    .ok()
                    .map(|h| h.actor_id)
                    .unwrap_or(0),
                _ => 0,
            };
            push(
                &this.queue,
                LuaCommand::SetLoginDirector {
                    player_id: this.snapshot.actor_id,
                    director_actor_id,
                },
            );
            Ok(())
        });
        methods.add_method("SetEventStatus", |_, _this, _status: Value| Ok(()));

        // --- Equipment / inventory ------------------------------------------
        methods.add_method("GetEquipment", |_, _this, _: ()| Ok(Value::Nil));
        methods.add_method("GetItem", |_, _this, _unique_id: u64| Ok(Value::Nil));
        methods.add_method("GetGearset", |_, _this, _class_id: u8| Ok(Value::Nil));

        // --- Trade ----------------------------------------------------------
        methods.add_method("GetOtherTrader", |_, _this, _: ()| Ok(Value::Nil));
        methods.add_method("GetTradeOfferings", |_, _this, _: ()| Ok(Value::Nil));
        methods.add_method("AddTradeItem", |_, _this, _: mlua::MultiValue| Ok(()));
        methods.add_method("RemoveTradeItem", |_, _this, _: mlua::MultiValue| Ok(()));
        methods.add_method("ClearTradeItems", |_, _this, _: ()| Ok(()));
        methods.add_method("FinishTradeTransaction", |_, _this, _: mlua::MultiValue| {
            Ok(())
        });

        // --- Session control ------------------------------------------------
        // `Disengage` is currently a no-op surface stub — the
        // dispatch-side equivalent runs in the battle-state machine
        // (`die_if_defender_fell`'s engaged-flag clear). The real
        // bindings for `DespawnMyRetainer` / `Logout` / `QuitGame`
        // live earlier in this `add_methods` body and emit
        // `LuaCommand::DespawnMyRetainer` / `Logout` / `QuitGame`
        // respectively; do NOT re-register no-op stubs for them
        // here because mlua's `add_method` overwrites earlier
        // registrations of the same name and the stub would
        // silently drop the script's drain.
        methods.add_method("Disengage", |_, _this, _: ()| Ok(()));

        // --- Party ----------------------------------------------------------
        methods.add_method("PartyLeave", |_, _this, _: ()| Ok(()));
        methods.add_method("PartyDisband", |_, _this, _: ()| Ok(()));
        methods.add_method("PartyKickPlayer", |_, _this, _name: String| Ok(()));
        methods.add_method("PartyOustPlayer", |_, _this, _name: String| Ok(()));
        methods.add_method("PartyPromote", |_, _this, _name: String| Ok(()));
        methods.add_method("RemoveFromCurrentPartyAndCleanup", |_, _this, _: ()| Ok(()));

        // --- Movement -------------------------------------------------------
        methods.add_method("ChangeState", |_, this, state: u16| {
            push(
                &this.queue,
                LuaCommand::ChangeState {
                    actor_id: this.snapshot.actor_id,
                    main_state: state,
                },
            );
            Ok(())
        });
        methods.add_method(
            "Warp",
            |_, this, (zone_id, x, y, z, rot): (u32, f32, f32, f32, Option<f32>)| {
                push(
                    &this.queue,
                    LuaCommand::Warp {
                        player_id: this.snapshot.actor_id,
                        zone_id,
                        x,
                        y,
                        z,
                        rotation: rot.unwrap_or(0.0),
                    },
                );
                Ok(())
            },
        );

        // `player:SetMod(modifier_key, value)` — apply a numeric
        // modifier (HP lock, speed, etc.). B3: queues
        // `LuaCommand::SetActorMod`, applied via `apply_set_actor_mod`
        // in `runtime/quest_apply.rs`, which writes through to the
        // character's `ModifierMap`. Combat math then reads the same
        // map (e.g. `Character::set_hp` clamps to floor 1 when
        // `MinimumHpLock >= 1`).
        methods.add_method("SetMod", |_, this, (modifier_key, value): (i64, i64)| {
            push(
                &this.queue,
                LuaCommand::SetActorMod {
                    actor_id: this.snapshot.actor_id,
                    modifier_key: modifier_key as u32,
                    value,
                },
            );
            Ok(())
        });

        // --- Lua-side table field access (player.positionX etc.) ------------
        methods.add_meta_method(mlua::MetaMethod::Index, |lua, this, key: String| {
            // `player.currentParty` returns a stub `LuaParty` userdata
            // so content scripts can chain `:AddMember(actor_id)`.
            // Real party state lives on world-server; this is the
            // userdata bridge.
            if key == "currentParty" {
                let party = LuaParty {
                    leader_actor_id: this.snapshot.actor_id,
                    queue: this.queue.clone(),
                };
                return lua.create_userdata(party).map(Value::UserData);
            }
            let out: Value = match key.as_str() {
                "positionX" => Value::Number(this.snapshot.pos.0 as f64),
                "positionY" => Value::Number(this.snapshot.pos.1 as f64),
                "positionZ" => Value::Number(this.snapshot.pos.2 as f64),
                "rotation" => Value::Number(this.snapshot.rotation as f64),
                "actorId" => Value::Integer(this.snapshot.actor_id as i64),
                "actorName" => Value::Nil, // deliberately unchangeable
                "isGM" => Value::Boolean(this.snapshot.is_gm),
                _ => Value::Nil,
            };
            Ok(out)
        });

        // The C# original exposed `player.positionX = …` mutators. We forward
        // those through the command queue via `SetPos`.
        methods.add_meta_method_mut(
            mlua::MetaMethod::NewIndex,
            |_, this, (key, value): (String, f32)| {
                let mut pos = this.snapshot.pos;
                let mut rot = this.snapshot.rotation;
                match key.as_str() {
                    "positionX" => pos.0 = value,
                    "positionY" => pos.1 = value,
                    "positionZ" => pos.2 = value,
                    "rotation" => rot = value,
                    _ => return Ok(()),
                }
                this.snapshot.pos = pos;
                this.snapshot.rotation = rot;
                push(
                    &this.queue,
                    LuaCommand::SetPos {
                        actor_id: this.snapshot.actor_id,
                        zone_id: this.snapshot.zone_id,
                        x: pos.0,
                        y: pos.1,
                        z: pos.2,
                        rotation: rot,
                    },
                );
                Ok(())
            },
        );
    }
}

// ---------------------------------------------------------------------------
// LuaZone — rudimentary zone handle exposing spawn/despawn and player lists.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct ZoneSnapshot {
    pub zone_id: u32,
    pub zone_name: String,
    pub player_ids: Vec<u32>,
    pub npc_ids: Vec<u32>,
    pub monster_ids: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct LuaZone {
    pub snapshot: ZoneSnapshot,
    pub queue: Arc<Mutex<CommandQueue>>,
}

impl UserData for LuaZone {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("GetZoneID", |_, this, _: ()| Ok(this.snapshot.zone_id));
        methods.add_method("GetZoneName", |_, this, _: ()| {
            Ok(this.snapshot.zone_name.clone())
        });
        methods.add_method("GetPlayers", |_, this, _: ()| {
            Ok(this.snapshot.player_ids.clone())
        });
        methods.add_method("GetMonsters", |_, this, _: ()| {
            Ok(this.snapshot.monster_ids.clone())
        });
        methods.add_method("GetAllies", |_, _this, _: ()| Ok(Vec::<u32>::new()));
        // `zone:CreateDirector(name, some_flag)` is called from
        // `battlenpc.lua`/`player.lua` `onBeginLogin` for the tutorial
        // opening. Computes the director's actor id per the C# formula
        // `(6 << 28) | (zone_actor_id << 19) | (director_local_id + 2)`
        // (see `Director.cs:49` ctor base call — the `+ 2` quirk
        // reserves slots 0 and 1 for system actors) and fires a
        // `CreateDirector` command so the host can emit the director's
        // spawn packets in the same pass as the zone-in bundle.
        // Returns a LuaDirectorHandle carrying that actor id so
        // `player:SetLoginDirector(director)` can read it back. For
        // the login director we only ever need one per zone, so
        // `director_local_id` is always 0 — actor_id ends up at
        // `(6 << 28) | (zone << 19) | 2`.
        methods.add_method(
            "CreateDirector",
            |lua, this, (name, _flag): (String, Option<bool>)| {
                let director_local_id: u32 = 0;
                let zone_actor_id = this.snapshot.zone_id;
                let director_actor_id = crate::director::director::encode_director_actor_id(
                    zone_actor_id,
                    director_local_id,
                );
                // C# `Director.init()` returns the classPath — for
                // OpeningDirector that's `/Director/OpeningDirector`.
                // We reconstruct the path deterministically from the
                // `name` arg rather than calling the director script's
                // `init()` here; it matches the OpeningDirector case
                // and avoids pulling the director script VM into this
                // userdata method.
                let class_path = format!("/Director/{name}");
                push(
                    &this.queue,
                    LuaCommand::CreateDirector {
                        director_actor_id,
                        zone_actor_id,
                        class_path: class_path.clone(),
                    },
                );
                let handle = LuaDirectorHandle {
                    name,
                    actor_id: director_actor_id,
                    class_path,
                    queue: this.queue.clone(),
                };
                lua.create_userdata(handle)
            },
        );
        methods.add_method(
            "SpawnActor",
            |_, this, (class_id, x, y, z, rotation): (u32, f32, f32, f32, Option<f32>)| {
                push(
                    &this.queue,
                    LuaCommand::SpawnActor {
                        zone_id: this.snapshot.zone_id,
                        actor_class_id: class_id,
                        x,
                        y,
                        z,
                        rotation: rotation.unwrap_or(0.0),
                    },
                );
                Ok(())
            },
        );
        methods.add_method("DespawnActor", |_, this, actor_id: u32| {
            push(
                &this.queue,
                LuaCommand::DespawnActor {
                    zone_id: this.snapshot.zone_id,
                    actor_id,
                },
            );
            Ok(())
        });
        // `area:CreateContentArea(player, classPath, areaName,
        // contentScript, directorName, ...args)` — combat-tutorial entry
        // point per `man0g0::doContentArea`. Allocates the director +
        // content-area actor ids client-side, queues the runtime command
        // that materialises the `PrivateAreaContent`, and returns a
        // `LuaContentArea` handle so the lua chain can call
        // `:GetContentDirector()` + `:SpawnActor(...)` next.
        methods.add_method(
            "CreateContentArea",
            |lua,
             this,
             (
                player_arg,
                area_class_path,
                area_name,
                content_script,
                director_name,
            ): (mlua::Value, String, String, String, String)| {
                let parent_zone_id = this.snapshot.zone_id;
                let director_actor_id = crate::director::director::encode_director_actor_id(
                    parent_zone_id,
                    /* director_local_id */ 1,
                );
                // Content area gets its own actor id sharing the
                // director's encoding. `+ 0x80000` pushes it into a
                // high local-id band so it doesn't collide with normal
                // directors. Synthetic but stable across calls.
                let content_area_actor_id = crate::director::director::encode_director_actor_id(
                    parent_zone_id,
                    0x80000 | 1,
                );
                // Extract the player's actor id from the first arg so
                // the runtime handler can look up the player snapshot
                // and fire the content script's `onCreate` hook
                // against it. Tolerant: LuaPlayer (preferred), raw
                // u32, or fall through to 0 if neither.
                let player_id = match player_arg {
                    mlua::Value::UserData(ud) => ud
                        .borrow::<LuaPlayer>()
                        .map(|p| p.snapshot.actor_id)
                        .unwrap_or(0),
                    mlua::Value::Integer(i) => i as u32,
                    _ => 0,
                };
                push(
                    &this.queue,
                    LuaCommand::CreateContentArea {
                        player_id,
                        parent_zone_id,
                        area_class_path: area_class_path.clone(),
                        area_name: area_name.clone(),
                        content_script: content_script.clone(),
                        director_name: director_name.clone(),
                        director_actor_id,
                        content_area_actor_id,
                    },
                );
                let handle = LuaContentArea {
                    parent_zone_id,
                    area_name,
                    area_class_path,
                    director_name: director_name.clone(),
                    director_actor_id,
                    queue: this.queue.clone(),
                };
                lua.create_userdata(handle)
            },
        );
    }
}

// ---------------------------------------------------------------------------
// LuaContentArea — handle returned by `LuaZone:CreateContentArea`. The
// instance is materialised server-side by the `CreateContentArea` runtime
// command; this struct just carries the actor ids + names back to the
// lua chain so it can fetch the director, spawn actors, and trigger the
// content-finished cleanup.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// LuaParty — placeholder party userdata returned by `LuaPlayer.currentParty`
// (field accessor, see `MetaMethod::Index`). Used by content scripts (e.g.
// `SimpleContent30010.lua::onCreate`) to call
// `starterPlayer.currentParty:AddMember(papalymo.actorId)` so combat-tutorial
// NPCs join the player's party.
//
// Phase-A stub: methods log + no-op. Real party plumbing already exists in
// `world-server/src/group/party.rs` (Party / PartyManager); the next layer
// of work routes these stub calls through that subsystem. Until then the
// stub keeps content-script onCreate from aborting on a nil method lookup.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LuaParty {
    pub leader_actor_id: u32,
    pub queue: Arc<Mutex<CommandQueue>>,
}

impl UserData for LuaParty {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // `currentParty:AddMember(actor_id)` — invite a (B)NPC into the
        // player's party. Used by combat-tutorial scripts to add the
        // ally NPCs (Yda + Papalymo) to the player's roster so the
        // party-list UI shows them.
        //
        // B2: pushes `LuaCommand::PartyAddMember`, applied by the
        // processor to (1) accumulate the member in the leader's
        // session-scoped transient roster and (2) re-broadcast the
        // GroupHeader / GroupMembersBegin / X08 / End sequence so the
        // client's party-list UI shows the freshly-added ally.
        methods.add_method("AddMember", |_, this, actor_id: u32| {
            push(
                &this.queue,
                LuaCommand::PartyAddMember {
                    leader_actor_id: this.leader_actor_id,
                    member_actor_id: actor_id,
                },
            );
            Ok(())
        });

        // `currentParty:GetLeader()` — return the leader actor id.
        // Some scripts read this to gate behaviour. Safe to implement
        // on the snapshot.
        methods.add_method("GetLeader", |_, this, _: ()| {
            Ok(this.leader_actor_id)
        });
    }
}

#[derive(Debug, Clone)]
pub struct LuaContentArea {
    pub parent_zone_id: u32,
    pub area_name: String,
    pub area_class_path: String,
    pub director_name: String,
    pub director_actor_id: u32,
    pub queue: Arc<Mutex<CommandQueue>>,
}

impl UserData for LuaContentArea {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // `contentArea:GetContentDirector()` — return the director that
        // `Zone.CreateContentArea` instantiated alongside this area.
        // Mirrors `PrivateAreaContent.GetContentDirector` (returns
        // `currentDirector`). Garlemald's lua flow then calls
        // `player:AddDirector(director)` + `director:StartDirector(false)`.
        methods.add_method("GetContentDirector", |lua, this, _: ()| {
            let class_path = format!("/Director/{}", this.director_name);
            let handle = LuaDirectorHandle {
                name: this.director_name.clone(),
                actor_id: this.director_actor_id,
                class_path,
                queue: this.queue.clone(),
            };
            lua.create_userdata(handle)
        });

        // `contentArea:SpawnActor(classId, name, x, y, z, rot)` — fired
        // from `QuestDirectorMan0g001.lua::onCreateContentArea` to drop
        // Yda/Papalymo/mobs into the instance. Reuse the zone-level
        // SpawnActor command for now; the proper content-area variant
        // ships with the full PrivateAreaContent runtime.
        methods.add_method(
            "SpawnActor",
            |_, this, (class_id, _name, x, y, z, rotation): (
                u32,
                String,
                f32,
                f32,
                f32,
                Option<f32>,
            )| {
                push(
                    &this.queue,
                    LuaCommand::SpawnActor {
                        zone_id: this.parent_zone_id,
                        actor_class_id: class_id,
                        x,
                        y,
                        z,
                        rotation: rotation.unwrap_or(0.0),
                    },
                );
                Ok(())
            },
        );

        // `contentArea:ContentFinished()` — flag the area for cleanup
        // once the last player leaves. Mirrors
        // `PrivateAreaContent.ContentFinished`.
        methods.add_method("ContentFinished", |_, this, _: ()| {
            push(
                &this.queue,
                LuaCommand::ContentFinished {
                    parent_zone_id: this.parent_zone_id,
                    area_name: this.area_name.clone(),
                },
            );
            Ok(())
        });
    }
}

// ---------------------------------------------------------------------------
// LuaWorldManager — scripts reach for `GetWorldManager():DoZoneChange(...)`
// and friends.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct LuaWorldManager {
    pub queue: Arc<Mutex<CommandQueue>>,
}

impl UserData for LuaWorldManager {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method(
            "DoZoneChange",
            #[allow(clippy::type_complexity)]
            |_,
             this,
             (player_id, zone_id, private_area, private_area_type, spawn_type, x, y, z, rot): (
                u32,
                u32,
                Option<String>,
                Option<u32>,
                Option<u8>,
                f32,
                f32,
                f32,
                Option<f32>,
            )| {
                push(
                    &this.queue,
                    LuaCommand::DoZoneChange {
                        player_id,
                        zone_id,
                        private_area,
                        private_area_type: private_area_type.unwrap_or(0),
                        spawn_type: spawn_type.unwrap_or(0),
                        x,
                        y,
                        z,
                        rotation: rot.unwrap_or(0.0),
                    },
                );
                Ok(())
            },
        );

        // `GetWorldManager():DoZoneChangeContent(player, contentArea, x,
        // y, z, rot, spawnType)` — combat-tutorial zone-change entry
        // point per `man0g0::doContentArea`. The contentArea userdata is
        // accepted but the runtime command keys off the parent zone +
        // area name carried by the handle. Ported from C#
        // `WorldManager.DoZoneChangeContent` (Map Server/WorldManager.cs:971).
        methods.add_method(
            "DoZoneChangeContent",
            |_,
             this,
             (player_id, content_area, x, y, z, rotation, spawn_type): (
                u32,
                mlua::AnyUserData,
                f32,
                f32,
                f32,
                f32,
                Option<u8>,
            )| {
                let area = content_area.borrow::<LuaContentArea>()?;
                push(
                    &this.queue,
                    LuaCommand::DoZoneChangeContent {
                        player_id,
                        parent_zone_id: area.parent_zone_id,
                        area_name: area.area_name.clone(),
                        director_actor_id: area.director_actor_id,
                        spawn_type: spawn_type.unwrap_or(16),
                        x,
                        y,
                        z,
                        rotation,
                    },
                );
                Ok(())
            },
        );

        // `GetWorldManager().SpawnBattleNpcById(id, contentArea)` —
        // dot-callable function used by combat-tutorial content
        // scripts (`SimpleContent30010.lua::onCreate`) to spawn the
        // tutorial's named BattleNpcs (Yda + Papalymo + 3 wolves)
        // into the area. Returns a `LuaActor` userdata so the
        // calling chain can read `.actorId` to feed party-add /
        // director-add side-effects.
        //
        // B1 wire-up:
        //   * Pre-computes the actor id deterministically from the
        //     parent zone + a high-band actor_number (`0x40000 | id`)
        //     using the same formula as `Npc::new`:
        //     `(4 << 28) | (zone << 19) | (actor_number & 0x7FFFF)`.
        //     This puts the spawned BattleNpc in the standard
        //     `0x4xxx_xxxx` range distinct from sequential
        //     allocations (which start at 1).
        //   * Pushes `LuaCommand::SpawnBattleNpcById { bnpc_id,
        //     parent_zone_id, expected_actor_id }`. The runtime
        //     applier reads the `server_battlenpc_*` join, builds
        //     the ActorClass-keyed BattleNpc with the same id, and
        //     broadcasts the spawn packet trio. Subsequent
        //     `director:AddMember(actor)` /
        //     `currentParty:AddMember(actor.actorId)` calls during
        //     the same `onCreate` drain resolve to the same id.
        //
        // Note: registered as `add_function` (no implicit `self`)
        // because the script uses `.` access:
        // `GetWorldManager().SpawnBattleNpcById(...)` — not `:`.
        methods.add_function(
            "SpawnBattleNpcById",
            |lua, (id, content_area): (u32, mlua::AnyUserData)| {
                let (parent_zone_id, area_name, queue) = {
                    let area = content_area.borrow::<LuaContentArea>()?;
                    (
                        area.parent_zone_id,
                        area.area_name.clone(),
                        area.queue.clone(),
                    )
                };
                // Standard NPC actor-id formula. Using `0x40000 | id`
                // for actor_number puts these into the high half of
                // the per-zone actor-number space (sequential
                // allocations start at 1) — collision-free for any
                // realistic zone.
                let actor_number = 0x40000u32 | (id & 0x3FFFF);
                let expected_actor_id =
                    (4u32 << 28) | (parent_zone_id << 19) | (actor_number & 0x7FFFF);
                push(
                    &queue,
                    LuaCommand::SpawnBattleNpcById {
                        bnpc_id: id,
                        parent_zone_id,
                        expected_actor_id,
                    },
                );
                tracing::debug!(
                    bnpc_id = id,
                    parent_zone = parent_zone_id,
                    area = %area_name,
                    actor_id = format!("0x{:08X}", expected_actor_id),
                    "WorldManager.SpawnBattleNpcById queued",
                );
                let actor = LuaActor {
                    actor_id: expected_actor_id,
                    name: format!("bnpc_{id}"),
                    class_name: String::new(),
                    class_path: String::new(),
                    unique_id: format!("bnpc_{id}"),
                    zone_id: parent_zone_id,
                    zone_name: String::new(),
                    state: 0,
                    pos: (0.0, 0.0, 0.0),
                    rotation: 0.0,
                    queue,
                };
                lua.create_userdata(actor)
            },
        );

        // The remaining WorldManager methods (DoPlayerMoveInZone,
        // CreateInvitePartyGroup, CreateTradeGroup, AcceptTrade, …) queue
        // log-only stubs so scripts don't abort. Concrete handlers ship in
        // later phases.
        for stub in [
            "DoPlayerMoveInZone",
            "CreateInvitePartyGroup",
            "CreateTradeGroup",
            "AcceptTrade",
            "CancelTrade",
            "CompleteTrade",
            "RefuseTrade",
            "GroupInviteResult",
            "ReloadZone",
            "AddToBazaar",
            "BazaarBuyOperation",
            "BazaarSellOperation",
        ] {
            let name: &'static str = stub;
            methods.add_method(name, move |_, this, _: mlua::MultiValue| {
                push(
                    &this.queue,
                    LuaCommand::LogError(format!("WorldManager:{name} (stub)")),
                );
                Ok(())
            });
        }

        methods.add_method("GetPCInWorld", |_, _, _name: String| {
            Ok(Value::Nil) // TODO: resolve to LuaPlayer once player registry is live
        });

        // `GetWorldManager():GetArea(zoneId)` — retail returns the
        // `Zone` actor for the requested id; in-Lua callers (e.g.
        // `quests/man/man0l1.lua::seq000_onTalk`) chain
        // `:CreateDirector("AfterQuestWarpDirector", false)` on it to
        // stage a director in a foreign zone the player is about to
        // warp into. Returns a `LuaZone` userdata that shares this
        // WorldManager's command queue, with the snapshot pointed at
        // the requested zone id. The actual zone-registration lookup
        // is deferred — the LuaZone's only role here is to thread the
        // zone id into the resulting `LuaCommand::CreateDirector`
        // payload (which already carries `zone_actor_id` in its body)
        // so `send_zone_in_bundle` on the destination zone can emit
        // the director's 7-packet spawn sequence for the next
        // zone-in.
        methods.add_method("GetArea", |lua, this, zone_id: u32| {
            let zone = LuaZone {
                snapshot: ZoneSnapshot {
                    zone_id,
                    zone_name: String::new(),
                    player_ids: Vec::new(),
                    npc_ids: Vec::new(),
                    monster_ids: Vec::new(),
                },
                queue: this.queue.clone(),
            };
            lua.create_userdata(zone)
        });
    }
}

// ---------------------------------------------------------------------------
// LuaItemData / LuaGuildleveData — read-only gamedata views.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LuaItemData {
    pub id: u32,
    pub name: String,
    pub stack_size: u32,
    pub item_level: u16,
    pub equip_level: u16,
    pub price: u32,
    pub icon: u16,
    pub rarity: u16,
}

impl UserData for LuaItemData {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("GetId", |_, this, _: ()| Ok(this.id));
        methods.add_method("GetName", |_, this, _: ()| Ok(this.name.clone()));
        methods.add_method("GetStackSize", |_, this, _: ()| Ok(this.stack_size));
        methods.add_method("GetItemLevel", |_, this, _: ()| Ok(this.item_level));
        methods.add_method("GetEquipLevel", |_, this, _: ()| Ok(this.equip_level));
        methods.add_method("GetPrice", |_, this, _: ()| Ok(this.price));
        methods.add_method("GetIcon", |_, this, _: ()| Ok(this.icon));
        methods.add_method("GetRarity", |_, this, _: ()| Ok(this.rarity));
    }
}

// ---------------------------------------------------------------------------
// LuaDirectorHandle — stub returned by `Zone:CreateDirector(...)`. All the
// method chains scripts call on a director (`StartDirector`, `KickEvent`,
// `EndDirector`, etc.) are no-ops at the userdata layer; the packet-level
// actor spawn that would normally accompany them is deliberately omitted
// because emitting an ActorInstantiate for an unresolved director crashes
// the 1.23b client (earlier observation with master-actor spawns). The
// whole point of this handle is to let `battlenpc.lua`/`player.lua`
// `onBeginLogin` reach the `player:SetLoginDirector(director)` call without
// aborting on a nil-method error.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LuaDirectorHandle {
    pub name: String,
    pub actor_id: u32,
    pub class_path: String,
    pub queue: Arc<Mutex<CommandQueue>>,
}

impl UserData for LuaDirectorHandle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("GetName", |_, this, _: ()| Ok(this.name.clone()));
        // Common methods scripts call on directors. All no-ops at the
        // moment — propagating real director state requires spawning the
        // director as a tracked actor, which is the follow-up.
        // `director:StartDirector(spawn_immediate)` — kicks off the
        // director's `main(thisDirector)` coroutine. Pushed as
        // `LuaCommand::StartDirectorMain`; the processor's
        // `apply_start_director_main` resolves the script via
        // `lua.resolver().director(class_name)`, calls
        // `LuaEngine::spawn_director_main`, and drains the initial
        // slice's emitted commands. Yields on `wait(N)` park the
        // coroutine in the shared scheduler; the ticker's per-tick
        // `engine.tick()` call resumes them.
        methods.add_method("StartDirector", |_, this, spawn_immediate: Option<bool>| {
            let class_name = this
                .class_path
                .rsplit('/')
                .next()
                .unwrap_or(&this.class_path)
                .to_string();
            push(
                &this.queue,
                LuaCommand::StartDirectorMain {
                    director_actor_id: this.actor_id,
                    class_path: this.class_path.clone(),
                    director_name: class_name,
                    spawn_immediate: spawn_immediate.unwrap_or(true),
                },
            );
            Ok(())
        });
        methods.add_method("EndDirector", |_, _this, _: ()| Ok(()));
        methods.add_method("StartSceneSession", |_, _this, _: Option<Value>| Ok(()));
        methods.add_method("EndSceneSession", |_, _this, _: ()| Ok(()));
        methods.add_method("AddMember", |_, _this, _member: Value| Ok(()));
        methods.add_method("RemoveMember", |_, _this, _member: Value| Ok(()));
        methods.add_method("GetContentMembers", |_, _this, _: ()| Ok(Vec::<u32>::new()));
        methods.add_method("SetLeader", |_, _this, _actor: Value| Ok(()));
        methods.add_method("IsInstanceRaid", |_, _this, _: ()| Ok(false));
        // `director:EndGuildleve(was_completed)` — drives the
        // production drain that lets a `directors/Guildleve/*.lua`
        // `main` coroutine actually finish the leve. Pushes
        // `LuaCommand::EndGuildleve`; the processor handler decodes
        // the zone from the director's composite actor id, calls
        // `GuildleveDirector::end_guildleve` into a local
        // `DirectorOutbox`, and immediately drains the resulting
        // `GuildleveEnded` event through `dispatch_director_event`
        // — closing the loop on yesterday's
        // `award_leve_completion_seals` so it actually fires for live
        // players, not just integration tests.
        methods.add_method("EndGuildleve", |_, this, was_completed: Option<bool>| {
            push(
                &this.queue,
                LuaCommand::EndGuildleve {
                    director_actor_id: this.actor_id,
                    was_completed: was_completed.unwrap_or(true),
                },
            );
            Ok(())
        });
        // The remaining leve-side bindings are the same shape — push
        // a `LuaCommand::*` with the director's composite actor id so
        // the processor can decode the zone from the actor-id bits
        // without a separate lookup. Together with `EndGuildleve`
        // above they cover the full surface a
        // `directors/Guildleve/*.lua` `main` coroutine touches.
        methods.add_method("StartGuildleve", |_, this, _: ()| {
            push(
                &this.queue,
                LuaCommand::StartGuildleve {
                    director_actor_id: this.actor_id,
                },
            );
            Ok(())
        });
        methods.add_method("AbandonGuildleve", |_, this, _: ()| {
            push(
                &this.queue,
                LuaCommand::AbandonGuildleve {
                    director_actor_id: this.actor_id,
                },
            );
            Ok(())
        });
        methods.add_method(
            "UpdateAimNumNow",
            |_, this, (index, value): (u8, i8)| {
                push(
                    &this.queue,
                    LuaCommand::UpdateAimNumNow {
                        director_actor_id: this.actor_id,
                        index,
                        value,
                    },
                );
                Ok(())
            },
        );
        methods.add_method(
            "UpdateUIState",
            |_, this, (index, value): (u8, i8)| {
                push(
                    &this.queue,
                    LuaCommand::UpdateUiState {
                        director_actor_id: this.actor_id,
                        index,
                        value,
                    },
                );
                Ok(())
            },
        );
        methods.add_method(
            "UpdateMarkers",
            |_, this, (index, x, y, z): (u8, f32, f32, f32)| {
                push(
                    &this.queue,
                    LuaCommand::UpdateMarkers {
                        director_actor_id: this.actor_id,
                        index,
                        x,
                        y,
                        z,
                    },
                );
                Ok(())
            },
        );
        methods.add_method("SyncAllInfo", |_, this, _: ()| {
            push(
                &this.queue,
                LuaCommand::SyncAllInfo {
                    director_actor_id: this.actor_id,
                },
            );
            Ok(())
        });
    }
}

// ---------------------------------------------------------------------------
// LuaRetainer — the return of `player:GetSpawnedRetainer()`. Carries a
// snapshot of the [`SpawnedRetainerSnapshot`] so script-level reads
// (`retainer:GetName()`, `retainer:GetItemPackage(...)`) don't need a
// DB hit. `GetItemPackage(code)` returns a [`LuaItemPackage`] bound to
// the retainer's actor id — item events still flow through the same
// `LuaCommand::AddItem` / `RemoveItem` variants, the processor
// decides how to persist based on the owner id.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LuaRetainer {
    pub retainer_id: u32,
    pub actor_class_id: u32,
    pub name: String,
    pub position: (f32, f32, f32),
    pub rotation: f32,
    pub queue: Arc<Mutex<CommandQueue>>,
    /// Actor id of the owning player — threaded through so per-
    /// character mutations (`Rename`, retainer inventory writes) can
    /// key correctly. `0` in contexts where no player is attached
    /// (currently only the unit-test seed in
    /// `lua_retainer_add_item_emits_retainer_command_variant`,
    /// which doesn't exercise the `Rename` path).
    pub player_actor_id: u32,
}

impl UserData for LuaRetainer {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("GetName", |_, this, _: ()| Ok(this.name.clone()));
        methods.add_method("GetRetainerId", |_, this, _: ()| Ok(this.retainer_id));
        methods.add_method("GetActorClassId", |_, this, _: ()| {
            Ok(this.actor_class_id)
        });
        methods.add_method("GetPos", |_, this, _: ()| {
            Ok((
                this.position.0,
                this.position.1,
                this.position.2,
                this.rotation,
            ))
        });
        // Matches the C# `Retainer.GetItemPackage(code)` method —
        // returns a `LuaItemPackage` bound to the retainer's id.
        // Scripts then call `:AddItem(id, qty, quality)` /
        // `:RemoveItemAtSlot(slot, qty)` as with a player package.
        methods.add_method("GetItemPackage", |_, this, pkg_code: u16| {
            Ok(LuaItemPackage {
                // `owner_actor_id` carries the retainer_id so the
                // emitted `AddItemToRetainer` command keys correctly
                // into `characters_retainer_inventory`. The
                // `is_retainer: true` flag is what actually toggles
                // the command variant in `AddItem` below.
                owner_actor_id: this.retainer_id,
                package_code: pkg_code,
                queue: this.queue.clone(),
                // Retainer inventory snapshot threading is a separate
                // follow-up — for now `HasItem` / `GetItemQuantity`
                // on a retainer-owned package answer "no" / 0, which
                // matches the conservative C# default and keeps
                // bazaar-side scripts safe until the live pipeline
                // lands.
                inventory_snapshot: Vec::new(),
                is_retainer: true,
            })
        });
        // `retainer:Rename(newName)` — per-character retainer
        // rename. Emits [`LuaCommand::RenameRetainer`] which the
        // processor drains through `Database::rename_retainer`
        // (writes the `customName` column on `characters_retainers`
        // rather than mutating `server_retainers.name`). Tier 4
        // #14 E.
        methods.add_method("Rename", |_, this, new_name: String| {
            if this.player_actor_id == 0 {
                // No owning player — skip rather than write a
                // garbage row. Hits the fresh-unit-test path only.
                return Ok(());
            }
            push(
                &this.queue,
                LuaCommand::RenameRetainer {
                    player_id: this.player_actor_id,
                    retainer_id: this.retainer_id,
                    new_name,
                },
            );
            Ok(())
        });
        // `retainer:AddBazaarItem(itemId, qty, quality, priceGil)` —
        // list a stack of `itemId` at `priceGil` gil per unit in the
        // retainer's BAZAAR bag. Emits `LuaCommand::AddRetainerBazaarItem`
        // which the processor drains into the `characters_retainer_bazaar`
        // table. Quality defaults to 0 (common) and quantity defaults to
        // 1 — matches the C# `Retainer.AddBazaarItem` convenience shape
        // used by staff bazaar-seed scripts.
        methods.add_method(
            "AddBazaarItem",
            |_,
             this,
             (item_id, qty, quality, price_gil): (
                u32,
                Option<i32>,
                Option<u8>,
                Option<i32>,
            )| {
                push(
                    &this.queue,
                    LuaCommand::AddRetainerBazaarItem {
                        retainer_id: this.retainer_id,
                        item_id,
                        quantity: qty.unwrap_or(1),
                        quality: quality.unwrap_or(0),
                        price_gil: price_gil.unwrap_or(0),
                    },
                );
                Ok(())
            },
        );
    }
}

// ---------------------------------------------------------------------------
// LuaQuestHandle — stub returned by `player:GetQuest(id)`. Scripts chain
// `:ClearQuestData()` / `:ClearQuestFlags()` / `:SetQuestFlag(...)` on
// the return. All no-ops for now; the quest journal lives on the Rust
// side and is mutated via LuaCommand variants (AddQuest/AbandonQuest).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
/// `player:GetItemPackage(code)` returns one of these. The C# side is an
/// `ItemPackage` wrapping a live slot array; here we only need to capture
/// `AddItem`/`AddItems` calls — the wider inventory surface (enumeration,
/// stacking, bazaar) is deferred to the full phase-8 port. Returning a
/// real userdata (not `nil`) keeps `player.lua:onLogin`'s
/// `GetItemPackage(0):AddItems({...})` chain from erroring out the entire
/// hook the first time it's invoked for a fresh character.
pub struct LuaItemPackage {
    pub owner_actor_id: u32,
    pub package_code: u16,
    pub queue: Arc<Mutex<CommandQueue>>,
    /// `(item_id, total_quantity)` snapshot the script can query via
    /// `HasItem` / `GetItemQuantity` without going back through Lua.
    /// Cloned from the owning `LuaPlayer.snapshot.inventory` at
    /// `GetItemPackage(...)` time. Empty for retainer-owned packages
    /// — those answer through the live Retainer struct via a future
    /// retainer-inventory plumbing pass; for now the bag answers
    /// `HasItem == false` (matches the conservative C# default of
    /// "missing item").
    pub inventory_snapshot: Vec<(u32, i32)>,
    /// `true` when the owning handle is a [`LuaRetainer`], `false`
    /// when it's a [`LuaPlayer`]. Routes `AddItem` through
    /// [`LuaCommand::AddItemToRetainer`] (Tier 4 #14 C) instead of
    /// `LuaCommand::AddItem` so retainer inventory lands in the
    /// dedicated `characters_retainer_inventory` table rather than
    /// colliding into `characters_inventory` under the retainer id.
    pub is_retainer: bool,
}

impl UserData for LuaItemPackage {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("AddItem", |_, this, args: mlua::MultiValue| {
            // `AddItem(catalogId)` or `AddItem(catalogId, qty)`.
            let mut iter = args.into_iter();
            let Some(Value::Integer(catalog)) = iter.next() else {
                return Ok(());
            };
            let qty = match iter.next() {
                Some(Value::Integer(q)) => q as i32,
                _ => 1,
            };
            // Retainer-owned packages emit the dedicated retainer
            // variant so the DB write lands in
            // `characters_retainer_inventory` keyed by
            // `(retainerId, serverItemId)`, not conflated into the
            // player-scoped `characters_inventory` table.
            let cmd = if this.is_retainer {
                LuaCommand::AddItemToRetainer {
                    retainer_id: this.owner_actor_id,
                    item_package: this.package_code,
                    item_id: catalog as u32,
                    quantity: qty,
                }
            } else {
                LuaCommand::AddItem {
                    actor_id: this.owner_actor_id,
                    item_package: this.package_code,
                    item_id: catalog as u32,
                    quantity: qty,
                }
            };
            push(&this.queue, cmd);
            Ok(())
        });
        methods.add_method("AddItems", |_, this, items: mlua::Table| {
            // `AddItems({id1, id2, …})`. C# auto-infers qty=1 for each
            // catalog id; we follow suit. Tables with explicit {id, qty}
            // pairs aren't used by `player.lua` so we don't support them
            // yet.
            for pair in items.pairs::<mlua::Value, mlua::Value>() {
                let Ok((_, v)) = pair else { continue };
                if let Value::Integer(catalog) = v {
                    push(
                        &this.queue,
                        LuaCommand::AddItem {
                            actor_id: this.owner_actor_id,
                            item_package: this.package_code,
                            item_id: catalog as u32,
                            quantity: 1,
                        },
                    );
                }
            }
            Ok(())
        });
        methods.add_method("RemoveItem", |_, this, catalog: u32| {
            push(
                &this.queue,
                LuaCommand::RemoveItem {
                    actor_id: this.owner_actor_id,
                    item_package: this.package_code,
                    server_item_id: catalog as u64,
                },
            );
            Ok(())
        });
        // `package:HasItem(catalogId, [minQty])` — answer locally from
        // the inventory snapshot the owning `LuaPlayer` cloned in at
        // `GetItemPackage(...)` time. Mirrors C#
        // `ItemPackage.HasItem(itemId, quantity)` (defaults `quantity=1`).
        // Used by `gcseals.lua::AddGCSeals` (cap-check) and
        // `PopulaceCompanyOfficer.lua` (promotion-cost check).
        methods.add_method(
            "HasItem",
            |_, this, (catalog_id, min_quantity): (u32, Option<i32>)| {
                let min = min_quantity.unwrap_or(1);
                Ok(this
                    .inventory_snapshot
                    .iter()
                    .any(|(id, q)| *id == catalog_id && *q >= min))
            },
        );
        // `package:GetItemQuantity(catalogId)` — total quantity of
        // `catalogId` in the snapshot, summed across stacks. Returns
        // `0` when absent (rather than the C# `-1` sentinel) since
        // every script call site (`gcseals.lua::GetGCSeals`, the
        // `PopulaceCompanyShop` cost-vs-balance branch) treats the
        // missing-item case as a zero balance and the explicit `-1`
        // would force every caller to special-case it.
        methods.add_method("GetItemQuantity", |_, this, catalog_id: u32| {
            let total: i32 = this
                .inventory_snapshot
                .iter()
                .filter(|(id, _)| *id == catalog_id)
                .map(|(_, q)| *q)
                .sum();
            Ok(total)
        });
    }
}

/// `player:GetQuest(id)` return value. Carries a snapshot of the live
/// quest's flags/counters/sequence taken when the userdata was created,
/// so getters like `GetQuestFlag(bit)` return a useful value without
/// needing a round-trip through the command queue. Mutations enqueue
/// `LuaCommand::Quest*` variants the processor applies after the script
/// returns — the Rust-side `Quest` is the source of truth.
pub struct LuaQuestHandle {
    pub player_id: u32,
    pub quest_id: u32,
    pub has_quest: bool,
    /// Mirror of `Quest.sequence` at the time the handle was built.
    pub sequence: u32,
    /// Mirror of `QuestData.flags`.
    pub flags: u32,
    /// Mirror of `QuestData.counters`.
    pub counters: [u16; 3],
    pub queue: Arc<Mutex<CommandQueue>>,
}

impl UserData for LuaQuestHandle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("GetQuestId", |_, this, _: ()| Ok(this.quest_id));
        methods.add_method("HasQuest", |_, this, _: ()| Ok(this.has_quest));
        methods.add_method("GetSequence", |_, this, _: ()| Ok(this.sequence));
        // Lowercase alias — MoonSharp (Meteor) is case-insensitive; scripts
        // ported from the quest_system branch mix `quest:getSequence()`
        // and `quest:GetSequence()`. mlua is case-sensitive, so we add
        // a direct lowercase alias rather than force every script to
        // normalise.
        methods.add_method("getSequence", |_, this, _: ()| Ok(this.sequence));

        // `quest:OnNotice(player)` — `AfterQuestWarpDirector` (and
        // any other director that resumes a quest mid-flow) calls this
        // out of `onEventStarted` to hand control back to the quest's
        // scripted `onNotice(player, quest, target)` hook. Mirrors C#
        // `Quest.OnNotice(Player, string)`. We emit a
        // `QuestOnNotice` command and let `apply_quest_on_notice`
        // perform the cross-script dispatch — same spawn_blocking +
        // `apply_runtime_lua_commands` drain path that `onKillBNpc`
        // uses in `runtime/quest_hook.rs`.
        methods.add_method("OnNotice", |_, this, _player: mlua::AnyUserData| {
            push(
                &this.queue,
                LuaCommand::QuestOnNotice {
                    player_id: this.player_id,
                    quest_id: this.quest_id,
                },
            );
            Ok(())
        });

        // --- Mutations queued as LuaCommand::Quest* --------------------
        methods.add_method("ClearQuestData", |_, this, _: ()| {
            push(
                &this.queue,
                LuaCommand::QuestClearData {
                    player_id: this.player_id,
                    quest_id: this.quest_id,
                },
            );
            Ok(())
        });
        methods.add_method("ClearQuestFlags", |_, this, _: ()| {
            push(
                &this.queue,
                LuaCommand::QuestClearFlags {
                    player_id: this.player_id,
                    quest_id: this.quest_id,
                },
            );
            Ok(())
        });
        methods.add_method("SetQuestFlag", |_, this, args: mlua::MultiValue| {
            // `SetQuestFlag(bit)` — the C# 2-arg form `SetQuestFlag(bit, value)`
            // treats `value=false` as a clear; scripts overwhelmingly use the
            // single-arg set form. Accept both for parity.
            let mut iter = args.into_iter();
            let Some(Value::Integer(bit)) = iter.next() else {
                return Ok(());
            };
            let set = match iter.next() {
                Some(Value::Boolean(b)) => b,
                Some(Value::Nil) | None => true,
                _ => true,
            };
            let bit = bit as u8;
            let cmd = if set {
                LuaCommand::QuestSetFlag {
                    player_id: this.player_id,
                    quest_id: this.quest_id,
                    bit,
                }
            } else {
                LuaCommand::QuestClearFlag {
                    player_id: this.player_id,
                    quest_id: this.quest_id,
                    bit,
                }
            };
            push(&this.queue, cmd);
            Ok(())
        });
        methods.add_method("GetQuestFlag", |_, this, bit: Option<u32>| {
            let bit = bit.unwrap_or(0) as u8;
            if bit >= 32 {
                return Ok(false);
            }
            Ok((this.flags & (1u32 << bit)) != 0)
        });
        methods.add_method(
            "SetQuestScenarioCounter",
            |_, this, args: mlua::MultiValue| {
                // `SetQuestScenarioCounter(slot, value)`. Meteor exposes
                // `SetCounter(num, value)` on QuestData; Lua scripts call
                // this form on the Quest handle too.
                let mut iter = args.into_iter();
                let Some(Value::Integer(idx)) = iter.next() else {
                    return Ok(());
                };
                let Some(Value::Integer(value)) = iter.next() else {
                    return Ok(());
                };
                push(
                    &this.queue,
                    LuaCommand::QuestSetCounter {
                        player_id: this.player_id,
                        quest_id: this.quest_id,
                        idx: idx as u8,
                        value: (value.max(0).min(u16::MAX as i64)) as u16,
                    },
                );
                Ok(())
            },
        );
        methods.add_method("StartSequence", |_, this, sequence: u32| {
            push(
                &this.queue,
                LuaCommand::QuestStartSequence {
                    player_id: this.player_id,
                    quest_id: this.quest_id,
                    sequence,
                },
            );
            Ok(())
        });

        // --- GetData() → LuaQuestDataHandle -----------------------------
        methods.add_method("GetData", |lua, this, _: ()| {
            let handle = LuaQuestDataHandle {
                player_id: this.player_id,
                quest_id: this.quest_id,
                flags: this.flags,
                counters: this.counters,
                queue: this.queue.clone(),
            };
            lua.create_userdata(handle)
        });

        // --- UpdateENPCs / SetENpc wire through to packet emit ---------
        methods.add_method("UpdateENPCs", |_, this, _: ()| {
            push(
                &this.queue,
                LuaCommand::QuestUpdateEnpcs {
                    player_id: this.player_id,
                    quest_id: this.quest_id,
                },
            );
            Ok(())
        });
        methods.add_method("SetENpc", |_, this, args: mlua::MultiValue| {
            // Meteor: `quest:SetENpc(classId, flagType=0, isTalkEnabled=true,
            // isPushEnabled=false, isEmoteEnabled=false, isSpawned=false)`.
            let mut iter = args.into_iter();
            let class_id = match iter.next() {
                Some(Value::Integer(i)) if i >= 0 => i as u32,
                _ => return Ok(()),
            };
            let flag_type = match iter.next() {
                Some(Value::Integer(i)) => (i.max(0).min(255)) as u8,
                _ => 0,
            };
            fn bool_or(v: Option<Value>, default: bool) -> bool {
                match v {
                    Some(Value::Boolean(b)) => b,
                    Some(Value::Nil) | None => default,
                    // Meteor scripts occasionally pass `1`/`0` instead of
                    // `true`/`false`; preserve that ergonomics.
                    Some(Value::Integer(i)) => i != 0,
                    _ => default,
                }
            }
            let is_talk_enabled = bool_or(iter.next(), true);
            let is_push_enabled = bool_or(iter.next(), false);
            let is_emote_enabled = bool_or(iter.next(), false);
            let is_spawned = bool_or(iter.next(), false);
            push(
                &this.queue,
                LuaCommand::QuestSetEnpc {
                    player_id: this.player_id,
                    quest_id: this.quest_id,
                    actor_class_id: class_id,
                    quest_flag_type: flag_type,
                    is_talk_enabled,
                    is_push_enabled,
                    is_emote_enabled,
                    is_spawned,
                },
            );
            Ok(())
        });
        methods.add_method("GetENpc", |_, _this, _: u32| Ok(Value::Nil));
        methods.add_method("HasENpc", |_, _this, _: u32| Ok(false));
    }
}

/// `quest:GetData()` return value — Meteor's `QuestData`. Exposes flag
/// and counter ops; `SetTime` / NPC-LS fields aren't persisted by the
/// current schema so their setters are stubbed for now.
pub struct LuaQuestDataHandle {
    pub player_id: u32,
    pub quest_id: u32,
    pub flags: u32,
    pub counters: [u16; 3],
    pub queue: Arc<Mutex<CommandQueue>>,
}

impl UserData for LuaQuestDataHandle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // --- Read-side getters (served from the snapshot) --------------
        methods.add_method("GetFlags", |_, this, _: ()| Ok(this.flags));
        methods.add_method("GetFlag", |_, this, bit: u32| {
            let bit = bit as u8;
            if bit >= 32 {
                return Ok(false);
            }
            Ok((this.flags & (1u32 << bit)) != 0)
        });
        methods.add_method("GetCounter", |_, this, idx: u32| {
            let idx = idx as usize;
            Ok(if idx < this.counters.len() {
                this.counters[idx]
            } else {
                0
            })
        });

        // --- Mutations queued through the processor ---------------------
        methods.add_method("SetFlag", |_, this, bit: u32| {
            push(
                &this.queue,
                LuaCommand::QuestSetFlag {
                    player_id: this.player_id,
                    quest_id: this.quest_id,
                    bit: bit as u8,
                },
            );
            Ok(())
        });
        methods.add_method("ClearFlag", |_, this, bit: u32| {
            push(
                &this.queue,
                LuaCommand::QuestClearFlag {
                    player_id: this.player_id,
                    quest_id: this.quest_id,
                    bit: bit as u8,
                },
            );
            Ok(())
        });
        methods.add_method(
            "SetCounter",
            |_, this, (idx, value): (u32, u32)| {
                push(
                    &this.queue,
                    LuaCommand::QuestSetCounter {
                        player_id: this.player_id,
                        quest_id: this.quest_id,
                        idx: idx as u8,
                        value: value.min(u16::MAX as u32) as u16,
                    },
                );
                Ok(())
            },
        );
        methods.add_method("IncCounter", |_, this, idx: u32| {
            push(
                &this.queue,
                LuaCommand::QuestIncCounter {
                    player_id: this.player_id,
                    quest_id: this.quest_id,
                    idx: idx as u8,
                },
            );
            // Meteor's `IncCounter` returns the post-inc value; without a
            // live read of the mutated counter we echo the snapshot+1 so
            // scripts comparing against the return value see a reasonable
            // number. The processor applies the real wrapping increment.
            let idx_u = idx as usize;
            if idx_u < this.counters.len() {
                Ok(this.counters[idx_u].wrapping_add(1))
            } else {
                Ok(0u16)
            }
        });
        methods.add_method("DecCounter", |_, this, idx: u32| {
            push(
                &this.queue,
                LuaCommand::QuestDecCounter {
                    player_id: this.player_id,
                    quest_id: this.quest_id,
                    idx: idx as u8,
                },
            );
            let idx_u = idx as usize;
            if idx_u < this.counters.len() {
                Ok(this.counters[idx_u].wrapping_sub(1))
            } else {
                Ok(0u16)
            }
        });
        methods.add_method("ClearData", |_, this, _: ()| {
            push(
                &this.queue,
                LuaCommand::QuestClearData {
                    player_id: this.player_id,
                    quest_id: this.quest_id,
                },
            );
            Ok(())
        });

        // --- Time / NpcLs — not persisted by the new schema yet ---------
        methods.add_method("SetTimeNow", |_, _this, _: ()| Ok(()));
        methods.add_method("GetTime", |_, _this, _: ()| Ok(0u32));
        methods.add_method("SetNpcLsFrom", |_, _this, _: u32| Ok(()));
        methods.add_method("IncrementNpcLsMsgStep", |_, _this, _: ()| Ok(()));
        methods.add_method("GetNpcLsFrom", |_, _this, _: ()| Ok(0u32));
        methods.add_method("GetMsgStep", |_, _this, _: ()| Ok(0u8));
        methods.add_method("ClearNpcLs", |_, _this, _: ()| Ok(()));
    }
}

// ---------------------------------------------------------------------------
// LuaRecipe / LuaRecipeResolver — crafting catalog bindings
//
// Meteor's `CraftCommand.lua` reads every Recipe field via *dot* syntax
// (`chosenRecipe.resultItemID`, not `chosenRecipe:GetResultItemID()`), so
// we expose them as userdata fields via `add_field_method_get` rather
// than methods. The same applies to `recipeResolver.GetRecipeFromMats(...)`
// — dot-call with no `self`, so we register those as `add_function` on
// the resolver userdata (the closure captures a clone of the Arc to find
// itself).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LuaRecipe {
    pub inner: Recipe,
}

impl UserData for LuaRecipe {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        // Dot-accessed fields — matches the Lua call sites
        // `chosenRecipe.resultItemID` etc. Names kept in the same camelCase
        // Meteor's C# DynValue exposes.
        fields.add_field_method_get("id", |_, this| Ok(this.inner.id));
        fields.add_field_method_get("resultItemID", |_, this| Ok(this.inner.result_item_id));
        fields.add_field_method_get("resultQuantity", |_, this| Ok(this.inner.result_quantity));
        fields.add_field_method_get("crystalId1", |_, this| Ok(this.inner.crystal_id_1));
        fields.add_field_method_get("crystalQuantity1", |_, this| {
            Ok(this.inner.crystal_quantity_1)
        });
        fields.add_field_method_get("crystalId2", |_, this| Ok(this.inner.crystal_id_2));
        fields.add_field_method_get("crystalQuantity2", |_, this| {
            Ok(this.inner.crystal_quantity_2)
        });
        fields.add_field_method_get("tier", |_, this| Ok(this.inner.tier));
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // The materials array is the only multi-value field; exposing it
        // as a method that returns a 1-indexed table avoids shipping a
        // whole Lua-side sequence-userdata just for one field.
        methods.add_method("GetMaterials", |lua, this, _: ()| {
            let tbl = lua.create_table()?;
            for (i, m) in this.inner.materials.iter().enumerate() {
                tbl.raw_set(i as i64 + 1, *m)?;
            }
            Ok(tbl)
        });
    }
}

#[derive(Clone)]
pub struct LuaRecipeResolver {
    pub resolver: Arc<RecipeResolver>,
}

impl UserData for LuaRecipeResolver {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // Colon-callable variants (kept alongside dot-callable ones for
        // scripts that mix the two idioms; the C# MoonSharp bindings
        // supported both transparently).
        methods.add_method("GetRecipeByID", |_, this, id: u32| {
            Ok(this
                .resolver
                .by_id(id)
                .map(|r| LuaRecipe { inner: r.clone() }))
        });
        methods.add_method("GetRecipeByItemID", |_, this, id: u32| {
            Ok(this
                .resolver
                .by_item_id(id)
                .map(|r| LuaRecipe { inner: r.clone() }))
        });
        methods.add_method("GetNumRecipes", |_, this, _: ()| {
            Ok(this.resolver.num_recipes() as u32)
        });

        // Dot-callable variants — Meteor's Lua calls these as
        // `recipeResolver.GetRecipeFromMats(...)` without a self, so we
        // register as `add_function` which doesn't bind `self`. The first
        // arg of the closure is the invoking userdata — Lua still passes
        // it through the dot-indexed metamethod lookup; we pull the Arc
        // out of it so callers don't need to supply the resolver again.
        methods.add_function(
            "GetRecipeFromMats",
            |lua,
             args: (
                AnyUserData,
                Option<u32>,
                Option<u32>,
                Option<u32>,
                Option<u32>,
                Option<u32>,
                Option<u32>,
                Option<u32>,
                Option<u32>,
            )| {
                let this: LuaRecipeResolver = args.0.borrow::<LuaRecipeResolver>()?.clone();
                let mats: [u32; 8] = [
                    args.1.unwrap_or(0),
                    args.2.unwrap_or(0),
                    args.3.unwrap_or(0),
                    args.4.unwrap_or(0),
                    args.5.unwrap_or(0),
                    args.6.unwrap_or(0),
                    args.7.unwrap_or(0),
                    args.8.unwrap_or(0),
                ];
                let recipes = this.resolver.by_mats(mats);
                if recipes.is_empty() {
                    return Ok(Value::Nil);
                }
                let tbl = lua.create_table()?;
                for (i, r) in recipes.iter().enumerate() {
                    tbl.raw_set(
                        i as i64 + 1,
                        LuaRecipe {
                            inner: (*r).clone(),
                        },
                    )?;
                }
                Ok(Value::Table(tbl))
            },
        );

        methods.add_function(
            "RecipesToItemIdTable",
            |lua, args: (AnyUserData, Option<mlua::Table>)| {
                let tbl = lua.create_table()?;
                if let Some(recipes) = args.1 {
                    for i in 0..8 {
                        let val = recipes.raw_get::<Option<AnyUserData>>(i as i64 + 1)?;
                        let item_id = val
                            .and_then(|u| u.borrow::<LuaRecipe>().ok().map(|r| r.inner.result_item_id))
                            .unwrap_or(0);
                        tbl.raw_set(i as i64 + 1, item_id)?;
                    }
                } else {
                    for i in 0..8 {
                        tbl.raw_set(i as i64 + 1, 0u32)?;
                    }
                }
                Ok(tbl)
            },
        );

        methods.add_function(
            "RecipeToMatIdTable",
            |lua, args: (AnyUserData, Option<AnyUserData>)| {
                let tbl = lua.create_table()?;
                if let Some(ud) = args.1 {
                    if let Ok(recipe) = ud.borrow::<LuaRecipe>() {
                        for (i, m) in recipe.inner.materials.iter().enumerate() {
                            tbl.raw_set(i as i64 + 1, *m)?;
                        }
                        return Ok(tbl);
                    }
                }
                for i in 0..8 {
                    tbl.raw_set(i as i64 + 1, 0u32)?;
                }
                Ok(tbl)
            },
        );
    }
}

// ---------------------------------------------------------------------------
// LuaGatherNode / LuaGatherNodeItem / LuaGatherResolver — gathering catalog
//
// Dot-accessed fields keep Meteor-era field naming (`itemCatalogId`,
// `maxYield`) so `DummyCommand.lua` can address them the same way
// `CraftCommand.lua` addresses `chosenRecipe.resultItemID`. The resolver
// exposes `BuildAimSlots(id)` which does the full Rust-side pivot the
// old hardcoded Lua `BuildHarvestNode` helper was doing — returning an
// 11-entry table with `{empty, itemKey, itemCatalogId, remainder,
// sweetspot, maxYield}` per slot.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LuaGatherNode {
    pub inner: GatherNode,
}

impl UserData for LuaGatherNode {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("id", |_, this| Ok(this.inner.id));
        fields.add_field_method_get("grade", |_, this| Ok(this.inner.grade as u32));
        fields.add_field_method_get("attempts", |_, this| Ok(this.inner.attempts as u32));
        fields.add_field_method_get("numItems", |_, this| Ok(this.inner.num_items() as u32));
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("GetItemKeys", |lua, this, _: ()| {
            let tbl = lua.create_table()?;
            let mut i = 1i64;
            for key in this.inner.active_item_keys() {
                tbl.raw_set(i, key)?;
                i += 1;
            }
            Ok(tbl)
        });
    }
}

#[derive(Debug, Clone)]
pub struct LuaGatherNodeItem {
    pub inner: GatherNodeItem,
}

impl UserData for LuaGatherNodeItem {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("id", |_, this| Ok(this.inner.id));
        fields.add_field_method_get("itemCatalogId", |_, this| Ok(this.inner.item_catalog_id));
        fields.add_field_method_get("remainder", |_, this| Ok(this.inner.remainder as u32));
        fields.add_field_method_get("aim", |_, this| Ok(this.inner.aim as u32));
        fields.add_field_method_get("sweetspot", |_, this| Ok(this.inner.sweetspot as u32));
        fields.add_field_method_get("maxYield", |_, this| Ok(this.inner.max_yield));
    }
}

#[derive(Clone)]
pub struct LuaGatherResolver {
    pub resolver: Arc<GatherResolver>,
}

impl UserData for LuaGatherResolver {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("GetNode", |_, this, id: u32| {
            Ok(this
                .resolver
                .get_node(id)
                .cloned()
                .map(|n| LuaGatherNode { inner: n }))
        });
        methods.add_method("GetNodeItem", |_, this, id: u32| {
            Ok(this
                .resolver
                .get_item(id)
                .cloned()
                .map(|i| LuaGatherNodeItem { inner: i }))
        });
        methods.add_method("GetNumNodes", |_, this, _: ()| {
            Ok(this.resolver.num_nodes() as u32)
        });
        methods.add_method("GetNumItems", |_, this, _: ()| {
            Ok(this.resolver.num_items() as u32)
        });

        // Build the 11-slot aim pivot table. Returns an array-style
        // Lua table whose keys are 1..=11 and whose values are
        // `{itemKey, itemCatalogId, remainder, sweetspot, maxYield}`
        // sub-tables — matches the shape the old `BuildHarvestNode`
        // helper in `DummyCommand.lua` was building. Empty slots
        // render as `{0, 0, 0, 0, 0}` so index-based access in Lua
        // doesn't need a nil guard.
        methods.add_method("BuildAimSlots", |lua, this, node_id: u32| {
            let Some(slots) = this.resolver.build_aim_slots(node_id) else {
                return Ok(Value::Nil);
            };
            let tbl = lua.create_table()?;
            for (i, slot) in slots.iter().enumerate() {
                let row = lua.create_table()?;
                if slot.empty {
                    row.raw_set("empty", true)?;
                    row.raw_set("itemKey", 0u32)?;
                    row.raw_set("itemCatalogId", 0u32)?;
                    row.raw_set("remainder", 0u32)?;
                    row.raw_set("sweetspot", 0u32)?;
                    row.raw_set("maxYield", 0u32)?;
                    // Also mirror the old Lua shape `{0, 0, 0, 0}` so
                    // positional indexing (`slot[1]`) works alongside
                    // named access.
                    row.raw_set(1i64, 0u32)?;
                    row.raw_set(2i64, 0u32)?;
                    row.raw_set(3i64, 0u32)?;
                    row.raw_set(4i64, 0u32)?;
                } else {
                    row.raw_set("empty", false)?;
                    row.raw_set("itemKey", slot.item_key)?;
                    row.raw_set("itemCatalogId", slot.item_catalog_id)?;
                    row.raw_set("remainder", slot.remainder as u32)?;
                    row.raw_set("sweetspot", slot.sweetspot as u32)?;
                    row.raw_set("maxYield", slot.max_yield)?;
                    // Legacy `{itemId, remainder, sweetspot, yield}`
                    // positional shape for scripts that still use
                    // `nodeTable[i][1]` etc.
                    row.raw_set(1i64, slot.item_catalog_id)?;
                    row.raw_set(2i64, slot.remainder as u32)?;
                    row.raw_set(3i64, slot.sweetspot as u32)?;
                    row.raw_set(4i64, slot.max_yield)?;
                }
                tbl.raw_set(i as i64 + 1, row)?;
            }
            Ok(Value::Table(tbl))
        });
    }
}


// ---------------------------------------------------------------------------
// LuaRegionalLeve / LuaRegionalLeveResolver — fieldcraft + battlecraft
// leve catalog (Tier 3 #13 Lua bindings).
//
// Dot-accessed fields keep the Rust-side snake_case mapped to
// camelCase (`leveType`, `recommendedClass`, `recommendedLevel`) so
// scripts read the same names a ported Meteor script would expect
// when we eventually land the levemete-side NPC flow.
//
// The per-band arrays (`objectiveTargetId`, `objectiveQuantity`,
// `recommendedLevel`, `rewardItemId`, `rewardQuantity`, `rewardGil`)
// are surfaced through per-band getter methods (`:GetObjectiveTargetId(band)`)
// rather than auto-flattened into tables — leves read one band at a
// time based on the quest's active `currentDifficulty`, so the
// getter shape is the common case and sidesteps building four
// tables that scripts would immediately index into anyway.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LuaRegionalLeve {
    pub inner: RegionalLeveData,
}

impl UserData for LuaRegionalLeve {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("id", |_, this| Ok(this.inner.id));
        fields.add_field_method_get("leveType", |_, this| {
            Ok(this.inner.leve_type as u8 as u32)
        });
        fields.add_field_method_get("isFieldcraft", |_, this| {
            Ok(this.inner.leve_type == LeveType::Fieldcraft)
        });
        fields.add_field_method_get("isBattlecraft", |_, this| {
            Ok(this.inner.leve_type == LeveType::Battlecraft)
        });
        fields.add_field_method_get("plateId", |_, this| Ok(this.inner.plate_id));
        fields.add_field_method_get("borderId", |_, this| Ok(this.inner.border_id));
        fields.add_field_method_get("recommendedClass", |_, this| {
            Ok(this.inner.recommended_class)
        });
        fields.add_field_method_get("issuingLocation", |_, this| {
            Ok(this.inner.issuing_location)
        });
        fields.add_field_method_get("leveLocation", |_, this| {
            Ok(this.inner.leve_location)
        });
        fields.add_field_method_get("deliveryDisplayName", |_, this| {
            Ok(this.inner.delivery_display_name)
        });
        fields.add_field_method_get("region", |_, this| Ok(this.inner.region));
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // Per-band getters. `band` is 0..=3; out-of-range values
        // saturate to 3 to match `RegionalLeveData::clamp_difficulty`
        // so scripts that pass a raw player-picked band index
        // can't panic.
        methods.add_method("GetObjectiveTargetId", |_, this, band: i32| {
            Ok(this.inner.objective_target_id
                [RegionalLeveData::clamp_difficulty(band)])
        });
        methods.add_method("GetObjectiveQuantity", |_, this, band: i32| {
            Ok(this.inner.objective_quantity
                [RegionalLeveData::clamp_difficulty(band)])
        });
        methods.add_method("GetRecommendedLevel", |_, this, band: i32| {
            Ok(this.inner.recommended_level
                [RegionalLeveData::clamp_difficulty(band)])
        });
        methods.add_method("GetRewardItemId", |_, this, band: i32| {
            Ok(this.inner.reward_item_id
                [RegionalLeveData::clamp_difficulty(band)])
        });
        methods.add_method("GetRewardQuantity", |_, this, band: i32| {
            Ok(this.inner.reward_quantity
                [RegionalLeveData::clamp_difficulty(band)])
        });
        methods.add_method("GetRewardGil", |_, this, band: i32| {
            Ok(this.inner.reward_gil
                [RegionalLeveData::clamp_difficulty(band)])
        });
    }
}

#[derive(Clone)]
pub struct LuaRegionalLeveResolver {
    pub resolver: Arc<RegionalLeveResolver>,
}

impl UserData for LuaRegionalLeveResolver {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("GetLeve", |_, this, id: u32| {
            Ok(this
                .resolver
                .by_id(id)
                .cloned()
                .map(|inner| LuaRegionalLeve { inner }))
        });
        methods.add_method("GetNumLeves", |_, this, _: ()| {
            Ok(this.resolver.num_leves() as u32)
        });
        methods.add_method("GetNumFieldcraft", |_, this, _: ()| {
            Ok(this.resolver.num_fieldcraft() as u32)
        });
        methods.add_method("GetNumBattlecraft", |_, this, _: ()| {
            Ok(this.resolver.num_battlecraft() as u32)
        });

        // Reverse lookups mirroring the Rust-side secondary indexes.
        // Return Lua array tables (1-indexed) for parity with
        // `GetRecipeFromMats` / `RecipesToItemIdTable`.
        methods.add_method("FieldcraftLevesForItem", |lua, this, item_id: u32| {
            let ids = this.resolver.fieldcraft_leves_for_item(item_id);
            let tbl = lua.create_table()?;
            for (i, id) in ids.iter().enumerate() {
                tbl.raw_set(i as i64 + 1, *id)?;
            }
            Ok(tbl)
        });
        methods.add_method(
            "BattlecraftLevesForClass",
            |lua, this, actor_class_id: u32| {
                let ids = this.resolver.battlecraft_leves_for_class(actor_class_id);
                let tbl = lua.create_table()?;
                for (i, id) in ids.iter().enumerate() {
                    tbl.raw_set(i as i64 + 1, *id)?;
                }
                Ok(tbl)
            },
        );
    }
}
