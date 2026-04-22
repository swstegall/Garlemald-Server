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

//! The original C# `Efficient{32,64}bitHashTable<T>` were hand-rolled open-
//! addressing tables written before `Dictionary<TKey, TValue>` became a hot
//! path optimization. In Rust, `HashMap<u32/u64, T>` is already a good fit,
//! so these wrappers exist only to preserve the call sites from the ports.

use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct Efficient32BitHashTable<T> {
    inner: HashMap<u32, T>,
}

impl<T> Efficient32BitHashTable<T> {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: HashMap::with_capacity(capacity),
        }
    }

    pub fn add(&mut self, key: u32, value: T) {
        self.inner.insert(key, value);
    }

    pub fn get(&self, key: u32) -> Option<&T> {
        self.inner.get(&key)
    }

    pub fn has(&self, key: u32) -> bool {
        self.inner.contains_key(&key)
    }

    pub fn count(&self) -> usize {
        self.inner.len()
    }
}

#[derive(Debug, Default)]
pub struct Efficient64BitHashTable<T> {
    inner: HashMap<u64, T>,
}

impl<T> Efficient64BitHashTable<T> {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: HashMap::with_capacity(capacity),
        }
    }

    pub fn add(&mut self, key: u64, value: T) {
        self.inner.insert(key, value);
    }

    pub fn get(&self, key: u64) -> Option<&T> {
        self.inner.get(&key)
    }

    pub fn has(&self, key: u64) -> bool {
        self.inner.contains_key(&key)
    }

    pub fn count(&self) -> usize {
        self.inner.len()
    }
}
