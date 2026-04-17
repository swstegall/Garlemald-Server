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
        let mask = if length >= 32 { u32::MAX } else { (1u32 << length) - 1 };
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
        let mask = if length >= 64 { u64::MAX } else { (1u64 << length) - 1 };
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
