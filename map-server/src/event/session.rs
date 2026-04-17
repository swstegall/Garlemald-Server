//! `EventSession` — per-player "currently running event" state. Port of
//! the `currentEventOwner`, `currentEventName`, `currentEventType`
//! fields on `PlayerWork` / `Player`, plus the `StartEvent /
//! UpdateEvent / EndEvent / KickEvent` entry points.
//!
//! The session lives on the `Character` — call sites grab the
//! `ActorHandle` for a player and drop the session in place. Every
//! mutation emits `EventEvent`s into a shared `EventOutbox`; dispatch
//! happens in the ticker.

#![allow(dead_code)]

use common::luaparam::LuaParam;

use super::outbox::{EventEvent, EventOutbox};

#[derive(Debug, Clone, Default)]
pub struct EventSession {
    /// Actor id of the object that owns the running event — usually
    /// the NPC the player interacted with. `0` when no event is active.
    pub current_event_owner: u32,
    pub current_event_name: String,
    pub current_event_type: u8,
}

impl EventSession {
    pub fn is_in_event(&self) -> bool {
        self.current_event_owner != 0 || !self.current_event_name.is_empty()
    }

    /// `Player::StartEvent(owner, packet)` — record which event is
    /// running and emit the `EventStarted` hook for Lua.
    pub fn start_event(
        &mut self,
        player_actor_id: u32,
        owner_actor_id: u32,
        event_name: impl Into<String>,
        event_type: u8,
        lua_params: Vec<LuaParam>,
        outbox: &mut EventOutbox,
    ) {
        let event_name: String = event_name.into();
        self.current_event_owner = owner_actor_id;
        self.current_event_name = event_name.clone();
        self.current_event_type = event_type;
        outbox.push(EventEvent::EventStarted {
            player_actor_id,
            owner_actor_id,
            event_name,
            event_type,
            lua_params,
        });
    }

    /// `Player::UpdateEvent(packet)` — handle a client response mid-event.
    pub fn update_event(
        &self,
        player_actor_id: u32,
        trigger_actor_id: u32,
        event_type: u8,
        lua_params: Vec<LuaParam>,
        outbox: &mut EventOutbox,
    ) {
        outbox.push(EventEvent::EventUpdated {
            player_actor_id,
            trigger_actor_id,
            event_type,
            lua_params,
        });
    }

    /// `player:run_event_function(fn, ...)` → emit a RunEventFunctionPacket
    /// to the client. The Lua script typically calls this to drive the
    /// event forward.
    pub fn run_event_function(
        &self,
        player_actor_id: u32,
        function_name: impl Into<String>,
        lua_params: Vec<LuaParam>,
        outbox: &mut EventOutbox,
    ) {
        outbox.push(EventEvent::RunEventFunction {
            player_actor_id,
            trigger_actor_id: player_actor_id,
            owner_actor_id: self.current_event_owner,
            event_name: self.current_event_name.clone(),
            event_type: self.current_event_type,
            function_name: function_name.into(),
            lua_params,
        });
    }

    /// `Player::EndEvent()` — tears the event down client-side, clears
    /// session state.
    pub fn end_event(&mut self, player_actor_id: u32, outbox: &mut EventOutbox) {
        let owner = self.current_event_owner;
        let event_name = std::mem::take(&mut self.current_event_name);
        let event_type = self.current_event_type;
        self.current_event_owner = 0;
        self.current_event_type = 0;
        outbox.push(EventEvent::EndEvent {
            player_actor_id,
            owner_actor_id: owner,
            event_name,
            event_type,
        });
    }

    /// `Player::KickEvent(target, fn, ...)` — force an event onto another
    /// actor (e.g. NPC initiates a popup).
    pub fn kick_event(
        &self,
        player_actor_id: u32,
        target_actor_id: u32,
        event_name: impl Into<String>,
        event_type: u8,
        lua_params: Vec<LuaParam>,
        outbox: &mut EventOutbox,
    ) {
        outbox.push(EventEvent::KickEvent {
            player_actor_id,
            target_actor_id,
            owner_actor_id: target_actor_id,
            event_name: event_name.into(),
            event_type,
            lua_params,
        });
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_event_records_session_and_emits() {
        let mut s = EventSession::default();
        let mut ob = EventOutbox::new();
        s.start_event(1, 42, "quest_man0l0", 2, vec![], &mut ob);
        assert!(s.is_in_event());
        assert_eq!(s.current_event_owner, 42);
        assert_eq!(s.current_event_name, "quest_man0l0");
        assert!(ob.events.iter().any(|e| matches!(e, EventEvent::EventStarted { .. })));
    }

    #[test]
    fn end_event_clears_session() {
        let mut s = EventSession::default();
        let mut ob = EventOutbox::new();
        s.start_event(1, 42, "quest_man0l0", 2, vec![], &mut ob);
        s.end_event(1, &mut ob);
        assert!(!s.is_in_event());
        assert!(ob.events.iter().any(|e| matches!(e, EventEvent::EndEvent { .. })));
    }

    #[test]
    fn run_event_function_targets_current_owner() {
        let mut s = EventSession::default();
        let mut ob = EventOutbox::new();
        s.start_event(1, 42, "quest_x", 0, vec![], &mut ob);
        ob.drain();
        s.run_event_function(1, "nextDialog", vec![], &mut ob);
        let ran = ob.events.iter().find(|e| matches!(e, EventEvent::RunEventFunction { .. }));
        match ran {
            Some(EventEvent::RunEventFunction {
                owner_actor_id,
                function_name,
                event_name,
                ..
            }) => {
                assert_eq!(*owner_actor_id, 42);
                assert_eq!(event_name, "quest_x");
                assert_eq!(function_name, "nextDialog");
            }
            _ => panic!("expected RunEventFunction event"),
        }
    }
}
