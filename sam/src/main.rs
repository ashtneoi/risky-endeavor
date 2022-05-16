use sam::{from_hex, parse_reg};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, prelude::*};

#[derive(Clone, Copy)]
enum InsnType {
    P, // pseudo
    X, // inherent
    R,
    I,
    S,
    B,
    U,
    J,
    Sxli,
    F, // fence
    C, // csr??
    Ci, // csr??i
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
        n |= b;
    }
    Ok(n)
}

fn u32_to_hex(x: u32) -> String {
    format!("{:04X}'{:04X}", x >> 16, x & 0xFFFF)
}

fn upper_imm20_to_hex(x: u32) -> String {
    format!("{:04X}'{:01X}", x >> 4, x & 0xF)
}

fn assemble_line(
    mnemonics: &HashMap<&str, (InsnType, u32)>,
    addr: u32,
    string_lens: &HashMap<String, u32>,
    labels: &HashMap<String, u32>,
    line: &str,
    print_insns: bool,
) -> Vec<u32> {
    let mut parts = line.split_ascii_whitespace();
    let mnemonic = parts.next().expect("missing mnemonic");
    let (insn_type, template) = *mnemonics.get(&mnemonic)
        .unwrap_or_else(|| panic!("unknown mnemonic '{}'", &mnemonic));
    let mut insns = vec![];
    let print_first_insn = match insn_type {
        InsnType::P => {
            if mnemonic == "li" {
                let rd_str = parts.next().expect("missing rd");
                let _ = parse_reg(rd_str).unwrap();
                let imm_str = parts.next().expect("missing imm32");
                let imm_str1: String;
                let imm_str2: String;
                if imm_str.starts_with('%') {
                    let label = &imm_str["%".len()..];
                    if !string_lens.contains_key(label) {
                        panic!("unknown label '{}'", label);
                    }
                    imm_str1 = imm_str.to_owned();
                    imm_str2 = imm_str.to_owned();
                } else if labels.contains_key(imm_str) {
                    imm_str1 = imm_str.to_owned();
                    imm_str2 = imm_str.to_owned();
                } else {
                    let imm32 = from_hex(imm_str, 32).unwrap();
                    imm_str1 = format!("#{}", upper_imm20_to_hex(
                        ((imm32 >> 12) + ((imm32 >> 11) & 1)) & 0xF_FFFF
                    ));
                    imm_str2 = format!("#{:03X}", imm32 & 0xFFF);
                };
                insns.append(&mut assemble_line(
                    mnemonics,
                    addr,
                    string_lens,
                    labels,
                    &format!("lui {} {}", &rd_str, &imm_str1),
                    print_insns,
                ));
                insns.append(&mut assemble_line(
                    mnemonics,
                    addr + 4,
                    string_lens,
                    labels,
                    &format!("addi {} {} {}", &rd_str, &rd_str, &imm_str2),
                    print_insns,
                ));
                false
            } else {
                unreachable!();
            }
        },
        InsnType::X => {
            insns.push(template);
            print_insns
        },
        InsnType::R => {
            insns.push(template);
            let rd = parts.next().expect("missing rd");
            let rd = parse_reg(rd).unwrap();
            let rs1 = parts.next().expect("missing rs1");
            let rs1 = parse_reg(rs1).unwrap();
            let rs2 = parts.next().expect("missing rs2");
            let rs2 = parse_reg(rs2).unwrap();
            insns[0] += (rd << 7) + (rs1 << 15) + (rs2 << 20);
            print_insns
        },
        InsnType::Sxli => {
            insns.push(template);
            let rd = parts.next().expect("missing rd");
            let rd = parse_reg(rd).unwrap();
            let rs1 = parts.next().expect("missing rs1");
            let rs1 = parse_reg(rs1).unwrap();
            let imm = parts.next().expect("missing imm5");
            let imm = from_hex(imm, 5).unwrap();
            insns[0] += (rd << 7) + (rs1 << 15) + (imm << 20);
            print_insns
        },
        InsnType::I => {
            insns.push(template);
            let rd = parts.next().expect("missing rd");
            let rd = parse_reg(rd).unwrap();
            let rs1 = parts.next().expect("missing rs1");
            let rs1 = parse_reg(rs1).unwrap();
            let imm_str = parts.next().expect("missing imm12");
            let imm = if imm_str.starts_with('%') {
                let label = &imm_str["%".len()..];
                let len = *string_lens.get(label)
                    .unwrap_or_else(
                        || panic!("unknown label '{}'", label))
                    as u32;
                len & 0xFFF
            } else if let Some(&x) = labels.get(imm_str) {
                x & 0xFFF
            } else {
                from_hex(imm_str, 12).unwrap()
            };
            insns[0] += (rd << 7) + (rs1 << 15) + (imm << 20);
            print_insns
        },
        InsnType::S => {
            insns.push(template);
            let rs2 = parts.next().expect("missing rs2");
            let rs2 = parse_reg(rs2).unwrap();
            let rs1 = parts.next().expect("missing rs1");
            let rs1 = parse_reg(rs1).unwrap();
            let imm = parts.next().expect("missing imm12");
            let imm = from_hex(imm, 12).unwrap();
            insns[0] += (rs1 << 15) + (rs2 << 20);
            insns[0] += imm << (31-11) >> (31-11+5) << 25;
            insns[0] += imm << (31-4) >> (31-4+0) << 7;
            print_insns
        },
        InsnType::B => {
            insns.push(template);
            let rs1 = parts.next().expect("missing rs1");
            let rs1 = parse_reg(rs1).unwrap();
            let rs2 = parts.next().expect("missing rs2");
            let rs2 = parse_reg(rs2).unwrap();
            let imm_str = parts.next().expect("missing imm13");
            let imm = if imm_str.starts_with('#')
                    || imm_str.starts_with('-') {
                let imm = from_hex(imm_str, 13).unwrap();
                if imm & 0x3 != 0 {
                    panic!("low two bits of imm13 must be 0");
                }
                imm
            } else {
                let label_addr = labels.get(imm_str).unwrap_or_else(
                    || panic!("unknown label '{}'", imm_str));
                let displacement = label_addr.wrapping_sub(addr);
                if (displacement as i32) < -0x1000
                        || (displacement as i32) > 0xFFF {
                    panic!(
                        "displacement {} is too large \
                            for 13-bit immediate",
                        u32_to_hex(displacement),
                    );
                }
                displacement
            };
            insns[0] += (rs1 << 15) + (rs2 << 20);
            insns[0] += imm << (31-12) >> (31-12+12) << 31;
            insns[0] += imm << (31-10) >> (31-10+5) << 25;
            insns[0] += imm << (31-4) >> (31-4+1) << 8;
            insns[0] += imm << (31-11) >> (31-11+11) << 7;
            print_insns
        },
        InsnType::U => {
            insns.push(template);
            let rd = parts.next().expect("missing rd");
            let rd = parse_reg(rd).unwrap();
            let imm_str = parts.next().expect("missing imm20");
            let imm = if imm_str.starts_with('%') {
                let label = &imm_str["%".len()..];
                let len = *string_lens.get(label)
                    .unwrap_or_else(
                        || panic!("unknown label '{}'", label))
                    as u32;
                // counteract sign extension of addi immediate
                (len >> 12) + ((len >> 11) & 1)
            } else if let Some(&x) = labels.get(imm_str) {
                // counteract sign extension of addi immediate
                (x >> 12) + ((x >> 11) & 1)
            } else {
                from_hex(imm_str, 20).unwrap()
            };
            insns[0] += (rd << 7) + (imm << 12);
            print_insns
        },
        InsnType::J => {
            insns.push(template);
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
                let label_addr = labels.get(imm_str).unwrap_or_else(
                    || panic!("unknown label '{}'", imm_str));
                let displacement = label_addr.wrapping_sub(addr);
                if (displacement as i32) < -0x10_0000
                        || (displacement as i32) > 0xF_FFFF {
                    panic!(
                        "displacement {} is too large \
                            for 21-bit immediate",
                        u32_to_hex(displacement),
                    );
                }
                displacement
            };
            insns[0] += rd << 7;
            insns[0] += imm << (31-20) >> (31-20+20) << 31;
            insns[0] += imm << (31-10) >> (31-10+1) << 21;
            insns[0] += imm << (31-11) >> (31-11+11) << 20;
            insns[0] += imm << (31-19) >> (31-19+12) << 12;
            print_insns
        },
        InsnType::F => {
            insns.push(template);
            let pred = parts.next().expect("missing pred");
            let pred = parse_pred_succ(pred).unwrap();
            let succ = parts.next().expect("missing succ");
            let succ = parse_pred_succ(succ).unwrap();
            insns[0] += (pred << 24) + (succ << 20);
            print_insns
        },
        InsnType::C => {
            insns.push(template);
            let rd = parts.next().expect("missing rd");
            let rd = parse_reg(rd).unwrap();
            let rs1 = parts.next().expect("missing rs1");
            let rs1 = parse_reg(rs1).unwrap();
            let csr = parts.next().expect("missing csr");
            let csr = from_hex(csr, 12).unwrap();
            insns[0] += (rd << 7) + (rs1 << 15) + (csr << 20);
            print_insns
        },
        InsnType::Ci => {
            insns.push(template);
            let rd = parts.next().expect("missing rd");
            let rd = parse_reg(rd).unwrap();
            let uimm = parts.next().expect("missing uimm5");
            let uimm = from_hex(uimm, 5).unwrap();
            let csr = parts.next().expect("missing csr");
            let csr = from_hex(csr, 12).unwrap();
            insns[0] += (rd << 7) + (uimm << 15) + (csr << 20);
            print_insns
        },
    };
    assert!(insns.len() >= 1);
    assert_eq!(parts.next(), None, "trailing operands");

    if print_first_insn {
        println!(
            "{}: {}  {}", u32_to_hex(addr), u32_to_hex(insns[0]), line
        );
    }

    insns
}

fn main() {
    let mut mnemonics = HashMap::new();
    mnemonics.insert( "inval", (InsnType::X,    0x0000_0000));
    mnemonics.insert(   "lui", (InsnType::U,    0x0000_0037));
    mnemonics.insert( "auipc", (InsnType::U,    0x0000_0017));
    mnemonics.insert(   "jal", (InsnType::J,    0x0000_006F));
    mnemonics.insert(  "jalr", (InsnType::I,    0x0000_0067));
    mnemonics.insert(   "ret", (InsnType::X,    0x0000_8067));
    mnemonics.insert(   "beq", (InsnType::B,    0x0000_0063));
    mnemonics.insert(   "bne", (InsnType::B,    0x0000_1063));
    mnemonics.insert(   "blt", (InsnType::B,    0x0000_4063));
    mnemonics.insert(   "bge", (InsnType::B,    0x0000_5063));
    mnemonics.insert(  "bltu", (InsnType::B,    0x0000_6063));
    mnemonics.insert(    "lb", (InsnType::I,    0x0000_0003));
    mnemonics.insert(    "lh", (InsnType::I,    0x0000_1003));
    mnemonics.insert(    "lw", (InsnType::I,    0x0000_2003));
    mnemonics.insert(    "sb", (InsnType::S,    0x0000_0023));
    mnemonics.insert(    "sh", (InsnType::S,    0x0000_1023));
    mnemonics.insert(    "sw", (InsnType::S,    0x0000_2023));
    mnemonics.insert(  "addi", (InsnType::I,    0x0000_0013));
    mnemonics.insert(   "nop", (InsnType::X,    0x0000_0013));
    mnemonics.insert(   "ori", (InsnType::I,    0x0000_6013));
    mnemonics.insert(  "andi", (InsnType::I,    0x0000_7013));
    mnemonics.insert(  "slli", (InsnType::Sxli, 0x0000_1013));
    mnemonics.insert(  "srli", (InsnType::Sxli, 0x0000_5013));
    mnemonics.insert(   "add", (InsnType::R,    0x0000_0033));
    mnemonics.insert(   "sub", (InsnType::R,    0x4000_0033));
    mnemonics.insert(  "sltu", (InsnType::R,    0x0000_3033));
    mnemonics.insert(   "xor", (InsnType::R,    0x0000_4033));
    mnemonics.insert(    "or", (InsnType::R,    0x0000_6033));
    mnemonics.insert(   "and", (InsnType::R,    0x0000_7033));
    mnemonics.insert( "fence", (InsnType::F,    0x0000_000F));
    mnemonics.insert( "ecall", (InsnType::X,    0x0000_0073));
    mnemonics.insert("ebreak", (InsnType::X,    0x0010_0073));
    mnemonics.insert( "csrrw", (InsnType::C,    0x0000_1073));
    mnemonics.insert( "csrrs", (InsnType::C,    0x0000_2073));
    mnemonics.insert( "csrrc", (InsnType::C,    0x0000_3073));
    mnemonics.insert("csrrsi", (InsnType::Ci,   0x0000_6073));
    mnemonics.insert(  "mret", (InsnType::X,    0x3020_0073));
    mnemonics.insert(   "wfi", (InsnType::X,    0x1050_0073));
    mnemonics.insert(    "li", (InsnType::P,              0));
    let mnemonics = mnemonics;

    let mut labels = HashMap::new();
    let mut string_lens = HashMap::new();

    let mut addr: u32 = 0x8000_0000;

    let args: Vec<_> = env::args_os().collect();
    assert_eq!(args.len(), 3);
    let mut input = fs::File::open(&args[1]).unwrap();
    let mut output;
    if &args[2] == "-" {
        output = None;
    } else {
        output = Some(fs::File::create(&args[2]).unwrap());
    }

    // Get an early error if the input file isn't seekable.
    input.seek(io::SeekFrom::Start(0)).unwrap();

    let mut pending_labels = Vec::new();
    let mut labels_in_order = Vec::new();

    for line_full in io::BufReader::new(&input).lines() {
        let line_full = line_full.unwrap();
        let line_trimmed = line_full.trim_start();
        let line = match line_trimmed.find(';') {
            Some(comment_start) => &line_trimmed[..comment_start],
            None => line_trimmed,
        };
        if line.starts_with('$') {
            // label
            let label = line["$".len()..].to_string();
            if labels.contains_key(&label) {
                panic!("duplicate label '{}'", &label);
            }
            pending_labels.push(label.clone());
            if output.is_none() {
                labels_in_order.push(label);
            }
        } else if line.starts_with(".utf8 ") {
            // UTF-8 string
            let len = (line.len() - ".utf8 ".len()) as u32;
            for label in pending_labels.drain(..) {
                labels.insert(label.clone(), addr);
                string_lens.insert(label, len);
            }
            addr += len;
        } else if line.is_empty() {
            // nothing
        } else {
            // instruction
            addr = (addr + 3) & !3;
            for label in pending_labels.drain(..) {
                labels.insert(label, addr);
            }
            let mut parts = line.split_ascii_whitespace();
            let mnemonic = parts.next().expect("missing mnemonic");
            let (insn_type, _) = *mnemonics.get(&mnemonic)
                .unwrap_or_else(|| panic!("unknown mnemonic '{}'", &mnemonic));
            match insn_type {
                InsnType::P => {
                    if mnemonic == "li" {
                        addr += 8;
                    } else {
                        unreachable!();
                    }
                },
                _ => addr += 4,
            }
        }
    }
    for label in pending_labels.drain(..) {
        labels.insert(label, addr);
    }

    if output.is_none() {
        for label in &labels_in_order {
            let addr = labels[label];
            println!(
                "{}: {}", u32_to_hex(addr), &label);
        }
        println!();
    }

    addr = 0x8000_0000;
    input.seek(io::SeekFrom::Start(0)).unwrap();

    for line_full in io::BufReader::new(&input).lines() {
        let line_full = line_full.unwrap();
        let line_trimmed = line_full.trim_start();
        let line = match line_trimmed.find(';') {
            Some(comment_start) => &line_trimmed[..comment_start],
            None => line_trimmed,
        };
        if line.starts_with('$') {
            // label
        } else if line.starts_with(".utf8 ") {
            // UTF-8 string
            let s = &line[".utf8 ".len()..];
            if let Some(ref mut output) = output {
                output.write_all(s.as_bytes()).unwrap();
            } else {
                println!(
                    "{}: \"{}\"",
                    u32_to_hex(addr),
                    s,
                );
            }
            addr += s.len() as u32;
        } else if line.is_empty() {
            // nothing
        } else {
            // instruction
            if addr & 3 != 0 {
                let pad = 4 - (addr & 3);
                if let Some(ref mut output) = output {
                    for _ in 0..pad {
                        output.write_all(&[0u8]).unwrap();
                    }
                } else {
                    println!(
                        "{}: pad ({})",
                        u32_to_hex(addr),
                        pad,
                    );
                }
                addr += pad;
            }
            let insns = assemble_line(
                &mnemonics, addr, &string_lens, &labels, line, output.is_none()
            );
            for insn in insns {
                if let Some(ref mut output) = output {
                    output.write_all(&insn.to_le_bytes()).unwrap();
                }
                addr += 4;
            }
        }
    }

    output.map(|o| o.sync_data().unwrap());
}
