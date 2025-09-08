// pass2_ingest.rs
//
// 第2パスの結果取り込み：Watch を評価 → 確証マージを反映 → Phase C 再実行 → RP の逆ポートを反映
//
// 使い方はファイル末尾のサンプル参照。

use std::collections::{HashMap, HashSet};

use crate::candidate_gen::Hits;
use crate::candidate_gen::{Candidate, CandidateList};
use crate::pass2_scheduler::{PlanOutput, RpTaskPlan, WatchEntry, WatchKind};
use crate::phase_c::{run_phase_c, MergeResult};

/// 取り込み評価サマリ
#[derive(Debug, Clone)]
pub struct Pass2Eval {
    // ID
    pub id_confirmed: Vec<usize>,    // plan_out.id_tasks のインデックス
    pub id_refuted: Vec<usize>,      // 期待色と不一致（別室確証）
    pub id_inconclusive: Vec<usize>, // 参照不能など

    // RP
    pub rp_hit: HashMap<usize, u8>,  // rp_index -> j_hit
    pub rp_inconclusive: Vec<usize>, // 参照不能 or すべて不一致
}

/// 取り込みの総合結果
#[derive(Debug, Clone)]
pub struct IngestOutcome {
    pub eval: Pass2Eval,
    pub merged: MergeResult, // 再クラスタ後（RP 反映済み）の最終結果
}

/// ラベル列（'0'..'3'）を Vec<u8> に変換
pub fn parse_labels(s: &str) -> Vec<u8> {
    s.chars()
        .filter_map(|ch| {
            let d = ch as u32;
            if (b'0' as u32) <= d && d <= (b'3' as u32) {
                Some((d - b'0' as u32) as u8)
            } else {
                None
            }
        })
        .collect()
}

/// Step1: Watch の評価（/explore 返答ラベル列 Y2 を照合）
pub fn evaluate_pass2(plan: &PlanOutput, y2: &[u8]) -> Pass2Eval {
    let mut id_seen_pos: HashSet<usize> = HashSet::new(); // id_index で重複評価を防ぐ
    let mut id_confirmed = Vec::new();
    let mut id_refuted = Vec::new();
    let mut id_inconclusive = Vec::new();

    let mut rp_hit: HashMap<usize, u8> = HashMap::new(); // rp_index -> j
    let mut rp_checked_any: HashSet<usize> = HashSet::new();
    let mut rp_inconclusive = Vec::new();

    for (wi, w) in plan.watches.iter().enumerate() {
        if w.pos >= y2.len() {
            // 参照不能
            match &w.kind {
                WatchKind::IdCheck { id_index, .. } => {
                    if !id_seen_pos.contains(id_index) {
                        id_inconclusive.push(*id_index);
                        id_seen_pos.insert(*id_index);
                    }
                }
                WatchKind::RpCheck { rp_index, .. } => {
                    rp_inconclusive.push(*rp_index);
                }
            }
            continue;
        }
        let obs = y2[w.pos];
        match &w.kind {
            WatchKind::IdCheck { id_index, .. } => {
                if id_seen_pos.contains(id_index) {
                    continue;
                } // ID は 1 つの Watch で判断
                if obs == w.expect_color {
                    id_confirmed.push(*id_index);
                } else {
                    id_refuted.push(*id_index);
                }
                id_seen_pos.insert(*id_index);
            }
            WatchKind::RpCheck { rp_index, j, .. } => {
                rp_checked_any.insert(*rp_index);
                if obs == w.expect_color {
                    // 当たりを最初の 1 つだけ記録
                    rp_hit.entry(*rp_index).or_insert(*j);
                }
            }
        }
    }

    // RP で一度もチェックされなかった/全て不一致だったものは保留に
    for (i, _task) in plan.rp_tasks.iter().enumerate() {
        if !rp_checked_any.contains(&i) {
            rp_inconclusive.push(i);
        } else if !rp_hit.contains_key(&i) {
            rp_inconclusive.push(i);
        }
    }

    Pass2Eval {
        id_confirmed,
        id_refuted,
        id_inconclusive,
        rp_hit,
        rp_inconclusive,
    }
}

/// Step2: 確証マージを CandidateList に追加（代表時刻ペアを超高スコアで）
/// 代表時刻は pass1 の MergeResult から取得
fn augment_candidates_with_forced_merges(
    base: &CandidateList,
    pass1_merge: &MergeResult,
    plan: &PlanOutput,
    eval: &Pass2Eval,
) -> CandidateList {
    let mut list = base.list.clone();

    const BIG: f64 = 1e9f64; // 超高スコア

    for &id_idx in &eval.id_confirmed {
        if let Some(task) = plan.id_tasks.get(id_idx) {
            let ca = task.cluster_a;
            let cb = task.cluster_b;
            if ca == cb {
                continue;
            }

            // Pass1 の代表時刻を取って時刻ペア化
            let ta = pass1_merge.cluster_representatives[ca];
            let tb = pass1_merge.cluster_representatives[cb];

            list.push(Candidate {
                a: ta as u32,
                b: tb as u32,
                score: BIG,
                hits: Hits {
                    f1: 0,
                    b1: 0,
                    f2: 0,
                    b2: 0,
                    mix: 0,
                },
            });
        }
    }

    // 統計はざっくりと再構成
    let mut out = base.clone();
    out.list = list;
    out
}

/// Step3: Phase C を再実行して再クラスタリング
fn rerun_phase_c_with_forced_merges(
    w: &[u8],
    y1: &[u8],
    augmented: &CandidateList,
    target_n: usize,
) -> MergeResult {
    run_phase_c(w, y1, augmented, target_n)
}

/// Step4: RP ヒットを最終 δ に反映（Ct の j_hit を Cf に向ける）
/// ※ cluster ID は再クラスタ後に変化しているので、Pass1 の代表時刻を介して mapping する
fn apply_rp_hits_to_delta(
    res: &mut MergeResult,     // 再クラスタ後
    pass1_merge: &MergeResult, // 代表時刻の取得元
    plan: &PlanOutput,
    eval: &Pass2Eval,
) {
    for (rp_index, &j_hit) in eval.rp_hit.iter() {
        if let Some(task) = plan.rp_tasks.get(*rp_index) {
            let cf_old = task.from_cluster;
            let ct_old = task.to_cluster;

            // Pass1 の代表時刻
            let t_cf = pass1_merge.cluster_representatives[cf_old];
            let t_ct = pass1_merge.cluster_representatives[ct_old];

            // 再クラスタ後の cluster_id
            let cf_new = res.time_to_cluster[t_cf];
            let ct_new = res.time_to_cluster[t_ct];

            // 反映
            res.delta_by_cluster[ct_new][j_hit as usize] = Some(cf_new);
        }
    }
}

/// 一括パイプライン：評価 → 候補増強 → 再クラスタ → RP 反映
pub fn apply_pass2_and_recluster(
    w: &[u8],
    y1: &[u8],
    pass1_merge: &MergeResult,
    base_candidates: &CandidateList,
    plan: &PlanOutput,
    y2: &[u8],
    target_n: usize,
) -> IngestOutcome {
    let eval = evaluate_pass2(plan, y2);

    // 確証マージを候補へ注入
    let augmented =
        augment_candidates_with_forced_merges(base_candidates, pass1_merge, plan, &eval);

    // 再クラスタ
    let mut merged = rerun_phase_c_with_forced_merges(w, y1, &augmented, target_n);

    // RP の当たりを δ へ反映
    apply_rp_hits_to_delta(&mut merged, pass1_merge, plan, &eval);

    IngestOutcome { eval, merged }
}

// ======================= 使用例（テスト/サンプル） =======================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::candidate_gen::{build_candidates, CandParams};
    use crate::pass2_scheduler::{build_pass2_plan, SchedulerParams};
    use crate::phase_c::run_phase_c;
    use crate::signature_index::{build_signature_index, BuildParams as SigParams};

    #[test]
    fn pipeline_demo() {
        // 仮の小規模データ
        let w = vec![0, 1, 2, 3, 4, 5, 0, 1, 2, 3, 4, 5];
        let y1 = vec![1, 0, 1, 1, 2, 3, 0, 1, 0, 1, 1, 2, 3]; // L+1

        let n = 6usize;

        // Phase A/B/C
        let sig = build_signature_index(&w, &y1, SigParams::default()).unwrap();
        let cands = build_candidates(&w, &y1, &sig, CandParams::default());
        let pass1 = run_phase_c(&w, &y1, &cands, n);

        // Pass2 plan（ID/RP をいくつか含む）
        let plan_out = build_pass2_plan(&w, &pass1, &cands, n, SchedulerParams::default());

        // ここではテスト用に「観測ラベル列 = 期待通り」を仮定
        // 実戦では /explore(plan_out.plan) の返答を parse_labels() に通す
        // 便宜上、全 watch を「期待色」で埋めたラベル列を合成
        let mut y2 = vec![0u8; plan_out.stats.total_actions + 1];
        for wch in &plan_out.watches {
            if wch.pos < y2.len() {
                y2[wch.pos] = wch.expect_color;
            }
        }

        let outcome = apply_pass2_and_recluster(&w, &y1, &pass1, &cands, &plan_out, &y2, n);

        // 何らかのマージが確証されていれば cluster_count は小さくなるはず
        assert!(outcome.merged.cluster_count <= pass1.cluster_count);
    }
}
