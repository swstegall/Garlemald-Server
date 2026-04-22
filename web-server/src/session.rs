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

//! 56-character session token generator. The lobby/world/map servers read
//! whatever string the client hands them back as a session id; the client
//! extracts it from the `ffxiv://login_success?sessionId=…` redirect and
//! hard-asserts `len == 56` in
//! `garlemald-client/src/login/webview.rs::parse_session_id`.
//!
//! 56 hex chars == 28 bytes == 224 bits of entropy, which is more than
//! enough for an offline-friendly private-server token.

use rand::RngCore;

pub const SESSION_ID_LEN: usize = 56;
const RANDOM_BYTES: usize = SESSION_ID_LEN / 2;

pub fn generate() -> String {
    let mut bytes = [0u8; RANDOM_BYTES];
    rand::rng().fill_bytes(&mut bytes);
    let mut s = String::with_capacity(SESSION_ID_LEN);
    for b in bytes {
        // hex-encode without pulling in the `hex` crate.
        s.push(nibble(b >> 4));
        s.push(nibble(b & 0x0f));
    }
    debug_assert_eq!(s.len(), SESSION_ID_LEN);
    s
}

fn nibble(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        _ => (b'a' + (n - 10)) as char,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_is_56_chars() {
        for _ in 0..32 {
            let id = generate();
            assert_eq!(id.len(), SESSION_ID_LEN);
            assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    #[test]
    fn session_ids_are_unique() {
        let a = generate();
        let b = generate();
        assert_ne!(a, b);
    }
}
