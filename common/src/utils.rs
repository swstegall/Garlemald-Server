use std::fmt::Write as _;
use std::io::{self, Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::math::Vector3;

const HEX: &[u8; 16] = b"0123456789ABCDEF";

/// Canonical hex dump with offset column and printable-ASCII tail, matching
/// the C# `Utils.ByteArrayToHex` output shape.
pub fn byte_array_to_hex(bytes: &[u8], offset: usize, bytes_per_line: usize) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    let mut out = String::with_capacity(bytes.len() * 4);
    let mut i = 0;
    while i < bytes.len() {
        let h = i + offset;
        let _ = write!(out, "{h:08X}   ");

        for j in 0..bytes_per_line {
            if j > 0 && j & 7 == 0 {
                out.push(' ');
            }
            if i + j >= bytes.len() {
                out.push_str("   ");
            } else {
                let b = bytes[i + j];
                out.push(HEX[(b >> 4) as usize] as char);
                out.push(HEX[(b & 0xF) as usize] as char);
                out.push(' ');
            }
        }
        out.push(' ');
        for j in 0..bytes_per_line {
            if i + j >= bytes.len() {
                out.push(' ');
            } else {
                let b = bytes[i + j];
                out.push(if b < 32 { '.' } else { b as char });
            }
        }
        out.push('\n');
        i += bytes_per_line;
    }
    out.trim_end().to_owned()
}

pub fn unix_timestamp() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0)
}

pub fn millis_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub trait SwapEndian: Copy {
    fn swap_endian(self) -> Self;
}

impl SwapEndian for u16 {
    fn swap_endian(self) -> Self {
        self.swap_bytes()
    }
}
impl SwapEndian for u32 {
    fn swap_endian(self) -> Self {
        self.swap_bytes()
    }
}
impl SwapEndian for u64 {
    fn swap_endian(self) -> Self {
        self.swap_bytes()
    }
}
impl SwapEndian for i32 {
    fn swap_endian(self) -> Self {
        self.swap_bytes()
    }
}

/// Mutable ASCII hash used by the original client; reads 4 bytes at a time
/// backwards from the end of the string. Preserves the exact pointer walk
/// from the C# source so hashes remain identical.
pub fn murmur_hash2(key: &str, seed: u32) -> u32 {
    let data = key.as_bytes();
    const M: u32 = 0x5bd1e995;
    const R: u32 = 24;
    let mut len = key.len() as i64;
    let mut data_index = len - 4;
    let mut h = seed ^ key.len() as u32;

    while len >= 4 {
        h = h.wrapping_mul(M);

        let di = data_index as usize;
        let k_raw = i32::from_le_bytes([data[di], data[di + 1], data[di + 2], data[di + 3]]) as u32;

        let mut k = ((k_raw >> 24) & 0xff)
            | ((k_raw << 8) & 0x00ff_0000)
            | ((k_raw >> 8) & 0x0000_ff00)
            | ((k_raw << 24) & 0xff00_0000);

        k = k.wrapping_mul(M);
        k ^= k >> R;
        k = k.wrapping_mul(M);

        h ^= k;

        data_index -= 4;
        len -= 4;
    }

    let tail = len as usize;
    let data_len = data.len();
    match tail {
        3 => {
            h ^= (data[0] as u32) << 16;
            h ^= (data[data_len.saturating_sub(2)] as u32) << 8;
            h ^= data[data_len.saturating_sub(1)] as u32;
            h = h.wrapping_mul(M);
        }
        2 => {
            h ^= (data[data_len.saturating_sub(2)] as u32) << 8;
            h ^= data[data_len.saturating_sub(1)] as u32;
            h = h.wrapping_mul(M);
        }
        1 => {
            h ^= data[data_len.saturating_sub(1)] as u32;
            h = h.wrapping_mul(M);
        }
        _ => {}
    }

    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^= h >> 15;

    h
}

pub fn bool_array_to_binary_stream(array: &[bool]) -> Vec<u8> {
    let len = array.len();
    let out_len = len.div_ceil(8);
    let mut data = vec![0u8; out_len];
    let mut counter = 0;
    let mut i = 0;
    while i < len {
        for bit in 0..8 {
            if i + bit >= len {
                break;
            }
            data[counter] |= (array[i + bit] as u8) << (7 - bit);
        }
        counter += 1;
        i += 8;
    }
    data
}

pub fn to_string_base63(number: i32) -> String {
    const LOOKUP: &[u8] =
        b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let len = LOOKUP.len() as i32;
    let second = LOOKUP[(number / len) as usize] as char;
    let first = LOOKUP[(number % len) as usize] as char;
    let mut s = String::with_capacity(2);
    s.push(second);
    s.push(first);
    s
}

pub fn read_null_term_string<R: Read>(reader: &mut R, max_size: usize) -> io::Result<String> {
    let mut buf = vec![0u8; max_size];
    reader.read_exact(&mut buf)?;
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    Ok(String::from_utf8_lossy(&buf[..end]).into_owned())
}

pub fn write_null_term_string<W: Write>(
    writer: &mut W,
    value: &str,
    max_size: usize,
) -> io::Result<()> {
    let bytes = value.as_bytes();
    let n = bytes.len().min(max_size);
    writer.write_all(&bytes[..n])
}

/// 16-bit rotate, matching the quirky C# implementation used by the login
/// string codec. Note that for `bits == 0` the semantics are undefined in
/// the original; we preserve the `(value << 0) | (value >> 16)` behaviour.
pub fn rotate_left(value: u32, bits: u32) -> u32 {
    (value << bits) | (value >> (16 - bits))
}

pub fn rotate_right(value: u32, bits: u32) -> u32 {
    (value >> bits) | (value << (16 - bits))
}

pub fn ffxiv_login_string_decode(data: &[u8]) -> String {
    let mut result = String::new();
    let mut key = ((data[0] as u32) << 8) | data[1] as u32;
    let key2 = data[2] as u32;
    key = rotate_right(key, 1) & 0xFFFF;
    key = key.wrapping_sub(0x22AF);
    let key2 = key2 ^ key;
    key = rotate_right(key, 1) & 0xFFFF;
    key = key.wrapping_sub(0x22AF);
    let mut final_key = key;
    let k3 = data[3] as u32;
    let mut count = (key2 & 0xFF) << 8;
    let k3 = k3 ^ final_key;
    let k3 = k3 & 0xFF;
    count |= k3;

    let mut idx = 0usize;
    while count != 0 {
        let encrypted = data[4 + idx] as u32;
        final_key = rotate_right(final_key, 1) & 0xFFFF;
        final_key = final_key.wrapping_sub(0x22AF);
        let ch = encrypted ^ (final_key & 0xFF);
        result.push(ch as u8 as char);
        count -= 1;
        idx += 1;
    }

    result
}

pub fn ffxiv_login_string_encode(mut key: u32, text: &str) -> Vec<u8> {
    key &= 0xFFFF;
    let ascii = text.as_bytes();
    let mut result = vec![0u8; 4 + text.len()];
    let mut count: u32 = 0;
    while (count as usize) < text.len() {
        let idx = result.len() - count as usize - 1;
        let src_idx = ascii.len() - count as usize - 1;
        result[idx] = ascii[src_idx] ^ (key & 0xFF) as u8;
        key = key.wrapping_add(0x22AF) & 0xFFFF;
        key = rotate_left(key, 1) & 0xFFFF;
        count += 1;
    }

    let count = count ^ key;
    result[3] = (count & 0xFF) as u8;

    key = key.wrapping_add(0x22AF) & 0xFFFF;
    key = rotate_left(key, 1) & 0xFFFF;
    result[2] = (key & 0xFF) as u8;

    key = key.wrapping_add(0x22AF) & 0xFFFF;
    key = rotate_left(key, 1) & 0xFFFF;
    result[1] = (key & 0xFF) as u8;
    result[0] = ((key >> 8) & 0xFF) as u8;

    result
}

pub fn distance(lhs: Vector3, rhs: Vector3) -> f32 {
    if lhs == rhs {
        return 0.0;
    }
    distance_squared(lhs, rhs).sqrt()
}

pub fn distance_squared(lhs: Vector3, rhs: Vector3) -> f32 {
    let dx = lhs.x - rhs.x;
    let dy = lhs.y - rhs.y;
    let dz = lhs.z - rhs.z;
    dx * dx + dy * dy + dz * dz
}

pub fn xz_distance(x: f32, z: f32, x2: f32, z2: f32) -> f32 {
    if x == x2 && z == z2 {
        return 0.0;
    }
    xz_distance_squared(x, z, x2, z2).sqrt()
}

pub fn xz_distance_squared(x: f32, z: f32, x2: f32, z2: f32) -> f32 {
    let dx = x - x2;
    let dz = z - z2;
    dx * dx + dz * dz
}

pub fn xz_distance_vec(lhs: Vector3, rhs: Vector3) -> f32 {
    xz_distance(lhs.x, lhs.z, rhs.x, rhs.z)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base63_roundtrip_zero() {
        assert_eq!(to_string_base63(0), "00");
    }

    #[test]
    fn login_string_roundtrip() {
        let key = 0x1234;
        let encoded = ffxiv_login_string_encode(key, "hello");
        let decoded = ffxiv_login_string_decode(&encoded);
        assert_eq!(decoded, "hello");
    }

    #[test]
    fn distance_zero() {
        let a = Vector3::new(1.0, 2.0, 3.0);
        assert_eq!(distance(a, a), 0.0);
    }
}
