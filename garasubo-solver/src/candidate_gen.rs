// Phase B: 候補生成＋スコアリング
//
// 入力：
//   - W: 扉列（0..=5, 長さ L）
//   - Y: ラベル列（0..=3, 長さ L+1）
//   - SigIndex: Phase A の署名索引（各署名キー→時刻ベクタ）
// 出力：
//   - CandidateList: スコア降順の候補ペア（i<j）と統計情報
//
// 依存：外部クレートなし

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use crate::signature_index::SigIndex;

#[derive(Debug, Clone, Copy)]
pub struct CandParams {
    /// 署名ごとの基礎重み
    pub w_f1: f64, // S_f1: (Y[t], W[t], Y[t+1])
    pub w_b1: f64,  // S_b1: (Y[t-1], W[t-1], Y[t])
    pub w_f2: f64,  // S_f2: (Y[t], W[t], Y[t+1], W[t+1], Y[t+2])
    pub w_b2: f64,  // S_b2: (Y[t-2], W[t-2], Y[t-1], W[t-1], Y[t])
    pub w_mix: f64, // S_mix: (Y[t-1], W[t-1], Y[t], W[t], Y[t+1])

    /// IDF: weight *= ln(1 + idf_scale * (Universe / bucket_size))^idf_power
    pub idf_scale: f64,
    pub idf_power: f64,

    /// 各ノード（時刻）につく候補の上限（双方向 OR マージ）
    /// 例: Some(32) なら各時刻につきスコア上位 32 本までを残す
    pub per_node_cap: Option<usize>,

    /// 生成後に全体から上位 M 本だけ残す
    pub max_pairs: Option<usize>,

    /// このしきい値未満の候補は捨てる（0.0 推奨）
    pub min_score: f64,
}
impl Default for CandParams {
    fn default() -> Self {
        Self {
            w_f1: 1.0,
            w_b1: 1.0,
            w_f2: 4.0,
            w_b2: 4.0,
            w_mix: 3.0,
            idf_scale: 1.2,
            idf_power: 1.2,
            per_node_cap: Some(64),
            max_pairs: None, // None なら制限なし
            min_score: 0.0,
        }
    }
}

/// 1 本の候補ペア
#[derive(Debug, Clone)]
pub struct Candidate {
    pub a: u32,     // 時刻（R_a）
    pub b: u32,     // 時刻（R_b）、常に a < b
    pub score: f64, // スコア（大きいほど優先）
    pub hits: Hits, // どの署名で何回支えられているか（デバッグ用）
}

#[derive(Debug, Clone, Default)]
pub struct Hits {
    pub f1: u16,
    pub b1: u16,
    pub f2: u16,
    pub b2: u16,
    pub mix: u16,
}

/// 全体の結果（候補と統計）
#[derive(Debug, Clone)]
pub struct CandidateList {
    pub list: Vec<Candidate>,
    pub stats: CandStats,
}
#[derive(Debug, Clone, Default)]
pub struct CandStats {
    pub total_pairs_before_dedupe: u64,
    pub unique_pairs: usize,
    pub after_per_node_cap: usize,
    pub final_pairs: usize,
}

/// メイン API：候補生成
pub fn build_candidates(w: &[u8], y: &[u8], idx: &SigIndex, params: CandParams) -> CandidateList {
    let l = w.len();
    let n_nodes = y.len(); // = L+1

    // 集約マップ：ペア → 累積スコア＋ヒット情報
    let mut agg: HashMap<PairKey, Acc> = HashMap::new();
    let mut total_pairs_before_dedupe: u64 = 0;

    // 各署名インデックスから加点
    // Universe は「その署名が定義される時刻の総数」（理想は L or L-1）
    // ※ Phase A のバケット制限後のサイズを使うため完全な IDF ではないが十分有効。
    if params.w_f1 > 0.0 {
        let universe = l.max(1);
        total_pairs_before_dedupe += accumulate_from_map(
            &idx.f1,
            y,
            params.w_f1,
            universe,
            &mut agg,
            SigKind::F1,
            params,
        );
    }
    if params.w_b1 > 0.0 {
        let universe = l.max(1);
        total_pairs_before_dedupe += accumulate_from_map(
            &idx.b1,
            y,
            params.w_b1,
            universe,
            &mut agg,
            SigKind::B1,
            params,
        );
    }
    if params.w_f2 > 0.0 && l >= 2 {
        let universe = (l - 1).max(1);
        total_pairs_before_dedupe += accumulate_from_map(
            &idx.f2,
            y,
            params.w_f2,
            universe,
            &mut agg,
            SigKind::F2,
            params,
        );
    }
    if params.w_b2 > 0.0 && l >= 2 {
        let universe = (l - 1).max(1);
        total_pairs_before_dedupe += accumulate_from_map(
            &idx.b2,
            y,
            params.w_b2,
            universe,
            &mut agg,
            SigKind::B2,
            params,
        );
    }
    if params.w_mix > 0.0 {
        if let Some(m) = idx.mix.as_ref() {
            let universe = l.saturating_sub(1).max(1);
            total_pairs_before_dedupe +=
                accumulate_from_map(m, y, params.w_mix, universe, &mut agg, SigKind::Mix, params);
        }
    }

    // ---- HashMap → Vec に変換
    let mut list: Vec<Candidate> = agg
        .into_iter()
        .map(|(k, acc)| Candidate {
            a: k.a,
            b: k.b,
            score: acc.score,
            hits: acc.hits,
        })
        .filter(|c| c.score >= params.min_score)
        .collect();

    let unique_pairs = list.len();

    // ---- （任意）各ノード当たり上位 K に制限
    let after_per_node_cap = if let Some(k) = params.per_node_cap {
        let allowed = per_node_topk(n_nodes as u32, &list, k);
        list.retain(|c| allowed.contains(&pair_key(c.a, c.b)));
        list.len()
    } else {
        list.len()
    };

    // ---- スコア降順で安定ソート（スコア同点は (a,b) で決定）
    list.sort_by(
        |x, y2| match y2.score.partial_cmp(&x.score).unwrap_or(Ordering::Equal) {
            Ordering::Equal => match x.a.cmp(&y2.a) {
                Ordering::Equal => x.b.cmp(&y2.b),
                o => o,
            },
            o => o,
        },
    );

    // ---- （任意）全体の上位 M 本だけ残す
    if let Some(m) = params.max_pairs {
        if list.len() > m {
            list.truncate(m);
        }
    }

    let final_pairs = list.len();

    CandidateList {
        list,
        stats: CandStats {
            total_pairs_before_dedupe,
            unique_pairs,
            after_per_node_cap,
            final_pairs,
        },
    }
}

// ============================ 内部実装 ============================

#[derive(Debug, Clone, Copy)]
pub(crate) enum SigKind {
    F1,
    B1,
    F2,
    B2,
    Mix,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Acc {
    pub(crate) score: f64,
    pub(crate) hits: Hits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PairKey {
    a: u32,
    b: u32,
}

#[inline]
pub(crate) fn pair_key(a: u32, b: u32) -> PairKey {
    if a < b {
        PairKey { a, b }
    } else {
        PairKey { a: b, b: a }
    }
}

/// 1 つの署名マップからペアを生成して集約に加点
fn accumulate_from_map(
    map: &HashMap<u64, Vec<u32>>,
    y: &[u8],
    base_w: f64,
    universe: usize,
    agg: &mut HashMap<PairKey, Acc>,
    kind: SigKind,
    params: CandParams,
) -> u64 {
    let mut raw_pairs: u64 = 0;
    for (_key, times) in map.iter() {
        let m = times.len();
        if m < 2 {
            continue;
        }

        // IDF 重み（希少な署名ほど強い）
        let idf = idf_factor(universe, m, params.idf_scale, params.idf_power);
        let w = base_w * idf;

        // 全組み合わせ（バケットは Phase A の cap 済み）
        for i in 0..m {
            let a = times[i] as usize;
            for j in (i + 1)..m {
                raw_pairs += 1;
                let b = times[j] as usize;
                // 同ラベルでなければスキップ（安全側のゲート）
                if y[a] != y[b] {
                    continue;
                }

                let k = pair_key(times[i], times[j]);
                let entry = agg.entry(k).or_insert_with(Acc::default);
                entry.score += w;
                match kind {
                    SigKind::F1 => entry.hits.f1 += 1,
                    SigKind::B1 => entry.hits.b1 += 1,
                    SigKind::F2 => entry.hits.f2 += 1,
                    SigKind::B2 => entry.hits.b2 += 1,
                    SigKind::Mix => entry.hits.mix += 1,
                }
            }
        }
    }
    raw_pairs
}

/// IDF = ln(1 + idf_scale * (Universe / m))^idf_power（Universe, m >= 1）
#[inline]
pub(crate) fn idf_factor(universe: usize, m: usize, scale: f64, power: f64) -> f64 {
    let ratio = (universe as f64 / m as f64).max(1.0);
    (1.0 + scale * ratio).ln().powf(power.max(0.0))
}

/// 各ノード当たり上位 K を残す（片側 OR：どちらかの端点が採用すれば残す）
/// 戻り値は残す PairKey の集合
fn per_node_topk(n_nodes: u32, list: &[Candidate], k: usize) -> HashSet<PairKey> {
    // 隣接リスト（両向き）
    let mut adj: Vec<Vec<(u32, f64)>> = vec![Vec::new(); n_nodes as usize];
    for c in list.iter() {
        adj[c.a as usize].push((c.b, c.score));
        adj[c.b as usize].push((c.a, c.score));
    }
    // 各ノードで上位 k を選ぶ
    let mut keep: HashSet<PairKey> = HashSet::new();
    for u in 0..(n_nodes as usize) {
        let edges = &mut adj[u];
        if edges.is_empty() {
            continue;
        }
        edges.sort_by(|x, y| {
            y.1.partial_cmp(&x.1)
                .unwrap_or(Ordering::Equal)
                .then_with(|| x.0.cmp(&y.0))
        });
        let cap = edges.len().min(k);
        for i in 0..cap {
            let v = edges[i].0;
            keep.insert(pair_key(u as u32, v));
        }
    }
    keep
}

// ============================ テスト ============================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signature_index::{build_signature_index, BuildParams as SigBuildParams};

    #[test]
    fn build_candidates_basic() {
        // 小さな例
        let w = vec![0, 1, 2, 3, 4, 5, 0, 1, 2, 3, 4, 5];
        let y = vec![1, 0, 1, 1, 2, 3, 0, 1, 0, 1, 2, 3, 0]; // 長さ L+1
        let idx = build_signature_index(&w, &y, SigBuildParams::default()).unwrap();

        let params = CandParams {
            per_node_cap: Some(8),
            max_pairs: Some(500),
            ..Default::default()
        };
        let out = build_candidates(&w, &y, &idx, params);

        assert!(!out.list.is_empty());
        // すべて同ラベル
        for c in &out.list {
            assert_eq!(y[c.a as usize], y[c.b as usize]);
        }
        // 降順
        for w in out.list.windows(2) {
            assert!(w[0].score >= w[1].score);
        }
        assert!(out.stats.final_pairs <= 500);
    }
}
