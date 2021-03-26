use std::collections::HashMap;
use std::io::{self, prelude::*};

#[derive(Clone, Copy)]
enum InsnType {
    R,
    I,
    S,
    B,
    U,
    J,
    Sxli,
}

fn parse_reg(s: &str) -> Result<u32, String> {
    if !s.starts_with('x') {
        return Err("invalid register".to_string());
    }
    let s = &s["x".len()..];
    let mut n: u32 = 0;
    for c in s.chars() {
        n *= 10;
        n += match c {
            '0'..='9' => c as u32 - '0' as u32,
            _ => return Err(format!("invalid decimal digit '{}'", c)),
        };
        if n >= 32 {
            return Err("number is larger than 5 bits".to_string());
        }
    }
    Ok(n)
}

fn from_hex(s: &str, width: u32) -> Result<u32, String> {
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
        if n >= 1 << (width-1) {
            return Err("number is too large to negate".to_string());
        }
        n = (!n).wrapping_add(1);
    }
    Ok(n)
}

fn main() {
    let mut labels = HashMap::new();

    let mut mnemonics = HashMap::new();
    mnemonics.insert("lui", (InsnType::U, 0x0000_0037));
    mnemonics.insert("auipc", (InsnType::U, 0x0000_0017));
    mnemonics.insert("jal", (InsnType::J, 0x0000_006F));
    let mnemonics = mnemonics;

    let mut addr: u32 = 0x8000_0000;
    let stdin_unlocked = io::stdin();
    let stdin = stdin_unlocked.lock();
    for line in stdin.lines() {
        let line = line.unwrap();
        if line.starts_with('$') {
            // label
            let label = line["$".len()..].to_string();
            labels.insert(label, addr);
        } else if line.chars().all(|c| c.is_ascii_whitespace()) {
            // nothing
        } else {
            // instruction
            let mut parts = line.split_ascii_whitespace();
            let mnemonic = parts.next().expect("missing mnemonic");
            let (insn_type, template) = mnemonics[&mnemonic];
            let mut insn = template;
            match insn_type {
                InsnType::U => {
                    let rd = parts.next().expect("missing rd");
                    let rd = parse_reg(rd).unwrap();
                    let imm = parts.next().expect("missing imm20");
                    let imm = from_hex(imm, 20).unwrap();
                    insn += (rd << 7) + (imm << 12);
                },
                InsnType::J => {
                    let rd = parts.next().expect("missing rd");
                    let rd = parse_reg(rd).unwrap();
                    let imm = parts.next().expect("missing imm21");
                    let imm = from_hex(imm, 21).unwrap();
                    if imm & 0x3 != 0 {
                        panic!("last two bits must be 0");
                    }
                    insn += rd << 7;
                    insn += imm << (31-20) >> (31-20+20) << 31;
                    insn += imm << (31-10) >> (31-10+1) << 21;
                    insn += imm << (31-11) >> (31-11+11) << 20;
                    insn += imm << (31-19) >> (31-19+12) << 12;
                },
                _ => todo!(),
            }
            println!(
                "{:04X}'{:04X}: {:04X}'{:04X}",
                addr >> 16,
                addr & 0xFFFF,
                insn >> 16,
                insn & 0xFFFF,
            );
            addr += 4;
        }
    }
}
