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

//! Tiny helper that replaces the C# reflection-based `PrimitiveConversion`
//! from `Bitfield.cs`. Instead of attribute-driven field walking, callers
//! describe `(value, bit_length)` tuples and the helper packs them LSB-first.
//!
//! The original C# code ONLY ever packed bitfields (no unpacking), so we
//! provide the same one-way shape. Idiomatic Rust callers will typically
//! just use plain `u32` shifts directly.

pub fn pack_u32(fields: &[(u32, u8)]) -> u32 {
    let mut r: u32 = 0;
    let mut offset: u32 = 0;
    for &(value, length) in fields {
        let length = length as u32;
        let mask = if length >= 32 {
            u32::MAX
        } else {
            (1u32 << length) - 1
        };
        r |= (value & mask) << offset;
        offset += length;
    }
    r
}

pub fn pack_u64(fields: &[(u64, u8)]) -> u64 {
    let mut r: u64 = 0;
    let mut offset: u32 = 0;
    for &(value, length) in fields {
        let length = length as u32;
        let mask = if length >= 64 {
            u64::MAX
        } else {
            (1u64 << length) - 1
        };
        r |= (value & mask) << offset;
        offset += length;
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_sequential() {
        assert_eq!(pack_u32(&[(0b101, 3), (0b11, 2)]), 0b11_101);
    }

    #[test]
    fn truncates_to_length() {
        assert_eq!(pack_u32(&[(0xFF, 4)]), 0x0F);
    }
}
