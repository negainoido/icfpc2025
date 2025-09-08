// pass2_id_double.rs
//
// 二重確証 ID（Double-Witness）プラン生成
//
// - 入力: 1本目パスの W, Y、直近の MergeResult、CandidateList（PhaseB）
// - 出力: バッチ（各クラスタ対ごとに 2 本のプランと Watch）
// - 特徴:
//   * 各ペア (A,B) について独立な2証人 (s1,r1), (s2,r2) を選ぶ（できないペアはスキップ）
//   * 各プランは W をベースに [c] を1回だけ挿入（ID専用でRPは含めない）
//   * 色 c は Y[r] と異なる値を自動選択（偶然一致の低減）
//   * forbid セット（否定確証ペア）で除外可
//
// 依存：
//   use crate::pass2_scheduler::{WatchEntry, WatchKind};
//   use crate::candidate_gen::CandidateList;
//   use crate::phase_c::MergeResult;

use crate::candidate_gen::CandidateList;
use crate::pass2_scheduler::{WatchEntry, WatchKind};
use crate::phase_c::MergeResult;
use std::collections::{HashMap, HashSet};

/// 同一性二重確認の生成パラメータ
#[derive(Debug, Clone, Copy)]
pub struct DoubleIdParams {
    /// 生成するペア数の上限（重い順に採用）
    pub max_pairs: usize,
    /// 2つの証人は方向を混在可（A→B と B→A を混ぜてもよい）
    pub allow_mixed_directions: bool,
    /// 2つの証人の (s,r) 区間が W 上で強く重なるのを避ける最小ギャップ（ベース時刻）
    pub min_separation: usize,
}

impl Default for DoubleIdParams {
    fn default() -> Self {
        Self {
            max_pairs: 16,
            allow_mixed_directions: true,
            min_separation: 10,
        }
    }
}

/// （オプション）否定確証ペアの除外キー
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PairKey {
    pub a: usize,
    pub b: usize,
}
impl PairKey {
    pub fn new(a: usize, b: usize) -> Self {
        if a < b {
            Self { a, b }
        } else {
            Self { a: b, b: a }
        }
    }
}

/// 1つのペアに対する 2 本のプラン
#[derive(Debug, Clone)]
pub struct IdDoublePlan {
    pub pair_index: usize, // バッチ内のインデックス
    pub cluster_a: usize,
    pub cluster_b: usize,
    pub plan1: String,
    pub watches1: Vec<WatchEntry>,
    pub plan2: String,
    pub watches2: Vec<WatchEntry>,
}

/// バッチ出力（/explore に投げるための形）
#[derive(Debug, Clone)]
pub struct DoubleIdBatch {
    /// 並び順：[(pair0-plan1), (pair0-plan2), (pair1-plan1), (pair1-plan2), ...]
    pub plans: Vec<String>,
    /// 各プランごとの WatchEntry 群（ plans[i] に対応 ）
    pub watches_per_plan: Vec<Vec<WatchEntry>>,
    /// ペア一覧（plans の 2 本ごとに1ペア）
    pub per_pair: Vec<(usize, usize)>, // (cluster_a, cluster_b)
    /// 参考：各ペアの内部詳細
    pub items: Vec<IdDoublePlan>,
}

/// メインAPI：候補（PhaseB）から重いクラスタ対を抽出→二重証人→2プラン×Nペアのバッチへ
pub fn build_double_id_plans_from_candidates(
    w: &[u8],              // ベースW（扉列）
    y: &[u8],              // ベースY（ラベル列、長さ = w.len()+1）
    merge: &MergeResult,   // 直近のクラスタリング
    cands: &CandidateList, // PhaseBの候補（時刻ペアベース）
    params: DoubleIdParams,
    forbid: Option<&HashSet<PairKey>>,
) -> DoubleIdBatch {
    let visits = times_by_cluster(merge);
    // 1) CandidateList → クラスタペアへ集約（スコア合算）
    let mut agg: HashMap<PairKey, f64> = HashMap::new();
    for c in cands.list.iter() {
        let a_t = c.a as usize;
        let b_t = c.b as usize;
        let ca = merge.time_to_cluster[a_t];
        let cb = merge.time_to_cluster[b_t];
        if ca == cb {
            continue;
        }
        let key = PairKey::new(ca, cb);
        if let Some(fb) = forbid {
            if fb.contains(&key) {
                continue;
            }
        }
        *agg.entry(key).or_insert(0.0) += c.score;
    }
    // 2) スコア降順にペア候補を並べる
    let mut pairs: Vec<(PairKey, f64)> = agg.into_iter().collect();
    pairs.sort_by(|x, y2| {
        y2.1.partial_cmp(&x.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| x.0.a.cmp(&y2.0.a))
            .then_with(|| x.0.b.cmp(&y2.0.b))
    });

    // 3) 各ペアについて二重証人 (s1,r1),(s2,r2) を見つけ、2プランを構築
    let mut items: Vec<IdDoublePlan> = Vec::new();
    for (pair_i, (pk, _score)) in pairs.into_iter().enumerate() {
        if items.len() >= params.max_pairs {
            break;
        }

        // A→B 優先で探し、無理なら B→A、さらに混在許容
        let w1 = pick_two_witnesses(&visits[pk.a], &visits[pk.b], params.min_separation);
        let (dir_ab, s1, r1, s2, r2) = if let Some((x1, x2)) = w1 {
            (true, x1.0, x1.1, x2.0, x2.1)
        } else if let Some((y1, y2)) =
            pick_two_witnesses(&visits[pk.b], &visits[pk.a], params.min_separation)
        {
            (false, y1.0, y1.1, y2.0, y2.1)
        } else if params.allow_mixed_directions {
            // 混在：まず A→B を1本、残りを B→A で
            if let Some(x1) = pick_one_witness(&visits[pk.a], &visits[pk.b]) {
                if let Some(y1) = pick_one_witness_disjoint(
                    &visits[pk.b],
                    &visits[pk.a],
                    x1,
                    params.min_separation,
                ) {
                    // x1: A→B, y1: B→A
                    // 記録の都合上、plan1=先に見つけた方
                    (true, x1.0, x1.1, y1.0, y1.1)
                } else {
                    continue; // 2本揃わないのでスキップ
                }
            } else {
                continue;
            }
        } else {
            continue;
        };

        // dir_ab=true なら (cluster_a=s側, cluster_b=r側) は A→B。false なら逆。
        let (ca, cb) = if dir_ab { (pk.a, pk.b) } else { (pk.b, pk.a) };

        // プラン1
        let c1 = choose_color_neq(y[r1]); // Y[r] と異なる色
        let (plan1, watch1) = build_one_id_plan(w, y, s1, r1, c1, ca, cb);
        // プラン2
        let c2 = choose_color_neq(y[r2]);
        let (plan2, watch2) = build_one_id_plan(w, y, s2, r2, c2, ca, cb);

        items.push(IdDoublePlan {
            pair_index: items.len(),
            cluster_a: ca,
            cluster_b: cb,
            plan1,
            watches1: watch1,
            plan2,
            watches2: watch2,
        });
    }

    // 4) バッチへ整形
    let mut plans: Vec<String> = Vec::with_capacity(items.len() * 2);
    let mut watches_per_plan: Vec<Vec<WatchEntry>> = Vec::with_capacity(items.len() * 2);
    let mut per_pair: Vec<(usize, usize)> = Vec::with_capacity(items.len());

    for it in &items {
        per_pair.push((it.cluster_a, it.cluster_b));
        plans.push(it.plan1.clone());
        watches_per_plan.push(it.watches1.clone());
        plans.push(it.plan2.clone());
        watches_per_plan.push(it.watches2.clone());
    }

    DoubleIdBatch {
        plans,
        watches_per_plan,
        per_pair,
        items,
    }
}

// ================== 内部ユーティリティ ==================

/// W 上で A の訪問列 a_times, B の訪問列 b_times から
/// a< b を満たす (s,r) を1つ
fn pick_one_witness(a_times: &[usize], b_times: &[usize]) -> Option<(usize, usize)> {
    if a_times.is_empty() || b_times.is_empty() {
        return None;
    }
    let mut j = 0usize;
    for &s in a_times {
        while j < b_times.len() && b_times[j] <= s {
            j += 1;
        }
        if j < b_times.len() {
            return Some((s, b_times[j]));
        }
    }
    None
}

/// 既に (s0,r0) を使った後、min_sep だけ離して 2本目を探す
fn pick_one_witness_disjoint(
    a_times: &[usize],
    b_times: &[usize],
    used: (usize, usize),
    min_sep: usize,
) -> Option<(usize, usize)> {
    if a_times.is_empty() || b_times.is_empty() {
        return None;
    }
    let (s0, r0) = used;
    let mut j = 0usize;
    for &s in a_times {
        if s + min_sep <= r0 || s >= r0 + min_sep {
            while j < b_times.len() && b_times[j] <= s {
                j += 1;
            }
            if j < b_times.len() {
                let r = b_times[j];
                if r + min_sep <= s0 || r >= s0 + min_sep {
                    return Some((s, r));
                }
            }
        }
    }
    None
}

/// A→B として 2 本（できれば同方向）を見つける
fn pick_two_witnesses(
    a_times: &[usize],
    b_times: &[usize],
    min_sep: usize,
) -> Option<((usize, usize), (usize, usize))> {
    let first = pick_one_witness(a_times, b_times)?;
    if let Some(second) = pick_one_witness_disjoint(a_times, b_times, first, min_sep) {
        return Some((first, second));
    }
    None
}

/// Y[r] と異なる色（0..3）を選ぶ
fn choose_color_neq(y_r: u8) -> u8 {
    ((y_r as u32 + 1) % 4) as u8
}

/// 1 本の ID プラン（W に [c] を1回挿入）と Watch を構築
fn build_one_id_plan(
    w: &[u8],
    y: &[u8],
    s: usize,
    r: usize,
    color: u8,
    cluster_a: usize,
    cluster_b: usize,
) -> (String, Vec<WatchEntry>) {
    let l = w.len();
    assert!(s < l + 1 && r < l + 1 && s < r);

    let mut plan = String::with_capacity(l + 1);
    let mut watches: Vec<WatchEntry> = Vec::new();
    let mut out_pos = 0usize;

    for t in 0..l {
        // s で [c]
        if t == s {
            plan.push('[');
            plan.push(char::from(b'0' + color));
            plan.push(']');
            out_pos += 1; // [c] もラベル1個
        }
        // 通常の扉
        plan.push(char::from(b'0' + w[t]));
        out_pos += 1;

        // r に到着した瞬間（t+1 == r）で読む
        if t + 1 == r {
            watches.push(WatchEntry {
                pos: out_pos,
                expect_color: color,
                kind: WatchKind::IdCheck {
                    id_index: 0, // 二重確認側で使わないのでダミー
                    cluster_a,
                    cluster_b,
                },
            });
        }
    }

    (plan, watches)
}

/// クラスタごとの訪問時刻列（W 上）を作る
fn times_by_cluster(merge: &MergeResult) -> Vec<Vec<usize>> {
    let n_times = merge.time_to_cluster.len();
    let k = merge.cluster_labels.len();
    let mut v = vec![Vec::<usize>::new(); k];
    for t in 0..n_times {
        let c = merge.time_to_cluster[t];
        v[c].push(t);
    }
    v
}
