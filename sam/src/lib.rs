use std::fmt::Display;
use std::process::exit;

// Having this return ! makes the type checker say e.g. "expected `!`, found `usize`".
pub fn ouch<E: Display, X>(e: E) -> X {
    eprintln!("{}", e);
    exit(1);
}

pub fn from_hex(s: &str, width: u32) -> Result<u32, String> {
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
