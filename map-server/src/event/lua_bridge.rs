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
                let owner = if *event_owner != 0 {
                    *event_owner
                } else {
                    session.current_event_owner
                };
                // pmeteor `Player.EndEvent` ends the CURRENT event. The
                // name the Lua snapshot captured can be stale: a coroutine
                // parked in `pushDefault` (man0l0's processTtrNomal002
                // popup) is resumed by the player's `talkDefault`
                // EventStart, which re-stamps the session first. Ending
                // "pushDefault" with talkDefault's type left the client
                // inside the talk, unable to talk again.
                let name = if event_name.is_empty()
                    || (owner == session.current_event_owner
                        && !session.current_event_name.is_empty())
                {
                    session.current_event_name.clone()
                } else {
                    event_name.clone()
                };
                outbox.push(EventEvent::EndEvent {
                    player_actor_id: *player_id,
                    owner_actor_id: owner,
                    event_name: name,
                    event_type: session.current_event_type,
                });
            }
            // KickEvent is INTENTIONALLY excluded — it flows through
            // `apply_login_lua_command`'s `LC::KickEvent` arm
            // (processor.rs:1003) which captures into
            // `session.pending_kick_event` for emission either at the
            // end of the next zone-in bundle (OpeningDirector path) OR
            // pre-warp by `apply_do_zone_change_content` (SEQ_005
            // content-director path). Runtime/event-bridge drains hit
            // `apply_runtime_lua_command`'s KickEvent arm instead
            // (immediate send — the mid-flow `kickEventContinue`
            // shape, #28 S1.1); both homes rely on this exclusion to
            // stay single-emission.
            //
            // Translating KickEvent here AS WELL produced a duplicate
            // wire emission per quest-hook-driven kick: one fires
            // here via dispatch_event_event, then the same LuaCommand
            // is also drained through apply_login_lua_command which
            // captures it AGAIN. Both then emit the same kick bytes
            // (after the f4e2422 field-correction fix). Two
            // back-to-back kicks confused the 1.x client's content-
            // group state machine — pmeteor's reference capture
            // sends only ONE kick per warp.
            //
            // For the OpeningDirector at session start, the kick
            // never goes through this translator (apply_login_lua_command
            // is called directly), so the deduplication only changes
            // the quest-hook-driven path.
            _ => {}
        }
    }
}

pub(crate) fn arg_to_lua_param(arg: &LuaCommandArg) -> LuaParam {
    match arg {
        LuaCommandArg::Int(i) => LuaParam::Int32(*i as i32),
        LuaCommandArg::UInt(u) => LuaParam::UInt32(*u as u32),
        // `LuaParam` is a retail-wire type with no float variant. The
        // retail scripts pass floats across as strings in the rare places
        // it happens — preserve that.
        LuaCommandArg::Float(f) => LuaParam::String(f.to_string()),
        LuaCommandArg::String(s) => LuaParam::String(s.clone()),
        LuaCommandArg::Bool(b) => {
            if *b {
                LuaParam::True
            } else {
                LuaParam::False
            }
        }
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
    fn send_data_packet_args_map_to_lua_params() {
        // SendDataPacket(9) == startTutorialMode → a single Int32(9) LuaParam.
        // This is the param that arms the client's active-mode (F) toggle in
        // SEQ_005; it must encode as Int32, not be dropped. (Garlemald #28.)
        let p = arg_to_lua_param(&LuaCommandArg::Int(9));
        assert!(matches!(p, common::luaparam::LuaParam::Int32(9)));

        // The full body for SendDataPacket(9): tag 0x00 + i32 BE 0x00000009 +
        // LUA_END 0x0F. Mirrors the pmeteor GenericDataPacket(9) wire bytes.
        let sub = crate::packets::send::player::build_generic_data(
            1,
            &[common::luaparam::LuaParam::Int32(9)],
        );
        assert_eq!(&sub.data[..6], &[0x00, 0x00, 0x00, 0x00, 0x09, 0x0F]);

        // The richer "attention" shape must marshal every slot (the generic
        // collector must not drop the Actor-userdata or trailing ints).
        let params: Vec<_> = [
            LuaCommandArg::String("attention".to_string()),
            LuaCommandArg::ActorId(0x4000_0001),
            LuaCommandArg::String(String::new()),
            LuaCommandArg::Int(51073),
            LuaCommandArg::Int(2),
        ]
        .iter()
        .map(arg_to_lua_param)
        .collect();
        assert_eq!(params.len(), 5);
        assert!(matches!(params[0], common::luaparam::LuaParam::String(_)));
        assert!(matches!(
            params[1],
            common::luaparam::LuaParam::Actor(0x4000_0001)
        ));
        assert!(matches!(
            params[3],
            common::luaparam::LuaParam::Int32(51073)
        ));
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
    fn kick_event_is_suppressed_from_outbox() {
        // KickEvent flows through apply_login_lua_command's KickEvent
        // arm (which captures into session.pending_kick_event) — NOT
        // through the EventOutbox. Translating it here AS WELL would
        // produce a duplicate wire emission. The translator
        // intentionally drops KickEvent into the catch-all `_ => {}`.
        let cmd = LuaCommand::KickEvent {
            player_id: 1,
            actor_id: 99,
            trigger: "teleport".to_string(),
            args: vec![],
        };
        let session = session_with_event();
        let mut outbox = EventOutbox::new();
        translate_lua_commands_into_outbox(&[cmd], &session, &mut outbox);
        assert!(
            outbox.events.is_empty(),
            "KickEvent must NOT be pushed to the EventOutbox; got {:?}",
            outbox.events,
        );
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
