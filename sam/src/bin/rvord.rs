use core::fmt;
use core::str::FromStr;
use sam::{from_hex, parse_reg};
use std::collections::HashMap;
use std::io::{self, prelude::*};

#[derive(Clone, Copy)]
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

#[derive(Clone, Copy)]
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

// Load values are assertions. Store and calc values are simply true.
pub enum Insn {
    Load { rd: u32, rx: u32, val: u32 },
    Store { rs: u32, rx: u32 },
    Amo { rd: u32, rx: u32, rs: u32, ann: Ann, load_val: u32, store_val: u32 },
    Lr { rd: u32, rx: u32, ann: Ann, val: u32 },
    Sc { rs: u32, rx: u32, ann: Ann }, // with our SC success constraint
    Calc { rd: u32, rs1: u32, rs2: u32, val: u32 },
    Fence { pred: OrdSet, succ: OrdSet },
}

impl FromStr for Insn {
    type Err = String; // TODO?
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let tokens: Vec<_> = s.split_ascii_whitespace().collect();
        if tokens.len() < 1 {
            return Err("missing kind".to_string());
        }

        let arg_count = match tokens[0] {
            "load" => 3,
            "store" => 2,
            "amo" => 6,
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
            "load" => Insn::Load {
                rd: parse_reg(args[0])?,
                rx: parse_reg(args[1])?,
                val: from_hex(args[2], 32)?,
            },
            "store" => Insn::Store {
                rs: parse_reg(args[0])?,
                rx: parse_reg(args[1])?,
            },
            "amo" => Insn::Amo {
                rd: parse_reg(args[0])?,
                rx: parse_reg(args[1])?,
                rs: parse_reg(args[2])?,
                ann: Ann::from_str(args[3])?,
                load_val: from_hex(args[4], 32)?,
                store_val: from_hex(args[5], 32)?,
            },
            "lr" => Insn::Lr {
                rd: parse_reg(args[0])?,
                rx: parse_reg(args[1])?,
                ann: Ann::from_str(args[2])?,
                val: from_hex(args[3], 32)?,
            },
            "sc" => Insn::Sc {
                rs: parse_reg(args[0])?,
                rx: parse_reg(args[1])?,
                ann: Ann::from_str(args[2])?,
            },
            "calc" => Insn::Calc {
                rd: parse_reg(args[0])?,
                rs1: parse_reg(args[1])?,
                rs2: parse_reg(args[2])?,
                val: from_hex(args[3], 32)?,
            },
            "fence" => Insn::Fence {
                pred: OrdSet::from_str(args[0])?,
                succ: OrdSet::from_str(args[1])?,
            },
            _ => unreachable!(),
        })
    }
}

impl Insn {
    pub fn is_load(&self) -> bool {
        use Insn::*;
        match *self {
            Load { .. } => true,
            Amo { .. } => true,
            Lr { .. } => true,
            _ => false,
        }
    }

    pub fn is_store(&self) -> bool {
        use Insn::*;
        match *self {
            Store { .. } => true,
            Amo { .. } => true,
            Sc { .. } => true,
            _ => false,
        }
    }

    pub fn annotation(&self) -> Option<Ann> {
        use Insn::*;
        match *self {
            Amo { ann, .. }
            | Lr { ann, .. }
            | Sc { ann, .. } => Some(ann),
            _ => None,
        }
    }

    pub fn src_addr(&self) -> Option<u32> {
        use Insn::*;
        match *self {
            Load { rx, .. } | Store { rx, .. } if rx != 0 => Some(rx),
            Amo { rx, .. } if rx != 0 => Some(rx),
            Lr { rx, .. } | Sc { rx, .. } if rx != 0 => Some(rx),
            _ => None,
        }
    }

    pub fn src_data(&self) -> Vec<u32> {
        use Insn::*;
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
        use Insn::*;
        match *self {
            Load { rd, .. } if rd != 0 => Some(rd),
            Amo { rd, .. } if rd != 0 => Some(rd),
            Lr { rd, .. } if rd != 0 => Some(rd),
            Calc { rd, .. } if rd != 0 => Some(rd),
            _ => None,
        }
    }

    pub fn carries_dep(&self) -> bool {
        use Insn::*;
        match *self {
            Calc { .. } => true,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum SynDepKind {
    Addr,
    Data,
}

impl fmt::Display for SynDepKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            SynDepKind::Addr => write!(f, "address"),
            SynDepKind::Data => write!(f, "data"),
        }
    }
}

pub struct SyntacticDeps {
    direct: Vec<(usize, usize, u32, SynDepKind)>, // at, on, reg, kind
    indirect: Vec<(usize, usize, u32, SynDepKind)>, // at, on, `at` reg, kind
}

impl SyntacticDeps {
    pub fn from_program(prog: &[Insn]) -> Self {
        let mut deps = Self { direct: Vec::new(), indirect: Vec::new() };

        for j in (0..prog.len()).rev() {
            let j_src_addr = prog[j].src_addr();
            let j_src_data = prog[j].src_data();
            if j_src_addr.is_some() || !j_src_data.is_empty() {
                let mut dests_found = Vec::new();
                for i in (0..j).rev() {
                    if let Some(s) = prog[i].dest() {
                        if let Some(r) = j_src_addr {
                            if r == s && !dests_found.contains(&r) {
                                deps.direct.push((j, i, r, SynDepKind::Addr));
                            }
                        }

                        if j_src_data.contains(&s)
                                && !dests_found.contains(&s) {
                            deps.direct.push((j, i, s, SynDepKind::Data));
                        }

                        dests_found.push(s);
                    }
                }
            }
        }

        for (x, &(j, m, r, kind)) in deps.direct.iter().enumerate() {
            let mut m = m;
            for &(m2, i, _, _) in &deps.direct[x+1..] {
                if m == m2 && prog[m2].carries_dep() {
                    deps.indirect.push((j, i, r, kind));
                    m = i;
                }
            }
        }

        deps
    }

    pub fn register_value(&self, prog: &[Insn], reg: u32, at: usize) -> u32 {
        for &(j, i, d, _) in &self.direct {
            if j == at && d == reg {
                return match prog[i] {
                    Insn::Load { rd, val, .. } if rd == reg => val,
                    Insn::Amo { rd, load_val, .. } if rd == reg => load_val,
                    Insn::Lr { rd, val, .. } if rd == reg => val,
                    Insn::Calc { rd, val, .. } if rd == reg => val,
                    _ => unreachable!(),
                };
            }
        }
        panic!("register {} is uninitialized", reg);
    }
}

type PreservedProgramOrder = Vec<(usize, usize, u32)>; // (before, after, rule)

type GlobalMemoryOrder = Vec<(usize, usize)>; // (hart, program index)

pub struct RegMod {
    reg: u32,
    load_addr: Option<u32>,
    val: u32,
}

pub fn load_value_axiom( // name sucks
    progs: &[&[Insn]],
    gmo: &GlobalMemoryOrder,
    gmo_idx: usize,
) -> u32 {
    use Insn::*;

    let (hart_id, prog_idx) = gmo[gmo_idx];
    let insn = &progs[hart_id][prog_idx];
    for g in (gmo_idx+1..gmo.len()).rev() {
        let (h, p) = gmo[g];
        if p < prog_idx {
            let i = &progs[h][p];
            if i.is_store() {
                let src_addr = i.src_addr();
                // figure out value of src_addr?

                // ???
            }
        }
    }
}

pub fn check_load_value_assertions(
    progs: &[&[Insn]],
    gmo: &GlobalMemoryOrder,
) -> Result<(), String> {
    use Insn::*;

    let mut reg_mods: Vec<Vec<RegMod>> = Vec::new();

    for (gmo_idx, (hart_id, prog_idx)) in gmo.iter().enumerate() {
        match progs[hart_id][prog_idx] {
            Load { rd, rx, val } => {
                // Use the load value axiom.
                todo!()
            },
            _ => todo!(),
        }
    }

    todo!()
}

/// None means the GMO violated one of our simplifying contraints.
pub fn compute_ppo(
    _hart_id: usize,
    prog: &[Insn],
    syn_deps: &SyntacticDeps,
    _gmo: &GlobalMemoryOrder,
) -> Option<PreservedProgramOrder> {
    // Implementation note: We can't assume our load value assertions here,
    // because they're assertions, not PPO constraints.

    use Insn::*;

    let mut ppo = PreservedProgramOrder::new();
    for a in 0..prog.len() {
        if let &Calc { .. } | Fence { .. } = &prog[a] {
            continue;
        }
        for b in a+1..prog.len() {
            if let &Calc { .. } | &Fence { .. } = &prog[b] {
                continue;
            }

            // 1.
            if prog[b].is_store() {
                let brx = prog[b].src_addr().unwrap();
                if let Some(arx) = prog[a].src_addr() {
                    // FIXME: register_value() uses load value assertions!!!
                    // can't do that
                    panic!("FIXME");
                    let a_addr = syn_deps.register_value(prog, arx, a);
                    let b_addr = syn_deps.register_value(prog, brx, b);
                    if a_addr == b_addr {
                        ppo.push((a, b, 1));
                    }
                }
            }

            // 2.
            if prog[a].is_load() && prog[b].is_load() {
                let arx = prog[a].src_addr().unwrap();
                let brx = prog[b].src_addr().unwrap();
                // FIXME: register_value() uses load value assertions!!! can't
                // do that
                panic!("FIXME");
                let a_addr = syn_deps.register_value(prog, arx, a);
                let b_addr = syn_deps.register_value(prog, brx, b);
                if a_addr == b_addr {
                    let mut no_store_between = true;
                    for m in a+1..b {
                        if prog[m].is_store() {
                            let mrx = prog[m].src_addr().unwrap();
                            // FIXME: register_value() uses load value
                            // assertions!!! can't do that
                            panic!("FIXME");
                            let m_addr = syn_deps.register_value(prog, mrx, m);
                            if m_addr == a_addr {
                                no_store_between = false;
                                break;
                            }
                        }
                    }
                    if no_store_between {
                        // TODO: we need the load value axiom
                        todo!();
                    }
                }
            }

            // 3.
            if let &Amo { .. } | &Sc { .. } = &prog[a] {
                if prog[b].is_load() {
                    // TODO: we need the load value axiom
                    todo!();
                }
            }

            // 4.
            for m in a+1..b {
                if let &Fence { pred, succ } = &prog[m] {
                    let a_pred =
                        (pred.r && prog[a].is_load())
                        || (pred.w && prog[a].is_store());
                    let b_succ =
                        (succ.r && prog[b].is_load())
                        || (succ.w && prog[b].is_store());
                    if a_pred && b_succ {
                        ppo.push((a, b, 4));
                        break;
                    }
                }
            }

            // 5.
            if let Some(ann) = prog[a].annotation() {
                if ann.aq {
                    ppo.push((a, b, 5));
                }
            }

            // 6.
            if let Some(ann) = prog[b].annotation() {
                if ann.rl {
                    ppo.push((a, b, 6));
                }
            }

            // 7.
            if let (Some(a_ann), Some(b_ann)) =
                    (prog[a].annotation(), prog[b].annotation()) {
                if (a_ann.aq || a_ann.rl) && (b_ann.aq || b_ann.rl) {
                    ppo.push((a, b, 7));
                }
            }

            // 8.
            if let (&Lr { .. }, &Sc { .. }) = (&prog[a], &prog[b]) {
                // TODO: Return None if the GMO violates our SC success
                // constraint.
                // TODO: Do the rest of 8.
                todo!();
            }

            // 9. and 10.
            for &(j, i, _, k) in
                    syn_deps.direct.iter().chain(&syn_deps.indirect) {
                if a == i && b == j {
                    match k {
                        SynDepKind::Addr => ppo.push((a, b, 9)),
                        SynDepKind::Data => ppo.push((a, b, 10)),
                    }
                }
            }

            // 11. is never true since we don't have control instructions.

            // 12.
            // TODO

            // 13.
            if prog[b].is_store() {
                for m in a+1..b {
                    for &(j, i, _, k) in
                            syn_deps.direct.iter().chain(&syn_deps.indirect) {
                        if a == i && m == j && k == SynDepKind::Addr {
                            ppo.push((a, b, 13));
                        }
                    }
                }
            }
        }
    }

    Some(ppo)
}

fn main() {
    let mut progs: Vec<Vec<Insn>> = vec![vec![]];
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line_untrimmed = line.unwrap();
        let line = line_untrimmed.trim();
        match line {
            "" => (),
            "-" => progs.push(vec![]),
            _ => progs.last_mut().unwrap().push(line.parse().unwrap()),
        }
    }

    let mut gmo = GlobalMemoryOrder::new();
    for (hart_id, prog) in progs.iter().enumerate() {
        for i in 0..prog.len() {
            gmo.push((hart_id, i));
        }
    }

    for (hart_id, prog) in progs.iter().enumerate() {
        let deps = SyntacticDeps::from_program(prog);

        println!("direct syntactic deps:");
        for &(j, i, d, k) in &deps.direct {
            println!("{} on {}, reg {}, kind {}", j + 1, i + 1, d, k);
        }
        println!();

        println!("indirect syntactic deps:");
        for &(j, i, d, k) in &deps.indirect {
            println!("{} on {}, reg {}, kind {}", j + 1, i + 1, d, k);
        }
        println!();

        // TODO: Somebody has to check the load value assertions.

        println!("preserved program order:");
        let ppo = compute_ppo(hart_id, prog, &deps, &gmo).unwrap();
        for (a, b, rule) in ppo {
            println!("{} before {} (rule {})", a + 1, b + 1, rule);
        }
        println!();
    }
}
