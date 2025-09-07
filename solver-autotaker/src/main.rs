use anyhow::{anyhow, Context, Result};
use clap::Parser;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
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

    /// Log progress every N iterations (emit only if verbose>0)
    #[arg(long)]
    log_every: Option<usize>,

    /// Save intermediate best map every N iterations (uses --output as base)
    #[arg(long)]
    save_every: Option<usize>,

    /// Initial temperature
    #[arg(long, default_value_t = 1.0_f32)]
    t0: f32,

    /// Cooling factor per iteration
    #[arg(long, default_value_t = 0.999_f32)]
    alpha: f32,

    /// Minimum temperature clamp
    #[arg(long, default_value_t = 1e-4_f32)]
    tmin: f32,

    /// Number of restarts (multi-start annealing)
    #[arg(long, default_value_t = 1usize)]
    restarts: usize,

    /// Reheat if no improvement for this many iterations
    #[arg(long)]
    reheat_every: Option<usize>,

    /// Temperature to reset to on reheat
    #[arg(long)]
    reheat_to: Option<f32>,

    /// Probability of 3-edge loop-change rewire move
    #[arg(long, default_value_t = 0.05_f32)]
    p_loopmove: f32,
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
    match_to: Vec<usize>,   // μ: involution on 0..6N-1, μ[μ[p]]==p
}

fn check_involution(m: &Model) -> bool {
    let n = m.match_to.len();
    for p in 0..n {
        let q = m.match_to[p];
        if q >= n { eprintln!("involution fail: p={} q(out)={}", p, q); return false; }
        let r = m.match_to[q];
        if r != p {
            eprintln!("involution fail: p={} q={} r={}", p, q, r);
            return false;
        }
    }
    true
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

fn build_initial(inst: &Instance, _rng: &mut StdRng) -> Model {
    let n = inst.n;
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

    // Build by tracing plans: wire (q, a) to some available destination port greedily
    for plan in inst.plans.iter() {
        let mut q = inst.s0;
        for &a in plan.iter() {
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
            // choose any destination room with a free port
            let mut q_to: Option<usize> = None;
            for cand in 0..n { if free[cand] != 0 { q_to = Some(cand); break; } }
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

    Model { match_to }
}

fn trace_rooms(match_to: &[usize], s0: Room, plan: &[Door]) -> Vec<Room> {
    let mut q = s0;
    let mut out = Vec::with_capacity(plan.len() + 1);
    out.push(q);
    let n_ports = match_to.len();
    for &a in plan {
        let p = to_port(q, a);
        let p2 = if p.0 < n_ports { match_to[p.0] } else { p.0 };
        let p2 = if p2 < n_ports { p2 } else { p.0 };
        let (q2, _c2) = from_port(PortIdx(p2));
        q = q2;
        out.push(q);
    }
    out
}

// Infer labels greedily by following all traces, enforcing strict quotas N//4 (+1 for first N%4 labels)
fn infer_labels(inst: &Instance, match_to: &[usize]) -> Vec<Label> {
    let n = inst.n;
    let mut labels: Vec<Option<Label>> = vec![None; n];
    let base = (n / 4) as i32;
    let rem = (n % 4) as usize;
    let mut remain = [0i32; 4];
    for l in 0..4usize { remain[l] = base + if l < rem { 1 } else { 0 }; }

    // helper to assign label if not set
    let mut set_label = |q: usize, want: Label| {
        if labels[q].is_some() { return; }
        let wl = want as usize;
        if remain[wl] > 0 { labels[q] = Some(want); remain[wl] -= 1; return; }
        // pick any other label with remaining quota
        for l in 0..4usize {
            if l == wl { continue; }
            if remain[l] > 0 { labels[q] = Some(l as u8); remain[l] -= 1; return; }
        }
        // as a last resort (shouldn't happen), assign the first label
        if labels[q].is_none() { labels[q] = Some(0); }
    };

    // Traverse all plans and assign greedily
    for (plan, expect) in inst.plans.iter().zip(inst.results.iter()) {
        let path = trace_rooms(match_to, inst.s0, plan);
        for (room, lab) in path.into_iter().zip(expect.iter().copied()) {
            set_label(room, lab);
        }
    }

    // Fill any remaining unlabeled rooms with leftover quotas
    for q in 0..n {
        if labels[q].is_none() {
            for l in 0..4usize {
                if remain[l] > 0 { labels[q] = Some(l as u8); remain[l] -= 1; break; }
            }
            if labels[q].is_none() { labels[q] = Some(0); }
        }
    }

    labels.into_iter().map(|x| x.unwrap_or(0)).collect()
}

fn energy(inst: &Instance, m: &Model, _lambda_bal: f32) -> Energy {
    // Greedily infer labels with strict quotas, then count observation mismatches
    let labels = infer_labels(inst, &m.match_to);
    let mut obs: i32 = 0;
    for (plan, expect) in inst.plans.iter().zip(inst.results.iter()) {
        let path = trace_rooms(&m.match_to, inst.s0, plan);
        for (room, &e) in path.into_iter().zip(expect.iter()) {
            if labels[room] != e { obs += 1; }
        }
    }
    Energy { obs, balance: 0, total: obs }
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

// removed unused swap_labels helper

#[inline]
fn should_accept(d_total: i32, t: f32, rng: &mut StdRng) -> bool {
    let d = d_total as f32;
    d <= 0.0 || rng.r#gen::<f32>() < (-d / t.max(1e-6)).exp()
}

#[inline]
fn update_after_accept(
    new_e: Energy,
    cur: &mut Energy,
    best: &mut Energy,
    best_model: &mut Model,
    model: &Model,
    last_best_iter: &mut usize,
    k: usize,
    since_log_accepts: &mut usize,
) {
    *cur = new_e;
    *since_log_accepts += 1;
    if new_e.total < best.total {
        *best = new_e;
        *best_model = model.clone();
        *last_best_iter = k;
    }
}

fn try_two_opt(
    inst: &Instance,
    model: &mut Model,
    rng: &mut StdRng,
    t: f32,
    verbose: u8,
    lambda_bal: f32,
    k: usize,
    cur: &mut Energy,
    best: &mut Energy,
    best_model: &mut Model,
    last_best_iter: &mut usize,
    since_log_accepts: &mut usize,
) -> bool {
    let n_ports = inst.n * 6;
    let p1 = rng.gen_range(0..n_ports);
    let mut p2 = rng.gen_range(0..n_ports);
    if p2 == p1 { p2 = (p2 + 1) % n_ports; }

    let a = p1;
    let b = model.match_to[a];
    let c = p2;
    let d = model.match_to[c];

    // Skip if either edge is a self-loop or edges overlap (would break involution)
    if a == b || c == d || a == c || a == d || b == c || b == d {
        return false;
    }

    #[cfg(debug_assertions)]
    {
        debug_assert!(check_involution(model), "pre two_opt: involution broken before move");
    }

    let old_a = b;
    let old_c = d;
    let pattern_b: bool = rng.gen_bool(0.5);
    if pattern_b { two_opt_b(model, a, c); } else { two_opt(model, a, c); }

    #[cfg(debug_assertions)]
    {
        debug_assert!(check_involution(model), "post two_opt apply: involution broken (pattern_b={})", pattern_b);
    }
    let new_e = energy(inst, model, lambda_bal);
    let d_total = new_e.total - cur.total;
    let accept = should_accept(d_total, t, rng);
    if verbose >= 2 {
        eprintln!(
            "dbg: mv=2opt patB={} a={} b={} c={} d={} dE={} T={:.4} acc={} cur->new {}->{}",
            pattern_b, a, old_a, c, old_c, d_total, t, accept, cur.total, new_e.total
        );
    }
    if accept {
        update_after_accept(new_e, cur, best, best_model, model, last_best_iter, k, since_log_accepts);
    } else {
        // revert
        model.match_to[a] = old_a;
        model.match_to[old_a] = a;
        model.match_to[c] = old_c;
        model.match_to[old_c] = c;

        #[cfg(debug_assertions)]
        {
            debug_assert!(check_involution(model), "post two_opt revert: involution broken");
        }
    }
    true
}

fn try_loopmove(
    inst: &Instance,
    model: &mut Model,
    rng: &mut StdRng,
    t: f32,
    verbose: u8,
    lambda_bal: f32,
    k: usize,
    cur: &mut Energy,
    best: &mut Energy,
    best_model: &mut Model,
    last_best_iter: &mut usize,
    since_log_accepts: &mut usize,
) -> bool {
    let n_ports = inst.n * 6;
    let dir_pair_to_loops: bool = rng.gen_bool(0.5);
    if dir_pair_to_loops {
        for _try in 0..16 {
            let a = rng.gen_range(0..n_ports);
            let b = model.match_to[a];
            if a == b { continue; }
            let c = rng.gen_range(0..n_ports);
            let d = model.match_to[c];
            if c == d { continue; }
            if a == c || a == d || b == c || b == d { continue; }
            #[cfg(debug_assertions)]
            { debug_assert!(check_involution(model)); }
            // (a-b),(c-d) -> (a-a),(c-c),(b-d)
            model.match_to[a] = a;
            model.match_to[b] = d;
            model.match_to[d] = b;
            model.match_to[c] = c;
            #[cfg(debug_assertions)]
            { debug_assert!(check_involution(model)); }
            let new_e = energy(inst, model, lambda_bal);
            let d_total = new_e.total - cur.total;
            let accept = should_accept(d_total, t, rng);
            if verbose >= 2 {
                eprintln!(
                    "dbg: mv=loopmove dir=pair->loops a={} b={} c={} d={} dE={} T={:.4} acc={} {}->{}",
                    a, b, c, d, d_total, t, accept, cur.total, new_e.total
                );
            }
            if accept {
                update_after_accept(new_e, cur, best, best_model, model, last_best_iter, k, since_log_accepts);
            } else {
                // revert
                model.match_to[a] = b;
                model.match_to[b] = a;
                model.match_to[c] = d;
                model.match_to[d] = c;
            }
            return true;
        }
        false
    } else {
        for _try in 0..16 {
            let a = rng.gen_range(0..n_ports);
            if model.match_to[a] != a { continue; }
            let c = rng.gen_range(0..n_ports);
            if c == a || model.match_to[c] != c { continue; }
            let b = rng.gen_range(0..n_ports);
            let d = model.match_to[b];
            if b == d || b == a || b == c || d == a || d == c { continue; }
            #[cfg(debug_assertions)]
            { debug_assert!(check_involution(model)); }
            // (a-a),(c-c),(b-d) -> (a-b),(c-d)
            model.match_to[a] = b;
            model.match_to[b] = a;
            model.match_to[c] = d;
            model.match_to[d] = c;
            #[cfg(debug_assertions)]
            { debug_assert!(check_involution(model)); }
            let new_e = energy(inst, model, lambda_bal);
            let d_total = new_e.total - cur.total;
            let accept = should_accept(d_total, t, rng);
            if verbose >= 2 {
                eprintln!(
                    "dbg: mv=loopmove dir=loops->pair a={} c={} b={} d={} dE={} T={:.4} acc={} {}->{}",
                    a, c, b, d, d_total, t, accept, cur.total, new_e.total
                );
            }
            if accept {
                update_after_accept(new_e, cur, best, best_model, model, last_best_iter, k, since_log_accepts);
            } else {
                // revert
                model.match_to[a] = a;
                model.match_to[c] = c;
                model.match_to[b] = d;
                model.match_to[d] = b;
            }
            return true;
        }
        false
    }
}

fn anneal(
    inst: &Instance,
    model: &mut Model,
    iters: usize,
    lambda_bal: f32,
    rng: &mut StdRng,
    time_limit: Option<Duration>,
    log_every: Option<usize>,
    verbose: u8,
    save_every: Option<usize>,
    save_base: Option<&Path>,
    t0: f32,
    alpha: f32,
    tmin: f32,
    reheat_every: Option<usize>,
    reheat_to: Option<f32>,
    p_loopmove: f32,
) -> Energy {
    let start_t = Instant::now();
    let mut cur = energy(inst, model, lambda_bal);
    let mut best = cur;
    let mut best_model = model.clone();
    let n_ports = inst.n * 6;

    // temperature schedule
    let mut t = t0.max(1e-8);
    let alpha = alpha;
    let mut since_log_moves: usize = 0;
    let mut since_log_accepts: usize = 0;
    let mut last_best_iter: usize = 0;
    if verbose > 0 {
        eprintln!(
            "anneal: start E={} (obs={}, bal={}), N={}, ports={}",
            cur.total, cur.obs, cur.balance, inst.n, n_ports
        );
    }
    for k in 0..iters {
        if let Some(limit) = time_limit { if start_t.elapsed() >= limit { break; } }
        let mv: f32 = rng.r#gen();
        let p_loopmove = p_loopmove.clamp(0.0, 1.0);
        let applied = if mv >= p_loopmove {
            try_two_opt(inst, model, rng, t, verbose, lambda_bal, k, &mut cur, &mut best, &mut best_model, &mut last_best_iter, &mut since_log_accepts)
        } else {
            try_loopmove(inst, model, rng, t, verbose, lambda_bal, k, &mut cur, &mut best, &mut best_model, &mut last_best_iter, &mut since_log_accepts)
        };
        if !applied { continue; }
        since_log_moves += 1;
        t *= alpha;
        if t < tmin { t = tmin; }
        // Early stop if we reached perfect total energy
        if best.total == 0 {
            // Save an additional snapshot following the same naming rule as other snapshots
            if let Some(base) = save_base {
                let save_path = derive_save_path(base, k + 1);
                if let Err(e) = write_output_path(inst, &best_model, inst.s0 as usize, &save_path) {
                    if verbose > 0 {
                        eprintln!("warn: failed to save {}: {}", save_path.display(), e);
                    }
                } else if verbose > 0 {
                    eprintln!("saved {} (E=0)", save_path.display());
                }
            }
            if verbose > 0 {
                eprintln!("anneal: reached E=0 at it={}", k + 1);
            }
            break;
        }

        // Optional reheat if stagnated
        if let Some(r_every) = reheat_every {
            if r_every > 0 && k.saturating_sub(last_best_iter) >= r_every {
                let new_t = reheat_to.unwrap_or(t0 * 0.1).max(t);
                if verbose > 0 {
                    eprintln!("anneal: reheat at it={} T {:.4} -> {:.4}", k + 1, t, new_t);
                }
                t = new_t;
                last_best_iter = k; // avoid immediate reheat repeat
            }
        }

        if let Some(every) = log_every {
            if verbose > 0 && every > 0 && (k + 1) % every == 0 {
                let acc_rate = if since_log_moves > 0 {
                    since_log_accepts as f32 / since_log_moves as f32
                } else { 0.0 };
                eprintln!(
                    "anneal: it={}/{} T={:.4} curE={} (obs={},bal={}) bestE={} acc={:.2} elapsed={:.2}s",
                    k + 1,
                    iters,
                    t,
                    cur.total,
                    cur.obs,
                    cur.balance,
                    best.total,
                    acc_rate,
                    start_t.elapsed().as_secs_f32()
                );
                since_log_moves = 0;
                since_log_accepts = 0;
            }
        }

        if let (Some(every), Some(base)) = (save_every, save_base) {
            if every > 0 && (k + 1) % every == 0 {
                // Save best-so-far
                let save_path = derive_save_path(base, k + 1);
                if let Err(e) = write_output_path(inst, &best_model, inst.s0 as usize, &save_path) {
                    if verbose > 0 {
                        eprintln!("warn: failed to save {}: {}", save_path.display(), e);
                    }
                } else if verbose > 0 {
                    eprintln!("saved {} (bestE={})", save_path.display(), best.total);
                }
            }
        }
    }
    *model = best_model;
    if verbose > 0 {
        eprintln!(
            "anneal: done bestE={} (obs={}, bal={}) elapsed={:.2}s",
            best.total, best.obs, best.balance, start_t.elapsed().as_secs_f32()
        );
    }
    best
}

fn emit_output(inst: &Instance, model: &Model, s0: Room) -> OutputMap {
    let n = inst.n;
    let labels = infer_labels(inst, &model.match_to);
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
    OutputMap { rooms: labels, starting_room: s0, connections: conns }
}

fn finalize_match_to(model: &mut Model) {
    // Make mapping safe without changing already valid pairs and self-loops.
    let n_ports = model.match_to.len();
    for p in 0..n_ports {
        let q = model.match_to[p];
        if q >= n_ports {
            // Out of range: clamp to self-loop
            model.match_to[p] = p;
            continue;
        }
        let r = model.match_to[q];
        if r != p {
            // Break inconsistent pair by turning both endpoints into self-loops
            model.match_to[p] = p;
            if q < n_ports { model.match_to[q] = q; }
        }
    }
}

fn derive_save_path(base: &Path, iter: usize) -> PathBuf {
    let dir = base.parent().unwrap_or_else(|| Path::new("."));
    let stem = base.file_stem().and_then(|s| s.to_str()).unwrap_or("out");
    let ext = base.extension().and_then(|e| e.to_str()).unwrap_or("json");
    let name = format!("{}-{:06}.{}", stem, iter, ext);
    dir.join(name)
}

fn write_output_path(inst: &Instance, model: &Model, s0: usize, path: &Path) -> Result<()> {
    let out = emit_output(inst, model, s0);
    let serialized = serde_json::to_string_pretty(&out)?;
    if let Some(parent) = path.parent() { if !parent.as_os_str().is_empty() { fs::create_dir_all(parent)?; } }
    fs::write(path, serialized)?;
    Ok(())
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

    // Save initial solution as iter 0 if output is a file
    let save_base = if args.output != "-" { Some(Path::new(&args.output)) } else { None };
    if let Some(base) = save_base {
        let p0 = derive_save_path(base, 0);
        write_output_path(&inst, &model, inst.s0 as usize, &p0)?;
        if args.verbose > 0 { eprintln!("saved {} (initial)", p0.display()); }
    }

    let time_limit = args.time_limit.map(|s| Duration::from_secs_f32(s));
    let log_every = if args.verbose > 0 { args.log_every.or(Some(10_000)) } else { None };

    // Multi-start annealing (restarts)
    let restarts = args.restarts.max(1);
    let mut best_overall_model = model.clone();
    let mut best_overall_e = energy(&inst, &model, args.lambda_bal);
    let mut base_seed = seed;
    for r in 0..restarts {
        if args.verbose > 0 && restarts > 1 {
            eprintln!("restart {}/{}", r + 1, restarts);
        }
        // Derive per-restart RNG
        let restart_seed = base_seed ^ ((r as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let mut rng = StdRng::seed_from_u64(restart_seed);

        // Fresh initial model per restart
        let mut m = build_initial(&inst, &mut rng);
        finalize_match_to(&mut m);
        let mut m_save_base = save_base; // may overwrite per restart; acceptable for now

        let best_e = anneal(
            &inst,
            &mut m,
            args.iters,
            args.lambda_bal,
            &mut rng,
            time_limit,
            log_every,
            args.verbose,
            args.save_every.or(log_every),
            m_save_base,
            args.t0,
            args.alpha,
            args.tmin,
            args.reheat_every,
            args.reheat_to,
            args.p_loopmove,
        );
        finalize_match_to(&mut m);
        if args.verbose > 0 {
            eprintln!("energy: obs={}, balance={}, total={}", best_e.obs, best_e.balance, best_e.total);
        }
        if best_e.total < best_overall_e.total {
            best_overall_e = best_e;
            best_overall_model = m;
        }
        if best_overall_e.total == 0 {
            if args.verbose > 0 && restarts > 1 {
                eprintln!("early stop after restart {} (E=0)", r + 1);
            }
            break;
        }
    }

    // Use best-overall model
    let out = emit_output(&inst, &best_overall_model, inst.s0);

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
