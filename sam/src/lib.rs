use std::collections::HashMap;
use std::fmt::Display;
use std::io;
use std::io::prelude::*;
use std::process::exit;

// Having this return ! makes the type checker say e.g. "expected `!`, found `usize`".
pub fn ouch<E: Display, X>(e: E) -> X {
    eprintln!("{}", e);
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
}

#[derive(Clone, Copy, Debug)]
pub struct Relocation {
    pub offset: u32,
    pub symbol_index: u32,
    pub value: RelocationValue,
}

#[derive(Clone, Copy, Debug)]
pub enum RelocationValue {
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
            RelocationValue::RelCodeBType => 1,
        };
        writer.write_all(&kind.to_le_bytes())?;
        writer.write_all(&[0; 2])?; // reserved
        Ok(())
    }
}

#[derive(Default)]
pub struct SymbolTable {
    symbols: Vec<Symbol>,
    name_index_to_symbol_index: HashMap<u32, u32>,
}

impl SymbolTable {
    fn get<'a>(&'a self, name_index: u32) -> Option<&'a Symbol> {
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

    pub fn serialize(&self, mut writer: impl Write, string_table: &StringTable) -> io::Result<()> {
        writer.write_all(&string_table.strings[self.name_index as usize].0.to_le_bytes())?;
        writer.write_all(&[0; 6])?;
        match self.value {
            SymbolValue::Metadata { value_index } => {
                writer.write_all(&0u8.to_le_bytes())?;
                writer.write_all(&[0])?;
                writer.write_all(&string_table.strings[value_index as usize].0.to_le_bytes())?;
                writer.write_all(&[0; 4])?;
            },
            SymbolValue::Code { type_index, offset } => {
                writer.write_all(&1u8.to_le_bytes())?;
                writer.write_all(&[if offset.is_some() { 1 } else { 0 }])?;
                writer.write_all(&string_table.strings[type_index as usize].0.to_le_bytes())?;
                writer.write_all(&offset.unwrap_or(0).to_le_bytes())?;
            },
            SymbolValue::Data { type_index, offset } => {
                writer.write_all(&2u8.to_le_bytes())?;
                writer.write_all(&[if offset.is_some() { 1 } else { 0 }])?;
                writer.write_all(&string_table.strings[type_index as usize].0.to_le_bytes())?;
                writer.write_all(&offset.unwrap_or(0).to_le_bytes())?;
            },
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct StringTable {
    len: u32,
    strings: Vec<(u32, String)>, // (offset, value)
    lookup: HashMap<String, u32>, // value -> index
}

impl StringTable {
    pub fn get_index_or_insert(&mut self, s: &str) -> u32 {
        if let Some(&index) = self.lookup.get(s) {
            index
        } else {
            self.insert(s.to_owned())
        }
    }

    pub fn get_offset_or_insert(&mut self, s: &str) -> u32 {
        let index = self.get_index_or_insert(s);
        self.strings[index as usize].0
    }

    /// Returns index. s must not be in the table already!
    fn insert(&mut self, s: String) -> u32 {
        let offset = self.len;
        let index = self.strings.len() as u32;
        self.len += 4 + s.len() as u32; // entries are length-prefixed
        self.strings.push((offset, s.clone()));
        self.lookup.insert(s, index);
        index
    }

    pub fn serialize(&self, mut writer: impl Write) -> io::Result<()> {
        for &(_, ref value) in &self.strings {
            writer.write_all(&(value.len() as u32).to_le_bytes())?;
            writer.write_all(value.as_bytes())?;
            for _ in 0..(value.len().wrapping_neg() & 0b11) {
                writer.write_all(&[0])?;
            }
        }
        Ok(())
    }
}
