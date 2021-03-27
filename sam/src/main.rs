use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, prelude::*};

#[derive(Clone, Copy)]
enum InsnType {
    X,  // inherent
    R,
    I,
    S,
    B,
    U,
    J,
    Sxli,
    F, // fence
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

fn parse_pred_succ(s: &str) -> Result<u32, String> {
    let mut n = 0;
    for c in s.chars() {
        let b = match c {
            'i' => 0x8,
            'o' => 0x4,
            'r' => 0x2,
            'w' => 0x1,
            _ => return Err(format!("invalid pred/succ char '{}'", c)),
        };
        if n & b != 0 {
            return Err(format!("duplicate '{}'", c));
        }
        n += b;
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
    mnemonics.insert("inval", (InsnType::X,    0x0000_0000));
    mnemonics.insert(  "lui", (InsnType::U,    0x0000_0037));
    mnemonics.insert("auipc", (InsnType::U,    0x0000_0017));
    mnemonics.insert(  "jal", (InsnType::J,    0x0000_006F));
    mnemonics.insert( "jalr", (InsnType::I,    0x0000_0067));
    mnemonics.insert(  "beq", (InsnType::B,    0x0000_0063));
    mnemonics.insert(   "lb", (InsnType::I,    0x0000_0003));
    mnemonics.insert(   "sb", (InsnType::S,    0x0000_0023));
    mnemonics.insert( "addi", (InsnType::I,    0x0000_0013));
    mnemonics.insert( "slli", (InsnType::Sxli, 0x0000_1013));
    mnemonics.insert( "srli", (InsnType::Sxli, 0x0000_5013));
    mnemonics.insert(  "add", (InsnType::R,    0x0000_0033));
    mnemonics.insert(   "or", (InsnType::R,    0x0000_6033));
    mnemonics.insert(  "and", (InsnType::R,    0x0000_7033));
    mnemonics.insert("fence", (InsnType::F,    0x0000_000F));
    let mnemonics = mnemonics;

    let mut addr: u32 = 0x8000_0000;

    let args: Vec<_> = env::args_os().collect();
    assert_eq!(args.len(), 3);
    let mut input = fs::File::open(&args[1]).unwrap();
    let mut output = fs::File::create(&args[2]).unwrap();

    // Get an early error if the input file isn't seekable.
    input.seek(io::SeekFrom::Start(0)).unwrap();

    for line in io::BufReader::new(&input).lines() {
        let line = line.unwrap();
        if line.starts_with('$') {
            // label
            let label = line["$".len()..].to_string();
            labels.insert(label, addr);
        } else if line.chars().all(|c| c.is_ascii_whitespace()) {
            // nothing
        } else {
            // instruction
            addr += 4;
        }
    }

    addr = 0;
    input.seek(io::SeekFrom::Start(0)).unwrap();

    for line in io::BufReader::new(&input).lines() {
        let line = line.unwrap();
        if line.starts_with('$') {
            // label
        } else if line.chars().all(|c| c.is_ascii_whitespace()) {
            // nothing
        } else {
            // instruction
            let mut parts = line.split_ascii_whitespace();
            let mnemonic = parts.next().expect("missing mnemonic");
            let (insn_type, template) = mnemonics[&mnemonic];
            let mut insn = template;
            match insn_type {
                InsnType::X => {
                    // already done
                },
                InsnType::R => {
                    let rd = parts.next().expect("missing rd");
                    let rd = parse_reg(rd).unwrap();
                    let rs1 = parts.next().expect("missing rs1");
                    let rs1 = parse_reg(rs1).unwrap();
                    let rs2 = parts.next().expect("missing rs2");
                    let rs2 = parse_reg(rs2).unwrap();
                    insn += (rd << 7) + (rs1 << 15) + (rs2 << 20);
                },
                InsnType::Sxli => {
                    let rd = parts.next().expect("missing rd");
                    let rd = parse_reg(rd).unwrap();
                    let rs1 = parts.next().expect("missing rs1");
                    let rs1 = parse_reg(rs1).unwrap();
                    let imm = parts.next().expect("missing imm5");
                    let imm = from_hex(imm, 5).unwrap();
                    insn += (rd << 7) + (rs1 << 15) + (imm << 20);
                },
                InsnType::I => {
                    let rd = parts.next().expect("missing rd");
                    let rd = parse_reg(rd).unwrap();
                    let rs1 = parts.next().expect("missing rs1");
                    let rs1 = parse_reg(rs1).unwrap();
                    let imm = parts.next().expect("missing imm12");
                    let imm = from_hex(imm, 12).unwrap();
                    insn += (rd << 7) + (rs1 << 15) + (imm << 20);
                },
                InsnType::S => {
                    let rs2 = parts.next().expect("missing rs2");
                    let rs2 = parse_reg(rs2).unwrap();
                    let rs1 = parts.next().expect("missing rs1");
                    let rs1 = parse_reg(rs1).unwrap();
                    let imm = parts.next().expect("missing imm12");
                    let imm = from_hex(imm, 12).unwrap();
                    insn += (rs1 << 15) + (rs2 << 20);
                    insn += imm << (31-11) >> (31-11+5) << 25;
                    insn += imm << (31-4) >> (31-4+0) << 0;
                },
                InsnType::B => {
                    let rs1 = parts.next().expect("missing rs1");
                    let rs1 = parse_reg(rs1).unwrap();
                    let rs2 = parts.next().expect("missing rs2");
                    let rs2 = parse_reg(rs2).unwrap();
                    let imm_str = parts.next().expect("missing imm13");
                    let imm = if imm_str.starts_with('#')
                            || imm_str.starts_with('-') {
                        let imm = from_hex(imm_str, 13).unwrap();
                        if imm & 0x3 != 0 {
                            panic!("last two bits of imm13 must be 0");
                        }
                        imm
                    } else {
                        labels[imm_str].wrapping_sub(addr)
                    };
                    insn += (rs1 << 15) + (rs2 << 20);
                    insn += imm << (31-12) >> (31-12+12) << 31;
                    insn += imm << (31-10) >> (31-10+5) << 25;
                    insn += imm << (31-4) >> (31-4+1) << 8;
                    insn += imm << (31-11) >> (31-11+11) << 7;
                },
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
                    let imm_str = parts.next().expect("missing imm21");
                    let imm = if imm_str.starts_with('#')
                            || imm_str.starts_with('-') {
                        let imm = from_hex(imm_str, 21).unwrap();
                        if imm & 0x3 != 0 {
                            panic!("last two bits of imm21 must be 0");
                        }
                        imm
                    } else {
                        labels[imm_str].wrapping_sub(addr)
                    };
                    insn += rd << 7;
                    insn += imm << (31-20) >> (31-20+20) << 31;
                    insn += imm << (31-10) >> (31-10+1) << 21;
                    insn += imm << (31-11) >> (31-11+11) << 20;
                    insn += imm << (31-19) >> (31-19+12) << 12;
                },
                InsnType::F => {
                    let pred = parts.next().expect("missing pred");
                    let pred = parse_pred_succ(pred).unwrap();
                    let succ = parts.next().expect("missing succ");
                    let succ = parse_pred_succ(succ).unwrap();
                    insn += (pred << 24) + (succ << 20);
                },
            }
            assert_eq!(parts.next(), None, "trailing operands");
            output.write_all(&insn.to_le_bytes()).unwrap();
            addr += 4;
        }
    }

    output.sync_data().unwrap();
}
