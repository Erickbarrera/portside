//! Steam's binary KeyValues format, as used by `shortcuts.vdf`: a tree of named maps,
//! strings and numbers. Reads and writes every type Steam uses there, so a file written back
//! keeps everything Portside didn't touch.

use crate::SteamError;

/// One value in the tree.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Map(Map),
    Str(String),
    Int(u32),
    Float(f32),
    Int64(u64),
}

/// Named values, in file order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Map(pub Vec<(String, Value)>);

const MAP: u8 = 0x00;
const STR: u8 = 0x01;
const INT: u8 = 0x02;
const FLOAT: u8 = 0x03;
const INT64: u8 = 0x07;
const END: u8 = 0x08;

impl Map {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v)
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key) {
            Some(Value::Str(s)) => Some(s),
            _ => None,
        }
    }

    pub fn int(&self, key: &str) -> Option<u32> {
        match self.get(key) {
            Some(Value::Int(n)) => Some(*n),
            _ => None,
        }
    }

    /// Replaces the value under `key` (keeping its place and spelling), or adds it.
    pub fn set(&mut self, key: &str, value: Value) {
        match self.0.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(key)) {
            Some((_, v)) => *v = value,
            None => self.0.push((key.to_owned(), value)),
        }
    }

    pub fn map_mut(&mut self, key: &str) -> Option<&mut Map> {
        match self.0.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(key)) {
            Some((_, Value::Map(m))) => Some(m),
            _ => None,
        }
    }
}

/// Reads a whole file: one top-level map.
pub fn parse(bytes: &[u8]) -> Result<Map, SteamError> {
    let mut r = Reader { bytes, at: 0 };
    let map = r.map()?;
    Ok(map)
}

/// Writes a top-level map.
pub fn write(map: &Map) -> Vec<u8> {
    let mut out = Vec::new();
    write_map(map, &mut out);
    out.push(END);
    out
}

fn write_map(map: &Map, out: &mut Vec<u8>) {
    for (key, value) in &map.0 {
        let (tag, body): (u8, Vec<u8>) = match value {
            Value::Map(m) => {
                let mut b = Vec::new();
                write_map(m, &mut b);
                b.push(END);
                (MAP, b)
            }
            Value::Str(s) => {
                let mut b = s.as_bytes().to_vec();
                b.push(0);
                (STR, b)
            }
            Value::Int(n) => (INT, n.to_le_bytes().to_vec()),
            Value::Float(f) => (FLOAT, f.to_le_bytes().to_vec()),
            Value::Int64(n) => (INT64, n.to_le_bytes().to_vec()),
        };
        out.push(tag);
        out.extend_from_slice(key.as_bytes());
        out.push(0);
        out.extend_from_slice(&body);
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn bad(&self) -> SteamError {
        SteamError::BadShortcuts(self.at)
    }

    fn byte(&mut self) -> Result<u8, SteamError> {
        let b = *self.bytes.get(self.at).ok_or_else(|| self.bad())?;
        self.at += 1;
        Ok(b)
    }

    fn take<const N: usize>(&mut self) -> Result<[u8; N], SteamError> {
        let end = self.at + N;
        let slice = self.bytes.get(self.at..end).ok_or_else(|| self.bad())?;
        self.at = end;
        slice.try_into().map_err(|_| self.bad())
    }

    fn cstr(&mut self) -> Result<String, SteamError> {
        let rest = self.bytes.get(self.at..).ok_or_else(|| self.bad())?;
        let len = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| self.bad())?;
        let s = String::from_utf8_lossy(&rest[..len]).into_owned();
        self.at += len + 1;
        Ok(s)
    }

    /// Entries up to the map's end marker (or the end of the file, for the top level).
    fn map(&mut self) -> Result<Map, SteamError> {
        let mut map = Map::default();
        loop {
            if self.at == self.bytes.len() {
                return Ok(map);
            }
            let tag = self.byte()?;
            if tag == END {
                return Ok(map);
            }
            let key = self.cstr()?;
            let value = match tag {
                MAP => Value::Map(self.map()?),
                STR => Value::Str(self.cstr()?),
                INT => Value::Int(u32::from_le_bytes(self.take()?)),
                FLOAT => Value::Float(f32::from_le_bytes(self.take()?)),
                INT64 => Value::Int64(u64::from_le_bytes(self.take()?)),
                _ => return Err(SteamError::BadShortcuts(self.at - 1)),
            };
            map.0.push((key, value));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_type() {
        let map = Map(vec![(
            "shortcuts".into(),
            Value::Map(Map(vec![(
                "0".into(),
                Value::Map(Map(vec![
                    ("appid".into(), Value::Int(0x8000_1234)),
                    ("AppName".into(), Value::Str("Example".into())),
                    ("ratio".into(), Value::Float(1.5)),
                    ("big".into(), Value::Int64(1 << 40)),
                    ("tags".into(), Value::Map(Map::default())),
                ])),
            )])),
        )]);
        let bytes = write(&map);
        assert_eq!(parse(&bytes).expect("parse"), map);
    }

    #[test]
    fn matches_steams_layout() {
        let map = Map(vec![(
            "shortcuts".into(),
            Value::Map(Map(vec![("a".into(), Value::Int(1))])),
        )]);
        assert_eq!(
            write(&map),
            b"\x00shortcuts\x00\x02a\x00\x01\x00\x00\x00\x08\x08".to_vec()
        );
    }

    #[test]
    fn refuses_a_cut_short_file() {
        assert!(parse(b"\x00shortcuts\x00\x01name\x00abc").is_err());
        assert!(parse(b"\x09x\x00").is_err());
    }
}
