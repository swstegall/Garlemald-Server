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

//! `Director` — content orchestrator. Port of `Actors/Director/Director.cs`.
//!
//! A director holds a member list + a script path + optional content-
//! group id. Lifecycle mutations emit typed events on a `DirectorOutbox`;
//! the game-loop dispatcher turns those into real packets and Lua calls.

#![allow(dead_code)]

use std::collections::BTreeSet;

use super::outbox::{DirectorEvent, DirectorOutbox};

/// Which flavour of director this is. Drives which extra event variants
/// fire on lifecycle transitions.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DirectorKind {
    /// Plain content director (weather cycle, trial, etc.).
    #[default]
    Generic = 0,
    Guildleve = 1,
}

/// Base director state. `GuildleveDirector` wraps this and adds leve-
/// specific state.
#[derive(Debug, Clone)]
pub struct Director {
    pub director_id: u32,
    pub zone_id: u32,
    pub actor_id: u32,

    pub director_script_path: String,
    pub class_path: String,
    pub class_name: String,
    pub actor_name: String,

    pub has_content_group: bool,
    pub content_group_id: u32,

    /// Lifecycle flags matching the C# `isCreated` / `isDeleted` /
    /// `isDeleting` trio.
    pub is_created: bool,
    pub is_deleted: bool,
    pub is_deleting: bool,

    pub kind: DirectorKind,

    /// Member actor ids. Ordered (BTreeSet) so iteration is deterministic
    /// — the C# uses a `List<Actor>` but the extra cost doesn't matter at
    /// the scale of "one director's member list".
    members: BTreeSet<u32>,
    /// Subset — just the actor ids whose `ActorKindTag == Player`. Tracked
    /// in parallel so "who's subscribed to this director's packets" stays
    /// O(1) even when the registry is huge.
    player_members: BTreeSet<u32>,
}

impl Director {
    /// `Director(id, zone, directorPath, hasContentGroup, …)` — matches
    /// the C# constructor. The `id` is the director's local sequence
    /// number; `actor_id` is the composite `6 << 28 | zone_id << 19 | id`.
    pub fn new(
        id: u32,
        zone_id: u32,
        director_script_path: impl Into<String>,
        has_content_group: bool,
    ) -> Self {
        let actor_id = encode_director_actor_id(zone_id, id);
        let script_path: String = director_script_path.into();
        let class_name = script_path
            .rsplit('/')
            .next()
            .unwrap_or(&script_path)
            .to_string();
        Self {
            director_id: id,
            zone_id,
            actor_id,
            director_script_path: script_path.clone(),
            class_path: script_path,
            class_name,
            actor_name: String::new(),
            has_content_group,
            content_group_id: 0,
            is_created: false,
            is_deleted: false,
            is_deleting: false,
            kind: DirectorKind::Generic,
            members: BTreeSet::new(),
            player_members: BTreeSet::new(),
        }
    }

    pub fn is_created(&self) -> bool {
        self.is_created
    }

    pub fn is_deleted(&self) -> bool {
        self.is_deleted
    }

    pub fn has_content_group(&self) -> bool {
        self.has_content_group
    }

    pub fn content_group_id(&self) -> u32 {
        self.content_group_id
    }

    pub fn set_content_group_id(&mut self, id: u32) {
        self.content_group_id = id;
    }

    pub fn members(&self) -> impl Iterator<Item = u32> + '_ {
        self.members.iter().copied()
    }

    pub fn player_members(&self) -> impl Iterator<Item = u32> + '_ {
        self.player_members.iter().copied()
    }

    pub fn npc_members(&self) -> impl Iterator<Item = u32> + '_ {
        self.members
            .iter()
            .filter(|id| !self.player_members.contains(id))
            .copied()
    }

    pub fn player_count(&self) -> usize {
        self.player_members.len()
    }

    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    /// `StartDirector(spawnImmediate, args)` — fires the `init()` hook.
    /// In the Rust port the Lua side runs on the dispatcher; we just
    /// record the intent + mark the director created. Callers pass the
    /// resolved `class_path` from their Lua return (the script's
    /// `init()` typically returns it) or let the default remain.
    pub fn start(
        &mut self,
        class_path: Option<String>,
        spawn_immediate: bool,
        outbox: &mut DirectorOutbox,
    ) {
        if let Some(cp) = class_path {
            self.class_path = cp.clone();
            self.class_name = cp.rsplit('/').next().unwrap_or(&cp).to_string();
        }
        self.generate_actor_name(&format!("field{:05}", self.zone_id));
        self.is_created = true;
        outbox.push(DirectorEvent::DirectorStarted {
            director_id: self.actor_id,
            zone_id: self.zone_id,
            class_path: self.class_path.clone(),
            class_name: self.class_name.clone(),
            actor_name: self.actor_name.clone(),
            spawn_immediate,
        });
        outbox.push(DirectorEvent::MainCoroutine {
            director_id: self.actor_id,
        });
    }

    /// `EndDirector()` — broadcasts remove packets, clears members,
    /// flags the director deleted.
    pub fn end(&mut self, outbox: &mut DirectorOutbox) {
        if self.is_deleted {
            return;
        }
        self.is_deleting = true;
        let members_snapshot: Vec<(u32, bool)> = self
            .members
            .iter()
            .copied()
            .map(|id| (id, self.player_members.contains(&id)))
            .collect();
        for (actor_id, is_player) in &members_snapshot {
            outbox.push(DirectorEvent::MemberRemoved {
                director_id: self.actor_id,
                actor_id: *actor_id,
                is_player: *is_player,
            });
        }
        self.members.clear();
        self.player_members.clear();
        self.is_deleted = true;
        outbox.push(DirectorEvent::DirectorEnded {
            director_id: self.actor_id,
            zone_id: self.zone_id,
        });
    }

    /// Add an actor to the director's scope.
    pub fn add_member(&mut self, actor_id: u32, is_player: bool, outbox: &mut DirectorOutbox) {
        if !self.members.insert(actor_id) {
            return;
        }
        if is_player {
            self.player_members.insert(actor_id);
        }
        outbox.push(DirectorEvent::MemberAdded {
            director_id: self.actor_id,
            actor_id,
            is_player,
        });
    }

    /// Remove an actor. If no players remain and we're not already
    /// deleting, triggers an `end()` sweep.
    pub fn remove_member(&mut self, actor_id: u32, is_player: bool, outbox: &mut DirectorOutbox) {
        let existed = self.members.remove(&actor_id);
        if is_player {
            self.player_members.remove(&actor_id);
        }
        if !existed {
            return;
        }
        outbox.push(DirectorEvent::MemberRemoved {
            director_id: self.actor_id,
            actor_id,
            is_player,
        });
        if self.player_members.is_empty() && !self.is_deleting {
            self.end(outbox);
        }
    }

    /// Fire `onEventStarted(player?, director, …)` on the director's
    /// script.
    pub fn on_event_started(&self, player_actor_id: Option<u32>, outbox: &mut DirectorOutbox) {
        outbox.push(DirectorEvent::EventStarted {
            director_id: self.actor_id,
            player_actor_id,
        });
    }

    /// Port of `GenerateActorName(actorNumber)`. The C# mashes a
    /// class-name-tail + an abbreviated zone name + base63 actor number
    /// + a hex suffix. We keep the shape but simplify the zone
    ///   abbreviation table to the substitutions used in retail.
    pub fn generate_actor_name(&mut self, zone_name: &str) {
        let mut class_name = self.class_name.clone();
        if let Some(first) = class_name.chars().next() {
            let rest: String = class_name.chars().skip(1).collect();
            class_name = format!("{}{}", first.to_ascii_lowercase(), rest);
        }

        let mut zone_name = abbreviate_zone_name(zone_name);
        if let Some(first) = zone_name.chars().next() {
            let rest: String = zone_name.chars().skip(1).collect();
            zone_name = format!("{}{}", first.to_ascii_lowercase(), rest);
        }
        // Retail caps the class name at `20 - zone_name.len()` chars.
        let cap = 20usize.saturating_sub(zone_name.len());
        if class_name.len() > cap {
            class_name.truncate(cap);
        }

        let class_number = to_base63(self.director_id);
        self.actor_name = format!(
            "{class_name}_{zone_name}_{class_number}@{zone_id:03X}{priv_level:02X}",
            zone_id = self.zone_id,
            priv_level = 0u32,
        );
    }
}

/// Compute the composite actor id. Prefix 6 is the director kind; the
/// C# packs the zone id in the mid 9 bits and the local sequence number
/// in the low 19.
pub fn encode_director_actor_id(zone_id: u32, local_id: u32) -> u32 {
    (6u32 << 28) | ((zone_id & 0x1FF) << 19) | (local_id & 0x7FFFF)
}

fn abbreviate_zone_name(name: &str) -> String {
    name.replace("Field", "Fld")
        .replace("Dungeon", "Dgn")
        .replace("Town", "Twn")
        .replace("Battle", "Btl")
        .replace("Test", "Tes")
        .replace("Event", "Evt")
        .replace("Ship", "Shp")
        .replace("Office", "Ofc")
}

/// `Utils.ToStringBase63` port. Uses the custom alphabet in the C# source
/// (0-9, a-z, A-Z, `_`). Returns an ASCII string.
pub fn to_base63(mut n: u32) -> String {
    const ALPHABET: &[u8; 63] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_";
    if n == 0 {
        return "0".to_string();
    }
    let mut out = Vec::new();
    while n > 0 {
        out.push(ALPHABET[(n % 63) as usize]);
        n /= 63;
    }
    out.reverse();
    String::from_utf8(out).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actor_id_is_6_prefixed() {
        let d = Director::new(7, 100, "Weather/Default", false);
        assert_eq!(d.actor_id >> 28, 6);
        assert_eq!(d.actor_id & 0x7FFFF, 7);
    }

    #[test]
    fn start_emits_started_and_main_events() {
        let mut d = Director::new(1, 100, "Weather/Default", false);
        let mut ob = DirectorOutbox::new();
        d.start(Some("/Area/Director/Weather/Default".into()), true, &mut ob);
        assert!(d.is_created());
        let events = ob.drain();
        assert!(matches!(events[0], DirectorEvent::DirectorStarted { .. }));
        assert!(matches!(events[1], DirectorEvent::MainCoroutine { .. }));
    }

    #[test]
    fn add_and_remove_member_tracks_player_subset() {
        let mut d = Director::new(1, 100, "Guildleve/Sweep", true);
        let mut ob = DirectorOutbox::new();
        d.add_member(0xA000_0001, /* is_player */ true, &mut ob);
        d.add_member(0x4000_0100, /* is_player */ false, &mut ob);
        assert_eq!(d.member_count(), 2);
        assert_eq!(d.player_count(), 1);

        // Remove the last player — `end()` should fire automatically.
        d.remove_member(0xA000_0001, true, &mut ob);
        assert!(d.is_deleted());
        let events = ob.drain();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, DirectorEvent::DirectorEnded { .. }))
        );
    }

    #[test]
    fn end_is_idempotent() {
        let mut d = Director::new(1, 100, "Guildleve/Sweep", true);
        let mut ob = DirectorOutbox::new();
        d.end(&mut ob);
        assert!(d.is_deleted());
        let first_count = ob.events.len();
        d.end(&mut ob);
        assert_eq!(ob.events.len(), first_count, "second end() is a no-op");
    }

    #[test]
    fn base63_encoding_matches_retail() {
        // Retail test vectors from the C# Utils.ToStringBase63 unit tests:
        assert_eq!(to_base63(0), "0");
        assert_eq!(to_base63(1), "1");
        assert_eq!(to_base63(62), "_");
        assert_eq!(to_base63(63), "10");
        assert_eq!(to_base63(63 * 2), "20");
    }

    #[test]
    fn actor_name_fmt_uses_lowercase_class_and_base63_id() {
        let mut d = Director::new(7, 0x101, "Guildleve/Sweep", true);
        d.class_path = "Guildleve/PrivateGLBattleHuntNormal".into();
        d.class_name = "PrivateGLBattleHuntNormal".into();
        d.generate_actor_name("FieldCoastline");
        // `FieldCoastline` → `fldCoastline` (12 chars). Retail caps the
        // class-name prefix at `20 - zone.len()` = 8.
        assert!(d.actor_name.starts_with("privateG"));
        assert!(d.actor_name.contains("_fldCoastline_7@"));
    }
}
