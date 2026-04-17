//! Bridge between Lua's `LuaCommand` queue and the event system's
//! typed `EventOutbox`.
//!
//! Lua scripts call `player:RunEventFunction(fn, ...)` and friends; the
//! userdata layer pushes `LuaCommand::RunEventFunction` (and its siblings)
//! onto a shared queue. Once per tick the ticker drains the queue and
//! calls `translate_lua_commands_into_outbox` here — event-shaped
//! commands move into `EventOutbox::RunEventFunction`/`EndEvent`/
//! `KickEvent`, and the dispatcher fans them out to sockets.
//!
//! Non-event `LuaCommand` variants (movement, inventory, combat, …) are
//! left untouched — other bridges translate those.

#![allow(dead_code)]

use common::luaparam::LuaParam;

use crate::lua::command::{LuaCommand, LuaCommandArg};

use super::outbox::{EventEvent, EventOutbox};
use super::session::EventSession;

/// Iterate over `commands`, convert each event-flavoured entry into a
/// matching `EventEvent`, and push the result into `outbox`.
///
/// * `session` — the player's `EventSession`. `RunEventFunction` and
///   `EndEvent` read owner/event-name/event-type from it so Lua doesn't
///   have to re-pass them every call.
/// * The caller decides how to get a session (the ticker looks up the
///   player by `player_id` via the `ActorRegistry` and grabs the field).
pub fn translate_lua_commands_into_outbox(
    commands: &[LuaCommand],
    session: &EventSession,
    outbox: &mut EventOutbox,
) {
    for cmd in commands {
        match cmd {
            LuaCommand::RunEventFunction {
                player_id,
                event_name,
                function_name,
                args,
            } => {
                outbox.push(EventEvent::RunEventFunction {
                    player_actor_id: *player_id,
                    trigger_actor_id: *player_id,
                    owner_actor_id: session.current_event_owner,
                    event_name: if event_name.is_empty() {
                        session.current_event_name.clone()
                    } else {
                        event_name.clone()
                    },
                    event_type: session.current_event_type,
                    function_name: function_name.clone(),
                    lua_params: args.iter().map(arg_to_lua_param).collect(),
                });
            }
            LuaCommand::EndEvent {
                player_id,
                event_owner,
                event_name,
            } => {
                outbox.push(EventEvent::EndEvent {
                    player_actor_id: *player_id,
                    owner_actor_id: if *event_owner != 0 {
                        *event_owner
                    } else {
                        session.current_event_owner
                    },
                    event_name: if event_name.is_empty() {
                        session.current_event_name.clone()
                    } else {
                        event_name.clone()
                    },
                    event_type: session.current_event_type,
                });
            }
            LuaCommand::KickEvent {
                player_id,
                actor_id,
                trigger,
                args,
            } => {
                outbox.push(EventEvent::KickEvent {
                    player_actor_id: *player_id,
                    target_actor_id: *actor_id,
                    owner_actor_id: *actor_id,
                    event_name: trigger.clone(),
                    event_type: 0,
                    lua_params: args.iter().map(arg_to_lua_param).collect(),
                });
            }
            _ => {}
        }
    }
}

fn arg_to_lua_param(arg: &LuaCommandArg) -> LuaParam {
    match arg {
        LuaCommandArg::Int(i) => LuaParam::Int32(*i as i32),
        LuaCommandArg::UInt(u) => LuaParam::UInt32(*u as u32),
        // `LuaParam` is a retail-wire type with no float variant. The
        // retail scripts pass floats across as strings in the rare places
        // it happens — preserve that.
        LuaCommandArg::Float(f) => LuaParam::String(f.to_string()),
        LuaCommandArg::String(s) => LuaParam::String(s.clone()),
        LuaCommandArg::Bool(b) => if *b { LuaParam::True } else { LuaParam::False },
        LuaCommandArg::Nil => LuaParam::Nil,
        LuaCommandArg::ActorId(id) => LuaParam::Actor(*id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_with_event() -> EventSession {
        EventSession {
            current_event_owner: 42,
            current_event_name: "quest_man0l0".to_string(),
            current_event_type: 2,
        }
    }

    #[test]
    fn run_event_function_inherits_owner_and_name() {
        let cmd = LuaCommand::RunEventFunction {
            player_id: 1,
            event_name: String::new(),
            function_name: "nextDialog".to_string(),
            args: vec![LuaCommandArg::Int(7)],
        };
        let session = session_with_event();
        let mut outbox = EventOutbox::new();
        translate_lua_commands_into_outbox(&[cmd], &session, &mut outbox);
        match &outbox.events[0] {
            EventEvent::RunEventFunction {
                owner_actor_id,
                event_name,
                function_name,
                ..
            } => {
                assert_eq!(*owner_actor_id, 42);
                assert_eq!(event_name, "quest_man0l0");
                assert_eq!(function_name, "nextDialog");
            }
            _ => panic!("expected RunEventFunction"),
        }
    }

    #[test]
    fn end_event_uses_session_name_when_empty() {
        let cmd = LuaCommand::EndEvent {
            player_id: 1,
            event_owner: 0,
            event_name: String::new(),
        };
        let session = session_with_event();
        let mut outbox = EventOutbox::new();
        translate_lua_commands_into_outbox(&[cmd], &session, &mut outbox);
        match &outbox.events[0] {
            EventEvent::EndEvent {
                owner_actor_id,
                event_name,
                ..
            } => {
                assert_eq!(*owner_actor_id, 42);
                assert_eq!(event_name, "quest_man0l0");
            }
            _ => panic!("expected EndEvent"),
        }
    }

    #[test]
    fn kick_event_routes_to_target() {
        let cmd = LuaCommand::KickEvent {
            player_id: 1,
            actor_id: 99,
            trigger: "teleport".to_string(),
            args: vec![],
        };
        let session = session_with_event();
        let mut outbox = EventOutbox::new();
        translate_lua_commands_into_outbox(&[cmd], &session, &mut outbox);
        match &outbox.events[0] {
            EventEvent::KickEvent {
                target_actor_id,
                event_name,
                ..
            } => {
                assert_eq!(*target_actor_id, 99);
                assert_eq!(event_name, "teleport");
            }
            _ => panic!("expected KickEvent"),
        }
    }

    #[test]
    fn non_event_commands_pass_through_ignored() {
        let cmds = [LuaCommand::LogError("oops".to_string())];
        let session = EventSession::default();
        let mut outbox = EventOutbox::new();
        translate_lua_commands_into_outbox(&cmds, &session, &mut outbox);
        assert!(outbox.events.is_empty());
    }
}
