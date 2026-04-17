//! Project Meteor's non-standard Blowfish variant.
//!
//! The P and S boxes are NOT derived from the digits of pi (standard Blowfish);
//! they are fixed byte tables baked into the client. The key schedule also
//! differs in one subtle way: key bytes are sign-extended to i32 before being
//! OR'd into the accumulator, meaning keys with bytes >= 0x80 produce a
//! different schedule than stock Blowfish. Both quirks are preserved here so
//! the server can decrypt traffic from the original 1.23b client.

use crate::error::PacketError;

mod blowfish_tables {
    include!("blowfish_tables.rs");
}

use blowfish_tables::{P_VALUES, S_VALUES};

const N: usize = 16;

pub struct Blowfish {
    p: [u32; N + 2],
    s: [[u32; 256]; 4],
}

fn load_u32_le(bytes: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]])
}

impl Blowfish {
    pub fn new(key: &[u8]) -> Self {
        let mut bf = Blowfish {
            p: [0u32; N + 2],
            s: [[0u32; 256]; 4],
        };
        bf.initialize(key);
        bf
    }

    fn initialize(&mut self, key: &[u8]) {
        for i in 0..(N + 2) {
            self.p[i] = load_u32_le(&P_VALUES, i * 4);
        }
        for i in 0..4 {
            for j in 0..256 {
                self.s[i][j] = load_u32_le(&S_VALUES, (i * 256 + j) * 4);
            }
        }

        let mut j = 0usize;
        for i in 0..(N + 2) {
            let mut data: u32 = 0;
            for _ in 0..4 {
                // C# did `(data << 8) | (SByte)key[j]`, which sign-extends the
                // key byte to an i32 before the OR. Reproduce that so the
                // schedule matches bit-for-bit.
                let signed = key[j] as i8 as i32 as u32;
                data = data.wrapping_shl(8) | signed;
                j = (j + 1) % key.len();
            }
            self.p[i] ^= data;
        }

        let mut datal: u32 = 0;
        let mut datar: u32 = 0;
        for i in (0..(N + 2)).step_by(2) {
            self.encipher_block(&mut datal, &mut datar);
            self.p[i] = datal;
            self.p[i + 1] = datar;
        }

        for i in 0..4 {
            for jj in (0..256).step_by(2) {
                self.encipher_block(&mut datal, &mut datar);
                self.s[i][jj] = datal;
                self.s[i][jj + 1] = datar;
            }
        }
    }

    #[inline]
    fn f(&self, mut x: u32) -> u32 {
        let d = (x & 0xFF) as usize;
        x >>= 8;
        let c = (x & 0xFF) as usize;
        x >>= 8;
        let b = (x & 0xFF) as usize;
        x >>= 8;
        let a = (x & 0xFF) as usize;
        let mut y = self.s[0][a].wrapping_add(self.s[1][b]);
        y ^= self.s[2][c];
        y = y.wrapping_add(self.s[3][d]);
        y
    }

    fn encipher_block(&self, xl: &mut u32, xr: &mut u32) {
        for i in 0..N {
            *xl ^= self.p[i];
            *xr ^= self.f(*xl);
            std::mem::swap(xl, xr);
        }
        std::mem::swap(xl, xr);
        *xr ^= self.p[N];
        *xl ^= self.p[N + 1];
    }

    fn decipher_block(&self, xl: &mut u32, xr: &mut u32) {
        for i in (2..=N + 1).rev() {
            *xl ^= self.p[i];
            *xr ^= self.f(*xl);
            std::mem::swap(xl, xr);
        }
        std::mem::swap(xl, xr);
        *xr ^= self.p[1];
        *xl ^= self.p[0];
    }

    pub fn encipher(&self, data: &mut [u8], offset: usize, length: usize) -> Result<(), PacketError> {
        if !length.is_multiple_of(8) {
            return Err(PacketError::BlowfishBlockMisaligned(length));
        }
        let end = offset + length;
        let mut i = offset;
        while i < end {
            let mut xl = load_u32_le(data, i);
            let mut xr = load_u32_le(data, i + 4);
            self.encipher_block(&mut xl, &mut xr);
            data[i..i + 4].copy_from_slice(&xl.to_le_bytes());
            data[i + 4..i + 8].copy_from_slice(&xr.to_le_bytes());
            i += 8;
        }
        Ok(())
    }

    pub fn decipher(&self, data: &mut [u8], offset: usize, length: usize) -> Result<(), PacketError> {
        if !length.is_multiple_of(8) {
            return Err(PacketError::BlowfishBlockMisaligned(length));
        }
        let end = offset + length;
        let mut i = offset;
        while i < end {
            let mut xl = load_u32_le(data, i);
            let mut xr = load_u32_le(data, i + 4);
            self.decipher_block(&mut xl, &mut xr);
            data[i..i + 4].copy_from_slice(&xl.to_le_bytes());
            data[i + 4..i + 8].copy_from_slice(&xr.to_le_bytes());
            i += 8;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_simple_key() {
        let bf = Blowfish::new(b"deadbeef");
        let mut buf = b"abcdefgh_ABCDEFGH".to_vec();
        buf.extend_from_slice(&[0u8; 7]);
        let len = 16;
        let original = buf[..len].to_vec();
        bf.encipher(&mut buf, 0, len).unwrap();
        assert_ne!(&buf[..len], &original[..]);
        bf.decipher(&mut buf, 0, len).unwrap();
        assert_eq!(&buf[..len], &original[..]);
    }

    #[test]
    fn round_trip_high_bit_key() {
        // Verifies sign-extension path by using key bytes with the high bit set.
        let bf = Blowfish::new(&[0x80, 0xFF, 0x7F, 0x01, 0x00, 0xAA, 0x55, 0xCC]);
        let original: [u8; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let mut buf = original.to_vec();
        bf.encipher(&mut buf, 0, 16).unwrap();
        assert_ne!(buf, original);
        bf.decipher(&mut buf, 0, 16).unwrap();
        assert_eq!(buf, original);
    }

    #[test]
    fn rejects_misaligned_length() {
        let bf = Blowfish::new(b"xxxxxxxx");
        let mut buf = vec![0u8; 16];
        assert!(bf.encipher(&mut buf, 0, 7).is_err());
    }
}
