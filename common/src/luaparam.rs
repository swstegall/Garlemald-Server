//! FFXIV 1.23b Lua parameter serialization, ported from
//! `World Server/DataObjects/LuaUtils.cs` + `LuaParam.cs`.
//!
//! Each param is a 1-byte type tag followed by big-endian-encoded payload
//! bytes (with a couple of exceptions noted inline). Type 0xF marks the end
//! of the stream. The tags are used by both the game message packet and the
//! group-work sync packets, so they live in the common crate.

use std::io::{Cursor, Read, Write};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

#[derive(Debug, Clone, PartialEq)]
pub enum LuaParam {
    /// 0x0 — signed int32.
    Int32(i32),
    /// 0x1 — unsigned int32 (encoded with type byte 0x0 on the wire, per the
    /// original C# special-case in `WriteLuaParams`).
    UInt32(u32),
    /// 0x2 — null-terminated ASCII string.
    String(String),
    /// 0x3 — boolean true.
    True,
    /// 0x4 — boolean false.
    False,
    /// 0x5 — nil.
    Nil,
    /// 0x6 — actor reference (by id).
    Actor(u32),
    /// 0x7 — inventory slot descriptor.
    Type7 {
        actor_id: u32,
        unknown: u8,
        slot: u8,
        inventory_type: u8,
    },
    /// 0x9 — two u64 longs (the original often only cared about the first).
    Type9 { item1: u64, item2: u64 },
    /// 0xC — byte.
    Byte(u8),
    /// 0x1B — short. Readable but NOT written in the original; we preserve
    /// that asymmetry.
    Short(u16),
}

pub const LUA_END: u8 = 0x0F;

impl LuaParam {
    fn type_id(&self) -> u8 {
        match self {
            LuaParam::Int32(_) => 0x0,
            // The C# writer forces type 0x0 for UInt32, matching the client's
            // expectation of a single signed-int tag.
            LuaParam::UInt32(_) => 0x0,
            LuaParam::String(_) => 0x2,
            LuaParam::True => 0x3,
            LuaParam::False => 0x4,
            LuaParam::Nil => 0x5,
            LuaParam::Actor(_) => 0x6,
            LuaParam::Type7 { .. } => 0x7,
            LuaParam::Type9 { .. } => 0x9,
            LuaParam::Byte(_) => 0xC,
            LuaParam::Short(_) => 0x1B,
        }
    }
}

pub fn write_lua_params<W: Write>(writer: &mut W, params: &[LuaParam]) -> std::io::Result<()> {
    for p in params {
        writer.write_u8(p.type_id())?;
        match p {
            LuaParam::Int32(v) => writer.write_i32::<BigEndian>(*v)?,
            LuaParam::UInt32(v) => writer.write_u32::<BigEndian>(*v)?,
            LuaParam::String(s) => {
                writer.write_all(s.as_bytes())?;
                writer.write_u8(0)?;
            }
            LuaParam::True | LuaParam::False | LuaParam::Nil => {}
            LuaParam::Actor(v) => writer.write_u32::<BigEndian>(*v)?,
            LuaParam::Type7 {
                actor_id,
                unknown,
                slot,
                inventory_type,
            } => {
                writer.write_u32::<BigEndian>(*actor_id)?;
                writer.write_u8(*unknown)?;
                writer.write_u8(*slot)?;
                writer.write_u8(*inventory_type)?;
            }
            LuaParam::Type9 { item1, item2 } => {
                writer.write_u64::<BigEndian>(*item1)?;
                writer.write_u64::<BigEndian>(*item2)?;
            }
            LuaParam::Byte(b) => writer.write_u8(*b)?,
            LuaParam::Short(_) => {
                // The original writer left this empty — the short payload is
                // never emitted on the wire. Keep the same asymmetry here.
            }
        }
    }
    writer.write_u8(LUA_END)?;
    Ok(())
}

pub fn read_lua_params(bytes: &[u8]) -> std::io::Result<Vec<LuaParam>> {
    let mut c = Cursor::new(bytes);
    read_lua_params_from(&mut c)
}

pub fn read_lua_params_from<R: Read>(reader: &mut R) -> std::io::Result<Vec<LuaParam>> {
    let mut out = Vec::new();
    loop {
        let code = reader.read_u8()?;
        match code {
            0x0 => out.push(LuaParam::Int32(reader.read_i32::<BigEndian>()?)),
            0x1 => out.push(LuaParam::UInt32(reader.read_u32::<BigEndian>()?)),
            0x2 => {
                let mut s = Vec::new();
                let mut buf = [0u8; 1];
                loop {
                    reader.read_exact(&mut buf)?;
                    if buf[0] == 0 {
                        break;
                    }
                    s.push(buf[0]);
                }
                out.push(LuaParam::String(String::from_utf8_lossy(&s).into_owned()));
            }
            0x3 => out.push(LuaParam::True),
            0x4 => out.push(LuaParam::False),
            0x5 => out.push(LuaParam::Nil),
            0x6 => out.push(LuaParam::Actor(reader.read_u32::<BigEndian>()?)),
            0x7 => {
                let actor_id = reader.read_u32::<BigEndian>()?;
                let unknown = reader.read_u8()?;
                let slot = reader.read_u8()?;
                let inventory_type = reader.read_u8()?;
                out.push(LuaParam::Type7 {
                    actor_id,
                    unknown,
                    slot,
                    inventory_type,
                });
            }
            0x9 => {
                let item1 = reader.read_u64::<BigEndian>()?;
                let item2 = reader.read_u64::<BigEndian>()?;
                out.push(LuaParam::Type9 { item1, item2 });
            }
            0xC => out.push(LuaParam::Byte(reader.read_u8()?)),
            0x1B => out.push(LuaParam::Short(reader.read_u16::<BigEndian>()?)),
            LUA_END => break,
            unknown => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unknown lua param type 0x{unknown:X}"),
                ));
            }
        }
    }
    Ok(out)
}

/// Pretty-print helper mirroring `LuaUtils.DumpParams`.
pub fn dump_params(params: &[LuaParam]) -> String {
    let mut out = String::new();
    for (i, p) in params.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        match p {
            LuaParam::Int32(v) => out.push_str(&format!("0x{v:X}")),
            LuaParam::UInt32(v) => out.push_str(&format!("0x{v:X}")),
            LuaParam::String(s) => out.push_str(&format!("\"{s}\"")),
            LuaParam::True => out.push_str("true"),
            LuaParam::False => out.push_str("false"),
            LuaParam::Nil => out.push_str("nil"),
            LuaParam::Actor(v) => out.push_str(&format!("0x{v:X}")),
            LuaParam::Type7 {
                actor_id,
                unknown,
                slot,
                inventory_type,
            } => {
                out.push_str(&format!(
                    "Type7 Param: (0x{actor_id:X}, 0x{unknown:X}, 0x{slot:X}, 0x{inventory_type:X})"
                ));
            }
            LuaParam::Type9 { item1, item2 } => {
                out.push_str(&format!("Type9 Param: (0x{item1:X}, 0x{item2:X})"))
            }
            LuaParam::Byte(b) => out.push_str(&format!("0x{b:X}")),
            LuaParam::Short(v) => out.push_str(&format!("0x{v:X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn end_sentinel_only() {
        let params: Vec<LuaParam> = Vec::new();
        let mut buf = Vec::new();
        write_lua_params(&mut buf, &params).unwrap();
        assert_eq!(buf, vec![LUA_END]);
        assert_eq!(read_lua_params(&buf).unwrap(), params);
    }

    #[test]
    fn mixed_roundtrip() {
        let params = vec![
            LuaParam::Int32(-42),
            LuaParam::String("hi".into()),
            LuaParam::True,
            LuaParam::False,
            LuaParam::Nil,
            LuaParam::Actor(0xDEADBEEF),
            LuaParam::Byte(0x7),
        ];
        let mut buf = Vec::new();
        write_lua_params(&mut buf, &params).unwrap();
        let parsed = read_lua_params(&buf).unwrap();
        // UInt32 isn't in the roundtrip set because the writer collapses it to
        // tag 0x0 — callers decode it as Int32. Mixed set skips that case.
        assert_eq!(parsed, params);
    }
}
