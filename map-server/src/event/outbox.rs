//! Events emitted by event / quest mutations. Same outbox pattern as
//! the rest of the server — the ticker drains these and the dispatcher
//! turns them into packets, DB writes, and Lua calls.

#![allow(dead_code)]

use common::luaparam::LuaParam;

#[derive(Debug, Clone)]
pub enum EventEvent {
    // ---- Incoming event lifecycle --------------------------------------
    /// `Player::StartEvent(owner, packet)` — player triggered an event.
    EventStarted {
        player_actor_id: u32,
        owner_actor_id: u32,
        event_name: String,
        event_type: u8,
        lua_params: Vec<LuaParam>,
    },
    /// `Player::UpdateEvent(packet)` — client responded mid-event.
    EventUpdated {
        player_actor_id: u32,
        trigger_actor_id: u32,
        event_type: u8,
        lua_params: Vec<LuaParam>,
    },
    /// Triggered by the Lua handler — emit `RunEventFunctionPacket`.
    RunEventFunction {
        player_actor_id: u32,
        trigger_actor_id: u32,
        owner_actor_id: u32,
        event_name: String,
        event_type: u8,
        function_name: String,
        lua_params: Vec<LuaParam>,
    },
    /// `Player::EndEvent()` — script finished; emit `EndEventPacket`.
    EndEvent {
        player_actor_id: u32,
        owner_actor_id: u32,
        event_name: String,
        event_type: u8,
    },
    /// `Player::KickEvent(actor, function, ...)` — force an event on a
    /// different actor (e.g. NPC pops a prompt at the player).
    KickEvent {
        player_actor_id: u32,
        target_actor_id: u32,
        owner_actor_id: u32,
        event_name: String,
        event_type: u8,
        lua_params: Vec<LuaParam>,
    },

    // ---- Quest side effects -------------------------------------------
    /// Fire `isObjectivesComplete(player, quest)` on the Lua side.
    QuestCheckCompletion { player_actor_id: u32, quest_id: u32 },
    /// Fire `onAbandonQuest(player, quest)`.
    QuestAbandonHook { player_actor_id: u32, quest_id: u32 },
    /// DB write — `Database::SaveQuest(player, quest)`.
    QuestSaveToDb {
        player_actor_id: u32,
        quest_id: u32,
        phase: u32,
        flags: u32,
        data: String,
    },
    /// `SendGameMessage(worldmaster, text_id, 0x20, quest_id)` — phase
    /// advance (25116), completion (25225), abandonment (25236).
    QuestGameMessage {
        player_actor_id: u32,
        text_id: u16,
        quest_id: u32,
    },
}

#[derive(Debug, Default)]
pub struct EventOutbox {
    pub events: Vec<EventEvent>,
}

impl EventOutbox {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, event: EventEvent) {
        self.events.push(event);
    }

    pub fn drain(&mut self) -> Vec<EventEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }
}
