//! Events emitted by status-effect mutations. Same pattern as the inventory
//! outbox: mutation methods take `&mut StatusOutbox` and the game loop
//! drains it into packet/DB/Lua side effects per tick.

#![allow(dead_code)]

#[derive(Debug, Clone)]
pub enum StatusEvent {
    /// `Database.SavePlayerStatusEffects` (player-only).
    DbSave { owner_actor_id: u32 },

    /// `SetActorStatusPacket.BuildPacket(owner, index, statusId)` — write a
    /// 16-bit status id to a specific slot in `charaWork.status[]`.
    PacketSetStatus { owner_actor_id: u32, slot_index: u16, status_id: u16 },
    /// The corresponding end-time update in `charaWork.statusShownTime[]`.
    PacketSetStatusTime { owner_actor_id: u32, slot_index: u16, expires_at: u32 },
    /// Container calls `owner.RecalculateStats()` — signal the game loop to
    /// recompute modifier-derived stats.
    RecalcStats { owner_actor_id: u32 },
    /// Regen/DoT HP tick. Delta is signed: negative = damage.
    HpTick { owner_actor_id: u32, delta: i32 },
    /// Refresh MP tick.
    MpTick { owner_actor_id: u32, delta: i32 },
    /// Regain TP tick.
    TpTick { owner_actor_id: u32, delta: i32 },

    /// Script hook: `LuaEngine.CallLuaStatusEffectFunction(owner, effect,
    /// "onGain", …)` etc. The game loop dispatches to mlua.
    LuaCall {
        owner_actor_id: u32,
        status_effect_id: u32,
        function_name: &'static str,
    },

    /// Client message: `worldmasterTextId` with the effect id packed into the
    /// param payload.  The C# shoves this into `CommandResultContainer`; we
    /// just record the intent.
    WorldMasterText {
        owner_actor_id: u32,
        text_id: u16,
        status_effect_id: u32,
        play_gain_animation: bool,
    },
}

#[derive(Debug, Default)]
pub struct StatusOutbox {
    pub events: Vec<StatusEvent>,
}

impl StatusOutbox {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, event: StatusEvent) {
        self.events.push(event);
    }

    pub fn drain(&mut self) -> Vec<StatusEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }
}
