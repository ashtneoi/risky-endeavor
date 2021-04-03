use core::str::FromStr;
use sam::{from_hex, parse_reg};
use std::io::{self, prelude::*};

pub struct Ann {
    pub aq: bool,
    pub rl: bool,
}

impl FromStr for Ann {
    type Err = String; // TODO?
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "z" => Ann { aq: false, rl: false },
            "aq" => Ann { aq: true, rl: false },
            "rl" => Ann { aq: false, rl: true },
            "aqrl" => Ann { aq: true, rl: true },
            _ => return Err("invalid ordering annotation".to_string()),
        })
    }
}

pub struct OrdSet {
    pub r: bool,
    pub w: bool,
}

impl FromStr for OrdSet {
    type Err = String; // TODO?
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "r" => OrdSet { r: true, w: false },
            "w" => OrdSet { r: false, w: true },
            "rw" => OrdSet { r: true, w: true },
            _ => return Err("invalid ordering set".to_string()),
        })
    }
}

pub enum MemOp {
    Load { rd: u32, rx: u32, val: u32 },
    Store { rs: u32, rx: u32 },
    Amo { rd: u32, rx: u32, rs: u32, load_val: u32, store_val: u32 },
    Lr { rd: u32, rx: u32, ann: Ann, val: u32 },
    Sc { rs: u32, rx: u32, ann: Ann },
    Calc { rd: u32, rs1: u32, rs2: u32, val: u32 },
    Fence { pred: OrdSet, succ: OrdSet },
}

impl FromStr for MemOp {
    type Err = String; // TODO?
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let tokens: Vec<_> = s.split_ascii_whitespace().collect();
        if tokens.len() < 1 {
            return Err("missing kind".to_string());
        }

        let arg_count = match tokens[0] {
            "load" => 3,
            "store" => 2,
            "amo" => 5,
            "lr" => 4,
            "sc" => 3,
            "calc" => 4,
            "fence" => 2,
            _ => return Err("invalid kind".to_string()),
        };
        let args = &tokens[1..];
        if args.len() != arg_count {
            return Err(format!("wrong number of args (wanted {})", arg_count));
        }
        Ok(match tokens[0] {
            "load" => MemOp::Load {
                rd: parse_reg(args[0])?,
                rx: parse_reg(args[1])?,
                val: from_hex(args[2], 32)?,
            },
            "store" => MemOp::Store {
                rs: parse_reg(args[0])?,
                rx: parse_reg(args[1])?,
            },
            "amo" => MemOp::Amo {
                rd: parse_reg(args[0])?,
                rx: parse_reg(args[1])?,
                rs: parse_reg(args[2])?,
                load_val: from_hex(args[3], 32)?,
                store_val: from_hex(args[4], 32)?,
            },
            "lr" => MemOp::Lr {
                rd: parse_reg(args[0])?,
                rx: parse_reg(args[1])?,
                ann: Ann::from_str(args[2])?,
                val: from_hex(args[3], 32)?,
            },
            "sc" => MemOp::Sc {
                rs: parse_reg(args[0])?,
                rx: parse_reg(args[1])?,
                ann: Ann::from_str(args[2])?,
            },
            "calc" => MemOp::Calc {
                rd: parse_reg(args[0])?,
                rs1: parse_reg(args[1])?,
                rs2: parse_reg(args[2])?,
                val: from_hex(args[3], 32)?,
            },
            "fence" => MemOp::Fence {
                pred: OrdSet::from_str(args[0])?,
                succ: OrdSet::from_str(args[1])?,
            },
            _ => unreachable!(),
        })
    }
}

impl MemOp {
    pub fn src_addr(&self) -> Option<u32> {
        use MemOp::*;
        match *self {
            Load { rx, .. } | Store { rx, .. } if rx != 0 => Some(rx),
            Amo { rx, .. } if rx != 0 => Some(rx),
            Lr { rx, .. } | Sc { rx, .. } if rx != 0 => Some(rx),
            _ => None,
        }
    }

    pub fn src_data(&self) -> Vec<u32> {
        use MemOp::*;
        match *self {
            Store { rs, .. } if rs != 0 => vec![rs],
            Amo { rs, .. } if rs != 0 => vec![rs],
            Sc { rs, .. } if rs != 0 => vec![rs],
            Calc { rs1, rs2, .. } => {
                let mut v = Vec::new();
                if rs1 != 0 {
                    v.push(rs1);
                }
                if rs2 != 0 {
                    v.push(rs2);
                }
                v
            }
            _ => vec![],
        }
    }

    pub fn dest(&self) -> Option<u32> {
        use MemOp::*;
        match *self {
            Load { rd, .. } if rd != 0 => Some(rd),
            Amo { rd, .. } if rd != 0 => Some(rd),
            Lr { rd, .. } if rd != 0 => Some(rd),
            Calc { rd, .. } if rd != 0 => Some(rd),
            _ => None,
        }
    }

    pub fn carries_dep(&self) -> bool {
        use MemOp::*;
        match *self {
            Calc { .. } => true,
            _ => false,
        }
    }
}

pub struct SyntacticDeps {
    addr: Vec<(usize, usize, u32)>, // at, on, reg
    data: Vec<(usize, usize, u32)>, // at, on, reg
}

pub fn get_syntactic_deps(mem_ops: &[MemOp]) -> SyntacticDeps {
    let mut deps = SyntacticDeps { addr: Vec::new(), data: Vec::new() };

    for j in (0..mem_ops.len()).rev() {
        let j_src_addr = mem_ops[j].src_addr();
        let j_src_data = mem_ops[j].src_data();
        if j_src_addr.is_some() || !j_src_data.is_empty() {
            let mut dests_found = Vec::new();
            for i in (0..j).rev() {
                if let Some(d) = mem_ops[i].dest() {
                    if let Some(sa) = j_src_addr {
                        if d == sa && !dests_found.contains(&d) {
                            deps.addr.push((j, i, d));
                        }
                    }

                    if j_src_data.contains(&d) && !dests_found.contains(&d) {
                        deps.data.push((j, i, d));
                    }

                    dests_found.push(d);
                }
            }
        }
    }

    deps
}

fn main() {
    let mut ops: Vec<MemOp> = Vec::new();
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        ops.push(line.parse().unwrap());
    }

    let deps = get_syntactic_deps(&ops);
    println!("direct syntactic address deps:");
    for (j, i, d) in deps.addr {
        println!("{} on {}, reg {}", j, i, d);
    }
    println!("direct syntactic data deps:");
    for (j, i, d) in deps.data {
        println!("{} on {}, reg {}", j, i, d);
    }
}
