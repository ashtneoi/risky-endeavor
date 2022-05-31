use std::collections::HashMap;
use std::fmt::{self, Display};
use std::io;
use std::io::prelude::*;
use std::process::exit;

// Having this return ! makes the type checker say e.g. "expected `!`, found `usize`".
pub fn ouch<E: Display, X>(e: E) -> X {
    eprintln!("Error: {}", e);
    exit(1);
}

pub fn from_hex(s: &str, width: u32) -> Result<u32, String> {
    assert!(width <= 32);
    let is_neg = s.starts_with('-');
    let s = if is_neg { &s["-".len()..] } else { s };
    if !s.starts_with('#') {
        return Err("number doesn't start with '#'".to_string());
    }
    let s = &s["#".len()..];
    if s.len() == 0 {
        return Err("number is empty".to_string());
    }
    let mut n: u32 = 0;
    for c in s.chars() {
        if let '\'' | '_' = c {
            continue;
        }
        if n >= 1 << (width-4) {
            return Err(format!("number is larger than {} bits", width));
        }
        n <<= 4;
        n += match c {
            '0'..='9' => c as u32 - '0' as u32,
            'A'..='F' => c as u32 - 'A' as u32 + 10,
            'a'..='f' => c as u32 - 'a' as u32 + 10,
            _ => return Err(format!("invalid hex digit '{}'", c)),
        };
    }
    if is_neg {
        assert!(width < 32);
        if n >= 1 << (width-1) {
            return Err("number is too large to negate".to_string());
        }
        n = (!n).wrapping_add(1);
    }
    Ok(n)
}

pub fn u32_to_hex(x: u32) -> String {
    format!("#{:04X}'{:04X}", x >> 16, x & 0xFFFF)
}

pub fn upper_imm20_to_hex(x: u32) -> String {
    format!("#{:04X}'{:01X}", x >> 4, x & 0xF)
}

pub fn write_len_prefixed_str(mut w: impl Write, s: &str) -> io::Result<()> {
    w.write_all(&(s.len() as u32).to_le_bytes())?;
    w.write_all(s.as_bytes())?;
    for _ in 0..(s.len().wrapping_neg() & 0b11) {
        w.write_all(&[0])?;
    }
    Ok(())
}

pub fn read_u8<R: Read>(mut r: R) -> io::Result<u8> {
    let mut buf = [0; 1];
    r.read_exact(&mut buf)?;
    Ok(buf[0])
}

pub fn read_u16<R: Read>(mut r: R) -> io::Result<u16> {
    let mut buf = [0; 2];
    r.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

pub fn read_u32<R: Read>(mut r: R) -> io::Result<u32> {
    let mut buf = [0; 4];
    r.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

pub fn read_len_prefixed_str(mut r: impl Read) -> io::Result<String> {
    let s_len = read_u32(&mut r)?;
    let mut buf = Vec::with_capacity(s_len as usize);
    buf.resize(s_len as usize, 0);
    r.read_exact(&mut buf)?;
    let s = String::from_utf8(buf).unwrap_or_else(ouch);
    let padding_len = s_len.wrapping_neg() as usize & 0b11;
    let mut buf = Vec::with_capacity(padding_len);
    buf.resize(padding_len, 0);
    r.read_exact(&mut buf)?;
    Ok(s)
}

pub fn parse_reg(s: &str) -> Result<u32, String> {
    Ok(match s {
        "zero" => 0,
        "ra" => 1,
        "sp" => 2,
        "gp" => 3,
        "tp" => 4,
        "fp" => 8,
        _ => {
            let mut cc = s.chars();
            let prefix = cc.next().unwrap();
            if !"xtsa".contains(prefix) {
                return Err(format!("invalid register prefix '{}'", prefix));
            }
            let mut n: u32 = 0;
            for c in cc {
                n *= 10;
                n += match c {
                    '0'..='9' => c as u32 - '0' as u32,
                    _ => return Err(format!("invalid decimal digit '{}'", c)),
                };
                if n > 31 {
                    return Err("no such register".to_string());
                }
            }
            match (prefix, n) {
                ('x', 0..=31) => n,
                ('t', 0..=2) => 5 + n,
                ('s', 0..=1) => 8 + n,
                ('a', 0..=7) => 10 + n,
                ('s', 2..=11) => 18 + n - 2,
                ('t', 3..=6) => 28 + n - 3,
                _ => return Err("no such register".to_string()),
            }
        },
    })
}

#[derive(Debug)]
pub enum DeserializationError {
    Io(io::Error),
    ReservedValue(String),
    PrematureEnd,
    DuplicateItem(String),
}

impl From<io::Error> for DeserializationError {
    fn from(e: io::Error) -> Self {
        DeserializationError::Io(e)
    }
}

impl Display for DeserializationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match *self {
            Self::Io(ref e) => <io::Error as Display>::fmt(e, f),
            Self::ReservedValue(ref s) => write!(f, "reserved value; {}", s),
            Self::PrematureEnd => write!(f, "premature end"),
            Self::DuplicateItem(ref s) => write!(f, "duplicate item; {}", s),
        }
    }
}

#[derive(Default)]
pub struct RelocationTable {
    pub relocations: Vec<Relocation>,
}

impl RelocationTable {
    pub fn serialize(&self, mut writer: impl Write) -> io::Result<()> {
        for relocation in &self.relocations {
            relocation.serialize(&mut writer)?;
        }
        Ok(())
    }

    pub fn deserialize(mut reader: impl Read, len: u32) -> Result<Self, DeserializationError> {
        if len & 0xF != 0 {
            return Err(DeserializationError::PrematureEnd);
        }
        let mut table: Self = Default::default();
        let mut count = 0;
        while count < len {
            let reloc = Relocation::deserialize(&mut reader)?;
            table.relocations.push(reloc);
            count += 0x10;
        }
        Ok(table)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Relocation {
    pub offset: u32,
    pub symbol_index: u32,
    pub value: RelocationValue,
}

#[derive(Clone, Copy, Debug)]
pub enum RelocationValue {
    UnusedEntry,
    RelCodeBType,
}

impl Relocation {
    pub fn symbol<'a>(&self, symbol_table: &'a SymbolTable) -> &'a Symbol {
        &symbol_table.symbols[self.symbol_index as usize]
    }

    pub fn serialize(&self, mut writer: impl Write) -> io::Result<()> {
        writer.write_all(&self.offset.to_le_bytes())?;
        writer.write_all(&self.symbol_index.to_le_bytes())?;
        let kind: u16 = match self.value {
            RelocationValue::UnusedEntry => 0,
            RelocationValue::RelCodeBType => 1,
        };
        writer.write_all(&kind.to_le_bytes())?;
        writer.write_all(&[0; 6])?; // reserved
        Ok(())
    }

    pub fn deserialize(mut reader: impl Read) -> Result<Self, DeserializationError> {
        let offset = read_u32(&mut reader)?;
        let symbol_index = read_u32(&mut reader)?;
        let value = match read_u16(&mut reader)? {
            0 => {
                read_u16(&mut reader)?;
                RelocationValue::UnusedEntry
            },
            1 => {
                read_u16(&mut reader)?;
                RelocationValue::RelCodeBType
            },
            n => return Err(DeserializationError::ReservedValue(
                format!("can't understand relocation value kind {}", n)
            )),
        };
        read_u32(&mut reader)?;
        Ok(Relocation { offset, symbol_index, value })
    }
}

#[derive(Default)]
pub struct SymbolTable {
    pub symbols: Vec<Symbol>,
    name_index_to_symbol_index: HashMap<u32, u32>,
}

impl SymbolTable {
    pub fn get<'a>(&'a self, name_index: u32) -> Option<&'a Symbol> {
        self.name_index_to_symbol_index.get(&name_index)
            .map(|&symbol_index| &self.symbols[symbol_index as usize])
    }

    pub fn get_index_or_insert(
        &mut self,
        name_index: u32,
        value: SymbolValue,
    ) -> u32 {
        if let Some(&index) = self.name_index_to_symbol_index.get(&name_index) {
            index
        } else {
            self.insert(name_index, value)
        }
    }

    // name_index may be in the table already. In this case, the symbol value is updated.
    pub fn insert(&mut self, name_index: u32, value: SymbolValue) -> u32 {
        if let Some(&index) = self.name_index_to_symbol_index.get(&name_index) {
            self.symbols[index as usize] = Symbol { name_index, value };
            index
        } else {
            let index = self.symbols.len() as u32;
            self.symbols.push(Symbol { name_index, value });
            self.name_index_to_symbol_index.insert(name_index, index);
            index
        }
    }

    pub fn contains_name(&self, name_index: u32) -> bool {
        self.name_index_to_symbol_index.contains_key(&name_index)
    }

    pub fn serialize(&self, mut writer: impl Write, string_table: &StringTable) -> io::Result<()> {
        for symbol in &self.symbols {
            symbol.serialize(&mut writer, string_table)?;
        }
        Ok(())
    }

    pub fn deserialize(
        mut reader: impl Read,
        len: u32,
        string_table: &StringTable,
    ) -> Result<Self, DeserializationError> {
        if len & 0xF != 0 {
            return Err(DeserializationError::PrematureEnd);
        }
        let mut table: Self = Default::default();
        let mut count = 0;
        while count < len {
            let symbol = Symbol::deserialize(&mut reader, string_table)?;
            if table.contains_name(symbol.name_index) {
                return Err(DeserializationError::DuplicateItem(
                    string_table.strings[symbol.name_index as usize].1.clone()
                ));
            }
            table.insert(symbol.name_index, symbol.value);
            count += 0x10;
        }
        Ok(table)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Symbol {
    name_index: u32,
    value: SymbolValue,
}

#[derive(Clone, Copy, Debug)]
pub enum SymbolValue {
    Metadata { value_index: u32 },
    Code { type_index: u32, offset: Option<u32> },
    Data { type_index: u32, offset: Option<u32> },
}

impl Symbol {
    pub fn name<'a>(&self, string_table: &'a StringTable) -> &'a str {
        &string_table.strings[self.name_index as usize].1
    }

    pub fn is_external(&self) -> bool {
        match self.value {
            SymbolValue::Metadata { .. } => false,
            SymbolValue::Code { offset, .. }
            | SymbolValue::Data { offset, .. } => offset.is_some(),
        }
    }

    pub fn serialize(&self, mut writer: impl Write, string_table: &StringTable) -> io::Result<()> {
        writer.write_all(&string_table.strings[self.name_index as usize].0.to_le_bytes())?;
        writer.write_all(&[0; 2])?;
        match self.value {
            SymbolValue::Metadata { value_index } => {
                writer.write_all(&[0])?;
                writer.write_all(&[0])?;
                writer.write_all(&string_table.strings[value_index as usize].0.to_le_bytes())?;
                writer.write_all(&[0; 4])?;
            },
            SymbolValue::Code { type_index, offset } => {
                writer.write_all(&[1])?;
                writer.write_all(&[if offset.is_some() { 1 } else { 0 }])?;
                writer.write_all(&string_table.strings[type_index as usize].0.to_le_bytes())?;
                writer.write_all(&offset.unwrap_or(0).to_le_bytes())?;
            },
            SymbolValue::Data { type_index, offset } => {
                writer.write_all(&[2])?;
                writer.write_all(&[if offset.is_some() { 1 } else { 0 }])?;
                writer.write_all(&string_table.strings[type_index as usize].0.to_le_bytes())?;
                writer.write_all(&offset.unwrap_or(0).to_le_bytes())?;
            },
        }
        Ok(())
    }

    pub fn deserialize(
        mut reader: impl Read,
        string_table: &StringTable,
    ) -> Result<Self, DeserializationError> {
        let name_offset = read_u32(&mut reader)?;
        let name_index = string_table.offset_to_index[&name_offset];
        read_u16(&mut reader)?;
        match read_u8(&mut reader)? {
            0 => {
                read_u8(&mut reader)?;
                let value_offset = read_u32(&mut reader)?;
                let value_index = string_table.offset_to_index[&value_offset];
                read_u32(&mut reader)?;
                Ok(Symbol {
                    name_index,
                    value: SymbolValue::Metadata { value_index },
                })
            },
            1 => {
                let external = read_u8(&mut reader)?;
                let type_offset = read_u32(&mut reader)?;
                let type_index = string_table.offset_to_index[&type_offset];
                let offset = read_u32(&mut reader)?;
                Ok(Symbol {
                    name_index,
                    value: SymbolValue::Code {
                        type_index,
                        offset: if external == 1 { Some(offset) } else { None },
                    },
                })
            },
            2 => {
                let external = read_u8(&mut reader)?;
                let type_offset = read_u32(&mut reader)?;
                let type_index = string_table.offset_to_index[&type_offset];
                let offset = read_u32(&mut reader)?;
                Ok(Symbol {
                    name_index,
                    value: SymbolValue::Data {
                        type_index,
                        offset: if external == 1 { Some(offset) } else { None },
                    },
                })
            },
            n => Err(DeserializationError::ReservedValue(
                format!("can't understand symbol value kind {}", n)
            )),
        }
    }
}

#[derive(Default)]
pub struct StringTable {
    len: u32,
    // FIXME: change String to Vec<u8>
    pub strings: Vec<(u32, String)>, // (offset, value). can have dupe strings.
    value_to_index: HashMap<String, u32>, // unable to have dupe keys.
    offset_to_index: HashMap<u32, u32>,
}

impl StringTable {
    pub fn get_index_or_insert(&mut self, s: &str) -> u32 {
        if let Some(&index) = self.value_to_index.get(s) {
            index
        } else {
            self.insert(s.to_owned())
        }
    }

    pub fn get_offset_or_insert(&mut self, s: &str) -> u32 {
        let index = self.get_index_or_insert(s);
        self.strings[index as usize].0
    }

    /// Returns index. s may be in the table already. In this case, value_to_index is not updated
    /// and continues to point to the first occurrence.
    fn insert(&mut self, s: String) -> u32 {
        let offset = self.len;
        let index = self.strings.len() as u32;
        let padding_len = s.len().wrapping_neg() & 0b11;
        self.len += 4 + s.len() as u32 + padding_len as u32;
        self.strings.push((offset, s.clone()));
        if !self.value_to_index.contains_key(&s) {
            self.value_to_index.insert(s, index);
        }
        self.offset_to_index.insert(offset, index);
        index
    }

    pub fn serialize(&self, mut writer: impl Write) -> io::Result<()> {
        for &(_, ref s) in &self.strings {
            write_len_prefixed_str(&mut writer, s)?;
        }
        Ok(())
    }

    pub fn deserialize(mut reader: impl Read, mut len: u32) -> Result<Self, DeserializationError> {
        let mut table: Self = Default::default();
        while len != 0 {
            if len < 4 {
                return Err(DeserializationError::PrematureEnd);
            }
            let s_len = read_u32(&mut reader)?;
            len -= 4;
            let padding_len = s_len.wrapping_neg() & 0b11;

            if len < s_len + padding_len {
                return Err(DeserializationError::PrematureEnd);
            }
            let mut buf = Vec::with_capacity(s_len as usize);
            buf.resize(s_len as usize, 0);
            reader.read_exact(&mut buf)?;
            len -= s_len;
            let s = String::from_utf8(buf).unwrap_or_else(ouch);
            table.insert(s);

            let mut buf = Vec::with_capacity(padding_len as usize);
            buf.resize(padding_len as usize, 0);
            reader.read_exact(&mut buf)?;
            len -= padding_len;
        }
        Ok(table)
    }
}
