//! `SpawnLocation` — pure-data record describing one NPC spawn seed.
//! Ported 1:1 from `Actors/Area/SpawnLocation.cs`.

#![allow(dead_code)]

#[derive(Debug, Clone)]
pub struct SpawnLocation {
    pub class_id: u32,
    pub unique_id: String,
    pub zone_id: u32,
    /// Empty when the spawn belongs to the zone root, otherwise the name
    /// of the private area it should land in.
    pub private_area_name: String,
    pub private_area_level: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rotation: f32,
    pub state: u16,
    pub animation_id: u32,
}

impl SpawnLocation {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        class_id: u32,
        unique_id: impl Into<String>,
        zone_id: u32,
        private_area_name: impl Into<String>,
        private_area_level: u32,
        x: f32,
        y: f32,
        z: f32,
        rotation: f32,
        state: u16,
        animation_id: u32,
    ) -> Self {
        Self {
            class_id,
            unique_id: unique_id.into(),
            zone_id,
            private_area_name: private_area_name.into(),
            private_area_level,
            x,
            y,
            z,
            rotation,
            state,
            animation_id,
        }
    }

    /// Convenience — does this spawn live inside a private area?
    pub fn is_in_private_area(&self) -> bool {
        !self.private_area_name.is_empty()
    }
}
