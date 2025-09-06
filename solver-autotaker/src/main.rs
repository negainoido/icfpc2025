use anyhow::{anyhow, Context, Result};
use clap::Parser;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Read, Write};
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
#[command(name = "solver-autotaker")]
#[command(about = "ICFPC 2025 solver (annealing MVP)", long_about = None)]
struct Args {
    /// Verbose output level
    #[arg(short, long, default_value_t = 0)]
    verbose: u8,

    /// Optional random seed
    #[arg(long)]
    seed: Option<u64>,

    /// Input JSON path ("-" for stdin)
    #[arg(short, long, default_value = "-")]
    input: String,

    /// Output JSON path ("-" for stdout)
    #[arg(short, long, default_value = "-")]
    output: String,

    /// Annealing iterations
    #[arg(long, default_value_t = 50_000usize)]
    iters: usize,

    /// Balance penalty weight (lambda)
    #[arg(long, default_value_t = 1.0_f32)]
    lambda_bal: f32,

    /// Optional time limit in seconds (stops annealing early)
    #[arg(long)]
    time_limit: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct InputProblem {
    plans: Vec<String>,
    results: Vec<Vec<u8>>, // each length should be plan.len()+1
    #[serde(rename = "N")]
    n: usize,
    #[serde(rename = "startingRoom")]
    starting_room: usize,
}

#[derive(Debug, Serialize)]
struct PortRef {
    room: usize,
    door: usize,
}

#[derive(Debug, Serialize)]
struct Connection {
    from: PortRef,
    to: PortRef,
}

#[derive(Debug, Serialize)]
struct OutputMap {
    rooms: Vec<u8>,
    #[serde(rename = "startingRoom")]
    starting_room: usize,
    connections: Vec<Connection>,
}

// --- Core model types ---
type Room = usize;
type Door = u8; // 0..=5
type Label = u8; // 0..=3

#[derive(Clone, Copy, Debug)]
struct PortIdx(pub usize); // p = q*6 + c

#[derive(Clone)]
struct Instance {
    plans: Vec<Vec<Door>>,    // normalized to 0..=5
    results: Vec<Vec<Label>>, // len = |plan|+1 for each
    n: usize,
    s0: Room,
}

#[derive(Clone)]
struct Model {
    labels: Vec<Label>,     // λ(q)
    match_to: Vec<usize>,   // μ: involution on 0..6N-1, μ[μ[p]]==p
}

#[derive(Default, Clone, Copy, Debug)]
struct Energy {
    obs: i32,
    balance: i32,
    total: i32,
}

fn to_port(q: Room, c: Door) -> PortIdx { PortIdx(q * 6 + c as usize) }
fn from_port(p: PortIdx) -> (Room, Door) { (p.0 / 6, (p.0 % 6) as u8) }

fn normalize_plans(plans: &[String]) -> Result<Vec<Vec<Door>>> {
    let mut out = Vec::with_capacity(plans.len());
    // detect if plans are 0..5 or 1..6 consistently
    let mut has_0_5 = false;
    let mut has_1_6 = false;
    for s in plans {
        for ch in s.chars() {
            if ch >= '0' && ch <= '5' { has_0_5 = true; }
            else if ch >= '1' && ch <= '6' { has_1_6 = true; }
            else if ch == '[' || ch == ']' { /* graffiti unsupported in MVP; ignore in detection */ }
            else { return Err(anyhow!("invalid plan character: {}", ch)); }
        }
    }
    if has_0_5 && has_1_6 {
        return Err(anyhow!("mixed 0..5 and 1..6 digits in plans are not supported"));
    }
    let dec = if has_1_6 { 1 } else { 0 };
    for s in plans {
        let mut v = Vec::with_capacity(s.len());
        let mut in_bracket = false;
        for ch in s.chars() {
            match ch {
                '[' => { in_bracket = true; },
                ']' => { in_bracket = false; },
                '0'..='9' => {
                    if in_bracket {
                        // graffiti write; MVP ignores these actions for modeling
                        continue;
                    }
                    let d = ch as u8 - b'0';
                    let d = d.saturating_sub(dec);
                    if d > 5 { return Err(anyhow!("door out of range after normalization")); }
                    v.push(d);
                }
                _ => {}
            }
        }
        out.push(v);
    }
    Ok(out)
}

fn validate_instance(n: usize, s0: usize, plans: &[Vec<Door>], results: &[Vec<Label>]) -> Result<()> {
    if n == 0 { return Err(anyhow!("N must be >= 1")); }
    if s0 >= n { return Err(anyhow!("startingRoom out of range")); }
    if plans.len() != results.len() { return Err(anyhow!("plans/results length mismatch")); }
    for (k, (p, r)) in plans.iter().zip(results.iter()).enumerate() {
        if r.len() != p.len() + 1 {
            return Err(anyhow!("results[{}].len() must be plan.len()+1", k));
        }
        if r.iter().any(|&x| x > 3) {
            return Err(anyhow!("results[{}] contains label > 3", k));
        }
    }
    Ok(())
}

fn build_initial(inst: &Instance, rng: &mut StdRng) -> Model {
    let n = inst.n;
    // Balanced labels (±1)
    let mut labels = vec![0u8; n];
    let m = (n / 4) as usize;
    let r = (n % 4) as usize;
    let mut pool: Vec<Label> = Vec::with_capacity(n);
    for l in 0..4u8 {
        let cnt = m + if (l as usize) < r { 1 } else { 0 };
        for _ in 0..cnt { pool.push(l); }
    }
    pool.shuffle(rng);
    for (i, &v) in pool.iter().enumerate() { labels[i] = v; }

    // Optionally enforce starting room label to match observed (use first plan's first label if any)
    if let Some(first) = inst.results.get(0).and_then(|v| v.first()).copied() {
        if first <= 3 { labels[inst.s0] = first; }
    }

    // match_to initialized to invalid
    let mut match_to = vec![usize::MAX; n * 6];
    // free bitmask per room: 6 bits set means free
    let mut free: Vec<u8> = vec![0b0011_1111; n];

    fn take_port(free: &mut [u8], q: usize, pref: Option<u8>) -> Option<u8> {
        if let Some(c) = pref {
            if (free[q] & (1u8 << c)) != 0 {
                free[q] &= !(1u8 << c);
                return Some(c);
            }
        }
        if free[q] == 0 { return None; }
        for c in 0u8..6u8 {
            if (free[q] & (1u8 << c)) != 0 {
                free[q] &= !(1u8 << c);
                return Some(c);
            }
        }
        None
    }

    fn connect(match_to: &mut [usize], p1: PortIdx, p2: PortIdx) {
        let i = p1.0; let j = p2.0;
        match_to[i] = j;
        match_to[j] = i;
    }

    // Build by tracing plans: wire (q, a) -> some q_to whose label matches next observed label
    for (plan, obs) in inst.plans.iter().zip(inst.results.iter()) {
        let mut q = inst.s0;
        for (i, &a) in plan.iter().enumerate() {
            let want = obs[i + 1];
            let p_from = to_port(q, a);
            // ensure from port is still free, otherwise skip (will close later)
            let from_free = (free[q] & (1u8 << a)) != 0;
            if !from_free {
                // cannot wire this move now; skip
                q = q; // stay; but for modeling, we still advance logically by following existing wiring if any
                if match_to[p_from.0] != usize::MAX {
                    // follow existing wiring if present
                    let p2 = match_to[p_from.0];
                    let (q2, _c2) = from_port(PortIdx(p2));
                    q = q2;
                }
                continue;
            }

            // choose destination room with label want and free port
            let mut q_to: Option<usize> = None;
            for cand in 0..n {
                if labels[cand] == want && free[cand] != 0 { q_to = Some(cand); break; }
            }
            if q_to.is_none() {
                // fallback: any room with free port
                for cand in 0..n { if free[cand] != 0 { q_to = Some(cand); break; } }
            }
            if let Some(q2) = q_to {
                // pick destination port, prefer opposite
                let pref = ((a as u8 + 3) % 6) as u8;
                let c2 = take_port(&mut free, q2, Some(pref)).or_else(|| take_port(&mut free, q2, None));
                if let Some(c2) = c2 {
                    // consume from-port now
                    let _ = take_port(&mut free, q, Some(a));
                    let p2 = to_port(q2, c2);
                    connect(&mut match_to, p_from, p2);
                    q = q2;
                    continue;
                }
            }
            // failed to choose destination; leave from-port dangling (we consumed nothing)
            // do nothing; will be closed later
        }
    }

    // Close remaining free ports greedily
    let mut free_ports: Vec<usize> = Vec::new();
    for q in 0..n {
        for c in 0..6u8 {
            if (free[q] & (1u8 << c)) != 0 {
                free[q] &= !(1u8 << c);
                free_ports.push(to_port(q, c).0);
            }
        }
    }
    // Pair sequentially
    let mut i = 0usize;
    while i + 1 < free_ports.len() {
        let p1 = free_ports[i];
        let p2 = free_ports[i + 1];
        match_to[p1] = p2;
        match_to[p2] = p1;
        i += 2;
    }
    if i < free_ports.len() {
        // one leftover -> self-loop
        let p = free_ports[i];
        match_to[p] = p;
    }

    Model { labels, match_to }
}

fn simulate(model: &Model, s0: Room, plan: &[Door]) -> Vec<Label> {
    let mut q = s0;
    let mut out = Vec::with_capacity(plan.len() + 1);
    out.push(model.labels[q]);
    for &a in plan {
        let p = to_port(q, a);
        let p2 = model.match_to[p.0];
        if p2 >= model.match_to.len() { // sanitize on the fly (shouldn't happen after finalize)
            // treat as self-loop to stay in the same room
            out.push(model.labels[q]);
            continue;
        }
        let (q2, _c2) = from_port(PortIdx(p2));
        q = q2;
        out.push(model.labels[q]);
    }
    out
}

fn energy(inst: &Instance, m: &Model, lambda_bal: f32) -> Energy {
    // obs mismatch
    let mut obs: i32 = 0;
    for (plan, expect) in inst.plans.iter().zip(inst.results.iter()) {
        let got = simulate(m, inst.s0, plan);
        for (g, &e) in got.iter().zip(expect.iter()) { if *g != e { obs += 1; } }
    }
    // balance
    let mut cnt = [0i32; 4];
    for &l in &m.labels { cnt[l as usize] += 1; }
    let n = inst.n as i32;
    let base = n / 4;
    let rem = (n % 4) as usize;
    let mut balance = 0i32;
    for l in 0..4usize {
        let target = base + if l < rem as usize { 1 } else { 0 };
        let d = cnt[l] - target;
        balance += d * d;
    }
    let balance = ((lambda_bal * balance as f32).round() as i32).max(0);
    Energy { obs, balance, total: obs + balance }
}

fn two_opt(m: &mut Model, p1: usize, p2: usize) {
    if p1 == p2 { return; }
    let a = p1; let b = m.match_to[p1];
    let c = p2; let d = m.match_to[p2];
    // choose randomly pattern A or B outside; caller will handle randomness; we'll implement A here and B by swapping args
    // Here we implement A: a<->c, b<->d
    m.match_to[a] = c; m.match_to[c] = a;
    m.match_to[b] = d; m.match_to[d] = b;
}

fn two_opt_b(m: &mut Model, p1: usize, p2: usize) {
    if p1 == p2 { return; }
    let a = p1; let b = m.match_to[p1];
    let c = p2; let d = m.match_to[p2];
    // pattern B: a<->d, b<->c
    m.match_to[a] = d; m.match_to[d] = a;
    m.match_to[b] = c; m.match_to[c] = b;
}

fn swap_labels(m: &mut Model, q1: usize, q2: usize) {
    if q1 == q2 { return; }
    m.labels.swap(q1, q2);
}

fn anneal(inst: &Instance, model: &mut Model, iters: usize, lambda_bal: f32, rng: &mut StdRng, time_limit: Option<Duration>) -> Energy {
    let start_t = Instant::now();
    let mut cur = energy(inst, model, lambda_bal);
    let mut best = cur;
    let mut best_model = model.clone();
    let n_ports = inst.n * 6;

    // temperature schedule
    let mut t = 1.0_f32;
    let alpha = 0.999_f32;
    for k in 0..iters {
        if let Some(limit) = time_limit { if start_t.elapsed() >= limit { break; } }
        // randomly choose move
        let mv: f32 = rng.r#gen();
        if mv < 0.7 {
            // 2-opt move
            let p1 = rng.gen_range(0..n_ports);
            let mut p2 = rng.gen_range(0..n_ports);
            if p2 == p1 { p2 = (p2 + 1) % n_ports; }
            let old_a = model.match_to[p1];
            let old_c = model.match_to[p2];
            let pattern_b: bool = rng.gen_bool(0.5);
            if pattern_b { two_opt_b(model, p1, p2); } else { two_opt(model, p1, p2); }
            let new_e = energy(inst, model, lambda_bal);
            let d = (new_e.total - cur.total) as f32;
            let accept = d <= 0.0 || rng.r#gen::<f32>() < (-d / t.max(1e-6)).exp();
            if accept {
                cur = new_e;
                if new_e.total < best.total { best = new_e; best_model = model.clone(); }
            } else {
                // revert
                model.match_to[p1] = old_a;
                model.match_to[old_a] = p1;
                model.match_to[p2] = old_c;
                model.match_to[old_c] = p2;
            }
        } else {
            // label swap
            let q1 = rng.gen_range(0..inst.n);
            let mut q2 = rng.gen_range(0..inst.n);
            if q2 == q1 { q2 = (q2 + 1) % inst.n; }
            model.labels.swap(q1, q2);
            let new_e = energy(inst, model, lambda_bal);
            let d = (new_e.total - cur.total) as f32;
            let accept = d <= 0.0 || rng.r#gen::<f32>() < (-d / t.max(1e-6)).exp();
            if accept {
                cur = new_e;
                if new_e.total < best.total { best = new_e; best_model = model.clone(); }
            } else {
                model.labels.swap(q1, q2);
            }
        }
        t *= alpha;
        if t < 1e-4 { t = 1e-4; }
        // cheap break if perfect fit on observations
        if best.obs == 0 { /* keep going to balance */ }
    }
    *model = best_model;
    best
}

fn emit_output(model: &Model, s0: Room) -> OutputMap {
    let n = model.labels.len();
    let mut conns: Vec<Connection> = Vec::new();
    for p in 0..(n * 6) {
        let q = model.match_to[p];
        if p > q { continue; } // each undirected edge once
        let (rq, dc) = from_port(PortIdx(p));
        let (rq2, dc2) = from_port(PortIdx(q));
        conns.push(Connection {
            from: PortRef { room: rq, door: dc as usize },
            to: PortRef { room: rq2, door: dc2 as usize },
        });
    }
    OutputMap { rooms: model.labels.clone(), starting_room: s0, connections: conns }
}

fn finalize_match_to(model: &mut Model) {
    let n_ports = model.match_to.len();
    // First, clamp out-of-range entries to self-loops
    for p in 0..n_ports {
        let q = model.match_to[p];
        if q >= n_ports { model.match_to[p] = p; }
    }
    // Ensure involution: if μ[μ[p]] != p, fix by mutual pairing or self-loop
    for p in 0..n_ports {
        let q = model.match_to[p];
        if q >= n_ports { model.match_to[p] = p; continue; }
        let qq = model.match_to[q];
        if qq != p {
            // Try to make them mutual
            model.match_to[p] = q;
            model.match_to[q] = p;
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    if args.verbose > 0 {
        eprintln!("solver-autotaker: starting (seed={:?}, iters={}, lambda_bal={})", args.seed, args.iters, args.lambda_bal);
    }
    // Read input JSON
    let mut input_json = String::new();
    if args.input == "-" {
        io::stdin().read_to_string(&mut input_json)?;
    } else {
        input_json = fs::read_to_string(&args.input)
            .with_context(|| format!("failed to read input file: {}", &args.input))?;
    }

    let raw: InputProblem = serde_json::from_str(&input_json).context("failed to parse input JSON")?;

    // Normalize and validate
    let plans = normalize_plans(&raw.plans)?;
    validate_instance(raw.n, raw.starting_room, &plans, &raw.results)?;
    let inst = Instance { plans, results: raw.results.clone(), n: raw.n, s0: raw.starting_room };

    // RNG
    let seed = args.seed.unwrap_or_else(|| {
        // derive some entropy from time
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap();
        (t.as_nanos() as u64) ^ 0x9E3779B97F4A7C15u64
    });
    let mut rng = StdRng::seed_from_u64(seed);

    // Build initial and anneal
    let mut model = build_initial(&inst, &mut rng);
    finalize_match_to(&mut model);
    let time_limit = args.time_limit.map(|s| Duration::from_secs_f32(s));
    let best_e = anneal(&inst, &mut model, args.iters, args.lambda_bal, &mut rng, time_limit);
    if args.verbose > 0 {
        eprintln!("energy: obs={}, balance={}, total={}", best_e.obs, best_e.balance, best_e.total);
    }

    let out = emit_output(&model, inst.s0);

    let serialized = serde_json::to_string_pretty(&out)?;

    if args.output == "-" {
        let mut stdout = io::stdout().lock();
        stdout.write_all(serialized.as_bytes())?;
        stdout.write_all(b"\n")?;
    } else {
        fs::write(&args.output, serialized)
            .with_context(|| format!("failed to write output file: {}", &args.output))?;
    }

    Ok(())
}
