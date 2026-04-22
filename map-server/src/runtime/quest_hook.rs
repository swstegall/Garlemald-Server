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

//! Quest hooks fireable from the runtime / battle layer.
//!
//! The processor (`map-server/src/processor.rs`) fires `onStart` /
//! `onFinish` / `onStateChange` / `onTalk` from its own command-
//! application pipeline, because those paths originate in packet
//! dispatch where the `PacketProcessor` has the full LuaCommand drain
//! helper (`apply_login_lua_command`).
//!
//! `onKillBNpc`, by contrast, triggers from mid-tick combat resolution
//! — `die_if_defender_fell` in `runtime/dispatcher.rs`. That path
//! doesn't own a `PacketProcessor` handle, so this module exposes a
//! free-function version that takes only `(ActorHandle, LuaEngine, …,
//! bnpc_class_id)`. Hook-emitted commands are drained through the
//! shared `runtime::quest_apply::apply_runtime_lua_commands` helper so
//! scripts can legitimately `player:AddExp(100)` / `quest:SetQuestFlag(5)`
//! on kill and have the side effects persist.

#![allow(dead_code)]

use std::sync::Arc;

use crate::database::Database;
use crate::lua::{LuaEngine, QuestHookArg, command::CommandQueue};
use crate::runtime::actor_registry::{ActorHandle, ActorRegistry};
use crate::world_manager::WorldManager;

/// Iterate the attacker's active quests and fire
/// `onKillBNpc(player, quest, bnpc_class_id)` for each. No-ops if:
///
/// * the attacker has no active quests,
/// * the quest id isn't in the `gamedata_quests` catalog (so there's
///   no className → no script),
/// * the script doesn't exist on disk, or
/// * the quest script doesn't define an `onKillBNpc` function.
///
/// Matches Meteor's "fire for every quest, let the script filter"
/// convention — scripts typically dispatch to a sub-handler keyed by
/// `bnpc_class_id` + `quest:GetSequence()`.
pub async fn fire_on_kill_bnpc(
    attacker: &ActorHandle,
    lua: &Arc<LuaEngine>,
    bnpc_class_id: u32,
    registry: &ActorRegistry,
    db: &Database,
    world: &WorldManager,
) {
    let (active_quest_ids, snapshot) = {
        let c = attacker.character.read().await;
        let ids: Vec<u32> = c
            .quest_journal
            .slots
            .iter()
            .flatten()
            .map(|q| q.quest_id())
            .collect();
        let snap = crate::lua::userdata::PlayerSnapshot {
            actor_id: c.base.actor_id,
            name: c.base.actor_name.clone(),
            zone_id: c.base.zone_id,
            pos: (c.base.position_x, c.base.position_y, c.base.position_z),
            rotation: c.base.rotation,
            state: c.base.current_main_state,
            hp: c.chara.hp,
            max_hp: c.chara.max_hp,
            mp: c.chara.mp,
            max_mp: c.chara.max_mp,
            tp: c.chara.tp,
            active_quests: ids.clone(),
            active_quest_states: c
                .quest_journal
                .slots
                .iter()
                .flatten()
                .map(|q| crate::lua::QuestStateSnapshot {
                    quest_id: q.quest_id(),
                    sequence: q.get_sequence(),
                    flags: q.get_flags(),
                    counters: [q.get_counter(0), q.get_counter(1), q.get_counter(2)],
                })
                .collect(),
            completed_quests: c.quest_journal.iter_completed().collect(),
            ..Default::default()
        };
        (ids, snap)
    };
    if active_quest_ids.is_empty() {
        return;
    }

    for quest_id in active_quest_ids {
        let Some(script_name) = lua.catalogs().quest_script_name(quest_id) else {
            continue;
        };
        let script_path = lua.resolver().quest(&script_name);
        if !script_path.exists() {
            continue;
        }

        // Rebuild the per-quest handle against a fresh snapshot slice.
        let (sequence, flags, counters) = snapshot
            .active_quest_states
            .iter()
            .find(|s| s.quest_id == quest_id)
            .map(|s| (s.sequence, s.flags, s.counters))
            .unwrap_or((0, 0, [0; 3]));
        let handle = crate::lua::LuaQuestHandle {
            player_id: snapshot.actor_id,
            quest_id,
            has_quest: true,
            sequence,
            flags,
            counters,
            queue: CommandQueue::new(),
        };

        let engine_clone = lua.clone();
        let snapshot_clone = snapshot.clone();
        let script_path_clone = script_path.clone();
        let bnpc_id_owned = bnpc_class_id;
        let result = tokio::task::spawn_blocking(move || {
            engine_clone.call_quest_hook(
                &script_path_clone,
                "onKillBNpc",
                snapshot_clone,
                handle,
                vec![QuestHookArg::Int(bnpc_id_owned as i64)],
            )
        })
        .await;

        let result = match result {
            Ok(r) => r,
            Err(join_err) => {
                tracing::warn!(
                    error = %join_err,
                    quest = quest_id,
                    "onKillBNpc dispatch panicked",
                );
                continue;
            }
        };
        if let Some(e) = result.error {
            tracing::debug!(
                error = %e,
                quest = quest_id,
                "onKillBNpc errored",
            );
        }
        if !result.commands.is_empty() {
            crate::runtime::quest_apply::apply_runtime_lua_commands(
                result.commands,
                registry,
                db,
                world,
                Some(lua),
            )
            .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::Character;
    use crate::actor::quest::{Quest, quest_actor_id};
    use crate::lua::LuaEngine;
    use crate::runtime::actor_registry::{ActorHandle, ActorKindTag};

    fn tmpdir() -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("garlemald-onkillbnpc-{nanos}"));
        std::fs::create_dir_all(dir.join("quests/man")).unwrap();
        dir
    }

    #[tokio::test]
    async fn fires_on_each_active_quest_with_bnpc_class_id() {
        let root = tmpdir();
        // The script loads into a cached per-script VM; setting a global
        // inside `onKillBNpc` lets the test read it back via `load_script`
        // after the hook runs (same VM, same global table).
        std::fs::write(
            root.join("quests/man/man0l0.lua"),
            r#"
                _last_kill_class_id = 0
                _last_kill_quest_sequence = -1
                function onKillBNpc(player, quest, classId)
                    _last_kill_class_id = classId
                    _last_kill_quest_sequence = quest:GetSequence()
                end
            "#,
        )
        .unwrap();

        // Catalog population — `fire_on_kill_bnpc` resolves quest id →
        // class name via `Catalogs::quest_script_name`. Without this the
        // function is a quiet no-op (matches the production behaviour
        // when `gamedata_quests` is missing).
        let lua = Arc::new(LuaEngine::new(&root));
        {
            let mut quests = std::collections::HashMap::new();
            quests.insert(
                110_001u32,
                crate::gamedata::QuestMeta {
                    id: 110_001,
                    quest_name: "Shapeless Melody".to_string(),
                    class_name: "Man0l0".to_string(),
                    prerequisite: 0,
                    min_level: 1,
                },
            );
            lua.catalogs().install_quests(quests);
        }

        // Register a Player actor with that quest parked at sequence 7.
        let mut character = Character::new(1);
        let mut quest = Quest::new(quest_actor_id(110_001), "Man0l0".to_string());
        quest.start_sequence(7);
        quest.clear_dirty();
        character.quest_journal.add(quest);
        let handle = ActorHandle::new(1, ActorKindTag::Player, 100, 42, character);

        // The hook-fire test doesn't exercise emitted-command drain
        // (the script just sets globals), so a dummy in-memory DB /
        // world / registry-of-one are enough.
        let registry = ActorRegistry::new();
        registry.insert(handle.clone()).await;
        let world = WorldManager::new();
        let db_path = std::env::temp_dir().join(format!(
            "garlemald-quest-hook-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let db = crate::database::Database::open(db_path).await.expect("db stub");

        fire_on_kill_bnpc(&handle, &lua, 1_000_438, &registry, &db, &world).await;

        // Reopen the per-script VM from the cache; its globals carry the
        // hook's side effects because the cache keeps the Lua instance
        // alive across calls.
        let script_path = root.join("quests/man/man0l0.lua");
        let (vm, _queue) = lua.load_script(&script_path).expect("reload script");
        let last_class: i64 = vm
            .globals()
            .get("_last_kill_class_id")
            .expect("global set by hook");
        let last_seq: i64 = vm
            .globals()
            .get("_last_kill_quest_sequence")
            .expect("global set by hook");
        assert_eq!(last_class, 1_000_438);
        assert_eq!(last_seq, 7);

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn no_op_when_attacker_has_no_active_quests() {
        // A fresh handle with an empty journal — the function should
        // return quickly without touching the lua engine.
        let character = Character::new(1);
        let handle = ActorHandle::new(1, ActorKindTag::Player, 100, 42, character);

        let root = tmpdir();
        let lua = Arc::new(LuaEngine::new(&root));
        // No quests installed in the catalog.

        let registry = ActorRegistry::new();
        registry.insert(handle.clone()).await;
        let world = WorldManager::new();
        let db_path = std::env::temp_dir().join(format!(
            "garlemald-quest-hook-empty-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let db = crate::database::Database::open(db_path).await.expect("db stub");

        fire_on_kill_bnpc(&handle, &lua, 999, &registry, &db, &world).await;
        // The assertion here is "no panic" — the function falls out of
        // the `active_quest_ids.is_empty()` branch before touching Lua.

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn emitted_add_exp_command_routes_through_runtime_drain() {
        use common::db::ConnCallExt;

        let root = tmpdir();
        // onKillBNpc awards 750 exp to the attacker's current class.
        std::fs::write(
            root.join("quests/man/man0l0.lua"),
            r#"
                function onKillBNpc(player, quest, classId)
                    player:AddExp(3, 750)
                end
            "#,
        )
        .unwrap();

        let lua = Arc::new(LuaEngine::new(&root));
        {
            let mut quests = std::collections::HashMap::new();
            quests.insert(
                110_001u32,
                crate::gamedata::QuestMeta {
                    id: 110_001,
                    quest_name: "Shapeless Melody".to_string(),
                    class_name: "Man0l0".to_string(),
                    prerequisite: 0,
                    min_level: 1,
                },
            );
            lua.catalogs().install_quests(quests);
        }

        let registry = ActorRegistry::new();
        let mut character = Character::new(7);
        let mut quest = Quest::new(quest_actor_id(110_001), "Man0l0".to_string());
        quest.clear_dirty();
        character.quest_journal.add(quest);
        let handle = ActorHandle::new(7, ActorKindTag::Player, 100, 42, character);
        registry.insert(handle.clone()).await;
        let world = WorldManager::new();

        let db_path = std::env::temp_dir().join(format!(
            "garlemald-quest-hook-drain-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let db = crate::database::Database::open(db_path).await.expect("db");
        // AddExp reaches into characters_class_exp — seed the row so the
        // UPDATE has a target.
        db.conn_for_test()
            .call_db(|c| {
                c.execute(
                    r"INSERT INTO characters (id, userId, slot, serverId, name)
                      VALUES (7, 0, 0, 0, 'Killer')",
                    [],
                )?;
                c.execute(
                    r"INSERT INTO characters_class_exp (characterId) VALUES (7)",
                    [],
                )?;
                c.execute(
                    r"INSERT INTO characters_class_levels (characterId) VALUES (7)",
                    [],
                )?;
                Ok(())
            })
            .await
            .unwrap();

        fire_on_kill_bnpc(&handle, &lua, 1_000_999, &registry, &db, &world).await;

        // Hook emitted AddExp → runtime drain → level-up rollover →
        // db.set_exp + db.set_level land. 750 SP on a fresh L1 GLA
        // crosses the 570 SP "1 → 2" threshold: final state is L2
        // with 180 SP carried over.
        let (gla_sp, gla_level) = db
            .conn_for_test()
            .call_db(|c| {
                let sp: i32 = c.query_row(
                    "SELECT gla FROM characters_class_exp WHERE characterId = 7",
                    [],
                    |r| r.get::<_, i32>(0),
                )?;
                let lvl: i32 = c
                    .query_row(
                        "SELECT gla FROM characters_class_levels WHERE characterId = 7",
                        [],
                        |r| r.get::<_, i32>(0),
                    )
                    .unwrap_or(0);
                Ok((sp, lvl))
            })
            .await
            .unwrap();
        assert_eq!(gla_sp, 180, "750 - 570 (1→2 threshold) = 180");
        assert_eq!(gla_level, 2, "level rolled over from 1 to 2");

        let _ = std::fs::remove_dir_all(root);
    }
}
