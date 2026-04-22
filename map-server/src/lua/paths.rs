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

//! Script path resolver. Ported from the `FILEPATH_*` constants in
//! `LuaEngine.cs`. Every function takes a script root and returns an
//! absolute path; callers decide whether the file exists.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct PathResolver {
    pub root: PathBuf,
}

impl PathResolver {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn player(&self) -> PathBuf {
        self.root.join("player.lua")
    }

    pub fn zone(&self, zone_name: &str) -> PathBuf {
        // C# `LuaEngine.GetLuaScriptPath` for `Area` targets returns
        // `./scripts/unique/{zoneName}/zone.lua`. In both Project Meteor's
        // `Data/scripts/` snapshot and our own `scripts/lua/`, the actual
        // on-disk `zone.lua` lives one level deeper — under
        // `unique/{zoneName}/PopulaceStandard/`. Prefer the flat path when
        // it exists (in case a zone has been promoted to the canonical
        // location) and fall back to the PopulaceStandard subdir so
        // `ocn0Battle02` (and every other tutorial/town/field zone) resolves.
        let flat = self.root.join(format!("unique/{zone_name}/zone.lua"));
        if flat.exists() {
            return flat;
        }
        self.root
            .join(format!("unique/{zone_name}/PopulaceStandard/zone.lua"))
    }

    pub fn npc(&self, zone_name: &str, class_name: &str, unique_id: &str) -> PathBuf {
        self.root
            .join(format!("unique/{zone_name}/{class_name}/{unique_id}.lua"))
    }

    pub fn npc_in_private_area(
        &self,
        zone_name: &str,
        area_name: &str,
        area_type: u32,
        class_name: &str,
        unique_id: &str,
    ) -> PathBuf {
        self.root.join(format!(
            "unique/{zone_name}/privatearea/{area_name}_{area_type}/{class_name}/{unique_id}.lua"
        ))
    }

    pub fn base_class(&self, class_path: &str) -> PathBuf {
        self.root.join(format!("base/{class_path}.lua"))
    }

    pub fn content(&self, content_name: &str) -> PathBuf {
        self.root.join(format!("content/{content_name}.lua"))
    }

    pub fn gm_command(&self, cmd: &str) -> PathBuf {
        self.root
            .join(format!("commands/gm/{}.lua", cmd.to_lowercase()))
    }

    pub fn battle_command(&self, folder: &str, command: &str) -> PathBuf {
        self.root.join(format!("commands/{folder}/{command}.lua"))
    }

    pub fn battle_command_default(&self, folder: &str) -> PathBuf {
        self.root.join(format!("commands/{folder}/default.lua"))
    }

    pub fn status_effect(&self, name: &str) -> PathBuf {
        self.root.join(format!("effects/{name}.lua"))
    }

    pub fn status_effect_default(&self) -> PathBuf {
        self.root.join("effects/default.lua")
    }

    pub fn director(&self, name: &str) -> PathBuf {
        self.root.join(format!("directors/{name}.lua"))
    }

    /// Quest scripts live under `quests/<first-3-chars-of-name>/<name>.lua`
    /// in the C# original; reproducing that prefix lookup here.
    pub fn quest(&self, quest_name: &str) -> PathBuf {
        let initial: String = quest_name.chars().take(3).collect();
        self.root.join(format!("quests/{initial}/{quest_name}.lua"))
    }

    pub fn exists(path: &Path) -> bool {
        path.exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quest_path_extracts_prefix() {
        let r = PathResolver::new("/srv");
        assert_eq!(
            r.quest("man0l0"),
            PathBuf::from("/srv/quests/man/man0l0.lua")
        );
    }

    #[test]
    fn gm_command_lowercases() {
        let r = PathResolver::new("/srv");
        assert_eq!(
            r.gm_command("WARP"),
            PathBuf::from("/srv/commands/gm/warp.lua")
        );
    }
}
