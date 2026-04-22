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

use mlua::{UserData, UserDataMethods, Value};

use super::command::{CommandQueue, LuaCommand};

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
}

impl From<&crate::actor::Player> for PlayerSnapshot {
    fn from(p: &crate::actor::Player) -> Self {
        let active_quests: Vec<u32> = p
            .helpers
            .quest_journal
            .slots
            .iter()
            .flatten()
            .map(|q| q.quest_id())
            .collect();
        let completed_quests: Vec<u32> =
            p.helpers.quest_journal.completed.iter().copied().collect();
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
            mount_state: p.player.mount_state,
            has_chocobo: p.player.has_chocobo,
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
            unlocked_aetherytes,
            traits,
            inventory,
            login_director_actor_id: p.character.chara.login_director_actor_id,
        }
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
            |_, this, (name, _args): (String, mlua::MultiValue)| {
                // MultiValue args are dropped for now — full parity awaits a
                // Lua-value → LuaCommandArg marshaller. The Rust server loop
                // still receives the event name, which is what the client
                // reacts to in most cases.
                push(
                    &this.queue,
                    LuaCommand::RunEventFunction {
                        player_id: this.snapshot.actor_id,
                        event_name: this.snapshot.current_event_name.clone(),
                        function_name: name,
                        args: Vec::new(),
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
            let pkg = LuaItemPackage {
                owner_actor_id: this.snapshot.actor_id,
                package_code: pkg_code,
                queue: this.queue.clone(),
            };
            lua.create_userdata(pkg)
        });
        methods.add_method("GetQuest", |lua, this, id: u32| {
            // Scripts chain `GetQuest(id):ClearQuestData()` / `:ClearQuestFlags()`
            // (e.g. the tutorial cleanup in battlenpc.lua `onBeginLogin`).
            // Returning an integer or nil here would error on the method
            // call; return a `LuaQuestHandle` userdata so the chain runs.
            // If the player doesn't have the quest, still return a handle
            // — the C# behaviour is similarly lenient (method no-ops on
            // missing quest).
            let handle = LuaQuestHandle {
                player_id: this.snapshot.actor_id,
                quest_id: id,
                has_quest: this.snapshot.active_quests.contains(&id),
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

        // --- Retainer -------------------------------------------------------
        methods.add_method("DespawnMyRetainer", |_, _this, _: ()| Ok(()));

        // --- Session control ------------------------------------------------
        methods.add_method("Disengage", |_, _this, _: ()| Ok(()));
        methods.add_method("Logout", |_, _this, _: ()| Ok(()));
        methods.add_method("QuitGame", |_, _this, _: ()| Ok(()));

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

        // --- Lua-side table field access (player.positionX etc.) ------------
        methods.add_meta_method(mlua::MetaMethod::Index, |_, this, key: String| {
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
        // `(6 << 28) | (zone_actor_id << 19) | director_local_id` (see
        // `Director.cs` ctor base call) and fires a `CreateDirector`
        // command so the host can emit the director's spawn packets in
        // the same pass as the zone-in bundle. Returns a LuaDirectorHandle
        // carrying that actor id so `player:SetLoginDirector(director)`
        // can read it back. For the login director we only ever need
        // one per zone, so `director_local_id` is always 0.
        methods.add_method(
            "CreateDirector",
            |lua, this, (name, _flag): (String, Option<bool>)| {
                let director_local_id: u32 = 0;
                let zone_actor_id = this.snapshot.zone_id;
                let director_actor_id = (6u32 << 28) | (zone_actor_id << 19) | director_local_id;
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

        // The remaining WorldManager methods (DoPlayerMoveInZone,
        // CreateInvitePartyGroup, CreateTradeGroup, AcceptTrade, …) queue
        // log-only stubs so scripts don't abort. Concrete handlers ship in
        // later phases.
        for stub in [
            "DoPlayerMoveInZone",
            "DoZoneChangeContent",
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
        methods.add_method("StartDirector", |_, _this, _: Option<bool>| Ok(()));
        methods.add_method("EndDirector", |_, _this, _: ()| Ok(()));
        methods.add_method("StartSceneSession", |_, _this, _: Option<Value>| Ok(()));
        methods.add_method("EndSceneSession", |_, _this, _: ()| Ok(()));
        methods.add_method("AddMember", |_, _this, _member: Value| Ok(()));
        methods.add_method("RemoveMember", |_, _this, _member: Value| Ok(()));
        methods.add_method("GetContentMembers", |_, _this, _: ()| Ok(Vec::<u32>::new()));
        methods.add_method("SetLeader", |_, _this, _actor: Value| Ok(()));
        methods.add_method("IsInstanceRaid", |_, _this, _: ()| Ok(false));
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
            push(
                &this.queue,
                LuaCommand::AddItem {
                    actor_id: this.owner_actor_id,
                    item_package: this.package_code,
                    item_id: catalog as u32,
                    quantity: qty,
                },
            );
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
    }
}

pub struct LuaQuestHandle {
    pub player_id: u32,
    pub quest_id: u32,
    pub has_quest: bool,
    pub queue: Arc<Mutex<CommandQueue>>,
}

impl UserData for LuaQuestHandle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("GetQuestId", |_, this, _: ()| Ok(this.quest_id));
        methods.add_method("HasQuest", |_, this, _: ()| Ok(this.has_quest));
        methods.add_method("ClearQuestData", |_, _this, _: ()| Ok(()));
        methods.add_method("ClearQuestFlags", |_, _this, _: ()| Ok(()));
        methods.add_method(
            "SetQuestFlag",
            |_, _this, _args: mlua::MultiValue| Ok(()),
        );
        methods.add_method(
            "GetQuestFlag",
            |_, _this, _slot: Option<u32>| Ok(false),
        );
        methods.add_method(
            "SetQuestData",
            |_, _this, _args: mlua::MultiValue| Ok(()),
        );
        methods.add_method(
            "GetQuestData",
            |_, _this, _slot: Option<u32>| Ok(Value::Nil),
        );
        methods.add_method(
            "SetQuestScenarioCounter",
            |_, _this, _counter: Option<u32>| Ok(()),
        );
    }
}
