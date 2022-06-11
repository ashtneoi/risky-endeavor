use sam::{
    from_hex,
    ouch,
    parse_reg,
    read_u32,
    Relocation,
    RelocationTable,
    RelocationValue,
    StringTable,
    Symbol,
    SymbolTable,
    SymbolValue,
    u32_to_hex,
    upper_imm20_to_hex,
};
use std::collections::{HashMap};
use std::error::Error;
use std::env;
use std::fmt::{self, Debug, Display, Formatter};
use std::fs;
use std::io::{self, prelude::*, SeekFrom};

// TODO: There should probably be a trait named Peekable, with this struct implementing it. That
// way we can separate the peeking and counting logic. Hopefully.
struct Peekable<I: Iterator> {
    iter: I,
    peeked: Option<Option<I::Item>>,
    pos: usize,
}

impl<I: Iterator> Peekable<I> {
    fn new(iter: I) -> Self {
        Self { iter, peeked: None, pos: 0 }
    }

    fn peek(&mut self) -> Option<&I::Item> {
        if let Some(ref p) = self.peeked {
            p.as_ref()
        } else {
            self.peeked = Some(self.iter.next());
            self.peeked.as_ref().unwrap().as_ref()
        }
    }
}

impl<I: Iterator> Iterator for Peekable<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.peeked.take()
            .and_then(|p| p)
            .or_else(|| self.iter.next());
        if item.is_some() {
            self.pos += 1;
        }
        item
    }
}

#[derive(Clone, Copy, Debug)]
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

#[derive(Debug)]
enum AssemblerError {
    Syntax {
        line_num: usize, // 0-based
        col_num: usize, // 0-based
        msg: String,
    },
    DuplicateLabel {
        line_num: usize, // 0-based
        col_num: usize, // 0-based
        label: String,
    },
    Write {
        line_num: usize, // 0-based
        inner: io::Error,
    },
}

impl Display for AssemblerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            &AssemblerError::Syntax {
                line_num, col_num, ref msg
            } => write!(f, "{}:{}: {}", line_num + 1, col_num + 1, msg),
            &AssemblerError::DuplicateLabel {
                line_num, col_num, ref label
            } => write!(f, "{}:{}: duplicate definition of label {:?}", line_num + 1, col_num + 1, label),
            &AssemblerError::Write {
                line_num, ref inner
            } => write!(f, "{}: {}", line_num + 1, inner),
        }
    }
}

impl Error for AssemblerError { }

fn skip_while<I: Iterator, F: Fn(&I::Item) -> bool>(
    iter: &mut Peekable<I>,
    f: F,
) -> usize {
    let mut count = 0;
    while iter.peek().map_or(false, &f) {
        iter.next();
        count += 1;
    }
    count
}

fn collect_while<I: Iterator, F: Fn(&I::Item) -> Option<char>>(
    iter: &mut Peekable<I>,
    f: F,
) -> (usize, String) {
    let mut s = String::new();
    let mut count = 0;
    loop {
        s.push(match iter.peek().and_then(&f) {
            Some(c) => c,
            None => break,
        });
        iter.next();
        count += 1;
    }
    return (count, s);
}

fn skip_whitespace<I: Iterator<Item = (usize, char)>>(
    iter: &mut Peekable<I>
) -> usize {
    skip_while(iter, |&(_, c)| c.is_whitespace())
}

fn collect_word<I: Iterator<Item = (usize, char)>>(
    iter: &mut Peekable<I>
) -> (usize, String) {
    collect_while(iter, |&(_, c)|
        if c.is_whitespace() {
            None
        } else {
            Some(c)
        }
    )
}

/// contains no whitespace, does not start with '#' or '-', and contains at least one non-numeric char
fn is_identifier(s: &str) -> bool {
    if s.starts_with('#') || s.starts_with('-') {
        return false;
    }
    let mut has_non_numeric = false;
    for c in s.chars() {
        if c.is_whitespace() {
            return false;
        } else if !c.is_numeric() {
            has_non_numeric = true;
        }
    }
    has_non_numeric
}

fn assemble_line2(
    mnemonics: &HashMap<&str, (InsnType, u32)>,
    line_num: usize,
    line: &str,
    insn_offset: u32, // object format only supports 2^32 bytes of code
    strings: &mut StringTable,
    symbols: &mut SymbolTable,
    relocations: &mut RelocationTable,
    code_and_data: &mut impl Write,
) -> Result<u32, AssemblerError> {
    let mut chars = Peekable::new(line.char_indices());

    // FIXME: Column numbers in error messages are counted by code point, not extended grapheme
    // cluster. :(

    skip_whitespace(&mut chars);
    if chars.peek().is_none() {
        return Ok(0)
    }

    let word_pos = chars.pos;
    let (_, word) = collect_word(&mut chars);
    if word.starts_with('$') {
        let name = &word["$".len()..];
        let name_index = strings.get_index_or_insert(name);
        if let Some(sym) = symbols.get(name_index) {
            if sym.is_defined() {
                return Err(AssemblerError::DuplicateLabel {
                    line_num,
                    col_num: word_pos,
                    label: name.to_owned(),
                });
            }
        }
        // TODO: we don't know it's code. add it to a pending set then choose symbol type based on insn/directive that follows
        symbols.insert(name_index, SymbolValue::Code {
            external: false,
            type_index: 0, // none
            offset: Some(insn_offset),
        });
        return Ok(0);
    }
    let (mnemonic, mnemonic_pos) = (word, word_pos);
    let &(insn_type, template) = mnemonics.get(&mnemonic as &str)
        .ok_or_else(|| AssemblerError::Syntax {
            line_num,
            col_num: word_pos,
            msg: format!("unknown mnemonic '{}'", &mnemonic),
        })?;

    skip_whitespace(&mut chars);

    let parse_reg_here = |field, chars: &mut Peekable<_>| {
        let pos = chars.pos;
        if chars.peek().is_none() {
            return Err(AssemblerError::Syntax {
                line_num,
                col_num: pos,
                msg: format!("missing {}", field),
            });
        }
        let (_, reg) = collect_word(chars);
        parse_reg(&reg).map_err(
            |e| AssemblerError::Syntax { line_num, col_num: pos, msg: e })
    };

    let parse_imm_here = |width, chars: &mut Peekable<_>| {
        let pos = chars.pos;
        if chars.peek().is_none() {
            return Err(AssemblerError::Syntax {
                line_num,
                col_num: pos,
                msg: format!("missing imm{}", width),
            });
        }
        let (_, imm) = collect_word(chars);
        from_hex(&imm, width).map_err(
            |e| AssemblerError::Syntax {line_num, col_num: pos, msg: e })
    };

    let insn_len = match insn_type {
        InsnType::X => {
            code_and_data.write_all(&template.to_le_bytes())
                .map_err(|e| AssemblerError::Write {
                    line_num,
                    inner: e,
                })?;
            4
        },
        InsnType::R => {
            let rd = parse_reg_here("rd", &mut chars)?;
            skip_whitespace(&mut chars);
            let rs1 = parse_reg_here("rs1", &mut chars)?;
            skip_whitespace(&mut chars);
            let rs2 = parse_reg_here("rs2", &mut chars)?;
            let insn = template + (rd << 7) + (rs1 << 15) + (rs2 << 20);
            code_and_data.write_all(&insn.to_le_bytes())
                .map_err(|e| AssemblerError::Write {
                    line_num,
                    inner: e,
                })?;
            4
        },
        InsnType::Sxli => {
            let rd = parse_reg_here("rd", &mut chars)?;
            skip_whitespace(&mut chars);
            let rs1 = parse_reg_here("rs1", &mut chars)?;
            skip_whitespace(&mut chars);
            let imm5 = parse_imm_here(5, &mut chars)?;
            let insn = template + (rd << 7) + (rs1 << 15) + (imm5 << 20);
            code_and_data.write_all(&insn.to_le_bytes())
                .map_err(|e| AssemblerError::Write {
                    line_num,
                    inner: e,
                })?;
            4
        },
        InsnType::I => {
            let rd = parse_reg_here("rd", &mut chars)?;
            skip_whitespace(&mut chars);
            let rs1 = parse_reg_here("rs1", &mut chars)?;
            let mut insn = template + (rd << 7) + (rs1 << 15);
            skip_whitespace(&mut chars);
            if chars.peek().is_none() {
                return Err(AssemblerError::Syntax {
                    line_num,
                    col_num: chars.pos,
                    msg: "missing imm12".to_owned(),
                });
            }
            let imm12_pos = chars.pos;
            let (_, imm12) = collect_word(&mut chars);
            if is_identifier(&imm12) {
                if !(mnemonic == "addi" || mnemonic == "jalr") {
                    return Err(AssemblerError::Syntax {
                        line_num,
                        col_num: imm12_pos,
                        msg: "I-type instructions with identifier immediate must be addi or jalr"
                            .to_owned(),
                    });
                }
                relocations.relocations.push(Relocation {
                    offset: insn_offset,
                    symbol_index: symbols.get_index_or_insert(
                        strings.get_index_or_insert(&imm12),
                        SymbolValue::Code { // FIXME: i guess we don't need both code and data?
                            external: false,
                            type_index: 0, // none
                            offset: None,
                        },
                    ),
                    value: RelocationValue::RelIType,
                });
            } else {
                let imm12 = from_hex(&imm12, 12).map_err(
                    |e| AssemblerError::Syntax {line_num, col_num: imm12_pos, msg: e })?;
                insn += imm12 << 20;
            }
            code_and_data.write_all(&insn.to_le_bytes())
                .map_err(|e| AssemblerError::Write {
                    line_num,
                    inner: e,
                })?;
            4
        },
        InsnType::S => {
            let rs2 = parse_reg_here("rs2", &mut chars)?;
            skip_whitespace(&mut chars);
            let rs1 = parse_reg_here("rs1", &mut chars)?;
            skip_whitespace(&mut chars);
            let imm12 = parse_imm_here(12, &mut chars)?;
            let mut insn = template;
            insn += (rs1 << 15) + (rs2 << 20);
            insn += imm12 << (31-11) >> (31-11+5) << 25;
            insn += imm12 << (31-4) >> (31-4+0) << 7;
            code_and_data.write_all(&insn.to_le_bytes())
                .map_err(|e| AssemblerError::Write {
                    line_num,
                    inner: e,
                })?;
            4
        },
        InsnType::B => {
            let rs1 = parse_reg_here("rs1", &mut chars)?;
            skip_whitespace(&mut chars);
            let rs2 = parse_reg_here("rs2", &mut chars)?;
            let mut insn = template + (rs1 << 15) + (rs2 << 20);
            skip_whitespace(&mut chars);
            if chars.peek().is_none() {
                return Err(AssemblerError::Syntax {
                    line_num,
                    col_num: chars.pos,
                    msg: "missing branch target".to_owned(),
                });
            }
            let target_pos = chars.pos;
            let (_, target) = collect_word(&mut chars);
            if is_identifier(&target) {
                // DANGER: Currently, initial immediate must be zero or it might not overflow
                // correctly. Fix this in Relocation::apply().
                relocations.relocations.push(Relocation {
                    offset: insn_offset,
                    symbol_index: symbols.get_index_or_insert(
                        strings.get_index_or_insert(&target),
                        SymbolValue::Code {
                            external: false,
                            type_index: 0, // none
                            offset: None,
                        },
                    ),
                    value: RelocationValue::RelCodeBType,
                });
            } else {
                let imm13 = from_hex(&target, 13).map_err(
                    |e| AssemblerError::Syntax {line_num, col_num: target_pos, msg: e })?;

                if imm13 & 0x3 != 0 {
                    return Err(AssemblerError::Syntax {
                        line_num,
                        col_num: target_pos,
                        msg: "low two bits of imm13 must be 0".to_owned(),
                    });
                }
                insn += imm13 << (31-12) >> (31-12+12) << 31;
                insn += imm13 << (31-10) >> (31-10+5) << 25;
                insn += imm13 << (31-4) >> (31-4+1) << 8;
                insn += imm13 << (31-11) >> (31-11+11) << 7;
            }
            code_and_data.write_all(&insn.to_le_bytes())
                .map_err(|e| AssemblerError::Write {
                    line_num,
                    inner: e,
                })?;
            4
        },
        InsnType::U => {
            let rd = parse_reg_here("rd", &mut chars)?;
            skip_whitespace(&mut chars);
            // TODO: Handle other kinds of immediates.
            let imm20 = parse_imm_here(20, &mut chars)?;
            let insn = template + (rd << 7) + (imm20 << 12);
            code_and_data.write_all(&insn.to_le_bytes())
                .map_err(|e| AssemblerError::Write {
                    line_num,
                    inner: e,
                })?;
            4
        },
        InsnType::J => {
            let rd = parse_reg_here("rd", &mut chars)?;
            let mut insn = template + (rd << 7);
            skip_whitespace(&mut chars);
            if chars.peek().is_none() {
                return Err(AssemblerError::Syntax {
                    line_num,
                    col_num: chars.pos,
                    msg: "missing jump target".to_owned(),
                });
            }
            let target_pos = chars.pos;
            let (_, target) = collect_word(&mut chars);
            if is_identifier(&target) {
                // DANGER: Currently, initial immediate must be zero or it might not overflow
                // correctly. Fix this in Relocation::apply().
                relocations.relocations.push(Relocation {
                    offset: insn_offset,
                    symbol_index: symbols.get_index_or_insert(
                        strings.get_index_or_insert(&target),
                        SymbolValue::Code {
                            external: false,
                            type_index: 0, // none
                            offset: None,
                        },
                    ),
                    value: RelocationValue::RelCodeJType,
                });
            } else {
                let imm21 = from_hex(&target, 21).map_err(
                    |e| AssemblerError::Syntax {line_num, col_num: target_pos, msg: e })?;

                if imm21 & 0x3 != 0 {
                    return Err(AssemblerError::Syntax {
                        line_num,
                        col_num: target_pos,
                        msg: "low two bits of imm21 must be 0".to_owned(),
                    });
                }
                insn += imm21 << (31-20) >> (31-20+20) << 31;
                insn += imm21 << (31-10) >> (31-10+1) << 21;
                insn += imm21 << (31-11) >> (31-11+11) << 20;
                insn += imm21 << (31-19) >> (31-19+12) << 12;
            }
            code_and_data.write_all(&insn.to_le_bytes())
                .map_err(|e| AssemblerError::Write {
                    line_num,
                    inner: e,
                })?;
            4
        },
        // InsnType::F => {
        //     insns.push(template);
        //     let pred = parts.next().expect("missing pred");
        //     let pred = parse_pred_succ(pred).unwrap();
        //     let succ = parts.next().expect("missing succ");
        //     let succ = parse_pred_succ(succ).unwrap();
        //     insns[0] += (pred << 24) + (succ << 20);
        //     print_insns
        // },
        InsnType::C => {
            let rd = parse_reg_here("rd", &mut chars)?;
            skip_whitespace(&mut chars);
            let rs1 = parse_reg_here("rs1", &mut chars)?;
            skip_whitespace(&mut chars);
            let csr = parse_imm_here(12, &mut chars)?;
            let insn = template + (rd << 7) + (rs1 << 15) + (csr << 20);
            code_and_data.write_all(&insn.to_le_bytes())
                .map_err(|e| AssemblerError::Write {
                    line_num,
                    inner: e,
                })?;
            4
        },
        InsnType::Ci => {
            let rd = parse_reg_here("rd", &mut chars)?;
            skip_whitespace(&mut chars);
            let imm5 = parse_imm_here(5, &mut chars)?;
            skip_whitespace(&mut chars);
            let csr = parse_imm_here(12, &mut chars)?;
            let insn = template + (rd << 7) + (imm5 << 15) + (csr << 20);
            code_and_data.write_all(&insn.to_le_bytes())
                .map_err(|e| AssemblerError::Write {
                    line_num,
                    inner: e,
                })?;
            4
        },
        InsnType::P => {
            if mnemonic == "li" {
                let rd = parse_reg_here("rd", &mut chars)?;
                skip_whitespace(&mut chars);
                let imm32_pos = chars.pos;
                let (_, imm32) = collect_word(&mut chars);
                let is_ident = is_identifier(&imm32);
                let mut imm20 = 0;
                let mut imm12 = 0;
                if !is_ident {
                    let imm32 = from_hex(&imm32, 32).map_err(
                        |e| AssemblerError::Syntax {line_num, col_num: imm32_pos, msg: e })?;
                    imm20 = (imm32 >> 12).wrapping_add((imm32 >> 11) & 1) & 0xF_FFFF;
                    imm12 = imm32 & 0xFFF;
                }
                let mut extra_insn_offset: u32 = 0;
                if is_ident {
                    relocations.relocations.push(Relocation {
                        offset: insn_offset + extra_insn_offset,
                        symbol_index: symbols.get_index_or_insert(
                            strings.get_index_or_insert(&imm32),
                            SymbolValue::Code { // FIXME: doesn't need to be code
                                external: false,
                                type_index: 0, // none
                                offset: None,
                            },
                        ),
                        value: RelocationValue::RelUType,
                    });
                    extra_insn_offset += assemble_line2(
                        mnemonics,
                        line_num,
                        &format!("auipc x{} {}", rd, u32_to_hex(imm20)),
                        insn_offset + extra_insn_offset,
                        &mut *strings,
                        &mut *symbols,
                        &mut *relocations,
                        &mut *code_and_data,
                    )?;
                } else {
                    extra_insn_offset += assemble_line2(
                        mnemonics,
                        line_num,
                        &format!("lui x{} {}", rd, u32_to_hex(imm20)),
                        insn_offset + extra_insn_offset,
                        &mut *strings,
                        &mut *symbols,
                        &mut *relocations,
                        &mut *code_and_data,
                    )?;
                }
                if is_ident {
                    // Relative to auipc's offset, addi's offset is extra_insn_offset too small. We
                    // have to compensate by adding that number.
                    imm12 = imm12.wrapping_add(extra_insn_offset) & 0xFFF;
                    relocations.relocations.push(Relocation {
                        offset: insn_offset + extra_insn_offset,
                        symbol_index: symbols.get_index_or_insert(
                            strings.get_index_or_insert(&imm32),
                            SymbolValue::Code { // FIXME: doesn't need to be code
                                external: false,
                                type_index: 0, // none
                                offset: None,
                            },
                        ),
                        value: RelocationValue::RelIType,
                    });
                }
                extra_insn_offset += assemble_line2(
                    mnemonics,
                    line_num,
                    &format!("addi x{} x{} {}", rd, rd, u32_to_hex(imm12)),
                    insn_offset + extra_insn_offset,
                    &mut *strings,
                    &mut *symbols,
                    &mut *relocations,
                    &mut *code_and_data,
                )?;
                extra_insn_offset
            } else if mnemonic == ".utf8" {
                let opening_quote_pos = chars.pos;
                if let Some((pos, c)) = chars.next() {
                    if c != '"' {
                        return Err(AssemblerError::Syntax {
                            line_num,
                            col_num: pos,
                            msg: "missing '\"'".to_owned(),
                        });
                    }
                } else {
                    return Err(AssemblerError::Syntax {
                        line_num,
                        col_num: chars.pos,
                        msg: "missing string".to_owned(),
                    });
                }
                let mut s = String::new();
                while let Some((_, c)) = chars.next() {
                    if c == '"' {
                        break;
                    } else if c == '\\' {
                        if let Some((_, c)) = chars.next() {
                            s.push(c);
                        } else {
                            return Err(AssemblerError::Syntax {
                                line_num,
                                col_num: chars.pos,
                                msg: "incomplete backslash escape".to_owned(),
                            });
                        }
                    } else {
                        s.push(c);
                    }
                }
                for _ in 0..(s.len().wrapping_neg() & 0b11) {
                    s.push('\0'); // conveniently 1 byte
                }
                code_and_data.write_all(s.as_bytes())
                    .map_err(|e| AssemblerError::Write {
                        line_num,
                        inner: e,
                    })?;
                if let Some(&(_, c)) = chars.peek() {
                    if !c.is_whitespace() {
                        return Err(AssemblerError::Syntax {
                            line_num,
                            col_num: chars.pos,
                            msg: "trailing characters after closing '\"'".to_owned(),
                        });
                    }
                }
                s.len() as u32
            } else {
                return Err(AssemblerError::Syntax {
                    line_num,
                    col_num: mnemonic_pos,
                    msg: format!("unknown mnemonic {:?}", mnemonic),
                });
            }
        },
        x => todo!("insn_type {:?}", x),
    };

    skip_whitespace(&mut chars);
    if chars.peek().is_some() {
        return Err(AssemblerError::Syntax {
            line_num,
            col_num: chars.pos,
            msg: "too many arguments".to_owned(),
        });
    }

    Ok(insn_len)
}

// fn assemble_line(
//     mnemonics: &HashMap<&str, (InsnType, u32)>,
//     addr: u32,
//     string_lens: &HashMap<String, u32>,
//     labels: &HashMap<String, u32>,
//     line: &str,
//     print_insns: bool,
// ) -> Vec<u32> {
//     let mut parts = line.split_ascii_whitespace();
//     let mnemonic = parts.next().expect("missing mnemonic");
//     let (insn_type, template) = *mnemonics.get(&mnemonic)
//         .unwrap_or_else(|| panic!("unknown mnemonic '{}'", &mnemonic));
//     let mut insns = vec![];
//     let print_first_insn = match insn_type {
//         InsnType::P => {
//             if mnemonic == "li" {
//                 let rd_str = parts.next().expect("missing rd");
//                 let _ = parse_reg(rd_str).unwrap();
//                 let imm_str = parts.next().expect("missing imm32");
//                 let imm_str1: String;
//                 let imm_str2: String;
//                 if imm_str.starts_with('%') {
//                     let label = &imm_str["%".len()..];
//                     if !string_lens.contains_key(label) {
//                         panic!("unknown label '{}'", label);
//                     }
//                     imm_str1 = imm_str.to_owned();
//                     imm_str2 = imm_str.to_owned();
//                 } else if labels.contains_key(imm_str) {
//                     imm_str1 = imm_str.to_owned();
//                     imm_str2 = imm_str.to_owned();
//                 } else {
//                     let imm32 = from_hex(imm_str, 32).unwrap();
//                     imm_str1 = format!("#{}", upper_imm20_to_hex(
//                         ((imm32 >> 12) + ((imm32 >> 11) & 1)) & 0xF_FFFF
//                     ));
//                     imm_str2 = format!("#{:03X}", imm32 & 0xFFF);
//                 };
//                 insns.append(&mut assemble_line(
//                     mnemonics,
//                     addr,
//                     string_lens,
//                     labels,
//                     &format!("lui {} {}", &rd_str, &imm_str1),
//                     print_insns,
//                 ));
//                 insns.append(&mut assemble_line(
//                     mnemonics,
//                     addr + 4,
//                     string_lens,
//                     labels,
//                     &format!("addi {} {} {}", &rd_str, &rd_str, &imm_str2),
//                     print_insns,
//                 ));
//                 false
//             } else {
//                 unreachable!();
//             }
//         },
//         InsnType::X => {
//             insns.push(template);
//             print_insns
//         },
//         InsnType::R => {
//             insns.push(template);
//             let rd = parts.next().expect("missing rd");
//             let rd = parse_reg(rd).unwrap();
//             let rs1 = parts.next().expect("missing rs1");
//             let rs1 = parse_reg(rs1).unwrap();
//             let rs2 = parts.next().expect("missing rs2");
//             let rs2 = parse_reg(rs2).unwrap();
//             insns[0] += (rd << 7) + (rs1 << 15) + (rs2 << 20);
//             print_insns
//         },
//         InsnType::Sxli => {
//             insns.push(template);
//             let rd = parts.next().expect("missing rd");
//             let rd = parse_reg(rd).unwrap();
//             let rs1 = parts.next().expect("missing rs1");
//             let rs1 = parse_reg(rs1).unwrap();
//             let imm = parts.next().expect("missing imm5");
//             let imm = from_hex(imm, 5).unwrap();
//             insns[0] += (rd << 7) + (rs1 << 15) + (imm << 20);
//             print_insns
//         },
//         InsnType::I => {
//             insns.push(template);
//             let rd = parts.next().expect("missing rd");
//             let rd = parse_reg(rd).unwrap();
//             let rs1 = parts.next().expect("missing rs1");
//             let rs1 = parse_reg(rs1).unwrap();
//             let imm_str = parts.next().expect("missing imm12");
//             let imm = if imm_str.starts_with('%') {
//                 let label = &imm_str["%".len()..];
//                 let len = *string_lens.get(label)
//                     .unwrap_or_else(
//                         || panic!("unknown label '{}'", label))
//                     as u32;
//                 len & 0xFFF
//             } else if let Some(&x) = labels.get(imm_str) {
//                 x & 0xFFF
//             } else {
//                 from_hex(imm_str, 12).unwrap()
//             };
//             insns[0] += (rd << 7) + (rs1 << 15) + (imm << 20);
//             print_insns
//         },
//         InsnType::S => {
//             insns.push(template);
//             let rs2 = parts.next().expect("missing rs2");
//             let rs2 = parse_reg(rs2).unwrap();
//             let rs1 = parts.next().expect("missing rs1");
//             let rs1 = parse_reg(rs1).unwrap();
//             let imm = parts.next().expect("missing imm12");
//             let imm = from_hex(imm, 12).unwrap();
//             insns[0] += (rs1 << 15) + (rs2 << 20);
//             insns[0] += imm << (31-11) >> (31-11+5) << 25;
//             insns[0] += imm << (31-4) >> (31-4+0) << 7;
//             print_insns
//         },
//         InsnType::B => {
//             insns.push(template);
//             let rs1 = parts.next().expect("missing rs1");
//             let rs1 = parse_reg(rs1).unwrap();
//             let rs2 = parts.next().expect("missing rs2");
//             let rs2 = parse_reg(rs2).unwrap();
//             let imm_str = parts.next().expect("missing imm13");
//             let imm = if imm_str.starts_with('#')
//                     || imm_str.starts_with('-') {
//                 let imm = from_hex(imm_str, 13).unwrap();
//                 if imm & 0x3 != 0 {
//                     panic!("low two bits of imm13 must be 0");
//                 }
//                 imm
//             } else {
//                 let label_addr = labels.get(imm_str).unwrap_or_else(
//                     || panic!("unknown label '{}'", imm_str));
//                 let displacement = label_addr.wrapping_sub(addr);
//                 if (displacement as i32) < -0x1000
//                         || (displacement as i32) > 0xFFF {
//                     panic!(
//                         "displacement {} is too large \
//                             for 13-bit immediate",
//                         u32_to_hex(displacement),
//                     );
//                 }
//                 displacement
//             };
//             insns[0] += (rs1 << 15) + (rs2 << 20);
//             insns[0] += imm << (31-12) >> (31-12+12) << 31;
//             insns[0] += imm << (31-10) >> (31-10+5) << 25;
//             insns[0] += imm << (31-4) >> (31-4+1) << 8;
//             insns[0] += imm << (31-11) >> (31-11+11) << 7;
//             print_insns
//         },
//         InsnType::U => {
//             insns.push(template);
//             let rd = parts.next().expect("missing rd");
//             let rd = parse_reg(rd).unwrap();
//             let imm_str = parts.next().expect("missing imm20");
//             let imm = if imm_str.starts_with('%') {
//                 let label = &imm_str["%".len()..];
//                 let len = *string_lens.get(label)
//                     .unwrap_or_else(
//                         || panic!("unknown label '{}'", label))
//                     as u32;
//                 // counteract sign extension of addi immediate
//                 (len >> 12) + ((len >> 11) & 1)
//             } else if let Some(&x) = labels.get(imm_str) {
//                 // counteract sign extension of addi immediate
//                 (x >> 12) + ((x >> 11) & 1)
//             } else {
//                 from_hex(imm_str, 20).unwrap()
//             };
//             insns[0] += (rd << 7) + (imm << 12);
//             print_insns
//         },
//         InsnType::J => {
//             insns.push(template);
//             let rd = parts.next().expect("missing rd");
//             let rd = parse_reg(rd).unwrap();
//             let imm_str = parts.next().expect("missing imm21");
//             let imm = if imm_str.starts_with('#')
//                     || imm_str.starts_with('-') {
//                 let imm = from_hex(imm_str, 21).unwrap();
//                 if imm & 0x3 != 0 {
//                     panic!("last two bits of imm21 must be 0");
//                 }
//                 imm
//             } else {
//                 let label_addr = labels.get(imm_str).unwrap_or_else(
//                     || panic!("unknown label '{}'", imm_str));
//                 let displacement = label_addr.wrapping_sub(addr);
//                 if (displacement as i32) < -0x10_0000
//                         || (displacement as i32) > 0xF_FFFF {
//                     panic!(
//                         "displacement {} is too large \
//                             for 21-bit immediate",
//                         u32_to_hex(displacement),
//                     );
//                 }
//                 displacement
//             };
//             insns[0] += rd << 7;
//             insns[0] += imm << (31-20) >> (31-20+20) << 31;
//             insns[0] += imm << (31-10) >> (31-10+1) << 21;
//             insns[0] += imm << (31-11) >> (31-11+11) << 20;
//             insns[0] += imm << (31-19) >> (31-19+12) << 12;
//             print_insns
//         },
//         InsnType::F => {
//             insns.push(template);
//             let pred = parts.next().expect("missing pred");
//             let pred = parse_pred_succ(pred).unwrap();
//             let succ = parts.next().expect("missing succ");
//             let succ = parse_pred_succ(succ).unwrap();
//             insns[0] += (pred << 24) + (succ << 20);
//             print_insns
//         },
//         InsnType::C => {
//             insns.push(template);
//             let rd = parts.next().expect("missing rd");
//             let rd = parse_reg(rd).unwrap();
//             let rs1 = parts.next().expect("missing rs1");
//             let rs1 = parse_reg(rs1).unwrap();
//             let csr = parts.next().expect("missing csr");
//             let csr = from_hex(csr, 12).unwrap();
//             insns[0] += (rd << 7) + (rs1 << 15) + (csr << 20);
//             print_insns
//         },
//         InsnType::Ci => {
//             insns.push(template);
//             let rd = parts.next().expect("missing rd");
//             let rd = parse_reg(rd).unwrap();
//             let uimm = parts.next().expect("missing uimm5");
//             let uimm = from_hex(uimm, 5).unwrap();
//             let csr = parts.next().expect("missing csr");
//             let csr = from_hex(csr, 12).unwrap();
//             insns[0] += (rd << 7) + (uimm << 15) + (csr << 20);
//             print_insns
//         },
//     };
//     assert!(insns.len() >= 1);
//     assert_eq!(parts.next(), None, "trailing operands");

//     if print_first_insn {
//         println!(
//             "{}: {}  {}", u32_to_hex(addr), u32_to_hex(insns[0]), line
//         );
//     }

//     insns
// }

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
    mnemonics.insert( ".utf8", (InsnType::P,              0));
    // TODO: .extern
    let mnemonics = mnemonics;

    let args: Vec<_> = env::args_os().collect();
    assert_eq!(args.len(), 4);
    let load_address: u32 = from_hex(args[1].to_str().expect("load address must be valid Unicode"), 32).unwrap_or_else(ouch);
    let mut input = fs::File::open(&args[2]).unwrap();
    let mut output = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&args[3]).unwrap();

    output.write_all(&[
        // magic = dc867b72-87f7-47da-a770-752af3299a3c
        0xdc, 0x86, 0x7b, 0x72, 0x87, 0xf7, 0x47, 0xda,
        0xa7, 0x70, 0x75, 0x2a, 0xf3, 0x29, 0x9a, 0x3c,

        0x00, // version
        0x00, 0x00, 0x00, // reserved

        0x00, 0x00, 0x00, 0x00, // load-address-offset
        0x00, 0x00, 0x00, 0x00, // code-and-data-offset
        0x00, 0x00, 0x00, 0x00, // string-table-offset
        0x00, 0x00, 0x00, 0x00, // symbol-table-offset
        0x00, 0x00, 0x00, 0x00, // relocation-table-offset
        0x01, 0x00, // arch = risc-v
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // reserved
    ]).unwrap_or_else(ouch);

    let load_address_offset = output.stream_position().unwrap_or_else(ouch) as u32;
    output.write_all(&load_address.to_le_bytes()).unwrap_or_else(ouch);

    let code_and_data_offset = output.stream_position().unwrap_or_else(ouch) as u32;

    let mut strings: StringTable = Default::default();
    strings.get_index_or_insert("");
    let mut symbols: SymbolTable = Default::default();
    let mut relocations: RelocationTable = Default::default();
    let mut insn_offset: u32 = 0;

    for (line_num, line) in io::BufReader::new(&input).lines().enumerate() {
        let mut line = line.unwrap();
        if let Some(n) = line.find(';') {
            line.truncate(n);
        }
        let byte_count = assemble_line2(
            &mnemonics,
            line_num,
            &line,
            insn_offset,
            &mut strings,
            &mut symbols,
            &mut relocations,
            &mut output,
        ).unwrap_or_else(ouch);
        insn_offset += byte_count as u32;
    }

    let string_table_offset = output.stream_position().unwrap_or_else(ouch) as u32;

    for reloc in &mut relocations.relocations {
        let sym = reloc.symbol(&symbols);
        if !sym.is_external() {
            print!("Applying relocation {:?} at offset {}\n", &reloc, u32_to_hex(reloc.offset));
            // Apply it.
            output.seek(SeekFrom::Start(
                code_and_data_offset as u64 + reloc.offset as u64
            )).unwrap_or_else(ouch);
            let insn = read_u32(&mut output).unwrap_or_else(ouch);
            let insn = reloc.apply(insn, &symbols).unwrap_or_else(ouch);
            output.seek(SeekFrom::Start(
                code_and_data_offset as u64 + reloc.offset as u64
            )).unwrap_or_else(ouch);
            output.write_all(&insn.to_le_bytes()).unwrap_or_else(ouch);
            reloc.value = RelocationValue::UnusedEntry;
        }
    }

    output.seek(SeekFrom::Start(string_table_offset as u64)).unwrap_or_else(ouch);
    strings.serialize(&mut output).unwrap_or_else(ouch);

    let symbol_table_offset = output.stream_position().unwrap_or_else(ouch) as u32;
    symbols.serialize(&mut output, &strings).unwrap_or_else(ouch);

    let relocation_table_offset = output.stream_position().unwrap_or_else(ouch) as u32;
    relocations.serialize(&mut output).unwrap_or_else(ouch);

    output.seek(SeekFrom::Start(0x14)).unwrap_or_else(ouch);
    output.write_all(&load_address_offset.to_le_bytes()).unwrap_or_else(ouch);
    output.write_all(&code_and_data_offset.to_le_bytes()).unwrap_or_else(ouch);
    output.write_all(&string_table_offset.to_le_bytes()).unwrap_or_else(ouch);
    output.write_all(&symbol_table_offset.to_le_bytes()).unwrap_or_else(ouch);
    output.write_all(&relocation_table_offset.to_le_bytes()).unwrap_or_else(ouch);

    output.sync_data().unwrap();
}
