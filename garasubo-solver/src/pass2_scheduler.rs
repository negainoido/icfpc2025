// pass2_scheduler.rs
//
// 第2パス：木炭マーカー挿入スケジューラ
//
// 入力：
//  - W: 扉列（0..=5, 長さ L）
//  - merge: Phase C の結果（time->cluster, cluster_labels, delta_by_cluster）
//  - cands: Phase B の候補（時刻ペア i,j とスコア）
//  - target_n: 既知の部屋数 n
//  - params: スケジューラ各種パラメータ（予算など）
//
// 出力：
//  - PlanOutput: 生成したプラン文字列、監視テーブル（Watch）、採用した ID/RP タスク一覧、各種統計

use std::cmp::{max, min, Ordering};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::candidate_gen::{Candidate, CandidateList};
use crate::phase_c::MergeResult;

// ========== 公開インターフェース ==========

#[derive(Debug, Clone, Copy)]
pub struct SchedulerParams {
    /// トークン上限比（歩数＋木炭＋RP往復の総アクション ≤ limit_ratio * n）
    pub limit_ratio: f64, // 通常 6.0
    /// ID タスクの“過剰選択”係数（need = k-n に対し ceil(need * overselect) を目安に選択）
    pub id_overselect: f64, // 1.5～2.0 を推奨
    /// ID タスクの最大数（None なら無制限・ただし予算で切れる）
    pub max_id_tasks: Option<usize>,
    /// RP タスク 1 件あたりで許す候補 j の最大数（j;j の総当り幅）
    pub max_rp_candidates_per_task: usize, // 2～3 推奨
    /// RP タスク総数の上限（None なら予算で切れる）
    pub max_rp_tasks: Option<usize>,
    /// ID と RP を両方積むか（false なら ID のみ）
    pub enable_rp: bool,
}
impl Default for SchedulerParams {
    fn default() -> Self {
        Self {
            limit_ratio: 6.0,
            id_overselect: 3.0,
            max_id_tasks: None,
            max_rp_candidates_per_task: 3,
            max_rp_tasks: None,
            enable_rp: true,
        }
    }
}

// ---- 生成プラン（出力） ----

#[derive(Debug, Clone)]
pub struct PlanOutput {
    pub plan: String,                // "/explore" に投げるプラン文字列
    pub watches: Vec<WatchEntry>,    // 出力ラベル列のどのインデックスをどう読むか
    pub id_tasks: Vec<IdTaskPlan>,   // 採用 ID タスク（色割り当て済み）
    pub rp_tasks: Vec<RpTaskPlan>,   // 採用 RP タスク（色割り当てはビルド時に確定/一部スキップあり）
    pub stats: PlanStats,
}

// 出力ラベル列の読み取り指示
#[derive(Debug, Clone)]
pub struct WatchEntry {
    /// 出力ラベル列の 0-based インデックス（初期状態が 0）
    pub pos: usize,
    /// 期待する色（0..=3）
    pub expect_color: u8,
    /// 種別
    pub kind: WatchKind,
}

#[derive(Debug, Clone)]
pub enum WatchKind {
    /// ID：クラスタ (a,b) の同一性確認（a と b は cluster_id）
    IdCheck { id_index: usize, cluster_a: usize, cluster_b: usize },
    /// RP：t における Cf --d--> Ct1 の後、Ct1 で j を試した結果を読む
    RpCheck { rp_index: usize, t: usize, from_cluster: usize, to_cluster: usize, j: u8 },
}

// 統計
#[derive(Debug, Clone, Default)]
pub struct PlanStats {
    pub baseline_steps: usize,     // |W|
    pub id_tasks_selected: usize,
    pub rp_tasks_selected: usize,  // （ビルド時に色が空かずスキップされた分は含まない）
    pub id_markers: usize,         // 実際に入った [c] の個数（=ID採用数）
    pub rp_markers: usize,         // 実際に入った RP 用 [c] の個数
    pub rp_loops_steps: usize,     // Σ(2*|J|)
    pub total_actions: usize,      // baseline + id_markers + rp_markers + rp_loops_steps
    pub token_budget: usize,       // floor(limit_ratio * n)
    pub budget_used_ratio: f64,
}

// ---- 内部で扱う ID/RP タスク ----

#[derive(Debug, Clone)]
pub struct IdTaskPlan {
    pub cluster_a: usize,
    pub cluster_b: usize,
    pub s_time: usize,  // マーカーを置く基準時刻 s（W 上）
    pub r_time: usize,  // 読む基準時刻 r（W 上、s < r）
    pub score: f64,
    pub color: Option<u8>, // 色は割り当て後に Some になる
}

#[derive(Debug, Clone)]
pub struct RpTaskPlan {
    pub t: usize,               // 基準時刻 t（W 上で Cf --d--> Ct1 を踏む位置）
    pub d: u8,                  // Cf から出る扉（W[t]）
    pub from_cluster: usize,    // Cf
    pub to_cluster: usize,      // Ct1
    pub j_candidates: Vec<u8>,  // Ct1 で試す候補ドア集合（max_rp_candidates_per_task 以下）
    pub color: Option<u8>,      // 実行時に空いていれば割り当て（そうでなければスキップ）
}

// ========== メイン API ==========

pub fn build_pass2_plan(
    w: &[u8],
    merge: &MergeResult,
    cands: &CandidateList,
    target_n: usize,
    params: SchedulerParams,
) -> PlanOutput {
    let n_rooms = target_n;
    let l = w.len();
    let token_budget = max((params.limit_ratio * n_rooms as f64).floor() as usize, l);

    // ---- 前処理：クラスタ来訪時刻の作成 ----
    let visits = times_by_cluster(merge);

    // ---- ID タスクの候補化＆選択 ----
    let need = merge.cluster_count.saturating_sub(n_rooms);
    let id_candidates = collect_id_candidates(cands, merge, &visits);
    let mut id_selected = select_id_tasks(id_candidates, need, params);
    // “同時アクティブ色 ≤ 4” 制約で間引き（s..r の重なりを制御）
    id_selected = enforce_color_concurrency(id_selected);

    // ---- 予算の概算（ID は [c] が 1 アクション/件）----
    let mut id_cost = id_selected.len(); // [c] が 1 つずつ
    if l + id_cost > token_budget {
        // 予算内に収める
        let can = token_budget.saturating_sub(l);
        id_selected.truncate(can);
        id_cost = id_selected.len();
    }

    // ---- RP タスクの候補化＆選択（ID 優先、余予算があれば）----
    let mut rp_selected: Vec<RpTaskPlan> = Vec::new();
    let mut rp_cost = 0usize;
    if params.enable_rp {
        let rp_candidates = collect_rp_candidates(w, merge, params.max_rp_candidates_per_task);
        let mut budget_left = token_budget.saturating_sub(l + id_cost);
        if budget_left >= 3 { // [c] + j;j（最小 1 候補で 1+2=3）
            rp_selected = select_rp_tasks(rp_candidates, &mut budget_left, params.max_rp_tasks);
            rp_cost = token_budget.saturating_sub(l + id_cost + budget_left);
        }
    }

    // ---- プラン生成（色割り当て & Watch 位置の確定）----
    let plan_build = build_plan_with_watch(w, &mut id_selected, &mut rp_selected, token_budget);

    PlanOutput {
        plan: plan_build.plan,
        watches: plan_build.watches,
        id_tasks: id_selected,
        rp_tasks: plan_build.rp_tasks_used, // 実際に回せた分
        stats: PlanStats {
            baseline_steps: l,
            id_tasks_selected: plan_build.id_count,
            rp_tasks_selected: plan_build.rp_count,
            id_markers: plan_build.id_count,
            rp_markers: plan_build.rp_count,
            rp_loops_steps: plan_build.rp_loop_steps,
            total_actions: plan_build.total_actions,
            token_budget,
            budget_used_ratio: (plan_build.total_actions as f64) / (token_budget as f64),
        },
    }
}

// ========== ID 候補の集約・選択 ==========

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PairKey(usize, usize); // (min,max)

fn collect_id_candidates(
    cands: &CandidateList,
    merge: &MergeResult,
    visits: &Vec<Vec<usize>>,
) -> Vec<IdTaskPlan> {
    // 1) 時刻ペア -> クラスタペアへ集約（スコア加算）
    let mut agg: HashMap<PairKey, f64> = HashMap::new();
    for c in cands.list.iter() {
        let a = c.a as usize;
        let b = c.b as usize;
        let ca = merge.time_to_cluster[a];
        let cb = merge.time_to_cluster[b];
        if ca == cb { continue; } // 既に同クラスタ
        let k = if ca < cb { PairKey(ca, cb) } else { PairKey(cb, ca) };
        *agg.entry(k).or_insert(0.0) += c.score;
    }
    // 2) 各ペアに証人 (s,r) を割り当て（W 上で s < r）
    let mut out: Vec<IdTaskPlan> = Vec::new();
    for (PairKey(a, b), score) in agg {
        if let Some((s, r)) = pick_witness_sr(&visits[a], &visits[b]) {
            out.push(IdTaskPlan {
                cluster_a: a,
                cluster_b: b,
                s_time: s,
                r_time: r,
                score,
                color: None,
            });
        } else if let Some((s, r)) = pick_witness_sr(&visits[b], &visits[a]) {
            // 逆向き（b を先に置いて a を読む）でも可
            out.push(IdTaskPlan {
                cluster_a: b,
                cluster_b: a,
                s_time: s,
                r_time: r,
                score,
                color: None,
            });
        } else {
            // このペアは 1 本の W 内では検証できない（スキップ）
        }
    }
    // スコア降順
    out.sort_by(|x, y| y.score.partial_cmp(&x.score).unwrap_or(Ordering::Equal)
        .then_with(|| x.s_time.cmp(&y.s_time)));
    out
}

/// 証人 (s,r) を見つける：A の来訪時刻列 a_times と B の来訪時刻列 b_times（昇順）
/// から、a ∈ A, b ∈ B で a < b を満たす最初の組を返す
fn pick_witness_sr(a_times: &[usize], b_times: &[usize]) -> Option<(usize, usize)> {
    if a_times.is_empty() || b_times.is_empty() { return None; }
    let mut j = 0usize;
    for &s in a_times {
        while j < b_times.len() && b_times[j] <= s { j += 1; }
        if j < b_times.len() {
            return Some((s, b_times[j]));
        }
    }
    None
}

/// need（必要合併数）に対して、互いに素（クラスタが重複しない）な ID タスクを選ぶ
fn select_id_tasks(
    mut pool: Vec<IdTaskPlan>,
    need: usize,
    params: SchedulerParams,
) -> Vec<IdTaskPlan> {
    if need == 0 { return Vec::new(); }
    let target = {
        let base = ((need as f64) * params.id_overselect).ceil() as usize;
        match params.max_id_tasks {
            Some(m) => min(base, m),
            None => base,
        }
    };

    let mut used: HashSet<usize> = HashSet::new();
    let mut out: Vec<IdTaskPlan> = Vec::new();
    for cand in pool.drain(..) {
        if used.contains(&cand.cluster_a) || used.contains(&cand.cluster_b) { continue; }
        used.insert(cand.cluster_a);
        used.insert(cand.cluster_b);
        out.push(cand);
        if out.len() >= target { break; }
    }
    out
}

/// “同時アクティブ色 ≤ 4” を満たすように [s,r] 区間の重なりを制御
fn enforce_color_concurrency(mut ids: Vec<IdTaskPlan>) -> Vec<IdTaskPlan> {
    // s 昇順で見て、任意の時点で同時に 4 を超えるならスコアの低い区間を捨てる
    ids.sort_by_key(|x| x.s_time);
    #[derive(Clone)]
    struct Act { r: usize, score: f64, idx: usize }
    let mut active: Vec<Act> = Vec::new();
    let mut keep = vec![true; ids.len()];

    for (i, it) in ids.iter().enumerate() {
        // 終了済みを削除
        active.retain(|a| a.r > it.s_time);
        // 追加
        active.push(Act { r: it.r_time, score: it.score, idx: i });
        // 同時 >4 なら、一番スコアの低いものを落とす（複数必要なら複数）
        while active.len() > 4 {
            // 最低スコアを探す
            let mut worst = 0usize;
            for k in 1..active.len() {
                if active[k].score < active[worst].score { worst = k; }
            }
            let drop_idx = active[worst].idx;
            keep[drop_idx] = false;
            active.swap_remove(worst);
        }
    }
    // keep だけ残す
    let mut out = Vec::new();
    for (i, it) in ids.into_iter().enumerate() {
        if keep[i] { out.push(it) }
    }
    // 色割り当て（0..3 を r で開放しながら s 順に割付）
    assign_colors_for_id(&mut out);
    out
}

fn assign_colors_for_id(ids: &mut [IdTaskPlan]) {
    // s 昇順
    let mut order: Vec<usize> = (0..ids.len()).collect();
    order.sort_by_key(|&i| ids[i].s_time);
    // (r, color, idx)
    let mut active: Vec<(usize, u8, usize)> = Vec::new();
    for i in order {
        let s = ids[i].s_time;
        // 期限切れ色を開放
        active.retain(|(r, _, _)| *r > s);
        let mut used = [false; 4];
        for &(_, c, _) in &active { used[c as usize] = true; }
        let mut color = None;
        for c in 0..4u8 {
            if !used[c as usize] { color = Some(c); break; }
        }
        if let Some(c) = color {
            // 採用
            ids[i].color = Some(c);
            active.push((ids[i].r_time, c, i));
        } else {
            // ここには来ないはず（enforce で同時 <= 4 にしている）
            ids[i].color = None;
        }
    }
}

// ========== RP 候補の集約・選択 ==========

fn collect_rp_candidates(
    w: &[u8],
    merge: &MergeResult,
    max_j_per_task: usize,
) -> Vec<RpTaskPlan> {
    let l = w.len();
    let mut out = Vec::new();
    // 同じ t に複数作らない
    for t in 0..l {
        let cf = merge.time_to_cluster[t];
        let ct = merge.time_to_cluster[t + 1];
        let d = w[t];
        // 既に Ct 側の “Cf へ戻るドア” が既知なら不要
        let mut unknown: Vec<u8> = Vec::new();
        for j in 0..6u8 {
            match merge.delta_by_cluster[ct][j as usize] {
                Some(dst) => {
                    // 既に Cf へ戻る j が分かっているならスキップ（RP 不要）
                    if dst == cf {
                        unknown.clear();
                        break;
                    }
                }
                None => unknown.push(j),
            }
        }
        if unknown.is_empty() { continue; }
        // 候補幅を制限（小さい j を優先）
        unknown.sort_unstable();
        unknown.truncate(max_j_per_task);

        out.push(RpTaskPlan {
            t,
            d,
            from_cluster: cf,
            to_cluster: ct,
            j_candidates: unknown,
            color: None,
        });
    }
    // |J| 昇順、t 昇順で安価なものを優先
    out.sort_by(|a, b| a.j_candidates.len().cmp(&b.j_candidates.len())
        .then_with(|| a.t.cmp(&b.t)));
    out
}

fn select_rp_tasks(
    mut pool: Vec<RpTaskPlan>,
    budget_left: &mut usize,
    max_rp_tasks: Option<usize>,
) -> Vec<RpTaskPlan> {
    let mut out = Vec::new();
    for cand in pool.drain(..) {
        let cost = 1 + 2 * cand.j_candidates.len(); // [c] + Σ(j;j)
        if *budget_left >= cost {
            *budget_left -= cost;
            out.push(cand);
            if let Some(m) = max_rp_tasks {
                if out.len() >= m { break; }
            }
        }
    }
    out
}

// ========== プラン生成（色割付＆Watch 位置確定） ==========

struct BuildResult {
    plan: String,
    watches: Vec<WatchEntry>,
    rp_tasks_used: Vec<RpTaskPlan>, // 色が空かずスキップされた分を除く
    id_count: usize,
    rp_count: usize,
    rp_loop_steps: usize,
    total_actions: usize,
}

fn build_plan_with_watch(
    w: &[u8],
    ids: &mut [IdTaskPlan],
    rps: &mut [RpTaskPlan],
    token_budget: usize,
) -> BuildResult {
    let l = w.len();

    // 事前：ID を s と r で索引化
    let mut id_by_s: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let mut id_by_r: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, it) in ids.iter().enumerate() {
        id_by_s.entry(it.s_time).or_default().push(i);
        id_by_r.entry(it.r_time).or_default().push(i);
    }
    // RP を t で索引化（1 t につき 1 件を前提）
    let mut rp_by_t: HashMap<usize, usize> = HashMap::new();
    for (i, rp) in rps.iter().enumerate() {
        rp_by_t.insert(rp.t, i);
    }

    // 実行時の色使用状況（ID：区間、RP：瞬間）
    let mut id_active_colors: BTreeMap<u8, usize> = BTreeMap::new(); // color -> 終了時刻 r
    let mut plan = String::new();
    plan.reserve(l + ids.len() + rps.len() * 3); // 概算
    let mut watches: Vec<WatchEntry> = Vec::new();

    // 出力ラベル列のインデックス（0-based）。初期 R_0 が 0 とする。
    let mut out_pos: usize = 0;

    // 統計
    let mut id_cnt = 0usize;
    let mut rp_cnt = 0usize;
    let mut rp_loop_steps = 0usize;

    // 予算監視
    let mut total_actions = 0usize;

    // ユーティリティ
    let mut push_marker = |plan: &mut String, c: u8, out_pos: &mut usize, total_actions: &mut usize| {
        plan.push('[');
        plan.push(char::from(b'0' + c));
        plan.push(']');
        *out_pos += 1;       // [c] でもラベルは 1 つ増える
        *total_actions += 1; // アクション +1
    };
    let mut push_door = |plan: &mut String, d: u8, out_pos: &mut usize, total_actions: &mut usize| {
        plan.push(char::from(b'0' + d));
        *out_pos += 1;       // 移動でラベル +1
        *total_actions += 1; // アクション +1
    };

    // 進行
    for t in 0..l {
        // 期限切れの ID 色を解放（r <= t）
        let expired: Vec<u8> = id_active_colors
            .iter()
            .filter(|(_, &r)| r <= t)
            .map(|(&c, _)| c)
            .collect();
        for c in expired { id_active_colors.remove(&c); }

        // (1) ここで “置く” ID タスク
        if let Some(list) = id_by_s.get(&t) {
            for &idx in list {
                if total_actions >= token_budget { break; }
                if let Some(color) = ids[idx].color {
                    // 同色がアクティブでないことを確認
                    if id_active_colors.contains_key(&color) {
                        // あり得ないが安全側：スキップ
                        continue;
                    }
                    push_marker(&mut plan, color, &mut out_pos, &mut total_actions);
                    id_active_colors.insert(color, ids[idx].r_time);
                    id_cnt += 1;
                }
            }
        }

        // (2) RP タスク（Cf --d--> Ct1 の直前に [c] を置き、そのまま実行）
        let mut did_rp = false;
        if let Some(&rp_idx) = rp_by_t.get(&t) {
            if total_actions < token_budget {
                // 空き色を探す（ID で使用中の色は避ける）
                let mut used = [false; 4];
                for (&c, _) in id_active_colors.iter() { used[c as usize] = true; }
                let mut color = None;
                for c in 0..4u8 {
                    if !used[c as usize] { color = Some(c); break; }
                }
                if let Some(c) = color {
                    // 実行：[c] → d → (j;j)*
                    push_marker(&mut plan, c, &mut out_pos, &mut total_actions);
                    if total_actions >= token_budget { /* 予算切れ */ }
                    push_door(&mut plan, rps[rp_idx].d, &mut out_pos, &mut total_actions);
                    // j;j を順に
                    for &j in &rps[rp_idx].j_candidates {
                        if total_actions + 2 > token_budget { break; }
                        push_door(&mut plan, j, &mut out_pos, &mut total_actions);
                        // 1 回目 j の直後に観測（当たれば色 c が見える）
                        watches.push(WatchEntry {
                            pos: out_pos, // いま到着したラベルの位置
                            expect_color: c,
                            kind: WatchKind::RpCheck {
                                rp_index: rp_cnt,
                                t,
                                from_cluster: rps[rp_idx].from_cluster,
                                to_cluster: rps[rp_idx].to_cluster,
                                j,
                            },
                        });
                        push_door(&mut plan, j, &mut out_pos, &mut total_actions);
                        rp_loop_steps += 2;
                    }
                    rps[rp_idx].color = Some(c);
                    rp_cnt += 1;
                    did_rp = true;
                }
            }
        }

        // (3) 通常の W の扉（RP をやった場合は既に踏んでいるのでスキップ）
        if !did_rp {
            if total_actions < token_budget {
                push_door(&mut plan, w[t], &mut out_pos, &mut total_actions);
            }
        }

        // (4) ここで “読む” ID タスク（r == t+1）
        if let Some(list) = id_by_r.get(&(t + 1)) {
            for &idx in list {
                if let Some(color) = ids[idx].color {
                    // 直後の到着ラベルが c であるか確認
                    watches.push(WatchEntry {
                        pos: out_pos, // いま到着した R_{t+1} のラベルの位置
                        expect_color: color,
                        kind: WatchKind::IdCheck {
                            id_index: idx,
                            cluster_a: ids[idx].cluster_a,
                            cluster_b: ids[idx].cluster_b,
                        },
                    });
                }
            }
        }
    }

    BuildResult {
        plan,
        watches,
        rp_tasks_used: rps.iter().cloned().filter(|r| r.color.is_some()).collect(),
        id_count: id_cnt,
        rp_count: rp_cnt,
        rp_loop_steps,
        total_actions,
    }
}

// ========== ユーティリティ ==========

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
