// pass3_unknown_explore_v2.rs
//
// 未踏ポートを1歩踏むだけでなく，遷移先ノードで ≥2 手の「プローブ尾」を実行する版。
// - 到達は既知 δ 上の BFS 最短経路で（パス1の長いプレフィクスより短くなりやすい）
// - 各プランが 6n 制限を超える場合は，尾長を 2 まで自動で縮める
// - 尾には 0,1,2,... を cyclic に使う（多様性を出したければ seed で回転）
//
// 依存：phase_c::MergeResult

use std::collections::{VecDeque, HashMap, HashSet};
use crate::phase_c::MergeResult;

#[derive(Debug, Clone, Copy)]
pub enum PerNodeMode {
    /// 各 v について "完全未踏辺数" (= 6 - (out_known + in_stubs)) だけ選ぶ（最小本数志向）
    Minimal,
    /// 各 v の「未知ポート」すべてを選ぶ（1 回で極力取り切る志向。プラン数は増える）
    AllUnknown,
}

#[derive(Debug, Clone, Copy)]
pub struct UnknownExploreOptions {
    /// 各ノードのタスク選び
    pub per_node_mode: PerNodeMode,
    /// 遷移先ノードで付ける「プローブ尾」の最大長（推奨 3）
    pub tail_max: usize,
    /// 尾の最小長（S_f2 を保証するため 2 を強く推奨）
    pub tail_min: usize,
    /// 1 プランの長さ上限を n から計算（既定 6n）
    pub limit_ratio: f64,
    /// 尾の開始ドアを回すためのシード（多様性）
    pub seed: u64,
}

impl Default for UnknownExploreOptions {
    fn default() -> Self {
        Self {
            per_node_mode: PerNodeMode::Minimal,
            tail_max: 10,
            tail_min: 2,
            limit_ratio: 6.0,
            seed: 0xA1B2_C3D4_E5F6_7788,
        }
    }
}

/// スタート（time 0 のクラスタ）から target クラスタへの「扉列」最短路を δ 上で探索。
/// 見つからなければ None（通常は Pass1 で到達しているので見つかる）
fn bfs_doors(
    start_c: usize,
    target_c: usize,
    delta_by_cluster: &Vec<[Option<usize>; 6]>,
) -> Option<Vec<u8>> {
    if start_c == target_c { return Some(Vec::new()); }
    let n = delta_by_cluster.len();
    let mut q = VecDeque::new();
    let mut seen = vec![false; n];
    // prev[c] = (prev_cluster, door_used)
    let mut prev: Vec<Option<(usize, u8)>> = vec![None; n];

    q.push_back(start_c);
    seen[start_c] = true;

    while let Some(u) = q.pop_front() {
        for d in 0..6u8 {
            if let Some(v) = delta_by_cluster[u][d as usize] {
                if !seen[v] {
                    seen[v] = true;
                    prev[v] = Some((u, d));
                    if v == target_c { // reconstruct
                        let mut path: Vec<u8> = Vec::new();
                        let mut cur = v;
                        while cur != start_c {
                            let (p, door) = prev[cur].unwrap();
                            path.push(door);
                            cur = p;
                        }
                        path.reverse();
                        return Some(path);
                    }
                    q.push_back(v);
                }
            }
        }
    }
    None
}

/// 未踏ポート（v, d）集合を抽出
fn collect_unknown_ports(
    merge: &MergeResult,
    mode: PerNodeMode,
) -> Vec<(usize, u8)> {
    let k = merge.cluster_labels.len();
    // 出・入の本数を集計
    let mut out_known = vec![0usize; k];
    let mut in_stubs  = vec![0usize; k];
    for u in 0..k {
        for d in 0..6u8 {
            if let Some(v) = merge.delta_by_cluster[u][d as usize] {
                out_known[u] += 1;
                in_stubs[v] += 1;
            }
        }
    }

    let mut tasks: Vec<(usize, u8)> = Vec::new();
    for v in 0..k {
        // v の未知ポート列
        let mut unknown_doors: Vec<u8> = (0..6u8)
            .filter(|&d| merge.delta_by_cluster[v][d as usize].is_none())
            .collect();
        if unknown_doors.is_empty() { continue; }

        match mode {
            PerNodeMode::AllUnknown => {
                unknown_doors.sort_unstable();
                tasks.extend(unknown_doors.into_iter().map(|d| (v, d)));
            }
            PerNodeMode::Minimal => {
                let need = 6isize - (out_known[v] as isize + in_stubs[v] as isize);
                if need <= 0 { continue; }
                unknown_doors.sort_unstable();
                let take = (need as usize).min(unknown_doors.len());
                for &d in &unknown_doors[..take] {
                    tasks.push((v, d));
                }
            }
        }
    }
    // 到達が早いノード優先（BFS が短くなる傾向）
    let start_c = merge.time_to_cluster[0];
    let mut with_key: Vec<(usize, (usize, u8))> = tasks.into_iter()
        .map(|(v,d)| {
            // 予測キー：とりあえず v 自身の最初の訪問時刻を近似に使う（無ければ大き値）
            let t_first = 0usize;
            (t_first, (v,d))
        }).collect();
    with_key.sort_by_key(|x| x.0);
    with_key.into_iter().map(|(_, pair)| pair).collect()
}

/// 尾のドア列を作る（長さ tail_len）
fn make_probe_tail(start_rot: usize, tail_len: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(tail_len);
    let mut cur = start_rot % 6;
    for _ in 0..tail_len {
        v.push(cur as u8);
        cur = (cur + 1) % 6;
    }
    v
}

/// 1 本分のミニプラン（到達最短路 + 未知ポート1歩 + 尾）の扉列を作る
fn build_one_plan_doors(
    start_c: usize,
    v: usize,
    d: u8,
    merge: &MergeResult,
    n_rooms: usize,
    opts: UnknownExploreOptions,
) -> Option<Vec<u8>> {
    let budget = ((opts.limit_ratio * n_rooms as f64).floor() as usize).max(1);
    // 到達最短路
    let mut doors = bfs_doors(start_c, v, &merge.delta_by_cluster)?;
    // 未知ポート1歩
    doors.push(d);
    // 尾（最大 tail_max、ダメなら短縮）
    let mut tail_len = opts.tail_max.max(opts.tail_min);
    loop {
        let need = doors.len() + tail_len;
        if need <= budget { break; }
        if tail_len > opts.tail_min { tail_len -= 1; } else { break; }
    }
    if tail_len >= opts.tail_min {
        // 適当に回転（多様性）
        let rot = ((v as u64 ^ ((d as u64) << 8) ^ opts.seed) % 6) as usize;
        let tail = make_probe_tail(rot, tail_len);
        doors.extend_from_slice(&tail);
    }
    Some(doors)
}

/// 文字列に直す
fn doors_to_string(doors: &[u8]) -> String {
    let mut s = String::with_capacity(doors.len());
    for &d in doors {
        debug_assert!(d <= 5);
        s.push(char::from(b'0' + d));
    }
    s
}

/// 公開 API：プローブ尾つきミニプラン群を作る
pub fn build_unknown_edge_plans_with_probe(
    merge: &MergeResult,
    n_rooms: usize,
    opts: UnknownExploreOptions,
) -> Vec<String> {
    let start_c = merge.time_to_cluster[0];
    let tasks = collect_unknown_ports(merge, opts.per_node_mode);
    let mut plans = Vec::new();

    for (v, d) in tasks {
        if let Some(doors) = build_one_plan_doors(start_c, v, d, merge, n_rooms, opts) {
            plans.push(doors_to_string(&doors));
        }
    }
    plans
}
