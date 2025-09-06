use anyhow::Result;
use clap::Parser;
use rand::{Rng, SeedableRng, rngs::StdRng};
use std::time::{Duration, Instant};

// 問題サイズ（固定）
const N: usize = 12;
// 与えられた単一長経路（0..5 の文字列想定。1..6 も parse で許可）
const PLAN: &str = "544054004012505045403132415045121344434123550012251105504250114353144151425322121105300020103405512351411245432153525343350045401333125514304052100010425231512023352345105152105305102520145332443052344120054345522511";
// 観測ラベル（各2bit、0..3）
const RESULT: [i32; 217] = [
    0, 1, 1, 1, 2, 1, 3, 2, 2, 0, 0, 3, 3, 0, 2, 1, 2, 0, 1, 1, 1, 0, 0, 2, 0, 0, 3, 3, 2, 3, 3, 0,
    3, 1, 0, 3, 0, 3, 3, 3, 1, 3, 1, 1, 3, 2, 2, 2, 1, 3, 3, 1, 3, 1, 1, 3, 2, 3, 1, 3, 2, 0, 0, 0,
    0, 2, 2, 2, 3, 2, 2, 2, 2, 0, 2, 1, 2, 1, 3, 0, 3, 1, 1, 1, 0, 0, 0, 2, 0, 0, 3, 1, 2, 3, 2, 0,
    1, 0, 3, 3, 3, 3, 1, 3, 1, 3, 3, 0, 1, 1, 1, 1, 1, 0, 0, 2, 3, 0, 0, 0, 0, 2, 1, 2, 0, 3, 0, 0,
    0, 3, 1, 2, 2, 2, 1, 1, 3, 1, 3, 3, 1, 3, 2, 1, 3, 0, 2, 0, 3, 1, 2, 0, 3, 0, 3, 3, 0, 1, 1, 1,
    1, 3, 3, 1, 1, 1, 1, 1, 1, 1, 2, 1, 1, 3, 1, 1, 2, 1, 0, 3, 0, 3, 1, 3, 3, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 3, 3, 1, 1, 1, 1, 1, 3, 1, 3, 1, 2, 1, 3, 3, 3, 3, 3, 1, 1, 3, 1, 3,
];

// 文字列 PLAN を Vec<usize> に変換（0..5 も 1..6 も許可）
fn parse_plan(plan: &str) -> anyhow::Result<Vec<usize>> {
    if plan.is_empty() {
        return Ok(Vec::new());
    }
    let is_0_5 = plan
        .chars()
        .all(|c| matches!(c, '0' | '1' | '2' | '3' | '4' | '5'));
    let is_1_6 = plan
        .chars()
        .all(|c| matches!(c, '1' | '2' | '3' | '4' | '5' | '6'));
    if is_0_5 {
        return Ok(plan.chars().map(|c| (c as u8 - b'0') as usize).collect());
    }
    if is_1_6 {
        return Ok(plan.chars().map(|c| (c as u8 - b'1') as usize).collect());
    }
    Err(anyhow::anyhow!(
        "プランに不正な文字が含まれています（0-5 または 1-6 のみ許可）: {}",
        plan
    ))
}

fn result_vec() -> Vec<u8> {
    RESULT.iter().map(|&x| x as u8).collect()
}

// 状態: 6ポート無向グラフ（相手ノードと相手ポート）と2bitラベル
#[derive(Clone)]
struct State {
    // neighbors[v][d] = (to_v, to_port)
    neighbors: Vec<[(usize, u8); 6]>,
    labels: Vec<u8>, // 0..3
}

impl State {
    fn new_disconnected() -> Self {
        let neighbors = vec![[(usize::MAX, 255u8); 6]; N];
        let labels = vec![0u8; N];
        Self { neighbors, labels }
    }

    fn is_assigned(&self, v: usize, d: u8) -> bool {
        self.neighbors[v][d as usize].0 != usize::MAX
    }

    fn connect(&mut self, a: usize, da: u8, b: usize, db: u8) {
        self.neighbors[a][da as usize] = (b, db);
        self.neighbors[b][db as usize] = (a, da);
    }
}

// 経路尊重リング初期化
fn init_route_respecting_ring(plan: &[usize], results: &[u8], rng: &mut StdRng) -> State {
    let mut st = State::new_disconnected();

    // 初訪時にラベルを設定
    let mut visited = vec![false; N];

    // 空きポート管理（スタック）
    let mut free_ports: Vec<Vec<u8>> = (0..N).map(|_| (0u8..6u8).rev().collect()).collect();

    // 経路に沿って、未割当ポートに限り (r, d_cur) と (r2, d_back) を接続
    for (j, &d_cur) in plan.iter().enumerate() {
        let r = j % N;
        let r2 = (j + 1) % N;
        if !visited[r] {
            st.labels[r] = results[j];
            visited[r] = true;
        }
        if !st.is_assigned(r, d_cur as u8) {
            // 優先: r2 に空きがあればそこを使う
            let mut partner_node = r2;
            if free_ports[partner_node].is_empty() {
                // r2 に空きが無ければ、ランダムに空きのあるノードを探す
                let mut pick = None;
                for _ in 0..(N * 2) {
                    let x = rng.gen_range(0..N);
                    if !free_ports[x].is_empty() {
                        pick = Some(x);
                        break;
                    }
                }
                partner_node = pick.unwrap_or(r); // 最悪自ノード
            }
            if let Some(db) = free_ports[partner_node].pop() {
                st.connect(r, d_cur as u8, partner_node, db);
            }
        }
    }
    // 最後の停止位置ラベル
    let last_room = plan.len() % N;
    st.labels[last_room] = results[plan.len()];

    // 残った未接続スタブをランダムにペアリング
    let mut stubs: Vec<(usize, u8)> = Vec::new();
    for v in 0..N {
        for d in 0u8..6u8 {
            if !st.is_assigned(v, d) {
                stubs.push((v, d));
            }
        }
    }
    // ランダムシャッフル
    for i in (1..stubs.len()).rev() {
        let j = rng.gen_range(0..=i);
        stubs.swap(i, j);
    }
    let mut i = 0;
    while i + 1 < stubs.len() {
        let (a, da) = stubs[i];
        let (b, db) = stubs[i + 1];
        st.connect(a, da, b, db);
        i += 2;
    }
    // 奇数個余った場合は自己ループで埋める（自己ループ許可仕様）
    if i < stubs.len() {
        let (a, da) = stubs[i];
        st.connect(a, da, a, da);
    }
    st
}

// 完全ランダム初期化（ポート結線とラベルを全てランダム）
fn init_fully_random(rng: &mut StdRng) -> State {
    let mut st = State::new_disconnected();

    // ラベルをランダムに設定
    for v in 0..N {
        st.labels[v] = rng.gen_range(0..4) as u8;
    }

    // すべてのスタブを列挙してシャッフルし、順にペアにして接続
    let mut stubs: Vec<(usize, u8)> = Vec::with_capacity(N * 6);
    for v in 0..N {
        for d in 0u8..6u8 {
            stubs.push((v, d));
        }
    }
    for i in (1..stubs.len()).rev() {
        let j = rng.gen_range(0..=i);
        stubs.swap(i, j);
    }
    let mut i = 0usize;
    while i + 1 < stubs.len() {
        let (a, da) = stubs[i];
        let (b, db) = stubs[i + 1];
        st.connect(a, da, b, db);
        i += 2;
    }
    st
}

// トレースを実行して矛盾総数を返す
fn simulate_and_score(st: &State, plan: &[usize], results: &[u8]) -> usize {
    let mut cur = 0usize;
    let mut mismatches = 0usize;
    let l = plan.len();
    for j in 0..l {
        assert!(cur < N);
        if st.labels[cur] != results[j] {
            mismatches += 1;
        }
        let d = plan[j] as u8;
        let (to, _back) = st.neighbors[cur][d as usize];
        cur = to;
    }
    if cur >= N || st.labels[cur] != results[l] {
        mismatches += 1;
    }
    mismatches
}

// 2-opt ポート交換（双方向不変を保つ）
fn apply_two_opt_swap(st: &mut State, rng: &mut StdRng) {
    let a1 = rng.gen_range(0..N);
    let d1 = rng.gen_range(0..6) as u8;
    let a2 = rng.gen_range(0..N);
    let d2 = rng.gen_range(0..6) as u8;
    if a1 == a2 && d1 == d2 {
        return;
    }
    let (b1, d1p) = st.neighbors[a1][d1 as usize];
    let (b2, d2p) = st.neighbors[a2][d2 as usize];
    st.connect(a1, d1, b2, d2p);
    st.connect(a2, d2, b1, d1p);
}

// ラベル変更
fn apply_label_flip(st: &mut State, rng: &mut StdRng) {
    let v = rng.gen_range(0..N);
    let old = st.labels[v];
    let mut new = rng.gen_range(0..4) as u8;
    if new == old {
        new = (new + 1) & 3;
    }
    st.labels[v] = new;
}

// 焼きなまし本体
fn sa_solve(
    plan: &[usize],
    results: &[u8],
    time_limit: Duration,
    seed: u64,
    verbose: u8,
    random_init: bool,
) -> (State, usize) {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut cur = if random_init {
        init_fully_random(&mut rng)
    } else {
        init_route_respecting_ring(plan, results, &mut rng)
    };
    let mut cur_score = simulate_and_score(&cur, plan, results);
    let mut best = cur.clone();
    let mut best_score = cur_score;

    let start = Instant::now();
    let mut t = 3.0f64; // 初期温度
    let alpha = 0.9996f64; // 減衰率

    if verbose > 0 {
        println!(
            "[init] initialized state: score={} (time={:.2}s)",
            cur_score,
            start.elapsed().as_secs_f32()
        );
    }

    let mut iter: u64 = 0;
    while start.elapsed() < time_limit {
        // ムーブ選択: 2-opt 70%, ラベル 30%
        let use_two_opt = rng.gen_bool(0.7);
        let mut next = cur.clone();
        if use_two_opt {
            apply_two_opt_swap(&mut next, &mut rng);
        } else {
            apply_label_flip(&mut next, &mut rng);
        }

        let next_score = simulate_and_score(&next, plan, results);
        let delta = (next_score as i64) - (cur_score as i64);
        if delta <= 0 {
            // 改善・同等受理
            let prev_cur_score = cur_score;
            cur = next;
            cur_score = next_score;
            if verbose > 1 && next_score < prev_cur_score {
                println!(
                    "[improve] iter={} time={:.2}s temp={:.4} cur: {} -> {}",
                    iter,
                    start.elapsed().as_secs_f32(),
                    t,
                    prev_cur_score,
                    next_score
                );
            }
            if cur_score < best_score {
                best = cur.clone();
                best_score = cur_score;
                if verbose > 0 {
                    println!(
                        "[best]    iter={} time={:.2}s score={}",
                        iter,
                        start.elapsed().as_secs_f32(),
                        best_score
                    );
                }
            }
        } else {
            let p = (-(delta as f64) / t).exp();
            if rng.gen_bool(p.clamp(0.0, 1.0)) {
                cur = next;
                cur_score = next_score;
            }
        }
        t *= alpha;
        iter += 1;
        if best_score == 0 {
            break;
        }
    }
    (best, best_score)
}

#[derive(Parser, Debug)]
#[command(name = "ICFPC 2025 Flowlight Solver")]
#[command(about = "単一路線＋2bitラベルに整合する6ポート無向グラフを焼きなましで復元", long_about = None)]
struct Args {
    /// 実行時間（秒）
    #[arg(long, default_value_t = 30)]
    time_limit: u64,

    /// 乱数シード（省略時は固定値）
    #[arg(long)]
    seed: Option<u64>,

    /// 冗長出力レベル
    #[arg(short, long, default_value_t = 0)]
    verbose: u8,

    /// 初期状態を完全ランダムにする
    #[arg(long, default_value_t = false)]
    random_init: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let plan = parse_plan(PLAN)?;
    let results = result_vec();
    assert_eq!(
        results.len(),
        plan.len() + 1,
        "RESULT の長さは PLAN+1 である必要があります"
    );

    let seed = args.seed.unwrap_or(20250906);
    let (state, best_score) = sa_solve(
        &plan,
        &results,
        Duration::from_secs(args.time_limit),
        seed,
        args.verbose,
        args.random_init,
    );

    if args.verbose > 0 {
        println!(
            "N = {} / PLAN length = {} / best_score = {}",
            N,
            plan.len(),
            best_score
        );
    } else {
        println!("best_score = {}", best_score);
    }

    // JSON 出力
    let labels_json: Vec<u8> = state.labels.clone();
    let mut ports_json = Vec::with_capacity(N);
    for v in 0..N {
        let mut arr = Vec::with_capacity(6);
        for d in 0..6 {
            let (to, back) = state.neighbors[v][d];
            arr.push(serde_json::json!({"to": to, "back": back}));
        }
        ports_json.push(arr);
    }

    let out = serde_json::json!({
        "labels": labels_json,
        "ports": ports_json,
        "score": best_score,
    });
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}
