// multi_run_pipeline.rs
//
// Phase A/B/C の「複数ラン」対応：
// - 署名索引: すべてのランを 1 つの索引にまとめる（時刻はグローバルオフセット）
// - 候補生成: 署名一致はランをまたいでも OK（IDF の “母数” はランごとの合計）
// - マージ: δ はランごとに張る（境界跨ぎの遷移は作らない）
//
// 既存の candidate_gen/phase_c をベースにしています。

use std::collections::HashMap;

use crate::signature_index::{BuildParams as SigBuildParams, SigIndex};
use crate::candidate_gen::{CandParams, Candidate, CandidateList, Hits};
use crate::phase_c::{MergeResult, run_phase_c};

/// マルチラン用のシグネチャ索引を構築（各ランの時刻にオフセットを足して 1 本化）
pub fn build_signature_index_multi(
    runs: &[(Vec<u8>, Vec<u8>)], // Vec<(W_i, Y_i)>
    params: SigBuildParams,
) -> Result<(SigIndex, SigUniverse), crate::signature_index::BuildError> {
    use crate::signature_index::{build_signature_index};
    let mut total_f1 = 0usize;
    let mut total_f2 = 0usize;
    let mut total_b1 = 0usize;
    let mut total_b2 = 0usize;
    let mut total_mix = 0usize;

    let mut acc = SigIndex::default();
    let mut offset = 0usize;

    for (w, y) in runs.iter() {
        let idx = build_signature_index(w, y, params)?;
        // オフセットを足して結合
        let shift = |map: &HashMap<u64, Vec<u32>>| -> HashMap<u64, Vec<u32>> {
            let mut out = HashMap::with_capacity(map.len());
            for (k, v) in map.iter() {
                let mut vv = Vec::with_capacity(v.len());
                for &t in v.iter() {
                    vv.push((t as usize + offset) as u32);
                }
                out.insert(*k, vv);
            }
            out
        };
        // 合流
        acc.f1.extend(shift(&idx.f1));
        acc.b1.extend(shift(&idx.b1));
        acc.f2.extend(shift(&idx.f2));
        acc.b2.extend(shift(&idx.b2));
        if params.enable_mix {
            let m = idx.mix.unwrap();
            let mut mm = HashMap::new();
            for (k, v) in m.iter() {
                let mut vv = Vec::with_capacity(v.len());
                for &t in v.iter() {
                    vv.push((t as usize + offset) as u32);
                }
                mm.insert(*k, vv);
            }
            if acc.mix.is_none() { acc.mix = Some(mm); }
            else { acc.mix.as_mut().unwrap().extend(mm); }
        }

        // Universe 母数を集計（ランごとの和）
        let l = w.len();
        total_f1 += l;
        total_b1 += l;
        if l >= 2 {
            total_f2 += l - 1;
            total_b2 += l - 1;
        }
        if params.enable_mix && l >= 1 {
            total_mix += l - 1;
        }

        offset += l + 1; // 各ランの時刻は 0..=l（= l+1 個）
    }

    Ok((acc, SigUniverse {
        f1: total_f1.max(1),
        b1: total_b1.max(1),
        f2: total_f2.max(1),
        b2: total_b2.max(1),
        mix: total_mix.max(1),
        total_nodes: offset, // = Σ(l_i+1)
    }))
}

#[derive(Debug, Clone, Copy)]
pub struct SigUniverse {
    pub f1: usize, pub b1: usize, pub f2: usize, pub b2: usize, pub mix: usize,
    pub total_nodes: usize,
}

/// マルチラン索引から候補を生成（IDF 母数はラン合計で）
pub fn build_candidates_multi(
    uni: SigUniverse,
    idx: &SigIndex,
    y_flat: &[u8], // 全ランの Y をオフセット結合した配列
    params: CandParams,
) -> CandidateList {
    use std::collections::HashMap;
    use crate::candidate_gen::{CandidateList as CL, CandStats};
    use crate::candidate_gen::{idf_factor, pair_key, Acc, SigKind};

    let mut agg: HashMap<(u32,u32), Acc> = HashMap::new();
    let mut total_pairs: u64 = 0;

    // 署名マップを順に集約（関数内に accumulate_from_map と同ロジックを簡約）
    let mut accumulate = |map: &HashMap<u64, Vec<u32>>, base_w: f64, universe: usize, kind: SigKind| {
        for (_k, times) in map.iter() {
            let m = times.len();
            if m < 2 { continue; }
            let idf = idf_factor(universe, m, params.idf_scale, params.idf_power);
            let w = base_w * idf;
            for i in 0..m {
                let a = times[i] as usize;
                for j in (i+1)..m {
                    total_pairs += 1;
                    let b = times[j] as usize;
                    if y_flat[a] != y_flat[b] { continue; } // 同ラベルのみ
                    let key = if times[i] < times[j] { (times[i], times[j]) } else { (times[j], times[i]) };
                    let e = agg.entry(key).or_insert_with(Acc::default);
                    e.score += w;
                    match kind {
                        SigKind::F1 => e.hits.f1 += 1,
                        SigKind::B1 => e.hits.b1 += 1,
                        SigKind::F2 => e.hits.f2 += 1,
                        SigKind::B2 => e.hits.b2 += 1,
                        SigKind::Mix => e.hits.mix += 1,
                    }
                }
            }
        }
    };

    if params.w_f1 > 0.0 { accumulate(&idx.f1, params.w_f1, uni.f1, SigKind::F1); }
    if params.w_b1 > 0.0 { accumulate(&idx.b1, params.w_b1, uni.b1, SigKind::B1); }
    if params.w_f2 > 0.0 { accumulate(&idx.f2, params.w_f2, uni.f2, SigKind::F2); }
    if params.w_b2 > 0.0 { accumulate(&idx.b2, params.w_b2, uni.b2, SigKind::B2); }
    if params.w_mix > 0.0 {
        if let Some(m) = idx.mix.as_ref() {
            accumulate(m, params.w_mix, uni.mix, SigKind::Mix);
        }
    }

    // Vec へ
    let mut list: Vec<Candidate> = agg.into_iter().map(|(k, a)| Candidate {
        a: k.0, b: k.1, score: a.score, hits: a.hits
    }).collect();

    // per_node_cap 等は既存の build_candidates と同様に後段で適用してもOK
    list.sort_by(|x,y| y.score.partial_cmp(&x.score).unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| x.a.cmp(&y.a)).then_with(|| x.b.cmp(&y.b)));
    let list_len = list.len();

    CandidateList {
        list,
        stats: CandStats {
            total_pairs_before_dedupe: total_pairs,
            unique_pairs: list_len,
            after_per_node_cap: list_len,
            final_pairs: list_len,
        }
    }
}

/// マルチランで Phase C 実行（δ はランごとに張る）
pub fn run_phase_c_multi(
    runs: &[(Vec<u8>, Vec<u8>)],
    cand: &CandidateList,
    target_n: usize,
) -> MergeResult {
    // 既存 run_phase_c は 1 本用なので、内部の MergeState を流用しつつ、
    // 「連結の張り方」だけを "ランごとに" にする別 API を用意するのが最善。
    // 簡便実装：runs を 1 本にフラット化＋「ラン境界をまたがないように δ を構築」
    use crate::phase_c::run_phase_c_internal_from_flat; // ← 既存実装を小改造した内部関数と仮定

    // フラット化
    let mut w_flat = Vec::<u8>::new();
    let mut y_flat = Vec::<u8>::new();
    let mut breaks = Vec::<usize>::new(); // 各ランの開始オフセット
    let mut off = 0usize;
    for (w, y) in runs.iter() {
        breaks.push(off);
        w_flat.extend_from_slice(&w);
        y_flat.extend_from_slice(&y);
        off += y.len(); // 時刻は + (L_i + 1)
    }
    // 内部関数は "breaks" を参照し、境界跨ぎの δ は張らない
    run_phase_c_internal_from_flat(&w_flat, &y_flat, &breaks, cand, target_n)
}
