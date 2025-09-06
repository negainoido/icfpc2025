use anyhow::Result;
use clap::Parser;
use rand::{Rng, SeedableRng, rngs::StdRng};
use std::cmp::Reverse;
use std::collections::BinaryHeap;
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

// グラフ構造固定でラベルを最適に貪欲割当したときの最小矛盾数
// 各ノードに対して、訪問時に観測されたラベルの頻度最大のラベルを選ぶだけで最適
fn score_structure_only(st: &State, plan: &[usize], results: &[u8]) -> usize {
    let l = plan.len();
    debug_assert_eq!(results.len(), l + 1);

    // counts[v][r]: ノードvで観測ラベルr(0..3)が現れた回数
    let mut counts = vec![[0usize; 4]; N];
    let mut cur = 0usize;
    for j in 0..l {
        let r = results[j] as usize;
        counts[cur][r] += 1;
        let d = plan[j] as usize;
        cur = st.neighbors[cur][d].0;
    }
    // 最終位置の観測
    counts[cur][results[l] as usize] += 1;

    // 総観測回数と、各ノードでの最多一致数の合計
    let mut total_obs = 0usize;
    let mut total_best = 0usize;
    for v in 0..N {
        let s = counts[v][0] + counts[v][1] + counts[v][2] + counts[v][3];
        total_obs += s;
        let best = *counts[v].iter().max().unwrap();
        total_best += best;
    }
    total_obs - total_best
}

// 挿入操作（任意のドアを開けて1歩進む、コスト=1）を
// 任意回適用した結果の最小コストを多源ダイクストラで求める
fn relax_insertions(st: &State, base: &[usize], inf: usize) -> Vec<usize> {
    let mut dist = base.to_vec();
    let mut pq: BinaryHeap<(Reverse<usize>, usize)> = BinaryHeap::new();
    for v in 0..N {
        if dist[v] < inf {
            pq.push((Reverse(dist[v]), v));
        }
    }
    while let Some((Reverse(c), v)) = pq.pop() {
        if c != dist[v] {
            continue;
        }
        for d in 0..6 {
            let to = st.neighbors[v][d].0;
            let nc = c.saturating_add(1);
            if nc < dist[to] {
                dist[to] = nc;
                pq.push((Reverse(nc), to));
            }
        }
    }
    dist
}

// 編集距離風の動的計画に基づくスコア
// 操作コスト:
//  - ラベル不一致: 1
//  - プランステップの読み飛ばし（削除）: 1
//  - 任意のドアを開けて1歩進む（挿入）: 1
fn score_edit_distance(st: &State, plan: &[usize], results: &[u8]) -> usize {
    let l = plan.len();
    debug_assert_eq!(results.len(), l + 1);
    const INF: usize = 1_000_000_000; // 十分大きい有限値

    // dp[j][v]: 先頭j個のラベルを処理し終えた時点でノードvにいる最小コスト
    let mut cur = vec![INF; N];
    cur[0] = 0; // 開始ノードは0

    for j in 0..l {
        // jの処理前に、任意回の挿入を閉じる
        let ins = relax_insertions(st, &cur, INF);
        let mut next = vec![INF; N];

        for v in 0..N {
            let base = ins[v];
            if base >= INF {
                continue;
            }
            let label_cost = if st.labels[v] == results[j] { 0 } else { 1 };

            // 削除（読み飛ばし）: その場でjを消費
            let del_cost = base + label_cost + 1;
            if del_cost < next[v] {
                next[v] = del_cost;
            }

            // 一致（プランに従って移動）: ポート plan[j] を使って遷移
            let d = plan[j] as usize;
            let to = st.neighbors[v][d].0;
            let mv_cost = base + label_cost;
            if mv_cost < next[to] {
                next[to] = mv_cost;
            }
        }
        cur = next;
    }

    // 最終ラベル（j = l）についても、事前に挿入を許可
    let ins = relax_insertions(st, &cur, INF);
    let mut best = INF;
    for v in 0..N {
        let base = ins[v];
        if base >= INF {
            continue;
        }
        let label_cost = if st.labels[v] == results[l] { 0 } else { 1 };
        let total = base + label_cost;
        if total < best {
            best = total;
        }
    }
    best
}

// ノードごとの最頻ラベルを計算（現トポロジーでの訪問に基づく）
fn majority_labels(st: &State, plan: &[usize], results: &[u8]) -> [u8; N] {
    let l = plan.len();
    debug_assert_eq!(results.len(), l + 1);
    let mut counts = vec![[0usize; 4]; N];
    let mut cur = 0usize;
    for j in 0..l {
        counts[cur][results[j] as usize] += 1;
        let d = plan[j] as usize;
        cur = st.neighbors[cur][d].0;
    }
    counts[cur][results[l] as usize] += 1;
    let mut labels = [0u8; N];
    for v in 0..N {
        let mut best_c = counts[v][0];
        let mut best_r = 0usize;
        for r in 1..4 {
            if counts[v][r] > best_c {
                best_c = counts[v][r];
                best_r = r;
            }
        }
        labels[v] = best_r as u8;
    }
    labels
}

// SAED: 最頻ラベルを固定し、その下で編集距離風コストを計算（各コスト=1）
fn score_saed(st: &State, plan: &[usize], results: &[u8]) -> usize {
    let labels = majority_labels(st, plan, results);
    let l = plan.len();
    const INF: usize = 1_000_000_000;
    let mut cur = vec![INF; N];
    cur[0] = 0;
    for j in 0..l {
        let ins = relax_insertions(st, &cur, INF);
        let mut next = vec![INF; N];
        for v in 0..N {
            let base = ins[v];
            if base >= INF {
                continue;
            }
            let label_cost = if labels[v] == results[j] { 0 } else { 1 };
            // 削除
            let del_cost = base + label_cost + 1;
            if del_cost < next[v] {
                next[v] = del_cost;
            }
            // 一致
            let to = st.neighbors[v][plan[j]].0;
            let mv_cost = base + label_cost;
            if mv_cost < next[to] {
                next[to] = mv_cost;
            }
        }
        cur = next;
    }
    let ins = relax_insertions(st, &cur, INF);
    let mut best = INF;
    for v in 0..N {
        let base = ins[v];
        if base >= INF {
            continue;
        }
        let label_cost = if labels[v] == results[l] { 0 } else { 1 };
        let total = base + label_cost;
        if total < best {
            best = total;
        }
    }
    best
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

// ラベル数の下限/上限（均等分布: floor(N/4), ceil(N/4)）
fn label_min_max() -> (usize, usize) {
    let minc = N / 4;
    let maxc = (N + 3) / 4; // ceil
    (minc, maxc)
}

// ラベルの度数を数える
fn label_counts(st: &State) -> [usize; 4] {
    let mut cnt = [0usize; 4];
    for &x in &st.labels {
        cnt[x as usize] += 1;
    }
    cnt
}

// 初期化後などに、ラベルの均等分布制約を満たすように修正
fn enforce_label_balance(st: &mut State, rng: &mut StdRng) {
    let (minc, maxc) = label_min_max();
    // バケツ: 各ラベルの頂点インデックス
    let mut buckets: [Vec<usize>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    for (i, &lab) in st.labels.iter().enumerate() {
        buckets[lab as usize].push(i);
    }
    let mut cnt = [0usize; 4];
    for c in 0..4 {
        cnt[c] = buckets[c].len();
    }

    // まず min 充足: min 未満のラベルを満たすため、min を超えるラベルから移す
    for b in 0..4 {
        while cnt[b] < minc {
            // donor を探す（max を超えるラベルを優先、なければ min を超えるラベル）
            let mut donor: Option<usize> = None;
            for a in 0..4 {
                if cnt[a] > maxc {
                    donor = Some(a);
                    break;
                }
            }
            if donor.is_none() {
                for a in 0..4 {
                    if cnt[a] > minc {
                        donor = Some(a);
                        break;
                    }
                }
            }
            let a = match donor {
                Some(x) => x,
                None => break,
            };
            if let Some(&v) = buckets[a].last() {
                buckets[a].pop();
                st.labels[v] = b as u8;
                buckets[b].push(v);
                cnt[a] -= 1;
                cnt[b] += 1;
            } else {
                break;
            }
        }
    }
    // 次に max 超過を解消: max を超えるラベルから max 未満のラベルへ移す
    for a in 0..4 {
        while cnt[a] > maxc {
            let mut recv: Option<usize> = None;
            for b in 0..4 {
                if cnt[b] < maxc {
                    recv = Some(b);
                    break;
                }
            }
            let b = match recv {
                Some(x) => x,
                None => break,
            };
            if let Some(&v) = buckets[a].last() {
                buckets[a].pop();
                st.labels[v] = b as u8;
                buckets[b].push(v);
                cnt[a] -= 1;
                cnt[b] += 1;
            } else {
                break;
            }
        }
    }
}

// 制約を満たす単一点ラベル変更（見つからなければ不作為）
fn apply_label_flip_constrained(st: &mut State, rng: &mut StdRng) {
    let (minc, maxc) = label_min_max();
    let mut cnt = label_counts(st);
    for _ in 0..(N * 8) {
        let v = rng.gen_range(0..N);
        let a = st.labels[v] as usize;
        if cnt[a] <= minc {
            continue;
        }
        // 受け取り可能ラベル候補（ランダム順）
        let mut cand = [0usize; 3];
        let mut idx = 0;
        for b in 0..4 {
            if b != a {
                cand[idx] = b;
                idx += 1;
            }
        }
        for i in (0..3).rev() {
            let j = rng.gen_range(0..=i);
            cand.swap(i, j);
        }
        for &b in &cand {
            if cnt[b] < maxc {
                st.labels[v] = b as u8;
                cnt[a] -= 1;
                cnt[b] += 1;
                return;
            }
        }
    }
}

// 異なる2頂点のラベルをスワップ（常に総数を保つ）
fn apply_label_swap(st: &mut State, rng: &mut StdRng) {
    if N < 2 {
        return;
    }
    for _ in 0..(N * 8) {
        let a = rng.gen_range(0..N);
        let mut b = rng.gen_range(0..N);
        if a == b {
            b = (b + 1) % N;
        }
        if st.labels[a] != st.labels[b] {
            let tmp = st.labels[a];
            st.labels[a] = st.labels[b];
            st.labels[b] = tmp;
            return;
        }
    }
}

// ノード内の6ポートをランダム置換（相手側のバックポートも調整）
fn apply_node_port_permutation(st: &mut State, rng: &mut StdRng) {
    let v = rng.gen_range(0..N);
    let mut perm: [u8; 6] = [0, 1, 2, 3, 4, 5];
    for i in (1..6).rev() {
        let j = rng.gen_range(0..=i);
        perm.swap(i, j);
    }
    let old = st.neighbors[v];
    for d_old in 0u8..6u8 {
        let d_new = perm[d_old as usize];
        let (to, back) = old[d_old as usize];
        st.neighbors[v][d_new as usize] = (to, back);
        st.neighbors[to][back as usize] = (v, d_new);
    }
}

// k本のエッジを同時にリワイア（k=3,4程度）
fn apply_k_rewire(st: &mut State, rng: &mut StdRng, k: usize) {
    if k < 2 || k > 6 {
        return;
    }
    use std::collections::HashSet;
    let mut chosen_left: Vec<(usize, u8)> = Vec::with_capacity(k);
    let mut seen: HashSet<(usize, u8, usize, u8)> = HashSet::new();
    let mut trials = 0;
    while chosen_left.len() < k && trials < 1000 {
        trials += 1;
        let a = rng.gen_range(0..N);
        let da = rng.gen_range(0..6) as u8;
        let (b, db) = st.neighbors[a][da as usize];
        let rep = if (a, da) <= (b, db) {
            (a, da, b, db)
        } else {
            (b, db, a, da)
        };
        if seen.insert(rep) {
            chosen_left.push((a, da));
        }
    }
    if chosen_left.len() < k {
        return;
    }
    let mut right: Vec<(usize, u8)> = Vec::with_capacity(k);
    for &(a, da) in &chosen_left {
        right.push(st.neighbors[a][da as usize]);
    }
    let mut idx: Vec<usize> = (0..k).collect();
    for i in (1..k).rev() {
        let j = rng.gen_range(0..=i);
        idx.swap(i, j);
    }
    // 不動点を避ける簡単処理
    for i in 0..k {
        if idx[i] == i {
            idx.swap(i, (i + 1) % k);
        }
    }
    for i in 0..k {
        let (a, da) = chosen_left[i];
        let (b, db) = right[idx[i]];
        st.connect(a, da, b, db);
    }
}

// 焼きなまし本体
fn sa_solve(
    plan: &[usize],
    results: &[u8],
    time_limit: Duration,
    seed: u64,
    verbose: u8,
    random_init: bool,
    edit_eval: bool,
    saed_eval: bool,
    structure_eval: bool,
    balanced_labels: bool,
    big_moves: bool,
    t0: f64,
    t1: f64,
) -> (State, usize) {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut cur = if random_init {
        init_fully_random(&mut rng)
    } else {
        init_route_respecting_ring(plan, results, &mut rng)
    };
    if balanced_labels {
        enforce_label_balance(&mut cur, &mut rng);
    }
    let mut cur_score = if structure_eval {
        score_structure_only(&cur, plan, results)
    } else if saed_eval {
        score_saed(&cur, plan, results)
    } else if edit_eval {
        score_edit_distance(&cur, plan, results)
    } else {
        simulate_and_score(&cur, plan, results)
    };
    let mut best = cur.clone();
    let mut best_score = cur_score;

    let start = Instant::now();
    let total = time_limit.as_secs_f64().max(1e-9);
    let t0 = t0.max(1e-12);
    let t1 = t1.max(1e-12);

    if verbose > 0 {
        println!(
            "[init] initialized state: score={} (time={:.2}s)",
            cur_score,
            start.elapsed().as_secs_f32()
        );
    }

    let mut iter: u64 = 0;
    while start.elapsed() < time_limit {
        // 時間割合 tau に応じた幾何補間温度
        let tau = (start.elapsed().as_secs_f64() / total).clamp(0.0, 1.0);
        let t = t0.powf(1.0 - tau) * t1.powf(tau);
        let mut next = cur.clone();
        if big_moves {
            // 2-opt 50%, ラベル 10%, ノード置換 20%, 3-edge 15%, 4-edge 5%
            let r: f64 = rng.gen_range(0.0..1.0);
            if r < 0.50 {
                apply_two_opt_swap(&mut next, &mut rng);
            } else if r < 0.60 {
                if !(structure_eval || saed_eval) {
                    if balanced_labels {
                        if rng.gen_bool(0.5) {
                            apply_label_flip_constrained(&mut next, &mut rng);
                        } else {
                            apply_label_swap(&mut next, &mut rng);
                        }
                    } else {
                        apply_label_flip(&mut next, &mut rng);
                    }
                } else {
                    apply_node_port_permutation(&mut next, &mut rng);
                }
            } else if r < 0.80 {
                apply_node_port_permutation(&mut next, &mut rng);
            } else if r < 0.95 {
                apply_k_rewire(&mut next, &mut rng, 3);
            } else {
                apply_k_rewire(&mut next, &mut rng, 4);
            }
        } else {
            // 既定: 2-opt 70%, ラベル 30%
            let use_two_opt = rng.gen_bool(0.7);
            if use_two_opt {
                apply_two_opt_swap(&mut next, &mut rng);
            } else if !(structure_eval || saed_eval) {
                if balanced_labels {
                    if rng.gen_bool(0.5) {
                        apply_label_flip_constrained(&mut next, &mut rng);
                    } else {
                        apply_label_swap(&mut next, &mut rng);
                    }
                } else {
                    apply_label_flip(&mut next, &mut rng);
                }
            } else {
                apply_two_opt_swap(&mut next, &mut rng);
            }
        }

        let next_score = if structure_eval {
            score_structure_only(&next, plan, results)
        } else if saed_eval {
            score_saed(&next, plan, results)
        } else if edit_eval {
            score_edit_distance(&next, plan, results)
        } else {
            simulate_and_score(&next, plan, results)
        };
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

    /// 編集距離風の評価関数を使う
    #[arg(long, default_value_t = false)]
    edit_eval: bool,

    /// SAED（最頻ラベル固定の編集距離）で評価
    #[arg(long, default_value_t = false)]
    saed_eval: bool,

    /// グラフ構造のみを評価（ラベルは貪欲最適とみなす）
    #[arg(long, default_value_t = false)]
    structure_eval: bool,

    /// 初期温度 T0（時間割合0での温度）
    #[arg(long, default_value_t = 3.0)]
    t0: f64,

    /// 最終温度 T1（時間割合1での温度）
    #[arg(long, default_value_t = 0.01)]
    t1: f64,

    /// 大きな遷移（ポート置換・3/4-edge再配線）を有効化
    #[arg(long, default_value_t = false)]
    big_moves: bool,

    /// ラベル分布制約を適用（各ラベル数が floor(N/4)..ceil(N/4)）
    #[arg(long, default_value_t = false)]
    balanced_labels: bool,
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
        args.edit_eval,
        args.saed_eval,
        args.structure_eval,
        args.balanced_labels,
        args.big_moves,
        args.t0,
        args.t1,
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
