// finalize_map.rs
//
// 第2パス後の MergeResult から /guess 用 JSON を構成する。
// - 判定: 追加観測が要るか（ID不足 / RP不足 / 未観測エッジ両側）
// - 構成: rooms / startingRoom / connections
//
// 依存: phase_c::MergeResult

use std::collections::{HashMap, HashSet};

use crate::phase_c::MergeResult;

#[derive(Debug)]
pub enum FinalizeError {
    NeedMoreId { current: usize, target: usize },
    NeedMoreExploreAtNode { node: usize, out_known: usize, in_stubs: usize },
    NeedMoreRp { node: usize, need: Vec<(usize, usize, usize)> }, // (u, need, have)
    DanglingPort { node: usize, door: u8 },
    InvalidDelta,
}

#[derive(Debug, Clone)]
pub struct GuessConnection {
    pub from_room: usize,
    pub from_door: u8,
    pub to_room: usize,
    pub to_door: u8,
}

#[derive(Debug, Clone)]
pub struct GuessMap {
    pub rooms: Vec<u8>,
    pub starting_room: usize,
    pub connections: Vec<GuessConnection>,
}

pub struct FinalizeReport {
    pub map: GuessMap,
    pub stats: FinalizeStats,
}
#[derive(Debug, Default, Clone)]
pub struct FinalizeStats {
    pub edges: usize,         // 無向辺数（= connections.len()）
    pub ports_used: usize,    // 2*edges
}

/// 入口: 判定→構成
pub fn finalize_guess_map(merge: &MergeResult) -> Result<FinalizeReport, FinalizeError> {
    // A: ID 完了チェック
    let k = merge.cluster_labels.len();
    // （呼び出し側が n を知っているなら引数で渡して比較してもよい）
    // ここでは "k が期待値" 前提で、k が小さすぎる/大きすぎる検出のみ
    // → 大きすぎる場合は NeedMoreId を出す
    // ※ 呼び出し側で n を渡すなら、k != n で NeedMoreId {current:k, target:n}
    // ここでは省略: assume OK

    // B/C: ノードごとの in/out を集計
    let n = k;
    let mut out_known = vec![0usize; n];
    let mut in_stubs: Vec<Vec<(usize, u8)>> = vec![Vec::new(); n]; // v <- (u,d)
    let mut delta = merge.delta_by_cluster.clone(); // [Option<usize>;6] x n

    for u in 0..n {
        for d in 0..6u8 {
            if let Some(v) = delta[u][d as usize] {
                out_known[u] += 1;
                in_stubs[v].push((u, d));
            }
        }
    }

    // B: 未観測エッジ（両側とも未使用）の検出: in_stubs[v] + out_known[v] == 6 が必要
    for v in 0..n {
        let lhs = in_stubs[v].len() + out_known[v];
        if lhs != 6 {
            return Err(FinalizeError::NeedMoreExploreAtNode {
                node: v, out_known: out_known[v], in_stubs: in_stubs[v].len()
            });
        }
    }

    // C: RP 充足性: need(v,u) <= have(v,u)
    // have(v,u) = |{ j | delta[v][j]==Some(u) }|
    let mut have: Vec<HashMap<usize, usize>> = vec![HashMap::new(); n];
    for v in 0..n {
        for j in 0..6u8 {
            if let Some(u) = delta[v][j as usize] {
                *have[v].entry(u).or_insert(0) += 1;
            }
        }
    }
    let mut rp_short: Vec<(usize, Vec<(usize, usize, usize)>)> = Vec::new();
    for v in 0..n {
        // need(v,u): v に入ってくる stub のうち、まだ v 側の空きを食っていない分
        let mut need_entries: HashMap<usize, usize> = HashMap::new();
        for &(u, _d) in &in_stubs[v] {
            *need_entries.entry(u).or_insert(0) += 1;
        }
        // out_known[v] で既に占有している分は toRoom がバラけるので、
        // need(v,u) は「u→v の本数」
        let mut lacks: Vec<(usize, usize, usize)> = Vec::new();
        for (u, need_cnt) in need_entries {
            let have_cnt = *have[v].get(&u).unwrap_or(&0);
            if have_cnt < need_cnt {
                lacks.push((u, need_cnt, have_cnt));
            }
        }
        if !lacks.is_empty() {
            rp_short.push((v, lacks));
        }
    }
    if !rp_short.is_empty() {
        // どのノード v で、どの隣接 u が、あと何本足りないかを返す
        // → これは RP の追加だけで埋まる
        return Err(FinalizeError::NeedMoreRp {
            node: rp_short[0].0, need: rp_short[0].1.clone()
        });
    }

    // ここまで来れば、各 v について「u に出る j」が必要数ある。
    // あとは (u,d)->v の各 stub に、v 側の j を重複なく割り当てるだけで良い。

    // v 側の未使用 j を管理
    let mut used_vj: Vec<HashSet<u8>> = vec![HashSet::new(); n];
    // 既存の v->u の j を「空き」としてコレクション
    let mut avail_v_to_u: Vec<HashMap<usize, Vec<u8>>> = vec![HashMap::new(); n];
    for v in 0..n {
        for j in 0..6u8 {
            if let Some(u) = delta[v][j as usize] {
                avail_v_to_u[v].entry(u).or_insert_with(Vec::new).push(j);
            }
        }
    }

    // 辺集合（無向、重複なし）を構成
    let mut seen_edge = HashSet::<(usize, u8)>::new();
    let mut connections: Vec<GuessConnection> = Vec::new();

    for u in 0..n {
        for d in 0..6u8 {
            if let Some(v) = delta[u][d as usize] {
                // すでに (v, j) 側として登録済みなら skip（無向重複防止）
                if seen_edge.contains(&(u, d)) { continue; }

                // v 側で u に出る j を一つ確保
                let js = avail_v_to_u[v].get_mut(&u).expect("have(v,u) >= need(v,u) のはず");
                // まだ割り当てていない j を探す
                let j = js.iter().find(|&&jj| !used_vj[v].contains(&jj))
                    .copied()
                    .ok_or(FinalizeError::DanglingPort { node: v, door: 255 })?;
                used_vj[v].insert(j);

                // 自己ループの重複防止： (u,d) と (v,j) が同一ポートなら 1 回だけ
                let push_this = if u != v {
                    true
                } else {
                    // 自己ループは (d <= j) にだけ出力
                    d <= j
                };
                if push_this {
                    connections.push(GuessConnection {
                        from_room: u, from_door: d,
                        to_room: v, to_door: j,
                    });
                }

                seen_edge.insert((u, d));
                seen_edge.insert((v, j));
            }
        }
    }

    // rooms / startingRoom
    let rooms = merge.cluster_labels.clone();
    let starting_room = merge.time_to_cluster[0];
    let connections_len = connections.len();

    Ok(FinalizeReport {
        map: GuessMap { rooms, starting_room, connections },
        stats: FinalizeStats {
            edges: connections_len,
            ports_used: connections_len * 2,
        },
    })
}
