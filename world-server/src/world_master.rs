//! Zone routing & global group manager hub. Ported from WorldMaster.cs, but
//! scoped to the Phase-3 surface: zone-server discovery, session routing
//! helpers, and holding the four group managers.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::data::ZoneServerHandle;
use crate::managers::{
    LinkshellManager, PartyManager, RelationGroupManager, RetainerGroupManager,
};

pub struct WorldMaster {
    pub party_manager: PartyManager,
    pub linkshell_manager: LinkshellManager,
    pub relation_manager: RelationGroupManager,
    pub retainer_manager: RetainerGroupManager,

    /// zone id → handle of the zone server that currently owns it.
    zone_routing: Mutex<HashMap<u32, Arc<ZoneServerHandle>>>,
}

impl WorldMaster {
    pub fn new() -> Self {
        Self {
            party_manager: PartyManager::new(),
            linkshell_manager: LinkshellManager::new(),
            relation_manager: RelationGroupManager::new(),
            retainer_manager: RetainerGroupManager::new(),
            zone_routing: Mutex::new(HashMap::new()),
        }
    }

    pub async fn register_zone_server(&self, zone_id: u32, handle: Arc<ZoneServerHandle>) {
        self.zone_routing.lock().await.insert(zone_id, handle);
    }

    pub async fn zone_server_for(&self, zone_id: u32) -> Option<Arc<ZoneServerHandle>> {
        self.zone_routing.lock().await.get(&zone_id).cloned()
    }
}

impl Default for WorldMaster {
    fn default() -> Self {
        Self::new()
    }
}
