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
            Ok((this.pos.0, this.pos.1, this.pos.2, this.rotation, this.zone_id))
        });
        methods.add_method("ChangeState", |_, this, state: u16| {
            push(&this.queue, LuaCommand::ChangeState { actor_id: this.actor_id, main_state: state });
            Ok(())
        });
        methods.add_method("PlayAnimation", |_, this, animation_id: u32| {
            push(&this.queue, LuaCommand::PlayAnimation { actor_id: this.actor_id, animation_id });
            Ok(())
        });
        methods.add_method(
            "SendMessage",
            |_, this, (message_type, sender, text): (u8, String, String)| {
                push(
                    &this.queue,
                    LuaCommand::SendMessage { actor_id: this.actor_id, message_type, sender, text },
                );
                Ok(())
            },
        );
        methods.add_method("GraphicChange", |_, this, (slot, graphic): (u8, u32)| {
            push(
                &this.queue,
                LuaCommand::GraphicChange { actor_id: this.actor_id, slot, graphic_id: graphic },
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
        methods.add_method("GetUniqueId", |_, this, _: ()| Ok(this.base.unique_id.clone()));
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
            Ok((this.base.pos.0, this.base.pos.1, this.base.pos.2, this.base.rotation, this.base.zone_id))
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
        methods.add_method("GetHighestLevel", |_, this, _: ()| Ok(this.snapshot.current_level));
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
            Ok(LuaPlayer::is_class_range(this.snapshot.current_class, 2..=8))
        });
        methods.add_method("IsDiscipleOfMagic", |_, this, _: ()| {
            Ok(LuaPlayer::is_class_range(this.snapshot.current_class, 22..=23))
        });
        methods.add_method("IsDiscipleOfHand", |_, this, _: ()| {
            Ok(LuaPlayer::is_class_range(this.snapshot.current_class, 29..=36))
        });
        methods.add_method("IsDiscipleOfLand", |_, this, _: ()| {
            Ok(LuaPlayer::is_class_range(this.snapshot.current_class, 39..=41))
        });

        // --- Location / money ------------------------------------------------
        methods.add_method("GetCurrentGil", |_, this, _: ()| Ok(this.snapshot.current_gil));
        methods.add_method("GetInitialTown", |_, this, _: ()| Ok(this.snapshot.initial_town));
        methods.add_method("GetHomePoint", |_, this, _: ()| Ok(this.snapshot.homepoint));
        methods.add_method("GetHomePointInn", |_, this, _: ()| Ok(this.snapshot.homepoint_inn));
        methods.add_method("SetHomePoint", |_, this, homepoint: u32| {
            push(&this.queue, LuaCommand::SetHomePoint { player_id: this.snapshot.actor_id, homepoint });
            Ok(())
        });
        methods.add_method("GetMountState", |_, this, _: ()| Ok(this.snapshot.mount_state));

        // --- Play time -------------------------------------------------------
        methods.add_method("GetPlayTime", |_, this, _do_update: Option<bool>| {
            Ok(this.snapshot.play_time)
        });

        // --- Status flags ----------------------------------------------------
        methods.add_method("IsEngaged", |_, this, _: ()| Ok(this.snapshot.is_engaged));
        methods.add_method("IsTrading", |_, this, _: ()| Ok(this.snapshot.is_trading));
        methods.add_method("IsTradeAccepted", |_, this, _: ()| Ok(this.snapshot.is_trade_accepted));
        methods.add_method("IsPartyLeader", |_, this, _: ()| Ok(this.snapshot.is_party_leader));
        methods.add_method("IsGM", |_, this, _: ()| Ok(this.snapshot.is_gm));

        // --- Identity helpers (aetheryte, traits, items) --------------------
        methods.add_method("HasAetheryteNodeUnlocked", |_, this, id: u32| {
            Ok(this.snapshot.unlocked_aetherytes.contains(&id))
        });
        methods.add_method("HasTrait", |_, this, id: u16| Ok(this.snapshot.traits.contains(&id)));
        methods.add_method("HasItem", |_, this, (catalog_id, min_quantity): (u32, Option<i32>)| {
            let min = min_quantity.unwrap_or(1);
            Ok(this.snapshot.inventory.iter().any(|(id, q)| *id == catalog_id && *q >= min))
        });

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
            push(&this.queue, LuaCommand::AddQuest { player_id: this.snapshot.actor_id, quest_id: id });
            Ok(())
        });
        methods.add_method("CompleteQuest", |_, this, id: u32| {
            push(&this.queue, LuaCommand::CompleteQuest { player_id: this.snapshot.actor_id, quest_id: id });
            Ok(())
        });
        methods.add_method("AbandonQuest", |_, this, id: u32| {
            push(&this.queue, LuaCommand::AbandonQuest { player_id: this.snapshot.actor_id, quest_id: id });
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
            |_, this, (_actor, trigger, _args): (Value, String, mlua::MultiValue)| {
                push(
                    &this.queue,
                    LuaCommand::KickEvent {
                        player_id: this.snapshot.actor_id,
                        actor_id: this.snapshot.current_event_owner,
                        trigger,
                        args: Vec::new(),
                    },
                );
                Ok(())
            },
        );

        // --- Economy / progression ------------------------------------------
        methods.add_method("AddExp", |_, this, (class_id, exp): (u8, i32)| {
            push(
                &this.queue,
                LuaCommand::AddExp { actor_id: this.snapshot.actor_id, class_id, exp },
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

        // --- Movement -------------------------------------------------------
        methods.add_method(
            "ChangeState",
            |_, this, state: u16| {
                push(
                    &this.queue,
                    LuaCommand::ChangeState { actor_id: this.snapshot.actor_id, main_state: state },
                );
                Ok(())
            },
        );
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
        methods.add_method("GetZoneName", |_, this, _: ()| Ok(this.snapshot.zone_name.clone()));
        methods.add_method("GetPlayers", |_, this, _: ()| Ok(this.snapshot.player_ids.clone()));
        methods.add_method("GetMonsters", |_, this, _: ()| Ok(this.snapshot.monster_ids.clone()));
        methods.add_method("GetAllies", |_, _this, _: ()| Ok(Vec::<u32>::new()));
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
                LuaCommand::DespawnActor { zone_id: this.snapshot.zone_id, actor_id },
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
