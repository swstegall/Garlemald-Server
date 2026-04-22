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

//! `Bitstream2048` — 256-byte dense bitfield. Port of Meteor's
//! `Common Class Lib/Bitstream.cs` sized to the 2048-bit quest-completion
//! space introduced by `ioncannon/quest_system`.
//!
//! Bit layout matches the C# source exactly: bit `at` lives in
//! `bytes[at / 8]` at shift `at % 8` (LSB-first inside the byte), so wire
//! and DB representations stay interoperable with a running Meteor
//! instance or a dumped VARBINARY column.

/// Number of bits in the 2048-bit completion bitfield.
pub const BITSTREAM_BITS: usize = 2048;
/// Number of bytes (`BITSTREAM_BITS / 8`) — fits the MySQL `VARBINARY(2048)`
/// column exactly.
pub const BITSTREAM_BYTES: usize = BITSTREAM_BITS / 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitstream2048 {
    data: [u8; BITSTREAM_BYTES],
}

impl Default for Bitstream2048 {
    fn default() -> Self {
        Self::new()
    }
}

impl Bitstream2048 {
    /// All bits cleared.
    pub const fn new() -> Self {
        Self {
            data: [0u8; BITSTREAM_BYTES],
        }
    }

    /// All bits set (`SetAll(true)`).
    pub const fn all_set() -> Self {
        Self {
            data: [0xFFu8; BITSTREAM_BYTES],
        }
    }

    /// Hydrate from a raw byte slice. Accepts any length up to
    /// [`BITSTREAM_BYTES`]; shorter inputs zero-pad the tail.
    pub fn from_slice(bytes: &[u8]) -> Self {
        let mut out = Self::new();
        let n = bytes.len().min(BITSTREAM_BYTES);
        out.data[..n].copy_from_slice(&bytes[..n]);
        out
    }

    /// Borrow the backing bytes for DB I/O.
    pub const fn as_bytes(&self) -> &[u8; BITSTREAM_BYTES] {
        &self.data
    }

    /// `SetAll(bool)`.
    pub fn set_all(&mut self, to: bool) {
        let fill = if to { 0xFF } else { 0x00 };
        self.data.fill(fill);
    }

    /// `Get(at)` — `at` is a bit index in `[0, BITSTREAM_BITS)`.
    ///
    /// Out-of-range indices return `false` instead of panicking so callers
    /// can pass arbitrary quest ids (which, in practice, are always
    /// `110_001..=112_048` and fit).
    pub fn get(&self, at: usize) -> bool {
        if at >= BITSTREAM_BITS {
            return false;
        }
        let byte_pos = at / 8;
        let bit_pos = at % 8;
        (self.data[byte_pos] & (1u8 << bit_pos)) != 0
    }

    /// `Set(at)` — no-op if `at >= BITSTREAM_BITS`.
    pub fn set(&mut self, at: usize) {
        if at >= BITSTREAM_BITS {
            return;
        }
        let byte_pos = at / 8;
        let bit_pos = at % 8;
        self.data[byte_pos] |= 1u8 << bit_pos;
    }

    /// `Clear(at)` — no-op if `at >= BITSTREAM_BITS`.
    pub fn clear(&mut self, at: usize) {
        if at >= BITSTREAM_BITS {
            return;
        }
        let byte_pos = at / 8;
        let bit_pos = at % 8;
        self.data[byte_pos] &= !(1u8 << bit_pos);
    }

    /// Count of set bits across the whole bitstream.
    pub fn count_ones(&self) -> u32 {
        self.data.iter().map(|b| b.count_ones()).sum()
    }

    /// Iterator over every set bit index.
    pub fn iter_set(&self) -> impl Iterator<Item = usize> + '_ {
        self.data.iter().enumerate().flat_map(|(byte_pos, byte)| {
            (0..8).filter_map(move |bit_pos| {
                if (byte & (1u8 << bit_pos)) != 0 {
                    Some(byte_pos * 8 + bit_pos)
                } else {
                    None
                }
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_all_zero() {
        let b = Bitstream2048::new();
        assert_eq!(b.count_ones(), 0);
        assert!(!b.get(0));
        assert!(!b.get(BITSTREAM_BITS - 1));
    }

    #[test]
    fn set_and_get_roundtrip() {
        let mut b = Bitstream2048::new();
        b.set(0);
        b.set(7);
        b.set(8);
        b.set(2047);
        assert!(b.get(0));
        assert!(b.get(7));
        assert!(b.get(8));
        assert!(b.get(2047));
        assert!(!b.get(1));
        assert!(!b.get(2046));
        assert_eq!(b.count_ones(), 4);
    }

    #[test]
    fn clear_clears_only_target_bit() {
        let mut b = Bitstream2048::all_set();
        b.clear(100);
        assert!(!b.get(100));
        assert!(b.get(99));
        assert!(b.get(101));
        assert_eq!(b.count_ones(), (BITSTREAM_BITS - 1) as u32);
    }

    #[test]
    fn out_of_range_is_noop() {
        let mut b = Bitstream2048::new();
        b.set(BITSTREAM_BITS);
        b.set(BITSTREAM_BITS + 1);
        assert_eq!(b.count_ones(), 0);
        assert!(!b.get(BITSTREAM_BITS));
        b.clear(BITSTREAM_BITS + 100);
    }

    #[test]
    fn wire_layout_matches_csharp() {
        // C# semantics: bit `at` is (data[at/8] & (1 << (at%8))) != 0.
        // Set bits 0, 1, and 9 → byte 0 = 0b0000_0011 (0x03), byte 1 = 0b0000_0010 (0x02).
        let mut b = Bitstream2048::new();
        b.set(0);
        b.set(1);
        b.set(9);
        let bytes = b.as_bytes();
        assert_eq!(bytes[0], 0x03);
        assert_eq!(bytes[1], 0x02);
        assert!(bytes[2..].iter().all(|&x| x == 0));
    }

    #[test]
    fn from_slice_accepts_short_and_full() {
        let short = [0x81u8, 0x02];
        let b = Bitstream2048::from_slice(&short);
        assert!(b.get(0));
        assert!(b.get(7));
        assert!(b.get(9));
        assert!(!b.get(1));

        let full = [0xAAu8; BITSTREAM_BYTES];
        let b2 = Bitstream2048::from_slice(&full);
        for i in 0..BITSTREAM_BITS {
            assert_eq!(b2.get(i), i % 2 == 1, "bit {i} mismatch");
        }
    }

    #[test]
    fn iter_set_yields_bit_indices() {
        let mut b = Bitstream2048::new();
        for idx in [0, 3, 9, 100, 2047] {
            b.set(idx);
        }
        let collected: Vec<usize> = b.iter_set().collect();
        assert_eq!(collected, vec![0, 3, 9, 100, 2047]);
    }

    #[test]
    fn set_all_toggles_every_bit() {
        let mut b = Bitstream2048::new();
        b.set_all(true);
        assert_eq!(b.count_ones(), BITSTREAM_BITS as u32);
        b.set_all(false);
        assert_eq!(b.count_ones(), 0);
    }
}
