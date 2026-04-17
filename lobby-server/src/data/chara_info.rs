//! Parsing / encoding helpers for CharaInfo. The wire format is a hand-rolled
//! binary blob that the client base64-encodes in the CharacterModify packet.

use std::io::{Cursor, Seek, SeekFrom, Write};

use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use super::{get_tribe_model, Appearance, CharaInfo, Character, FaceInfo};

pub fn parse_new_char_request(encoded: &str) -> Result<CharaInfo> {
    // Mirror the C# `encoded.Replace('-', '+').Replace('_', '/')` URL-safe
    // → standard base64 swap before decoding.
    let canonical: String = encoded.replace('-', "+").replace('_', "/");
    let trimmed = canonical.trim_end_matches('\0');
    let data = BASE64.decode(trimmed.as_bytes())?;

    let mut info = CharaInfo::default();
    let mut appearance = Appearance::default();
    let mut c = Cursor::new(&data[..]);

    let _version = c.read_u32::<LittleEndian>()?;
    let _unknown1 = c.read_u32::<LittleEndian>()?;
    info.tribe = c.read_u8()? as u32;
    appearance.size = c.read_u8()?;
    appearance.hair_style = c.read_u16::<LittleEndian>()?;
    appearance.hair_highlight_color = c.read_u8()? as u16;
    appearance.hair_variation = c.read_u8()? as u16;
    appearance.face_type = c.read_u8()?;
    appearance.characteristics = c.read_u8()?;
    appearance.characteristics_color = c.read_u8()?;

    let _ = c.read_u32::<LittleEndian>()?;

    appearance.face_eyebrows = c.read_u8()?;
    appearance.face_iris_size = c.read_u8()?;
    appearance.face_eye_shape = c.read_u8()?;
    appearance.face_nose = c.read_u8()?;
    appearance.face_features = c.read_u8()?;
    appearance.face_mouth = c.read_u8()?;
    appearance.ears = c.read_u8()?;
    appearance.hair_color = c.read_u16::<LittleEndian>()?;

    let _ = c.read_u32::<LittleEndian>()?;

    appearance.skin_color = c.read_u16::<LittleEndian>()?;
    appearance.eye_color = c.read_u16::<LittleEndian>()?;

    appearance.voice = c.read_u8()?;
    info.guardian = c.read_u8()? as u32;
    info.birth_month = c.read_u8()? as u32;
    info.birth_day = c.read_u8()? as u32;
    info.current_class = c.read_u16::<LittleEndian>()? as u32;

    let _ = c.read_u32::<LittleEndian>()?;
    let _ = c.read_u32::<LittleEndian>()?;
    let _ = c.read_u32::<LittleEndian>()?;

    c.seek(SeekFrom::Current(0x10))?;

    info.initial_town = c.read_u8()? as u32;
    info.appearance = appearance;

    Ok(info)
}

/// Produce the url-safe base64 appearance blob used by CharacterListPacket.
///
/// The original C# helper reads `memStream.GetBuffer()` (capacity 0x3B0) and
/// base64-encodes the full backing buffer, padded with zeros. We replicate
/// that by pre-allocating a fixed-size zeroed buffer and writing over it.
pub fn build_for_chara_list(chara: &Character, appearance: &Appearance) -> String {
    const BUF_SIZE: usize = 0x3B0;
    let mut buf = vec![0u8; BUF_SIZE];

    {
        let mut c = Cursor::new(&mut buf[..]);
        let face = FaceInfo {
            characteristics: appearance.characteristics as u32,
            characteristics_color: appearance.characteristics_color as u32,
            face_type: appearance.face_type as u32,
            ears: appearance.ears as u32,
            features: appearance.face_features as u32,
            eyebrows: appearance.face_eyebrows as u32,
            eye_shape: appearance.face_eye_shape as u32,
            iris_size: appearance.face_iris_size as u32,
            mouth: appearance.face_mouth as u32,
            nose: appearance.face_nose as u32,
            unknown: 0,
        };

        let location1 = b"prv0Inn01\0";
        let location2 = b"defaultTerritory\0";

        c.write_u32::<LittleEndian>(0x000004c0).unwrap();
        c.write_u32::<LittleEndian>(0x232327ea).unwrap();
        let name_bytes = {
            let mut v = chara.name.as_bytes().to_vec();
            v.push(0);
            v
        };
        c.write_u32::<LittleEndian>(name_bytes.len() as u32).unwrap();
        c.write_all(&name_bytes).unwrap();
        c.write_u32::<LittleEndian>(0x1c).unwrap();
        c.write_u32::<LittleEndian>(0x04).unwrap();
        c.write_u32::<LittleEndian>(get_tribe_model(chara.tribe)).unwrap();
        c.write_u32::<LittleEndian>(appearance.size as u32).unwrap();

        let color_val = appearance.skin_color as u32
            | ((appearance.hair_color as u32) << 10)
            | ((appearance.eye_color as u32) << 20);
        c.write_u32::<LittleEndian>(color_val).unwrap();

        c.write_u32::<LittleEndian>(face.to_u32()).unwrap();

        let hair_val = appearance.hair_highlight_color as u32
            | ((appearance.hair_variation as u32) << 5)
            | ((appearance.hair_style as u32) << 10);
        c.write_u32::<LittleEndian>(hair_val).unwrap();
        c.write_u32::<LittleEndian>(appearance.voice as u32).unwrap();
        c.write_u32::<LittleEndian>(appearance.main_hand).unwrap();
        c.write_u32::<LittleEndian>(appearance.off_hand).unwrap();

        for _ in 0..5 {
            c.write_u32::<LittleEndian>(0).unwrap();
        }

        c.write_u32::<LittleEndian>(appearance.head).unwrap();
        c.write_u32::<LittleEndian>(appearance.body).unwrap();
        c.write_u32::<LittleEndian>(appearance.legs).unwrap();
        c.write_u32::<LittleEndian>(appearance.hands).unwrap();
        c.write_u32::<LittleEndian>(appearance.feet).unwrap();
        c.write_u32::<LittleEndian>(appearance.waist).unwrap();

        c.write_u32::<LittleEndian>(appearance.neck).unwrap();
        c.write_u32::<LittleEndian>(appearance.right_ear).unwrap();
        c.write_u32::<LittleEndian>(appearance.left_ear).unwrap();
        c.write_u32::<LittleEndian>(appearance.right_index).unwrap();
        c.write_u32::<LittleEndian>(appearance.left_index).unwrap();
        c.write_u32::<LittleEndian>(appearance.right_finger).unwrap();
        c.write_u32::<LittleEndian>(appearance.left_finger).unwrap();

        for _ in 0..8 {
            c.write_u8(0).unwrap();
        }

        c.write_u32::<LittleEndian>(1).unwrap();
        c.write_u32::<LittleEndian>(1).unwrap();

        c.write_u8(chara.current_class as u8).unwrap();
        c.write_u16::<LittleEndian>(chara.current_level as u16).unwrap();
        c.write_u8(chara.current_job as u8).unwrap();
        c.write_u16::<LittleEndian>(1).unwrap();
        c.write_u8(chara.tribe).unwrap();

        c.write_u32::<LittleEndian>(0xe22222aa).unwrap();

        c.write_u32::<LittleEndian>(location1.len() as u32).unwrap();
        c.write_all(location1).unwrap();
        c.write_u32::<LittleEndian>(location2.len() as u32).unwrap();
        c.write_all(location2).unwrap();

        c.write_u8(chara.guardian).unwrap();
        c.write_u8(chara.birth_month).unwrap();
        c.write_u8(chara.birth_day).unwrap();

        c.write_u16::<LittleEndian>(0x17).unwrap();
        c.write_u32::<LittleEndian>(4).unwrap();
        c.write_u32::<LittleEndian>(4).unwrap();

        c.seek(SeekFrom::Current(0x10)).unwrap();

        c.write_u32::<LittleEndian>(chara.initial_town as u32).unwrap();
        c.write_u32::<LittleEndian>(chara.initial_town as u32).unwrap();
    }

    // URL-safe base64 variant: + → -, / → _.
    let encoded = BASE64.encode(&buf);
    encoded.replace('+', "-").replace('/', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_for_chara_list_is_stable_shape() {
        let chara = Character { name: "Test".to_string(), tribe: 2, ..Default::default() };
        let out = build_for_chara_list(&chara, &Appearance::default());
        // base64 of 0x3B0 (944) bytes is 1260 chars including padding ('=' since
        // 944 is not a multiple of 3 → 944 mod 3 == 2 → 2 padding chars).
        // 944 * 4 / 3 = 1258.66 → 1260 chars.
        assert_eq!(out.len(), 1260);
        assert!(!out.contains('+'));
        assert!(!out.contains('/'));
    }

    #[test]
    fn parse_accepts_url_safe_base64() {
        // Build a minimal blob: version(4) + unknown(4) + tribe=2 + size=5 + ...
        // Then ensure parse_new_char_request can round-trip a URL-safe encoding.
        let mut buf = vec![0u8; 0x60];
        buf[0..4].copy_from_slice(&1u32.to_le_bytes());
        buf[8] = 2; // tribe
        buf[9] = 5; // size
        let raw = BASE64.encode(&buf).replace('+', "-").replace('/', "_");
        let info = parse_new_char_request(&raw).unwrap();
        assert_eq!(info.tribe, 2);
        assert_eq!(info.appearance.size, 5);
    }
}
