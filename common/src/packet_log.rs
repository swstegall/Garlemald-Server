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

//! Raw packet hex-dump logger.
//!
//! Each server binary calls [`init`] once at boot. If the environment
//! variable `GARLEMALD_PACKET_LOG_DIR` is set, that directory receives one
//! append-only file per service (`{tag}-packets.log`) containing every
//! inbound and outbound byte sequence with subsecond timestamps, direction,
//! and peer address. When the env var is unset, [`log_inbound`] and
//! [`log_outbound`] degrade to a single atomic load with no allocation.
//!
//! This exists to debug client/server protocol divergence — in particular
//! the "Now Loading" stall after character creation in the FFXIV 1.23b
//! login flow. Format is human-readable so it tails cleanly alongside the
//! existing per-service tracing logs.

use std::fmt::Write as _;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const ENV_VAR: &str = "GARLEMALD_PACKET_LOG_DIR";

static LOGGER: OnceLock<Option<PacketLogger>> = OnceLock::new();

pub enum Direction {
    Inbound,
    Outbound,
}

impl Direction {
    fn arrow(&self) -> &'static str {
        match self {
            Direction::Inbound => "<-",
            Direction::Outbound => "->",
        }
    }
}

struct PacketLogger {
    tag: &'static str,
    file: Mutex<BufWriter<File>>,
    path: PathBuf,
}

/// Initialise the packet logger for the calling service. `tag` is the
/// same short service identifier passed to [`crate::logging::init`] —
/// surrounding brackets and padding whitespace are stripped so the on-disk
/// filename stays tidy.
///
/// Safe to call at most once per process. Subsequent calls are ignored.
pub fn init(tag: &'static str) {
    LOGGER.get_or_init(|| build_logger(tag));
}

fn build_logger(tag: &'static str) -> Option<PacketLogger> {
    let dir = std::env::var_os(ENV_VAR)?;
    let dir = PathBuf::from(dir);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        tracing::warn!(error = %e, path = %dir.display(), "packet log dir create failed; disabled");
        return None;
    }

    let slug = slug(tag);
    let path = dir.join(format!("{slug}-packets.log"));
    let file = match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(), "packet log open failed; disabled");
            return None;
        }
    };

    tracing::info!(path = %path.display(), "packet logging enabled");
    Some(PacketLogger {
        tag,
        file: Mutex::new(BufWriter::new(file)),
        path,
    })
}

fn slug(tag: &str) -> String {
    tag.trim()
        .trim_matches(|c| c == '[' || c == ']')
        .trim()
        .to_ascii_lowercase()
}

pub fn log_inbound(peer: SocketAddr, bytes: &[u8]) {
    log(Direction::Inbound, PeerRepr::Addr(peer), bytes);
}

pub fn log_outbound(peer: SocketAddr, bytes: &[u8]) {
    log(Direction::Outbound, PeerRepr::Addr(peer), bytes);
}

/// Variant for peers identified by a human string (e.g. the outbound
/// connection the world server holds to a zone/map server, where we know
/// `ip:port` at connect time but don't carry a `SocketAddr` around).
pub fn log_inbound_named(peer: &str, bytes: &[u8]) {
    log(Direction::Inbound, PeerRepr::Named(peer), bytes);
}

pub fn log_outbound_named(peer: &str, bytes: &[u8]) {
    log(Direction::Outbound, PeerRepr::Named(peer), bytes);
}

enum PeerRepr<'a> {
    Addr(SocketAddr),
    Named(&'a str),
}

fn log(direction: Direction, peer: PeerRepr<'_>, bytes: &[u8]) {
    let Some(Some(logger)) = LOGGER.get() else {
        return;
    };
    if bytes.is_empty() {
        return;
    }
    let mut buf = String::with_capacity(bytes.len() * 4 + 96);
    let ts = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ");
    let arrow = direction.arrow();
    match peer {
        PeerRepr::Addr(addr) => {
            let _ = writeln!(
                buf,
                "{} {} {} {} ({} bytes)",
                logger.tag,
                ts,
                arrow,
                addr,
                bytes.len()
            );
        }
        PeerRepr::Named(name) => {
            let _ = writeln!(
                buf,
                "{} {} {} {} ({} bytes)",
                logger.tag,
                ts,
                arrow,
                name,
                bytes.len()
            );
        }
    }
    append_hex_dump(&mut buf, bytes);

    // Serialise writes — packet logging is best-effort and not on any hot
    // path. If another thread poisoned the mutex we drop the record rather
    // than unwind.
    let Ok(mut file) = logger.file.lock() else {
        return;
    };
    if let Err(e) = file.write_all(buf.as_bytes()) {
        tracing::warn!(error = %e, path = %logger.path.display(), "packet log write failed");
    }
    let _ = file.flush();
}

/// Canonical `xxd`-style hex + printable ASCII pane, 16 bytes per row.
fn append_hex_dump(out: &mut String, bytes: &[u8]) {
    for (row_idx, chunk) in bytes.chunks(16).enumerate() {
        let _ = write!(out, "  {:08x}:", row_idx * 16);
        for (i, b) in chunk.iter().enumerate() {
            if i == 8 {
                out.push(' ');
            }
            let _ = write!(out, " {b:02x}");
        }
        let pad = (16 - chunk.len()) * 3 + if chunk.len() <= 8 { 1 } else { 0 };
        for _ in 0..pad {
            out.push(' ');
        }
        out.push_str("  |");
        for b in chunk {
            let c = *b;
            if (0x20..0x7f).contains(&c) {
                out.push(c as char);
            } else {
                out.push('.');
            }
        }
        out.push_str("|\n");
    }
}

/// Returns the log file path, if logging is active. Intended for tests
/// and diagnostic logging — callers should not hold this across the
/// lifetime of the process since the file may be rotated externally.
#[allow(dead_code)]
pub fn log_path() -> Option<&'static Path> {
    LOGGER.get().and_then(|o| o.as_ref()).map(|l| l.path.as_path())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_dump_full_row() {
        let mut s = String::new();
        append_hex_dump(&mut s, b"Hello, world!\x00\x01\x02");
        assert!(s.contains("Hello, world!"));
        assert!(s.contains("48 65 6c 6c 6f"));
    }

    #[test]
    fn hex_dump_short_row() {
        let mut s = String::new();
        append_hex_dump(&mut s, b"abc");
        assert!(s.contains("61 62 63"));
        assert!(s.contains("|abc|"));
    }

    #[test]
    fn slug_strips_brackets() {
        assert_eq!(slug("[LOBBY]"), "lobby");
        assert_eq!(slug("[MAP]  "), "map");
        assert_eq!(slug("[WORLD]"), "world");
    }
}
