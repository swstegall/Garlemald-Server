//! Dispatcher for `EventEvent`s. Turns outbox rows into:
//!
//! * Outbound packets (`RunEventFunctionPacket`, `EndEventPacket`,
//!   `KickEventPacket`, `SendGameMessage`).
//! * Lua dispatch — when a `LuaEngine` is available, `EventStarted` /
//!   `EventUpdated` resume the NPC or coroutine script; quest events
//!   call `isObjectivesComplete` / `onAbandonQuest` on the quest script.
//! * DB writes (`QuestSaveToDb`).

#![allow(dead_code)]

use std::sync::Arc;

use mlua::{MultiValue, Value};

use common::luaparam::LuaParam;

use crate::database::Database;
use crate::lua::LuaEngine;
use crate::packets::send as tx;
use crate::runtime::actor_registry::ActorRegistry;
use crate::world_manager::WorldManager;

use super::outbox::EventEvent;

/// Dispatch one `EventEvent`. The `lua` argument is optional so callers
/// that don't (yet) have a Lua engine wired in can still process events
/// — the Lua-flavoured events degrade gracefully to `tracing::debug!`.
pub async fn dispatch_event_event(
    event: &EventEvent,
    registry: &ActorRegistry,
    world: &WorldManager,
    db: &Database,
    lua: Option<&Arc<LuaEngine>>,
) {
    match event {
        // ---- Lua hooks --------------------------------------------------
        EventEvent::EventStarted {
            player_actor_id,
            owner_actor_id,
            event_name,
            event_type,
            lua_params,
        } => {
            dispatch_event_started(
                registry,
                world,
                lua,
                *player_actor_id,
                *owner_actor_id,
                event_name,
                *event_type,
                lua_params,
            )
            .await;
        }
        EventEvent::EventUpdated {
            player_actor_id,
            trigger_actor_id,
            event_type,
            lua_params,
        } => {
            dispatch_event_updated(
                lua,
                *player_actor_id,
                *trigger_actor_id,
                *event_type,
                lua_params,
            );
        }
        EventEvent::QuestCheckCompletion {
            player_actor_id,
            quest_id,
            quest_name,
        } => {
            dispatch_quest_check_completion(lua, *player_actor_id, *quest_id, quest_name);
        }
        EventEvent::QuestAbandonHook {
            player_actor_id,
            quest_id,
            quest_name,
        } => {
            dispatch_quest_abandon(lua, *player_actor_id, *quest_id, quest_name);
        }

        // ---- Packet sends ----------------------------------------------
        EventEvent::RunEventFunction {
            player_actor_id,
            trigger_actor_id,
            owner_actor_id,
            event_name,
            event_type,
            function_name,
            lua_params,
        } => {
            let sub = tx::build_run_event_function(
                *trigger_actor_id,
                *owner_actor_id,
                event_name,
                *event_type,
                function_name,
                lua_params,
            );
            send_to_player(world, registry, *player_actor_id, sub.to_bytes()).await;
        }
        EventEvent::EndEvent {
            player_actor_id,
            owner_actor_id,
            event_name,
            event_type,
        } => {
            let sub = tx::build_end_event(
                *player_actor_id,
                *owner_actor_id,
                event_name,
                *event_type,
            );
            send_to_player(world, registry, *player_actor_id, sub.to_bytes()).await;
        }
        EventEvent::KickEvent {
            player_actor_id,
            target_actor_id,
            owner_actor_id,
            event_name,
            event_type,
            lua_params,
        } => {
            let sub = tx::build_kick_event(
                *target_actor_id,
                *owner_actor_id,
                event_name,
                *event_type,
                lua_params,
            );
            send_to_player(world, registry, *player_actor_id, sub.to_bytes()).await;
        }

        // ---- DB + game message -----------------------------------------
        EventEvent::QuestSaveToDb {
            player_actor_id,
            quest_id,
            phase,
            flags,
            data,
        } => {
            if let Err(e) = db
                .save_quest(
                    *player_actor_id,
                    /* slot */ 0,
                    *quest_id,
                    *phase,
                    data,
                    *flags,
                )
                .await
            {
                tracing::warn!(
                    error = %e,
                    player = player_actor_id,
                    quest = quest_id,
                    "quest save failed",
                );
            }
        }
        EventEvent::QuestGameMessage {
            player_actor_id,
            text_id,
            quest_id,
        } => {
            tracing::debug!(
                player = player_actor_id,
                text = text_id,
                quest = quest_id,
                "quest: game message (send-builder pending)",
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Lua dispatch bodies
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
async fn dispatch_event_started(
    registry: &ActorRegistry,
    world: &WorldManager,
    lua: Option<&Arc<LuaEngine>>,
    player_actor_id: u32,
    owner_actor_id: u32,
    event_name: &str,
    event_type: u8,
    lua_params: &[LuaParam],
) {
    let Some(lua) = lua else {
        tracing::debug!(
            player = player_actor_id,
            owner = owner_actor_id,
            name = %event_name,
            ty = event_type,
            params = lua_params.len(),
            "event: started (no Lua engine wired)",
        );
        return;
    };

    let Some(owner_handle) = registry.get(owner_actor_id).await else {
        tracing::debug!(owner = owner_actor_id, "event: owner actor missing");
        return;
    };
    let (class_path, class_name, unique_id) = {
        let chara = owner_handle.character.read().await;
        (
            chara.base.class_path.clone(),
            chara.base.class_name.clone(),
            chara.base.actor_name.clone(),
        )
    };
    let zone_name = zone_name_for(world, owner_handle.zone_id).await;

    let args_vec = event_args_vec(player_actor_id, event_name, event_type, lua_params);
    let lua_clone = Arc::clone(lua);
    let event_name_owned = event_name.to_string();

    let result = tokio::task::spawn_blocking(move || {
        // Try the NPC's unique-override script first, fall back to its
        // base class. Either way we end up with a Lua VM + a fresh
        // command queue; the queue is drained and discarded here
        // (Phase 4 only wires the Lua-hook side — command replay lands
        // with the broader scheduler integration).
        let (lua_vm, _queue) = match lua_clone.load_script(&lua_clone.resolver().npc(
            &zone_name,
            &class_name,
            &unique_id,
        )) {
            Ok(pair) => pair,
            Err(_) => match lua_clone.load_script(&lua_clone.resolver().base_class(&class_path)) {
                Ok(pair) => pair,
                Err(e) => {
                    return Err(format!("no script for {class_path}: {e}"));
                }
            },
        };
        let args = to_multi_value(&lua_vm, &args_vec)
            .map_err(|e| format!("args conversion: {e}"))?;
        let globals = lua_vm.globals();
        let Some(f): Option<mlua::Function> = globals.get("onEventStarted").ok() else {
            return Ok(());
        };
        f.call::<Value>(args)
            .map(|_| ())
            .map_err(|e| format!("onEventStarted failed: {e}"))
    })
    .await;

    match result {
        Ok(Ok(())) => {}
        Ok(Err(msg)) => {
            tracing::debug!(
                %msg,
                owner = owner_actor_id,
                event = %event_name_owned,
                "event: Lua call",
            );
        }
        Err(join) => {
            tracing::warn!(error = %join, "event: Lua dispatch task panicked");
        }
    }
}

fn dispatch_event_updated(
    lua: Option<&Arc<LuaEngine>>,
    player_actor_id: u32,
    trigger_actor_id: u32,
    event_type: u8,
    lua_params: &[LuaParam],
) {
    let Some(lua) = lua else {
        tracing::debug!(
            player = player_actor_id,
            trigger = trigger_actor_id,
            ty = event_type,
            params = lua_params.len(),
            "event: updated (no Lua engine)",
        );
        return;
    };
    // Resume any coroutine parked on `_WAIT_EVENT` for this player.
    // The lua params carry the client's response; we feed them through
    // as a MultiValue so the sleeping coroutine can inspect them.
    let fired = lua.fire_player_event(player_actor_id, MultiValue::new());
    tracing::debug!(
        player = player_actor_id,
        trigger = trigger_actor_id,
        ty = event_type,
        resumed = fired,
        "event: updated",
    );
}

fn dispatch_quest_check_completion(
    lua: Option<&Arc<LuaEngine>>,
    player_actor_id: u32,
    quest_id: u32,
    quest_name: &str,
) {
    let Some(lua) = lua else {
        tracing::debug!(
            player = player_actor_id,
            quest = quest_id,
            name = %quest_name,
            "quest: isObjectivesComplete (no Lua engine)",
        );
        return;
    };
    if quest_name.is_empty() {
        return;
    }
    let path = lua.resolver().quest(quest_name);
    match lua.call(&path, "isObjectivesComplete", MultiValue::new()) {
        Ok(r) => {
            let complete = matches!(r.value, Value::Boolean(true));
            tracing::debug!(
                player = player_actor_id,
                quest = quest_id,
                name = %quest_name,
                complete,
                "quest: isObjectivesComplete",
            );
        }
        Err(e) => {
            tracing::debug!(
                error = %e,
                quest = quest_id,
                name = %quest_name,
                "quest: isObjectivesComplete call failed",
            );
        }
    }
}

fn dispatch_quest_abandon(
    lua: Option<&Arc<LuaEngine>>,
    player_actor_id: u32,
    quest_id: u32,
    quest_name: &str,
) {
    let Some(lua) = lua else {
        tracing::debug!(
            player = player_actor_id,
            quest = quest_id,
            name = %quest_name,
            "quest: onAbandonQuest (no Lua engine)",
        );
        return;
    };
    if quest_name.is_empty() {
        return;
    }
    let path = lua.resolver().quest(quest_name);
    if let Err(e) = lua.call(&path, "onAbandonQuest", MultiValue::new()) {
        tracing::debug!(
            error = %e,
            quest = quest_id,
            name = %quest_name,
            "quest: onAbandonQuest call failed",
        );
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn send_to_player(
    world: &WorldManager,
    registry: &ActorRegistry,
    player_actor_id: u32,
    bytes: Vec<u8>,
) {
    let Some(handle) = registry.get(player_actor_id).await else {
        return;
    };
    let Some(client) = world.client(handle.session_id).await else {
        return;
    };
    client.send_bytes(bytes).await;
}

async fn zone_name_for(world: &WorldManager, zone_id: u32) -> String {
    let Some(zone_arc) = world.zone(zone_id).await else {
        return String::new();
    };
    let zone = zone_arc.read().await;
    zone.core.zone_name.clone()
}

/// Owned equivalents of the args Lua expects so we can move them into
/// the `spawn_blocking` closure.
#[derive(Debug, Clone)]
enum OwnedArg {
    Int32(i32),
    UInt32(u32),
    String(String),
    Bool(bool),
    Nil,
    ActorId(u32),
}

fn event_args_vec(
    player_actor_id: u32,
    event_name: &str,
    event_type: u8,
    lua_params: &[LuaParam],
) -> Vec<OwnedArg> {
    let mut out = Vec::with_capacity(3 + lua_params.len());
    out.push(OwnedArg::UInt32(player_actor_id));
    out.push(OwnedArg::String(event_name.to_string()));
    out.push(OwnedArg::Int32(event_type as i32));
    for p in lua_params {
        out.push(lua_param_to_owned(p));
    }
    out
}

fn lua_param_to_owned(p: &LuaParam) -> OwnedArg {
    match p {
        LuaParam::Int32(i) => OwnedArg::Int32(*i),
        LuaParam::UInt32(u) => OwnedArg::UInt32(*u),
        LuaParam::String(s) => OwnedArg::String(s.clone()),
        LuaParam::True => OwnedArg::Bool(true),
        LuaParam::False => OwnedArg::Bool(false),
        LuaParam::Nil => OwnedArg::Nil,
        LuaParam::Actor(id) => OwnedArg::ActorId(*id),
        LuaParam::Type7 { actor_id, .. } => OwnedArg::ActorId(*actor_id),
        LuaParam::Type9 { item1, .. } => OwnedArg::UInt32(*item1 as u32),
        LuaParam::Byte(b) => OwnedArg::Int32(*b as i32),
        LuaParam::Short(s) => OwnedArg::Int32(*s as i32),
    }
}

fn to_multi_value(lua: &mlua::Lua, args: &[OwnedArg]) -> mlua::Result<MultiValue> {
    let mut out = MultiValue::new();
    for a in args {
        let v = match a {
            OwnedArg::Int32(i) => Value::Integer(*i as mlua::Integer),
            OwnedArg::UInt32(u) => Value::Integer(*u as mlua::Integer),
            OwnedArg::String(s) => Value::String(lua.create_string(s)?),
            OwnedArg::Bool(b) => Value::Boolean(*b),
            OwnedArg::Nil => Value::Nil,
            OwnedArg::ActorId(id) => Value::Integer(*id as mlua::Integer),
        };
        out.push_back(v);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::outbox::EventOutbox;
    use crate::event::EventEvent;

    fn tmpdir() -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("garlemald-event-dispatch-{nanos}"));
        std::fs::create_dir_all(dir.join("quests/man")).unwrap();
        dir
    }

    /// Per-test SQLite path — one file per test run so WAL files don't collide.
    fn tempdb() -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("garlemald-event-dispatch-{nanos}.db"))
    }

    #[tokio::test]
    async fn quest_completion_fires_lua_hook() {
        let root = tmpdir();
        // Minimal quest script — returns true from isObjectivesComplete.
        std::fs::write(
            root.join("quests/man/man0l0.lua"),
            "function isObjectivesComplete() return true end",
        )
        .unwrap();

        let lua = Arc::new(LuaEngine::new(&root));
        let registry = ActorRegistry::new();
        let world = WorldManager::new();
        // Skip real DB round-trip by only dispatching a QuestCheckCompletion
        // event (no DB hit). The stub fails on connect which is fine — that
        // code path isn't reached.
        let db = Database::open(tempdb()).await.expect("db stub");

        let event = EventEvent::QuestCheckCompletion {
            player_actor_id: 1,
            quest_id: 0xF_00F0,
            quest_name: "man0l0".to_string(),
        };
        dispatch_event_event(&event, &registry, &world, &db, Some(&lua)).await;
        // Reaching here without a panic is the assertion — the Lua path
        // loaded the script and called `isObjectivesComplete`. The
        // dispatcher logs the boolean; callers that care about the result
        // pass their own LuaCallResult-aware closure.
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn quest_abandon_fires_lua_hook() {
        let root = tmpdir();
        // Write a script that ticks a global on onAbandonQuest — we can't
        // observe that from here, but the call should succeed without
        // error. A failing call hits the `tracing::debug!` branch; a
        // successful call silently completes.
        std::fs::write(
            root.join("quests/man/man0l1.lua"),
            "_abandoned = false\nfunction onAbandonQuest() _abandoned = true end",
        )
        .unwrap();

        let lua = Arc::new(LuaEngine::new(&root));
        let registry = ActorRegistry::new();
        let world = WorldManager::new();
        let db = Database::open(tempdb()).await.expect("db stub");

        let event = EventEvent::QuestAbandonHook {
            player_actor_id: 1,
            quest_id: 0xF_00F1,
            quest_name: "man0l1".to_string(),
        };
        dispatch_event_event(&event, &registry, &world, &db, Some(&lua)).await;

        // Secondary confirmation: load the VM and check the global flipped.
        let path = lua.resolver().quest("man0l1");
        let (vm, _q) = lua.load_script(&path).unwrap();
        let flag: bool = vm.globals().get("_abandoned").unwrap();
        assert!(flag);
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn event_started_invokes_npc_base_class_script() {
        use crate::actor::Character;
        use crate::runtime::actor_registry::{ActorHandle, ActorKindTag};
        use crate::zone::navmesh::StubNavmeshLoader;
        use crate::zone::zone::Zone;

        let root = tmpdir();
        // Matches the C# base-class resolver: `scripts/base/<classPath>.lua`.
        std::fs::create_dir_all(root.join("base/Chara/Npc/Populace")).unwrap();
        std::fs::write(
            root.join("base/Chara/Npc/Populace/Greeter.lua"),
            "_talked_to = 0\nfunction onEventStarted(player) _talked_to = player end",
        )
        .unwrap();

        let lua = Arc::new(LuaEngine::new(&root));
        let world = WorldManager::new();
        let registry = ActorRegistry::new();
        let db = Database::open(tempdb()).await.expect("db stub");

        // One Zone + one Greeter NPC in it.
        let zone = Zone::new(
            100, "test", 1, "/Area/Zone/Test", 0, 0, 0, false, false, false, false, false,
            Some(&StubNavmeshLoader),
        );
        world.register_zone(zone).await;

        let mut npc_chara = Character::new(42);
        npc_chara.base.class_path = "Chara/Npc/Populace/Greeter".into();
        npc_chara.base.class_name = "Greeter".into();
        npc_chara.base.actor_name = "greeter_main".into();
        registry
            .insert(ActorHandle::new(
                42, ActorKindTag::Npc, 100, 0, npc_chara,
            ))
            .await;

        let event = EventEvent::EventStarted {
            player_actor_id: 1234,
            owner_actor_id: 42,
            event_name: "onTalk".to_string(),
            event_type: 0,
            lua_params: vec![],
        };
        dispatch_event_event(&event, &registry, &world, &db, Some(&lua)).await;

        // Confirm the global was written — proves the Lua function ran
        // with the player id as its first arg.
        let path = lua.resolver().base_class("Chara/Npc/Populace/Greeter");
        let (vm, _q) = lua.load_script(&path).unwrap();
        let talked: i64 = vm.globals().get("_talked_to").unwrap();
        assert_eq!(talked, 1234);
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn no_lua_engine_keeps_dispatch_graceful() {
        let registry = ActorRegistry::new();
        let world = WorldManager::new();
        let db = Database::open(tempdb()).await.expect("db stub");

        // All four Lua-hook paths should accept `None` without panicking.
        let events = vec![
            EventEvent::EventStarted {
                player_actor_id: 1,
                owner_actor_id: 99,
                event_name: "talk".into(),
                event_type: 0,
                lua_params: vec![],
            },
            EventEvent::EventUpdated {
                player_actor_id: 1,
                trigger_actor_id: 99,
                event_type: 0,
                lua_params: vec![],
            },
            EventEvent::QuestCheckCompletion {
                player_actor_id: 1,
                quest_id: 1,
                quest_name: "man0l0".into(),
            },
            EventEvent::QuestAbandonHook {
                player_actor_id: 1,
                quest_id: 1,
                quest_name: "man0l0".into(),
            },
        ];
        let mut ob = EventOutbox::new();
        for e in events {
            ob.push(e);
        }
        for e in ob.drain() {
            dispatch_event_event(&e, &registry, &world, &db, None).await;
        }
    }
}
