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
