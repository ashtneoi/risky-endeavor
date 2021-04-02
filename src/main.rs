struct Ann {
    aq: bool,
    rl: bool,
}

struct OrdSet {
    r: bool,
    w: bool,
}

enum MemOp {
    Load { rd: u32, rx: u32, val: u32 },
    Store { rs: u32, rx: u32 },
    Amo { rd: u32, rx: u32, rs: u32, load_val: u32, store_val: u32 },
    Lr { rd: u32, rx: u32, ann: Ann, val: u32 },
    Sc { rs: u32, rx: u32, ann: Ann },
    Calc { rd: u32, rs1: u32, rs2: u32, val: u32 },
    Fence { pred: OrdSet, succ: OrdSet },
}

impl MemOp {
    fn load(rd: u32, rx: u32, val: u32) -> Self {
        Self::Load { rd, rx, val }
    }

    fn store(rs: u32, rx: u32) -> Self {
        Self::Store { rs, rx }
    }

    fn calc(rd: u32, rs1: u32, rs2: u32, val: u32) -> Self {
        Self::Calc { rd, rs1, rs2, val }
    }

    fn src_addr(&self) -> Option<u32> {
        use MemOp::*;
        match *self {
            Load { rx, .. } | Store { rx, .. } if rx != 0 => Some(rx),
            Amo { rx, .. } if rx != 0 => Some(rx),
            Lr { rx, .. } | Sc { rx, .. } if rx != 0 => Some(rx),
            _ => None,
        }
    }

    fn src_data(&self) -> Vec<u32> {
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

    fn dest(&self) -> Option<u32> {
        use MemOp::*;
        match *self {
            Load { rd, .. } if rd != 0 => Some(rd),
            Amo { rd, .. } if rd != 0 => Some(rd),
            Lr { rd, .. } if rd != 0 => Some(rd),
            Calc { rd, .. } if rd != 0 => Some(rd),
            _ => None,
        }
    }

    fn carries_dep(&self) -> bool {
        use MemOp::*;
        match *self {
            Calc { .. } => true,
            _ => false,
        }
    }
}

struct SyntacticDeps {
    addr: Vec<(usize, usize, u32)>, // at, on, reg
    data: Vec<(usize, usize, u32)>, // at, on, reg
}

fn get_syntactic_deps(mem_ops: &[MemOp]) -> SyntacticDeps {
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
    let x = vec![
        MemOp::calc(2, 1, 1, 0),
        MemOp::calc(1, 0, 0, 0x200),
        MemOp::calc(2, 0, 0, 0x41),
        MemOp::calc(3, 0, 0, 0),
        MemOp::calc(3, 2, 0, 0x42),
        MemOp::store(1, 3),
    ];

    let deps = get_syntactic_deps(&x);
    println!("direct syntactic address deps:");
    for (j, i, d) in deps.addr {
        println!("{} on {}, reg {}", j, i, d);
    }
    println!("direct syntactic data deps:");
    for (j, i, d) in deps.data {
        println!("{} on {}, reg {}", j, i, d);
    }
}
