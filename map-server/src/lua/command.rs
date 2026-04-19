//! Commands emitted by Lua userdata methods.
//!
//! Lua scripts run synchronously inside a blocking context; they must not
//! touch async locks. Instead of mutating game state in-place, our userdata
//! types append to a shared `CommandQueue`, and the Map Server's game loop
//! drains it after the script returns.
//!
//! This is the same pattern that avoided deadlocks in the C# port — the
//! difference is just that our queue is typed.

use std::sync::{Arc, Mutex};

/// Every Lua-initiated side effect maps to one of these.
#[derive(Debug, Clone)]
pub enum LuaCommand {
    SendMessage {
        actor_id: u32,
        message_type: u8,
        sender: String,
        text: String,
    },
    EndEvent {
        player_id: u32,
        event_owner: u32,
        event_name: String,
    },
    ChangeState {
        actor_id: u32,
        main_state: u16,
    },
    ChangeMusic {
        player_id: u32,
        music_id: u16,
    },
    PlayAnimation {
        actor_id: u32,
        animation_id: u32,
    },
    SetPos {
        actor_id: u32,
        zone_id: u32,
        x: f32,
        y: f32,
        z: f32,
        rotation: f32,
    },
    GraphicChange {
        actor_id: u32,
        slot: u8,
        graphic_id: u32,
    },
    SpawnActor {
        zone_id: u32,
        actor_class_id: u32,
        x: f32,
        y: f32,
        z: f32,
        rotation: f32,
    },
    DespawnActor {
        zone_id: u32,
        actor_id: u32,
    },
    AddExp {
        actor_id: u32,
        class_id: u8,
        exp: i32,
    },
    AddItem {
        actor_id: u32,
        item_package: u16,
        item_id: u32,
        quantity: i32,
    },
    RemoveItem {
        actor_id: u32,
        item_package: u16,
        server_item_id: u64,
    },
    AddQuest {
        player_id: u32,
        quest_id: u32,
    },
    CompleteQuest {
        player_id: u32,
        quest_id: u32,
    },
    AbandonQuest {
        player_id: u32,
        quest_id: u32,
    },
    SetHomePoint {
        player_id: u32,
        homepoint: u32,
    },
    /// Mirrors `Player.SetLoginDirector(director)` in C# — used by
    /// `battlenpc.lua` / `player.lua` `onBeginLogin` on the tutorial path.
    /// Flipping this changes the LuaParam shape of the player's
    /// `ActorInstantiate` ScriptBind packet (C# `Player.CreateScriptBindPacket`
    /// branches on `loginInitDirector != null`). Without this command
    /// being fired on tutorial-zone login the 1.23b client stays at Now
    /// Loading because it never sees the "init director attached" variant.
    SetLoginDirector { player_id: u32 },
    RunEventFunction {
        player_id: u32,
        event_name: String,
        function_name: String,
        args: Vec<LuaCommandArg>,
    },
    KickEvent {
        player_id: u32,
        actor_id: u32,
        trigger: String,
        args: Vec<LuaCommandArg>,
    },
    Warp {
        player_id: u32,
        zone_id: u32,
        x: f32,
        y: f32,
        z: f32,
        rotation: f32,
    },
    DoZoneChange {
        player_id: u32,
        zone_id: u32,
        private_area: Option<String>,
        private_area_type: u32,
        spawn_type: u8,
        x: f32,
        y: f32,
        z: f32,
        rotation: f32,
    },
    LogError(String),
}

/// Value-equivalent of a Lua script parameter. Matches `common::LuaParam` but
/// lives here because the command queue predates wire-format concerns.
#[derive(Debug, Clone)]
pub enum LuaCommandArg {
    Int(i64),
    UInt(u64),
    Float(f64),
    String(String),
    Bool(bool),
    Nil,
    ActorId(u32),
}

/// Shared-ownership queue. Every userdata instance holds an `Arc<Mutex<…>>`
/// into the queue for the surrounding script invocation.
#[derive(Debug, Default)]
pub struct CommandQueue {
    commands: Vec<LuaCommand>,
}

impl CommandQueue {
    pub fn new() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self::default()))
    }

    pub fn push(queue: &Arc<Mutex<Self>>, command: LuaCommand) {
        if let Ok(mut inner) = queue.lock() {
            inner.commands.push(command);
        }
    }

    pub fn drain(queue: &Arc<Mutex<Self>>) -> Vec<LuaCommand> {
        match queue.lock() {
            Ok(mut inner) => std::mem::take(&mut inner.commands),
            Err(_) => Vec::new(),
        }
    }

    pub fn len(queue: &Arc<Mutex<Self>>) -> usize {
        queue.lock().map(|q| q.commands.len()).unwrap_or(0)
    }
}
