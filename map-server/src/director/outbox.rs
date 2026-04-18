//! Events emitted by director mutations. Drained by the game-loop
//! dispatcher and turned into packet sends, DB writes, and Lua calls.

#![allow(dead_code)]

#[derive(Debug, Clone)]
pub enum DirectorEvent {
    // ---- Lifecycle ----------------------------------------------------
    /// Director has been constructed + `init()` succeeded. Spawn its
    /// actor + init packets to all current player members.
    DirectorStarted {
        director_id: u32,
        zone_id: u32,
        class_path: String,
        class_name: String,
        actor_name: String,
        spawn_immediate: bool,
    },
    /// Director cleanup — broadcast `RemoveActorPacket` to members,
    /// delete the content group, purge from zone registry.
    DirectorEnded {
        director_id: u32,
        zone_id: u32,
    },

    /// `main(director, contentGroup)` coroutine should start.
    MainCoroutine {
        director_id: u32,
    },
    /// Script-driven `onEventStarted` from the player or director.
    EventStarted {
        director_id: u32,
        player_actor_id: Option<u32>,
    },

    // ---- Membership ---------------------------------------------------
    MemberAdded {
        director_id: u32,
        actor_id: u32,
        is_player: bool,
    },
    MemberRemoved {
        director_id: u32,
        actor_id: u32,
        is_player: bool,
    },

    // ---- Guildleve progress ------------------------------------------
    /// Start a guildleve — emit music/text/property packets to all
    /// player members.
    GuildleveStarted {
        director_id: u32,
        guildleve_id: u32,
        difficulty: u8,
        location: u32,
        time_limit_seconds: u32,
        start_time_unix: u32,
    },
    /// End a guildleve. `was_completed=true` triggers the victory music +
    /// reward messages; `false` is a time-out.
    GuildleveEnded {
        director_id: u32,
        guildleve_id: u32,
        was_completed: bool,
        completion_time_seconds: u32,
    },
    GuildleveAbandoned {
        director_id: u32,
        guildleve_id: u32,
    },
    /// Property update: `guildleveWork.aimNumNow[i] = value`.
    GuildleveAimUpdated {
        director_id: u32,
        index: u8,
        value: i8,
    },
    /// Property update: `guildleveWork.uiState[i] = value`.
    GuildleveUiUpdated {
        director_id: u32,
        index: u8,
        value: i8,
    },
    /// Property update: marker `[i]` moved to `(x,y,z)`.
    GuildleveMarkerUpdated {
        director_id: u32,
        index: u8,
        x: f32,
        y: f32,
        z: f32,
    },
    /// Full property resync — used on zone-in or after big state changes.
    GuildleveSyncAll {
        director_id: u32,
    },
}

#[derive(Debug, Default)]
pub struct DirectorOutbox {
    pub events: Vec<DirectorEvent>,
}

impl DirectorOutbox {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, event: DirectorEvent) {
        self.events.push(event);
    }

    pub fn drain(&mut self) -> Vec<DirectorEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }
}
