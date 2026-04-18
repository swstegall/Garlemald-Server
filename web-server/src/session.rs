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
