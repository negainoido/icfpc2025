use std::collections::HashSet;
use clap::Parser;
use tokio::signal;
use garasubo_solver::api::{ApiClient, Connection, GuessMap, RoomDoor};
use garasubo_solver::candidate_gen::{build_candidates, CandParams, Candidate, CandidateList, Hits};
use garasubo_solver::{cover_walk, phase_c};
use garasubo_solver::finalize_map::finalize_guess_map;
use garasubo_solver::multi_run_pipeline::{build_candidates_multi, build_signature_index_multi, run_phase_c_multi};
use garasubo_solver::pass2_id_double::{build_double_id_plans_from_candidates, DoubleIdBatch, DoubleIdParams, PairKey};
use garasubo_solver::pass3_unknown_explore::{build_unknown_edge_plans_with_probe, UnknownExploreOptions};
use garasubo_solver::phase_c::run_phase_c;
use garasubo_solver::rp_verify_batch::{apply_rp_hits_in_place, build_rp_verify_batch, eval_rp_verify_batch, RpBatchParams};
use garasubo_solver::session_manager::SessionManager;
use garasubo_solver::signature_index::{build_signature_index, BuildParams};

#[derive(Parser)]
#[command(name = "garasubo-solver")]
#[command(about = "ICFPC 2025 Problem Solver")]
struct Cli {
    problem_name: String,

    #[arg(long)]
    user_name: Option<String>,

    #[arg(long)]
    room_num: usize,

    #[arg(long, default_value = "https://negainoido.garasubo.com/api")]
    api_base_url: String,
}

// 評価: 各プランのラベル列と watch を照合して true/false を返す
fn eval_double_id_batch(batch: &DoubleIdBatch, results: &[Vec<i32>]) -> (Vec<(usize,usize)>, Vec<(usize,usize)>) {
    // plans は ペアごとに [plan1, plan2] の順
    let mut confirmed = Vec::<(usize,usize)>::new();
    let mut refuted   = Vec::<(usize,usize)>::new();
    for pair_idx in 0..batch.per_pair.len() {
        let (ca, cb) = batch.per_pair[pair_idx];
        let p1 = 2*pair_idx;
        let p2 = 2*pair_idx + 1;

        let mut ok1 = false;
        if let Some(watch_list) = batch.watches_per_plan.get(p1) {
            // この ID プランは watch は1個だけ
            if let Some(w) = watch_list.get(0) {
                let y = &results[p1];
                let pos = w.pos;
                if pos < y.len() {
                    ok1 = (y[pos] as u8) == w.expect_color;
                }
            }
        }
        let mut ok2 = false;
        if let Some(watch_list) = batch.watches_per_plan.get(p2) {
            if let Some(w) = watch_list.get(0) {
                let y = &results[p2];
                let pos = w.pos;
                if pos < y.len() {
                    ok2 = (y[pos] as u8) == w.expect_color;
                }
            }
        }
        if ok1 && ok2 {
            confirmed.push((ca, cb));
        } else if (!ok1) || (!ok2) {
            refuted.push((ca, cb));
        }
        // どちらも参照不能（pos範囲外）なら何もしない（次ラウンドで再挑戦）
    }
    (confirmed, refuted)
}

// BIG スコア候補を追加して Phase C を再実行
fn apply_forced_merges_and_rerun(
    w: &[u8], y: &[u8],
    base_cands: &CandidateList,
    merge: &phase_c::MergeResult,
    confirmed_pairs: &[(usize,usize)],
    target_n: usize
) -> (phase_c::MergeResult, CandidateList) {
    if confirmed_pairs.is_empty() {
        return (merge.clone(), base_cands.clone());
    }
    let mut cands = base_cands.clone();
    const BIG: f64 = 1e9;
    for &(ca, cb) in confirmed_pairs {
        if ca == cb { continue; }
        // 代表時刻をとって時刻ペア候補に
        let ta = merge.cluster_representatives[ca];
        let tb = merge.cluster_representatives[cb];
        cands.list.push(Candidate {
            a: ta as u32,
            b: tb as u32,
            score: BIG,
            hits: Hits { f1:0, b1:0, f2:0, b2:0, mix:0 },
        });
    }
    let merged2 = run_phase_c(w, y, &cands, target_n);
    (merged2, cands)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();
    let session_manager = SessionManager::new(ApiClient::new(&cli.api_base_url));

    let session_manager_for_signal = session_manager.current_session.clone();
    let api_client_for_signal = ApiClient::new(&cli.api_base_url);

    tokio::spawn(async move {
        if let Ok(()) = signal::ctrl_c().await {
            println!("\nReceived Ctrl+C, aborting session...");
            let session = session_manager_for_signal.lock().await;
            if let Some(ref session_id) = *session {
                if let Err(e) = api_client_for_signal.abort_session(session_id).await {
                    eprintln!(
                        "Warning: Failed to abort session {} on Ctrl+C: {:#}",
                        session_id, e
                    );
                } else {
                    println!("Session {} aborted successfully on Ctrl+C", session_id);
                }
            }
            std::process::exit(130); // Exit with SIGINT status
        }
    });

    let session_guard = session_manager
        .start_session_with_guard(cli.problem_name.clone(), cli.user_name)
        .await?;

    let n = cli.room_num;
    // まずはランダムウォークを生成して投げる
    let cover_walk = cover_walk::generate_cover_walk(n);
    println!("cover walk ({}): {:?}", cover_walk.len(), cover_walk);
    let initial_plan = vec![ cover_walk.iter().map(|i| ('0' as u8 + *i) as char).collect::<String>() ];
    let initial_result = session_guard.explore(&initial_plan).await?;
    //println!("initial random walk: {:?}", initial_result);
    let y = &initial_result.results[0];

    // signature indexを生成
    let index = build_signature_index(&cover_walk, &y, BuildParams::default())?;
    println!("index: {:?}", index);

    // phase_c
    let candidate = build_candidates(&cover_walk, &y, &index, CandParams::default());
    let merge_result = run_phase_c(&cover_walk, &y, &candidate, n);

    let mut cur_merge = merge_result.clone();
    let mut cur_cands = candidate.clone();
    let mut forbid: HashSet<PairKey> = HashSet::new();

    let max_id_rounds = 3usize;
    for round in 0..max_id_rounds {
        if cur_merge.cluster_count <= n { break; }

        // 1) 上位ペアから二重証人プランを作る
        let batch = build_double_id_plans_from_candidates(
            &cover_walk, &y, &cur_merge, &cur_cands,
            DoubleIdParams {
                max_pairs: 12,               // 1 ラウンドで検証するペア数
                allow_mixed_directions: true,
                min_separation: 10,
            },
            Some(&forbid)                    // 否定済みは除外
        );
        if batch.plans.is_empty() {
            eprintln!("[DW] no double-witness plans generated; break");
            break;
        }

        // 2) /explore を一括送信（ペア数×2 プラン）
        let dw_result = session_guard.explore(&batch.plans).await?;
        let results_u8: Vec<Vec<i32>> = dw_result.results.iter().map(|x| x.iter().map(|v| *v as i32).collect()).collect(); // そのまま i32 -> u8 で読む

        // 3) 判定：2 本とも一致 → confirmed、どちらか不一致 → refuted
        let (confirmed, refuted) = eval_double_id_batch(&batch, &results_u8);

        // 4) forbid 更新（否定確証を以降の候補から外す）
        for (a,b) in &refuted {
            forbid.insert(PairKey::new(*a,*b));
        }

        // 5) 確証だけ BIG 候補で注入 → Phase C 再実行
        let prev_clusters = cur_merge.cluster_count;
        let (merged2, cands2) = apply_forced_merges_and_rerun(
            &cover_walk, &y, &cur_cands, &cur_merge, &confirmed, n
        );
        cur_merge = merged2;
        cur_cands = cands2;

        eprintln!("[DW] round {}: confirmed={} refuted={} clusters {} -> {}",
                  round, confirmed.len(), refuted.len(), prev_clusters, cur_merge.cluster_count);

        // 進捗がなければ打ち切り
        if confirmed.is_empty() && cur_merge.cluster_count == prev_clusters { break; }
    }

    // 二重確証 ID 後の結果を以降に渡す
    let merge_after_id = cur_merge;
    let candidate_after_id = cur_cands;

    // 以降、未知エッジ埋め（あなたのコードの続き）に接続
    // 注意：mini plans（プローブ）は「木炭なし」なので、その結果はマルチランへ
    let mini_plan = build_unknown_edge_plans_with_probe(&merge_after_id, n, UnknownExploreOptions::default());
    let unknown_edge_result = session_guard.explore(&mini_plan).await?;
    let zipped_plans_and_results = mini_plan.iter().zip(unknown_edge_result.results.iter());

    let mut runs: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    runs.push((cover_walk, y.clone()));
    for (plan_str, y_vec_from_server) in zipped_plans_and_results {
        let wi: Vec<u8> = plan_str.as_bytes().iter().map(|&b| (b - b'0') as u8).collect();
        let yi: Vec<u8> = y_vec_from_server.iter().map(|&x| x as u8).collect();
        runs.push((wi, yi));
    }

    let (sig_idx, uni) = build_signature_index_multi(&runs, BuildParams::default())?;
    let y_flat: Vec<u8> = runs.iter().flat_map(|(_w,y)| y.clone()).collect();
    let cand = build_candidates_multi(uni, &sig_idx, &y_flat, CandParams::default());
    let mut merged3 = run_phase_c_multi(&runs, &cand, n);



    // finalize
    match finalize_guess_map(&merged3) {
        Ok(report) => {
            let map = report.map;

            let api_map = GuessMap {
                rooms: map.rooms.iter().map(|x| *x as i32).collect(),
                starting_room: map.starting_room,
                connections: map.connections.iter().map(|c| {
                    Connection {
                        from: RoomDoor { room: c.from_room, door: c.from_door as usize },
                        to: RoomDoor { room: c.to_room, door: c.to_door as usize },
                    }
                }).collect(),
            };
            let guess_response = session_guard.guess(api_map).await?;
            println!("Guess response: {:?}", guess_response);
            if guess_response.correct {
                println!("🎉 Guess was CORRECT!");
            } else {
                println!("❌ Guess was incorrect.");
            }
        }
        Err(e) => {
            eprintln!("[RP] finalize failed: {:?}", e);
            // 逆ポート不足を埋めたい場合：RP 検証を実施
            // ここでは常に実施例を示します（必要なら条件で分岐）
            let rp_batch = build_rp_verify_batch(&merged3, n, RpBatchParams {
                max_probes: 64,         // 一度に検証する本数
                prefer_singleton: true, // |J|=1 を優先
                limit_ratio: 6.0,
            });
            if !rp_batch.plans.is_empty() {
                let rp_result = session_guard.explore(&rp_batch.plans).await?;
                let hits = eval_rp_verify_batch(&rp_batch, &rp_result.results.iter().map(|x| x.iter().map(|v| *v as i32).collect::<Vec<i32>>()).collect::<Vec<Vec<i32>>>(),);
                if !hits.is_empty() {
                    // 命中を δ に反映（マルチランの索引には入れない）
                    apply_rp_hits_in_place(&mut merged3, &hits);
                }
            }

            // 反映後に finalize を再試行
            match finalize_guess_map(&merged3) {
                Ok(report2) => {
                    // -> /guess
                    let map = report2.map;

                    let api_map = GuessMap {
                        rooms: map.rooms.iter().map(|x| *x as i32).collect(),
                        starting_room: map.starting_room,
                        connections: map.connections.iter().map(|c| {
                            Connection {
                                from: RoomDoor { room: c.from_room, door: c.from_door as usize },
                                to: RoomDoor { room: c.to_room, door: c.to_door as usize },
                            }
                        }).collect(),
                    };
                    let guess_response = session_guard.guess(api_map).await?;
                    println!("Guess response: {:?}", guess_response);
                    if guess_response.correct {
                        println!("🎉 Guess was CORRECT!");
                    } else {
                        println!("❌ Guess was incorrect.");
                    }
                }
                Err(e2) => {
                    eprintln!("[RP] finalize failed: {:?}", e2);
                    // まだ NeedMoreExploreAtNode が出る場合は
                    //   1) 未知ポート拡張（tail >= 2）をもう1バッチ
                    //   2) その結果を runs に追加 → マルチラン再クラスタ → RP を再度
                    // の順で“収束ループ”を回してください。
                }
            }
        }
    }

    session_guard.mark_success();
    println!("Work completed successfully");

    Ok(())
}
