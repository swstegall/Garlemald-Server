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

//! Global function registrars. These are attached to every newly-loaded
//! script so the bundled `scripts/global.lua` works out of the box.
//!
//! The C# `LoadGlobals` helper does the same thing: it wires up
//! `GetWorldManager`, `GetStaticActor(ById)`, `GetItemGamedata`, etc.

#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use mlua::{Function, Lua, MultiValue, Value};

use super::catalogs::Catalogs;
use super::command::CommandQueue;
use super::userdata::{LuaItemData, LuaWorldManager};

/// Install the global functions referenced by `scripts/global.lua`.
pub fn install_globals(
    lua: &Lua,
    queue: Arc<Mutex<CommandQueue>>,
    catalogs: Arc<Catalogs>,
) -> mlua::Result<()> {
    let globals = lua.globals();

    // GetWorldManager() → LuaWorldManager
    {
        let queue = queue.clone();
        let f = lua.create_function(move |_, _: ()| {
            Ok(LuaWorldManager {
                queue: queue.clone(),
            })
        })?;
        globals.set("GetWorldManager", f)?;
    }

    // GetLuaInstance() → LuaWorldManager (scripts call OnSignal on this)
    {
        let queue = queue.clone();
        let f = lua.create_function(move |_, _: ()| {
            Ok(LuaWorldManager {
                queue: queue.clone(),
            })
        })?;
        globals.set("GetLuaInstance", f)?;
    }

    // GetWorldMaster() — scripts expect an opaque actor reference; the
    // command layer doesn't need it yet, so we return nil. Installing the
    // symbol prevents `nil call` errors.
    {
        let f = lua.create_function(|_, _: ()| Ok(Value::Nil))?;
        globals.set("GetWorldMaster", f)?;
    }

    // GetStaticActor(name) / GetStaticActorById(id) — return the actor id
    // registered under that name, or nil if unknown.
    {
        let cats = catalogs.clone();
        let f = lua.create_function(move |_, name: String| {
            Ok(cats
                .static_actors
                .read()
                .ok()
                .and_then(|s| s.get(&name).copied()))
        })?;
        globals.set("GetStaticActor", f)?;
    }
    {
        let cats = catalogs.clone();
        let f = lua.create_function(move |_, id: u32| {
            let s = cats.static_actors.read().ok();
            Ok(s.and_then(|map| {
                map.iter()
                    .find_map(|(name, actor_id)| (*actor_id == id).then(|| name.clone()))
            }))
        })?;
        globals.set("GetStaticActorById", f)?;
    }

    // GetItemGamedata(id) → LuaItemData | nil
    {
        let cats = catalogs.clone();
        let f = lua.create_function(move |_, id: u32| {
            let items = cats.items.read().ok();
            Ok(items.and_then(|m| {
                m.get(&id).map(|item| LuaItemData {
                    id: item.id,
                    name: item.name.clone(),
                    stack_size: item.stack_size,
                    item_level: item.item_level,
                    equip_level: item.equip_level,
                    price: item.price,
                    icon: item.icon,
                    rarity: item.rarity,
                })
            }))
        })?;
        globals.set("GetItemGamedata", f)?;
    }

    // GetGuildleveGamedata(id) — returns a plain table of fields.
    {
        let cats = catalogs.clone();
        let f = lua.create_function(move |lua, id: u32| -> mlua::Result<Value> {
            let Some(gd) = cats
                .guildleves
                .read()
                .ok()
                .and_then(|m| m.get(&id).cloned())
            else {
                return Ok(Value::Nil);
            };
            let t = lua.create_table()?;
            t.set("id", gd.id)?;
            t.set("zoneId", gd.zone_id)?;
            t.set("name", gd.name)?;
            t.set("difficulty", gd.difficulty)?;
            t.set("leveType", gd.leve_type)?;
            t.set("rewardExp", gd.reward_exp)?;
            t.set("rewardGil", gd.reward_gil)?;
            Ok(Value::Table(t))
        })?;
        globals.set("GetGuildleveGamedata", f)?;
    }

    // `print` → tracing::debug (so scripts don't spam stdout).
    {
        let print_fn: Function = lua.create_function(|_, args: MultiValue| {
            let pieces: Vec<String> = args
                .into_iter()
                .map(|v| match v {
                    Value::String(s) => s.to_str().map(|c| c.to_string()).unwrap_or_default(),
                    Value::Integer(i) => i.to_string(),
                    Value::Number(n) => n.to_string(),
                    Value::Boolean(b) => b.to_string(),
                    Value::Nil => "nil".into(),
                    other => format!("{other:?}"),
                })
                .collect();
            tracing::debug!(target: "lua", "{}", pieces.join("\t"));
            Ok(())
        })?;
        globals.set("print", print_fn)?;
    }

    Ok(())
}
