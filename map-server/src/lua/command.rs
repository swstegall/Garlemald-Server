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
    AddGil {
        actor_id: u32,
        amount: i32,
    },
    /// `player:Die()` — force the actor into the DEAD state (flipping
    /// `current_main_state`, zeroing HP, broadcasting `SetActorState`).
    /// Used by GM commands and by scripted death cutscenes.
    Die {
        actor_id: u32,
    },
    /// `player:Revive()` — bring the actor back from DEAD, restoring
    /// HP/MP to max and broadcasting the state change.
    Revive {
        actor_id: u32,
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
    /// `quest:ClearQuestData()` / `data:ClearData()` — reset every flag +
    /// counter on the live quest.
    QuestClearData {
        player_id: u32,
        quest_id: u32,
    },
    /// `quest:ClearQuestFlags()` — zero the flag bitfield but leave
    /// counters intact. Matches Meteor's `Quest.ClearQuestFlags()`.
    QuestClearFlags {
        player_id: u32,
        quest_id: u32,
    },
    /// `quest:SetQuestFlag(bit)` / `data:SetFlag(bit)`.
    QuestSetFlag {
        player_id: u32,
        quest_id: u32,
        bit: u8,
    },
    /// `data:ClearFlag(bit)`.
    QuestClearFlag {
        player_id: u32,
        quest_id: u32,
        bit: u8,
    },
    /// `data:SetCounter(idx, value)` — value is 0..=65535.
    QuestSetCounter {
        player_id: u32,
        quest_id: u32,
        idx: u8,
        value: u16,
    },
    /// `data:IncCounter(idx)` — wraps at 65_536.
    QuestIncCounter {
        player_id: u32,
        quest_id: u32,
        idx: u8,
    },
    /// `data:DecCounter(idx)` — wraps at 0.
    QuestDecCounter {
        player_id: u32,
        quest_id: u32,
        idx: u8,
    },
    /// `quest:StartSequence(sequence)` — flips Dirty; the dispatcher
    /// fires `onStateChange(player, quest, sequence)` after the current
    /// script finishes so its side effects land after the mutation.
    QuestStartSequence {
        player_id: u32,
        quest_id: u32,
        sequence: u32,
    },
    /// `quest:SetENpc(classId, flagType, isTalkEnabled, isPushEnabled,
    /// isEmoteEnabled, isSpawned)` — register an actively-tracked NPC
    /// for the current sequence. The processor routes this through
    /// `QuestState::add_enpc` and, when the resulting `AddEnpcOutcome`
    /// is `New` or `Updated`, emits `SetEventStatus` + `SetActorQuestGraphic`
    /// packets to the player.
    QuestSetEnpc {
        player_id: u32,
        quest_id: u32,
        actor_class_id: u32,
        quest_flag_type: u8,
        is_talk_enabled: bool,
        is_push_enabled: bool,
        is_emote_enabled: bool,
        is_spawned: bool,
    },
    /// `quest:UpdateENPCs()` — drain `QuestState::old` (ENPCs the new
    /// sequence didn't re-register) and emit clear packets for each.
    /// Meteor's scripts call this at the tail of `onTalk` / `onPush` /
    /// `onKillBNpc` after a mutation that might have changed which
    /// NPCs are quest-active; the engine batches the broadcast so the
    /// script doesn't need to re-emit per-NPC.
    QuestUpdateEnpcs {
        player_id: u32,
        quest_id: u32,
    },
    /// `player:SetQuestComplete(id, flag)` — Meteor's direct-set of
    /// the 2048-bit completion bitfield. Unlike `CompleteQuest`, this
    /// doesn't remove the quest from active slots — GM debug commands
    /// (`!completedQuest`) and cross-quest prerequisites use it to
    /// retroactively mark a completion without running the script's
    /// `onFinish`.
    SetQuestComplete {
        player_id: u32,
        quest_id: u32,
        flag: bool,
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
    SetLoginDirector {
        player_id: u32,
        director_actor_id: u32,
    },
    /// `zone:CreateDirector(path, hasContentGroup)` in Lua. The C# version
    /// creates a `Director` actor with `actor_id = (6 << 28) | (zone_actor_id
    /// << 19) | director_local_id` and loads its `.lua` script. We don't
    /// persist any director state cross-session yet; this command just
    /// carries the computed id + classPath back to the Rust side so
    /// `send_zone_in_bundle` can emit the director's 7-packet spawn
    /// sequence and `SetLoginDirector` can reference the right actor id.
    CreateDirector {
        director_actor_id: u32,
        zone_actor_id: u32,
        class_path: String,
    },
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
