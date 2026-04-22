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

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PacketError {
    #[error("packet too small: need {needed} bytes, have {have}")]
    TooSmall { needed: usize, have: usize },

    #[error("declared packet size {declared} does not match available data {available}")]
    SizeMismatch { declared: usize, available: usize },

    #[error("blowfish input length {0} is not a multiple of 8")]
    BlowfishBlockMisaligned(usize),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
