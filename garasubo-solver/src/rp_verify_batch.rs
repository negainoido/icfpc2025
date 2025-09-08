// rp_verify_batch.rs
//
// 逆ポート 2 ステップ帰還検証のバッチ生成・評価・適用
//
// 依存：phase_c::MergeResult, pass2_scheduler::{WatchEntry, WatchKind}

use std::collections::{VecDeque, HashMap, HashSet};
use crate::phase_c::MergeResult;
use crate::pass2_scheduler::{WatchEntry, WatchKind};

// ---- 入出力構造 ----

#[derive(Debug, Clone, Copy)]
pub struct RpProbe { pub u: usize, pub d: u8, pub v: usize, pub j: u8 }

#[derive(Debug, Clone)]
pub struct RpBatch {
    pub plans: Vec<String>,
    pub watches_per_plan: Vec<Vec<WatchEntry>>,
    pub probes: Vec<RpProbe>, // plans[i] に対応
}

#[derive(Debug, Clone, Copy)]
pub struct RpBatchParams {
    /// 一度に検証する最大本数（多すぎる場合の安全弁）
    pub max_probes: usize,
    /// まず |J|=1（候補1つ）の“安い”ものを優先
    pub prefer_singleton: bool,
    /// 1プランのアクション上限 = floor(limit_ratio * n)
    pub limit_ratio: f64,
}
impl Default for RpBatchParams {
    fn default() -> Self {
        Self { max_probes: 64, prefer_singleton: true, limit_ratio: 6.0 }
    }
}

// ---- 公開 API ----

/// 既知の片方向 u --d--> v に対し、v の未使用 j を列挙し、RP 検証バッチを作る
pub fn build_rp_verify_batch(
    merge: &MergeResult,
    n_rooms: usize,
    params: RpBatchParams,
) -> RpBatch {
    let mut probes_all: Vec<(RpProbe, usize /*|J|*/, usize /*dist(start,u)*/)> = Vec::new();

    // 1) 列挙：u --d--> v かつ v 側で delta[v][j] == None の j を候補に
    for u in 0..merge.cluster_labels.len() {
        for d in 0..6u8 {
            if let Some(v) = merge.delta_by_cluster[u][d as usize] {
                // v の未使用 j
                let mut js: Vec<u8> = (0..6u8)
                    .filter(|&j| merge.delta_by_cluster[v][j as usize].is_none())
                    .collect();
                if js.is_empty() { continue; }
                js.sort_unstable();

                // prefix_to(u) の BFS 距離を見積もり（短い順に優先）
                let dist = bfs_distance(merge.time_to_cluster[0], u, &merge.delta_by_cluster).unwrap_or(usize::MAX/2);

                for &j in &js {
                    probes_all.push((RpProbe { u, d, v, j }, js.len(), dist));
                }
            }
        }
    }

    // 2) 優先度ソート：|J| 昇順 → dist 昇順
    probes_all.sort_by_key(|(_, jlen, dist)| (*jlen, *dist));

    // 3) 上限で刈ってプラン化
    let budget = ((params.limit_ratio * n_rooms as f64).floor() as usize).max(1);
    let mut plans = Vec::<String>::new();
    let mut watches = Vec::<Vec<WatchEntry>>::new();
    let mut probes = Vec::<RpProbe>::new();

    for (rp, _jlen, _dist) in probes_all.into_iter().take(params.max_probes) {
        if let Some((p, w)) = build_one_rp_plan(merge, rp, budget) {
            probes.push(rp);
            plans.push(p);
            watches.push(w);
        }
    }

    RpBatch { plans, watches_per_plan: watches, probes }
}

/// /explore の応答を評価して「当たり」を返す（2 watch とも一致したもののみ）
pub fn eval_rp_verify_batch(
    batch: &RpBatch,
    results: &[Vec<i32>],
) -> Vec<RpProbe> {
    let mut hits: Vec<RpProbe> = Vec::new();
    for i in 0..batch.plans.len() {
        let y = &results[i]; // ラベル列（i32）
        let ws = &batch.watches_per_plan[i];
        if ws.len() < 2 { continue; }
        let ok1 = ws[0].pos < y.len() && (y[ws[0].pos] as u8) == ws[0].expect_color;
        let ok2 = ws[1].pos < y.len() && (y[ws[1].pos] as u8) == ws[1].expect_color;
        if ok1 && ok2 {
            hits.push(batch.probes[i]);
        }
    }
    hits
}

/// 命中を最終 δ に反映（**ラン時系列には入れない**。finalize や後続 RP の基礎に）
pub fn apply_rp_hits_in_place(merge: &mut MergeResult, hits: &[RpProbe]) {
    for h in hits {
        let v = h.v; let j = h.j as usize; let u = h.u;
        if merge.delta_by_cluster[v][j].is_none() {
            merge.delta_by_cluster[v][j] = Some(u);
        }
    }
}

// ---- 内部：プラン生成 & BFS ----

fn build_one_rp_plan(
    merge: &MergeResult,
    rp: RpProbe,
    plan_budget: usize,
) -> Option<(String, Vec<WatchEntry>)> {
    let start = merge.time_to_cluster[0];

    // prefix_to(u)
    let path = bfs_path(start, rp.u, &merge.delta_by_cluster)?;
    let mut s = String::with_capacity(path.len() + 8);
    let mut out_pos = 0usize;
    for &door in &path { s.push(char_for_door(door)); out_pos += 1; }

    // 色の選択：既知ラベルと異なる色
    let cu = choose_color_neq(merge.cluster_labels[rp.u]);
    let cv = choose_color_neq(merge.cluster_labels[rp.v]);

    // [cu]
    s.push('['); s.push(char_for_door(cu)); s.push(']'); out_pos += 1;
    // d → v
    s.push(char_for_door(rp.d)); out_pos += 1;
    // [cv] at v
    s.push('['); s.push(char_for_door(cv)); s.push(']'); out_pos += 1;
    // j
    s.push(char_for_door(rp.j)); out_pos += 1;

    let mut watches = Vec::<WatchEntry>::new();
    // watch1：j の直後（u に戻っていれば cu）
    watches.push(WatchEntry {
        pos: out_pos,
        expect_color: cu,
        kind: WatchKind::RpCheck {
            rp_index: 0, t: 0, from_cluster: rp.v, to_cluster: rp.u, j: rp.j,
        },
    });

    // さらに d（u から v へ戻る）
    s.push(char_for_door(rp.d)); out_pos += 1;
    // watch2：その直後（v に戻っていれば cv）
    watches.push(WatchEntry {
        pos: out_pos,
        expect_color: cv,
        kind: WatchKind::RpCheck {
            rp_index: 0, t: 0, from_cluster: rp.u, to_cluster: rp.v, j: rp.d,
        },
    });

    // 予算チェック（ラベル数 = アクション数に等しい）
    if out_pos + 1 > plan_budget {
        return None;
    }
    Some((s, watches))
}

fn char_for_door(x: u8) -> char { (b'0' + x) as char }

fn choose_color_neq(label: u8) -> u8 { ((label as u32 + 1) % 4) as u8 }

fn bfs_path(start: usize, goal: usize, delta: &Vec<[Option<usize>;6]>) -> Option<Vec<u8>> {
    if start == goal { return Some(vec![]); }
    let n = delta.len();
    let mut q = VecDeque::new();
    let mut seen = vec![false; n];
    let mut prev: Vec<Option<(usize, u8)>> = vec![None; n];
    q.push_back(start);
    seen[start] = true;

    while let Some(u) = q.pop_front() {
        for d in 0..6u8 {
            if let Some(v) = delta[u][d as usize] {
                if !seen[v] {
                    seen[v] = true;
                    prev[v] = Some((u, d));
                    if v == goal {
                        let mut path: Vec<u8> = Vec::new();
                        let mut cur = v;
                        while cur != start {
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
fn bfs_distance(start: usize, goal: usize, delta: &Vec<[Option<usize>;6]>) -> Option<usize> {
    bfs_path(start, goal, delta).map(|p| p.len())
}
