//! StatusEffect + StatusEffectContainer. Faithful port of
//! `Actors/Chara/Ai/StatusEffect.cs` + `StatusEffectContainer.cs`.
//!
//! Mutations do not touch the database or sockets directly. They record
//! events on a `StatusOutbox` that the game loop drains each tick, mirroring
//! the pattern used by the inventory runtime.

#![allow(dead_code)]

pub mod flags;
pub mod ids;
pub mod outbox;

pub use flags::{StatusEffectFlags, StatusEffectOverwrite};
pub use ids::status_id_of;
pub use outbox::{StatusEvent, StatusOutbox};

use crate::actor::modifier::{Modifier, ModifierMap};

/// Maximum concurrently-visible status effects, matching
/// `charaWork.status[20]`.
pub const MAX_EFFECTS: usize = 20;

/// Default client text id for "you gained the effect of X". Overridable
/// per-effect; matches the 30328 default in C#.
pub const DEFAULT_GAIN_TEXT_ID: u16 = 30328;
/// Default "you lost the effect of X" text id.
pub const DEFAULT_LOSS_TEXT_ID: u16 = 30331;
/// Replace-effect text id used by `replace_effect`.
pub const REPLACE_TEXT_ID: u16 = 30330;

/// Stance end-time sentinel — u32 max, copied from the C# so the client
/// doesn't blink the icon when endTime equals max u32.
pub const STANCE_END_TIME: u32 = 0xFFFF_FFFF;

// ---------------------------------------------------------------------------
// StatusEffect — one entry in the effect table.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StatusEffect {
    pub owner_actor_id: u32,
    pub source_actor_id: u32,
    pub id: u32,
    pub name: String,

    /// Unix-seconds timestamp of when this effect was applied. `RefreshTime`
    /// updates both this and `end_time`.
    pub start_time: u32,
    /// Unix-seconds timestamp of expiry. For stance flags this is coerced
    /// to `STANCE_END_TIME`.
    pub end_time: u32,
    /// ms-precision "when did this last tick" — compared against `tick_ms`.
    pub last_tick_ms: u64,

    pub duration: u32,
    pub tick_ms: u32,
    pub magnitude: f64,
    pub tier: u8,
    pub extra: f64,

    pub flags: StatusEffectFlags,
    pub overwrite: StatusEffectOverwrite,
    pub silent_on_gain: bool,
    pub silent_on_loss: bool,
    pub hidden: bool,
    pub status_gain_text_id: u16,
    pub status_loss_text_id: u16,
    pub animation_hit_effect: u32,
}

impl StatusEffect {
    /// Full constructor used by the WorldManager template loader —
    /// matches the 10-arg C# ctor.
    #[allow(clippy::too_many_arguments)]
    pub fn from_template(
        id: u32,
        name: impl Into<String>,
        flags: u32,
        overwrite: u8,
        tick_ms: u32,
        hidden: bool,
        silent_on_gain: bool,
        silent_on_loss: bool,
        status_gain_text_id: u16,
        status_loss_text_id: u16,
    ) -> Self {
        Self {
            owner_actor_id: 0,
            source_actor_id: 0,
            id,
            name: name.into(),
            start_time: 0,
            end_time: 0,
            last_tick_ms: 0,
            duration: 0,
            tick_ms,
            magnitude: 0.0,
            tier: 0,
            extra: 0.0,
            flags: StatusEffectFlags::from(flags),
            overwrite: StatusEffectOverwrite::from_u8(overwrite),
            silent_on_gain,
            silent_on_loss,
            hidden,
            status_gain_text_id,
            status_loss_text_id,
            animation_hit_effect: 0,
        }
    }

    /// Lightweight constructor matching `new StatusEffect(owner, id, mag,
    /// tickMs, duration, tier)` — used for inline-scripted effects.
    pub fn new(
        owner_actor_id: u32,
        id: u32,
        magnitude: f64,
        tick_ms: u32,
        duration: u32,
        tier: u8,
        now_ms: u64,
    ) -> Self {
        Self {
            owner_actor_id,
            source_actor_id: owner_actor_id,
            id,
            name: String::new(),
            start_time: (now_ms / 1000) as u32,
            end_time: 0,
            last_tick_ms: now_ms,
            duration,
            tick_ms,
            magnitude,
            tier,
            extra: 0.0,
            flags: StatusEffectFlags::NONE,
            overwrite: StatusEffectOverwrite::None,
            silent_on_gain: false,
            silent_on_loss: false,
            hidden: false,
            status_gain_text_id: DEFAULT_GAIN_TEXT_ID,
            status_loss_text_id: DEFAULT_LOSS_TEXT_ID,
            animation_hit_effect: 0,
        }
    }

    /// Clone a template onto a new owner. Equivalent to the copy-ctor in C#.
    pub fn cloned_for(&self, new_owner: u32) -> Self {
        let mut me = self.clone();
        me.owner_actor_id = new_owner;
        me.source_actor_id = new_owner;
        me
    }

    // --- simple getters (match the C# naming for script bindings) ---

    pub fn status_id(&self) -> u16 {
        status_id_of(self.id)
    }

    pub fn is_stance(&self) -> bool {
        self.flags.contains(StatusEffectFlags::STANCE)
    }

    pub fn set_start_time(&mut self, now_ms: u64) {
        let now_s = (now_ms / 1000) as u32;
        self.start_time = now_s;
        self.last_tick_ms = now_ms;
    }

    /// Set end time, with stance-sentinel handling.
    pub fn set_end_time(&mut self, end_unix_s: u32) {
        self.end_time = if self.is_stance() { STANCE_END_TIME } else { end_unix_s };
    }

    /// Refresh the end time based on the stored duration, and emit a
    /// `PacketSetStatusTime` for the slot (if the effect is visible).
    pub fn refresh_time(&mut self, now_ms: u64, slot_index: Option<u16>, outbox: &mut StatusOutbox) {
        let now_s = (now_ms / 1000) as u32;
        self.set_end_time(now_s.saturating_add(self.duration));
        if let Some(i) = slot_index {
            outbox.push(StatusEvent::PacketSetStatusTime {
                owner_actor_id: self.owner_actor_id,
                slot_index: i,
                expires_at: self.wire_end_time(),
            });
        }
    }

    /// Value the client expects in `statusShownTime[i]`. Stances send u32::MAX.
    pub fn wire_end_time(&self) -> u32 {
        if self.is_stance() { STANCE_END_TIME } else { self.end_time }
    }

    /// Return `true` if this effect has ticked or expired and the container
    /// should act. Emits an `onTick` LuaCall when appropriate. Expiry is
    /// detected by the container (when `wire_end_time() != STANCE_END_TIME`
    /// and `now_s >= end_time`).
    pub fn update(&mut self, now_ms: u64, outbox: &mut StatusOutbox) -> bool {
        if self.tick_ms != 0
            && now_ms.saturating_sub(self.last_tick_ms) >= self.tick_ms as u64
        {
            self.last_tick_ms = now_ms;
            outbox.push(StatusEvent::LuaCall {
                owner_actor_id: self.owner_actor_id,
                status_effect_id: self.id,
                function_name: "onTick",
            });
        }

        if self.is_stance() {
            return false;
        }
        let now_s = (now_ms / 1000) as u32;
        self.end_time != 0 && now_s >= self.end_time
    }
}

// ---------------------------------------------------------------------------
// StatusEffectContainer — per-character status effect bookkeeping.
// ---------------------------------------------------------------------------

/// 3-second regen/refresh cadence, matching the C#.
pub const REGEN_TICK_MS: u64 = 3_000;

#[derive(Debug, Clone, Default)]
pub struct StatusEffectContainer {
    owner_actor_id: u32,
    effects: std::collections::HashMap<u32, StatusEffect>,
    /// Parallel to `charaWork.status[20]` — 16-bit short-ids (0 = empty slot).
    pub status: [u16; MAX_EFFECTS],
    /// Parallel to `charaWork.statusShownTime[20]` — Unix seconds.
    pub status_shown_time: [u32; MAX_EFFECTS],
    last_regen_ms: u64,
}

impl StatusEffectContainer {
    pub fn new(owner_actor_id: u32) -> Self {
        Self {
            owner_actor_id,
            effects: std::collections::HashMap::new(),
            status: [0; MAX_EFFECTS],
            status_shown_time: [0; MAX_EFFECTS],
            last_regen_ms: 0,
        }
    }

    pub fn owner_actor_id(&self) -> u32 {
        self.owner_actor_id
    }

    pub fn len(&self) -> usize {
        self.effects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    pub fn has(&self, id: u32) -> bool {
        self.effects.contains_key(&id)
    }

    pub fn get(&self, id: u32) -> Option<&StatusEffect> {
        self.effects.get(&id)
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut StatusEffect> {
        self.effects.get_mut(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &StatusEffect> {
        self.effects.values()
    }

    pub fn get_by_flag(&self, flag: StatusEffectFlags) -> Vec<&StatusEffect> {
        self.effects.values().filter(|e| e.flags.intersects(flag)).collect()
    }

    pub fn has_by_flag(&self, flag: StatusEffectFlags) -> bool {
        self.effects.values().any(|e| e.flags.intersects(flag))
    }

    // --- Tick driver -------------------------------------------------------

    /// Drive the container for one game-loop tick. Mirrors the C# `Update`:
    ///
    /// * Every `REGEN_TICK_MS` of elapsed wall time, emit regen/refresh/
    ///   regain/DoT deltas read from the owner's modifier map.
    /// * For each effect, call `update(now_ms)`; if it returns `true` the
    ///   effect is removed with its standard loss text id.
    pub fn update(&mut self, now_ms: u64, mods: &ModifierMap, outbox: &mut StatusOutbox) {
        if now_ms.saturating_sub(self.last_regen_ms) >= REGEN_TICK_MS {
            self.regen_tick(mods, outbox);
            self.last_regen_ms = now_ms;
        }

        // Collect expiries without holding an immutable borrow during mutation.
        let ids: Vec<u32> = self.effects.keys().copied().collect();
        let mut expired = Vec::new();
        for id in ids {
            if let Some(effect) = self.effects.get_mut(&id)
                && effect.update(now_ms, outbox)
            {
                expired.push(id);
            }
        }
        for id in expired {
            let loss_text = self
                .effects
                .get(&id)
                .map(|e| e.status_loss_text_id)
                .unwrap_or(DEFAULT_LOSS_TEXT_ID);
            self.remove_status_effect(id, loss_text, /* play_effect */ true, outbox);
        }
    }

    /// The regen portion of the tick. Reads Modifier::Regen / Refresh /
    /// Regain / RegenDown and emits `HpTick` / `MpTick` / `TpTick` events.
    pub fn regen_tick(&mut self, mods: &ModifierMap, outbox: &mut StatusOutbox) {
        let dot = mods.get(Modifier::RegenDown) as i32;
        let regen = mods.get(Modifier::Regen) as i32;
        let refresh = mods.get(Modifier::Refresh) as i32;
        let regain = mods.get(Modifier::Regain) as i32;

        // DoTs tick before regen so the full damage shows even if partially
        // absorbed by regen downstream.
        if dot > 0 {
            outbox.push(StatusEvent::HpTick {
                owner_actor_id: self.owner_actor_id,
                delta: -dot,
            });
        }
        if regen != 0 {
            outbox.push(StatusEvent::HpTick {
                owner_actor_id: self.owner_actor_id,
                delta: regen,
            });
        }
        if refresh != 0 {
            outbox.push(StatusEvent::MpTick {
                owner_actor_id: self.owner_actor_id,
                delta: refresh,
            });
        }
        if regain != 0 {
            outbox.push(StatusEvent::TpTick {
                owner_actor_id: self.owner_actor_id,
                delta: regain,
            });
        }
    }

    // --- Add / Remove ------------------------------------------------------

    /// Add a status effect. Mirrors the complex `AddStatusEffect(StatusEffect
    /// newEffect, Character source, ...)` overload in C#.
    ///
    /// Returns `true` if the effect was applied (either a fresh add or a
    /// successful overwrite). Emits events for packets, DB save, stat
    /// recalculation, and `onGain` Lua hook.
    pub fn add_status_effect(
        &mut self,
        mut effect: StatusEffect,
        source_actor_id: u32,
        now_ms: u64,
        worldmaster_text_id: u16,
        outbox: &mut StatusOutbox,
    ) -> bool {
        let effect_id = effect.id;

        // If the same-id effect is already active, consult its overwrite rule.
        let can_overwrite = if let Some(existing) = self.effects.get(&effect_id) {
            match existing.overwrite {
                StatusEffectOverwrite::None => false,
                StatusEffectOverwrite::Always => true,
                StatusEffectOverwrite::GreaterOnly => {
                    existing.duration < effect.duration
                        || existing.magnitude < effect.magnitude
                }
                StatusEffectOverwrite::GreaterOrEqualTo => {
                    existing.duration <= effect.duration
                        || existing.magnitude <= effect.magnitude
                }
            }
        } else {
            false
        };

        let is_fresh = !self.effects.contains_key(&effect_id);
        if !is_fresh && !can_overwrite {
            return false;
        }

        // Client-visible gain message (unless silent).
        if !effect.silent_on_gain {
            outbox.push(StatusEvent::WorldMasterText {
                owner_actor_id: self.owner_actor_id,
                text_id: worldmaster_text_id,
                status_effect_id: effect_id,
                play_gain_animation: true,
            });
        }

        // On overwrite, drop the old entry but keep its slot so the new
        // effect lands in the same index.
        let prior_slot = if can_overwrite {
            let old_short = effect.status_id();
            self.effects.remove(&effect_id);
            self.status
                .iter()
                .position(|&s| s == old_short)
                .map(|i| i as u16)
        } else {
            None
        };

        // Initialize timing and ownership.
        effect.owner_actor_id = self.owner_actor_id;
        effect.source_actor_id = source_actor_id;
        effect.set_start_time(now_ms);
        let now_s = (now_ms / 1000) as u32;
        effect.set_end_time(now_s.saturating_add(effect.duration));

        // Hard cap at MAX_EFFECTS — matches the C# silent-drop behaviour.
        if self.effects.len() >= MAX_EFFECTS {
            return false;
        }

        outbox.push(StatusEvent::LuaCall {
            owner_actor_id: self.owner_actor_id,
            status_effect_id: effect_id,
            function_name: "onGain",
        });

        if !effect.hidden {
            let short_id = effect.status_id();
            let slot_index = prior_slot
                .or_else(|| self.status.iter().position(|&s| s == short_id).map(|i| i as u16))
                .or_else(|| self.status.iter().position(|&s| s == 0).map(|i| i as u16));

            if let Some(idx) = slot_index {
                self.set_status_at_index(idx, short_id, outbox);
                self.set_time_at_index(idx, effect.wire_end_time(), outbox);
            }
        }

        self.effects.insert(effect_id, effect);

        outbox.push(StatusEvent::RecalcStats {
            owner_actor_id: self.owner_actor_id,
        });

        true
    }

    /// Remove an effect by id. `play_effect` controls whether the gain/loss
    /// animation bit is packed into the world-master message.
    pub fn remove_status_effect(
        &mut self,
        effect_id: u32,
        worldmaster_text_id: u16,
        play_effect: bool,
        outbox: &mut StatusOutbox,
    ) -> bool {
        let Some(effect) = self.effects.get(&effect_id) else {
            return false;
        };

        if !effect.silent_on_loss {
            outbox.push(StatusEvent::WorldMasterText {
                owner_actor_id: self.owner_actor_id,
                text_id: worldmaster_text_id,
                status_effect_id: effect_id,
                play_gain_animation: play_effect,
            });
        }

        let short_id = effect.status_id();
        let hidden = effect.hidden;
        if !hidden
            && let Some(idx) = self.status.iter().position(|&s| s == short_id).map(|i| i as u16)
        {
            self.set_status_at_index(idx, 0, outbox);
            self.set_time_at_index(idx, 0, outbox);
        }

        self.effects.remove(&effect_id);
        outbox.push(StatusEvent::LuaCall {
            owner_actor_id: self.owner_actor_id,
            status_effect_id: effect_id,
            function_name: "onLose",
        });
        outbox.push(StatusEvent::RecalcStats {
            owner_actor_id: self.owner_actor_id,
        });
        true
    }

    /// Remove every effect whose flag set intersects `flag`.
    pub fn remove_by_flag(&mut self, flag: StatusEffectFlags, outbox: &mut StatusOutbox) -> bool {
        let ids: Vec<(u32, u16)> = self
            .effects
            .values()
            .filter(|e| e.flags.intersects(flag))
            .map(|e| (e.id, e.status_loss_text_id))
            .collect();

        let removed = !ids.is_empty();
        for (id, loss_text) in ids {
            self.remove_status_effect(id, loss_text, true, outbox);
        }
        removed
    }

    /// Overwrite one effect with a different status id, landing in the same
    /// slot so the UI icon stays put. Mirrors `ReplaceEffect` in C#.
    pub fn replace_effect(
        &mut self,
        old_effect_id: u32,
        new_effect: StatusEffect,
        now_ms: u64,
        outbox: &mut StatusOutbox,
    ) -> bool {
        let Some(old) = self.effects.remove(&old_effect_id) else {
            return false;
        };
        let old_short = old.status_id();

        outbox.push(StatusEvent::LuaCall {
            owner_actor_id: self.owner_actor_id,
            status_effect_id: old_effect_id,
            function_name: "onLose",
        });
        outbox.push(StatusEvent::LuaCall {
            owner_actor_id: self.owner_actor_id,
            status_effect_id: new_effect.id,
            function_name: "onGain",
        });

        let mut new_effect = new_effect;
        new_effect.owner_actor_id = self.owner_actor_id;
        new_effect.set_start_time(now_ms);
        let now_s = (now_ms / 1000) as u32;
        new_effect.set_end_time(now_s.saturating_add(new_effect.duration));

        if let Some(idx) = self.status.iter().position(|&s| s == old_short).map(|i| i as u16) {
            // The C# writes 0 first, then the new id — reproduce that so
            // the client replays the slot change. The intermediate packet
            // appears in the outbox stream.
            self.set_status_at_index(idx, 0, outbox);
            self.set_status_at_index(idx, new_effect.status_id(), outbox);
            self.set_time_at_index(idx, new_effect.wire_end_time(), outbox);
        }

        outbox.push(StatusEvent::WorldMasterText {
            owner_actor_id: self.owner_actor_id,
            text_id: REPLACE_TEXT_ID,
            status_effect_id: new_effect.id,
            play_gain_animation: true,
        });

        let new_id = new_effect.id;
        self.effects.insert(new_id, new_effect);
        true
    }

    /// Clone-and-add variant: `StatusEffectContainer.CopyEffect(effect)`.
    pub fn copy_effect(
        &mut self,
        template: &StatusEffect,
        now_ms: u64,
        outbox: &mut StatusOutbox,
    ) -> bool {
        let cloned = template.cloned_for(self.owner_actor_id);
        let source = template.source_actor_id;
        self.add_status_effect(cloned, source, now_ms, DEFAULT_GAIN_TEXT_ID, outbox)
    }

    /// Drop everything (e.g. on death, if `lose_on_death` flag matches).
    pub fn clear_all(&mut self, outbox: &mut StatusOutbox) {
        let ids: Vec<(u32, u16)> = self
            .effects
            .values()
            .map(|e| (e.id, e.status_loss_text_id))
            .collect();
        for (id, loss_text) in ids {
            self.remove_status_effect(id, loss_text, false, outbox);
        }
    }

    /// Trigger a player-side DB save.
    pub fn save_to_db(&self, outbox: &mut StatusOutbox) {
        outbox.push(StatusEvent::DbSave {
            owner_actor_id: self.owner_actor_id,
        });
    }

    // --- charaWork slot helpers -------------------------------------------

    fn set_status_at_index(&mut self, index: u16, status_id: u16, outbox: &mut StatusOutbox) {
        if let Some(slot) = self.status.get_mut(index as usize) {
            *slot = status_id;
            outbox.push(StatusEvent::PacketSetStatus {
                owner_actor_id: self.owner_actor_id,
                slot_index: index,
                status_id,
            });
        }
    }

    fn set_time_at_index(&mut self, index: u16, expires_at: u32, outbox: &mut StatusOutbox) {
        if let Some(slot) = self.status_shown_time.get_mut(index as usize) {
            *slot = expires_at;
            outbox.push(StatusEvent::PacketSetStatusTime {
                owner_actor_id: self.owner_actor_id,
                slot_index: index,
                expires_at,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::ids::{STATUS_POISON, STATUS_RAMPART, STATUS_STUN};

    fn mk(id: u32, duration: u32) -> StatusEffect {
        StatusEffect::new(/* owner */ 0, id, /* mag */ 1.0, /* tick_ms */ 0, duration, 0, /* now_ms */ 0)
    }

    #[test]
    fn add_and_remove_happy_path() {
        let mut c = StatusEffectContainer::new(42);
        let mut ob = StatusOutbox::new();

        let applied = c.add_status_effect(mk(STATUS_POISON, 30), 42, 0, DEFAULT_GAIN_TEXT_ID, &mut ob);
        assert!(applied);
        assert!(c.has(STATUS_POISON));
        assert_eq!(c.len(), 1);
        // Slot 0 should hold the short id.
        assert_eq!(c.status[0], status_id_of(STATUS_POISON));

        let removed = c.remove_status_effect(STATUS_POISON, DEFAULT_LOSS_TEXT_ID, true, &mut ob);
        assert!(removed);
        assert!(!c.has(STATUS_POISON));
        assert_eq!(c.status[0], 0);
    }

    #[test]
    fn overwrite_rule_none_rejects() {
        let mut c = StatusEffectContainer::new(1);
        let mut ob = StatusOutbox::new();

        let mut eff = mk(STATUS_POISON, 30);
        eff.overwrite = StatusEffectOverwrite::None;
        c.add_status_effect(eff, 1, 0, DEFAULT_GAIN_TEXT_ID, &mut ob);

        let second = mk(STATUS_POISON, 120);
        assert!(!c.add_status_effect(second, 1, 0, DEFAULT_GAIN_TEXT_ID, &mut ob));
        assert_eq!(c.get(STATUS_POISON).unwrap().duration, 30);
    }

    #[test]
    fn overwrite_rule_greater_only() {
        let mut c = StatusEffectContainer::new(1);
        let mut ob = StatusOutbox::new();
        let mut eff = mk(STATUS_POISON, 30);
        eff.overwrite = StatusEffectOverwrite::GreaterOnly;
        c.add_status_effect(eff, 1, 0, DEFAULT_GAIN_TEXT_ID, &mut ob);

        let mut equal = mk(STATUS_POISON, 30);
        equal.overwrite = StatusEffectOverwrite::GreaterOnly;
        assert!(!c.add_status_effect(equal, 1, 0, DEFAULT_GAIN_TEXT_ID, &mut ob));

        let mut longer = mk(STATUS_POISON, 60);
        longer.overwrite = StatusEffectOverwrite::GreaterOnly;
        assert!(c.add_status_effect(longer, 1, 0, DEFAULT_GAIN_TEXT_ID, &mut ob));
        assert_eq!(c.get(STATUS_POISON).unwrap().duration, 60);
    }

    #[test]
    fn expiry_tick_removes_effect() {
        let mut c = StatusEffectContainer::new(1);
        let mut ob = StatusOutbox::new();
        // 1s duration, applied at t=0.
        let mut eff = mk(STATUS_STUN, 1);
        eff.overwrite = StatusEffectOverwrite::Always;
        c.add_status_effect(eff, 1, 0, DEFAULT_GAIN_TEXT_ID, &mut ob);
        ob.drain();

        let mods = ModifierMap::default();
        c.update(/* t= */ 500, &mods, &mut ob);
        assert!(c.has(STATUS_STUN));

        c.update(/* t= */ 2_000, &mods, &mut ob);
        assert!(!c.has(STATUS_STUN));
    }

    #[test]
    fn regen_tick_emits_hp_and_mp_events() {
        let mut c = StatusEffectContainer::new(1);
        let mut ob = StatusOutbox::new();
        let mut mods = ModifierMap::default();
        mods.set(Modifier::Regen, 25.0);
        mods.set(Modifier::Refresh, 10.0);
        mods.set(Modifier::RegenDown, 4.0);
        mods.set(Modifier::Regain, 100.0);

        c.update(REGEN_TICK_MS, &mods, &mut ob);
        let events = ob.drain();
        assert!(events.iter().any(|e| matches!(e, StatusEvent::HpTick { delta: -4, .. })));
        assert!(events.iter().any(|e| matches!(e, StatusEvent::HpTick { delta: 25, .. })));
        assert!(events.iter().any(|e| matches!(e, StatusEvent::MpTick { delta: 10, .. })));
        assert!(events.iter().any(|e| matches!(e, StatusEvent::TpTick { delta: 100, .. })));
    }

    #[test]
    fn stance_flag_pins_end_time() {
        let mut c = StatusEffectContainer::new(1);
        let mut ob = StatusOutbox::new();
        let mut eff = mk(STATUS_RAMPART, 10);
        eff.flags = StatusEffectFlags::STANCE;
        c.add_status_effect(eff, 1, 0, DEFAULT_GAIN_TEXT_ID, &mut ob);
        assert_eq!(c.get(STATUS_RAMPART).unwrap().wire_end_time(), STANCE_END_TIME);
        // Stance effects never expire through the update path.
        let mods = ModifierMap::default();
        c.update(1_000_000_000, &mods, &mut ob);
        assert!(c.has(STATUS_RAMPART));
    }

    #[test]
    fn max_effects_cap_enforced() {
        let mut c = StatusEffectContainer::new(1);
        let mut ob = StatusOutbox::new();
        for i in 0..MAX_EFFECTS as u32 {
            let mut eff = mk(STATUS_POISON + i, 100);
            eff.overwrite = StatusEffectOverwrite::Always;
            c.add_status_effect(eff, 1, 0, DEFAULT_GAIN_TEXT_ID, &mut ob);
        }
        assert_eq!(c.len(), MAX_EFFECTS);
        let overflow = mk(999_999, 100);
        assert!(!c.add_status_effect(overflow, 1, 0, DEFAULT_GAIN_TEXT_ID, &mut ob));
    }

    #[test]
    fn remove_by_flag_purges_matching() {
        let mut c = StatusEffectContainer::new(1);
        let mut ob = StatusOutbox::new();
        let mut a = mk(STATUS_POISON, 60);
        a.flags = StatusEffectFlags::LOSE_ON_ESUNA;
        let b = mk(STATUS_STUN, 60);
        c.add_status_effect(a, 1, 0, DEFAULT_GAIN_TEXT_ID, &mut ob);
        c.add_status_effect(b, 1, 0, DEFAULT_GAIN_TEXT_ID, &mut ob);
        ob.drain();

        let any = c.remove_by_flag(StatusEffectFlags::LOSE_ON_ESUNA, &mut ob);
        assert!(any);
        assert!(!c.has(STATUS_POISON));
        assert!(c.has(STATUS_STUN));
    }
}
