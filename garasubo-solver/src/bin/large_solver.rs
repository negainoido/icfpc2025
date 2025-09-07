use clap::Parser;
use garasubo_solver::api::ApiClient;
use garasubo_solver::session_manager::SessionManager;
use tokio::signal;

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

use anyhow::{anyhow, bail, Context, Result};
use garasubo_solver::solver::{Dir, GuessMap, GuessRoom, Label, Solver};
use std::collections::{HashMap, VecDeque};

/// ------------------------------
/// Implementation
/// ------------------------------

#[derive(Clone, Debug)]
enum PlanKind {
    /// first fingerprint for s0 (plan = F only)
    InitF,
    /// transition probe from state `from` via door `dir` (plan = A[from] + dir + F)
    TransProbe { from: usize, dir: Dir },
}

#[derive(Clone, Debug)]
struct PendingPlan {
    route: String,
    meta: PlanKind,
}

#[derive(Clone, Debug)]
struct State {
    /// access word A(s): digits '0'..'5' only
    access: String,
    /// natural label λ(s) if observed (first visit to s *before* any chalk in that plan)
    label: Option<Label>,
    /// discovered transitions δ(s, i) = t (room index)
    trans: [Option<usize>; 6],
}

impl State {
    fn new(access: String) -> Self {
        Self {
            access,
            label: None,
            trans: [None, None, None, None, None, None],
        }
    }
}

#[derive(Clone, Debug)]
pub struct FingerprintSolver {
    /// known number of rooms (n)
    n: usize,
    /// per-plan door-step budget (6n)
    budget: usize,

    /// fixed fingerprint program body F (starts with a chalk "[d]")
    f_body: String,
    /// number of *door* steps inside F (chalk steps are not counted for budget)
    f_door_len: usize,

    /// discovered states (index is room id)
    states: Vec<State>,
    /// mapping from fingerprint (Vec<label>) to room id
    fp2id: HashMap<Vec<Label>, usize>,

    /// BFS queue of states to fully expand
    q: VecDeque<usize>,

    /// plans prepared by next_explore_batch and waiting for results
    pending: Vec<PendingPlan>,

    /// whether the initial fingerprint for s0 has been scheduled already
    scheduled_init_f: bool,
}

impl FingerprintSolver {
    /// Create a solver with known room count `n`.
    pub fn new(n: usize) -> Self {
        let budget = 6 * n;

        // Choose a *fixed* K so that K + (max possible |A(s)|) + 1 <= 6n.
        // In any connected graph with n rooms, shortest-path depth ≤ n-1.
        // Use a small safety margin to be conservative.
        let safety_margin = 8usize;
        let max_depth_bound = n.saturating_sub(1);
        let k = {
            let k_raw = budget.saturating_sub(1 + max_depth_bound + safety_margin);
            // keep it reasonably long
            std::cmp::max(20, k_raw)
        };

        let f_body = build_fingerprint_body(k, /*seed=*/ 0xC0FFEE_u64);

        FingerprintSolver {
            n,
            budget,
            f_body,
            f_door_len: k,
            states: vec![State::new(String::new())], // s0
            fp2id: HashMap::new(),
            q: VecDeque::from([0usize]),
            pending: Vec::new(),
            scheduled_init_f: false,
        }
    }

    /// number of discovered rooms so far
    fn discovered(&self) -> usize {
        self.states.len()
    }

    /// schedule transition probes for all known states with missing ports
    fn schedule_all_missing_ports(&mut self) {
        for s in 0..self.states.len() {
            for d in 0u8..=5u8 {
                if self.states[s].trans[d as usize].is_none() {
                    let route = format!(
                        "{}{}{}",
                        self.states[s].access,
                        (b'0' + d) as char,
                        self.f_body
                    );
                    self.pending.push(PendingPlan {
                        route,
                        meta: PlanKind::TransProbe { from: s, dir: d },
                    });
                }
            }
        }
    }

    /// Register a fingerprint -> id mapping (if fresh), return the id.
    fn intern_fingerprint(
        &mut self,
        fp: Vec<Label>,
        make_new_access: Option<String>,
    ) -> Result<usize> {
        if let Some(&id) = self.fp2id.get(&fp) {
            return Ok(id);
        }
        // fresh state
        let id = self.states.len();
        if id >= self.n {
            bail!(
                "discovered more than n={} states; fingerprint collision or inconsistent map",
                self.n
            );
        }
        let access = make_new_access.unwrap_or_default();
        self.states.push(State::new(access));
        self.fp2id.insert(fp, id);
        self.q.push_back(id);
        Ok(id)
    }
}

/// Build a fixed fingerprint body F consisting of:
///   for j in 0..k-1:
///       "[j%4]" + (dir_j) + "[(3*j+1)%4]"
///
/// - Starts with a chalk, so the "fingerprint bytes" always start with chalk output.
/// - `k` is the number of *door* steps inside F (chalks are not counted for the 6n budget).
fn build_fingerprint_body(k: usize, seed: u64) -> String {
    // simple LCG to make a deterministic pseudo-random door sequence in 0..=5
    let mut x = seed | 1;
    let mut s = String::new();
    for j in 0..k {
        // pre-chalk
        let c1 = (j % 4) as u8;
        s.push('[');
        s.push((b'0' + c1) as char);
        s.push(']');

        // door
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let door = ((x >> 60) % 6) as u8; // use high bits for variance
        s.push((b'0' + door) as char);

        // post-chalk (phase-shifted)
        let c2 = (((3 * j + 1) % 4) as u8) & 0b11;
        s.push('[');
        s.push((b'0' + c2) as char);
        s.push(']');
    }
    s
}

impl Solver for FingerprintSolver {
    /// Build the next batch of plans.
    /// Strategy:
    ///   - First call: schedule `F` for s0, and the 6 probes `i+F` from s0.
    ///   - Subsequent calls: schedule all missing ports for all currently known states.
    fn next_explore_batch(&mut self) -> Vec<String> {
        self.pending.clear();

        if !self.scheduled_init_f {
            // 1) s0 fingerprint by running F from s0 (starts with a chalk)
            self.pending.push(PendingPlan {
                route: self.f_body.clone(),
                meta: PlanKind::InitF,
            });
            self.scheduled_init_f = true;

            // 2) also expand s0 immediately (6 ports)
            for d in 0u8..=5u8 {
                let route = format!("{}{}{}", "", (b'0' + d) as char, self.f_body);
                self.pending.push(PendingPlan {
                    route,
                    meta: PlanKind::TransProbe { from: 0, dir: d },
                });
            }
        } else {
            // BFS layer expansion: all missing ports on all discovered states
            self.schedule_all_missing_ports();
        }

        self.pending.iter().map(|p| p.route.clone()).collect()
    }

    /// Consume the results of the last batch.
    fn apply_explore_results(
        &mut self,
        sent_routes: &[String],
        obs_labels: &[Vec<Label>],
    ) -> Result<()> {
        if sent_routes.len() != obs_labels.len() || sent_routes.len() != self.pending.len() {
            bail!(
            "apply_explore_results: length mismatch (sent_routes={}, obs_labels={}, pending={})",
            sent_routes.len(),
            obs_labels.len(),
            self.pending.len()
        );
        }

        for (idx, pending) in self.pending.clone().into_iter().enumerate() {
            let route = &sent_routes[idx];
            let obs = &obs_labels[idx];

            if &pending.route != route {
                bail!(
                    "result route mismatch at {}: expected '{}', got '{}'",
                    idx,
                    pending.route,
                    route
                );
            }

            match pending.meta {
                PlanKind::InitF => {
                    // F は 1 ドア歩につき 2 回のチョークを入れる設計なので、総オペ数は 3K
                    let k = self.f_door_len;
                    let ops_f = 3 * k;

                    // 開始ラベルが含まれているか？
                    let has_initial = match obs.len() {
                        l if l == ops_f => false,    // [なし] ちょうど 3K
                        l if l == ops_f + 1 => true, // [あり] 3K+1
                        l => bail!(
                            "InitF: unexpected obs length {}, expected {} or {}",
                            l,
                            ops_f,
                            ops_f + 1
                        ),
                    };

                    // 開始ラベルがあれば s0 の自然ラベルを埋める
                    if has_initial && self.states[0].label.is_none() {
                        self.states[0].label = Some(obs[0]);
                    }

                    // 指紋は常に「F の最初のチョーク」から開始
                    let fp_start = if has_initial { 1 } else { 0 };
                    if obs.len() <= fp_start {
                        bail!("InitF: empty fingerprint slice");
                    }
                    let fp: Vec<Label> = obs[fp_start..].to_vec();

                    if let Some(&prev) = self.fp2id.get(&fp) {
                        if prev != 0 {
                            bail!(
                                "InitF: fingerprint already mapped to different room id {}",
                                prev
                            );
                        }
                    } else {
                        self.fp2id.insert(fp, 0);
                    }
                }

                PlanKind::TransProbe { from, dir } => {
                    // アクセス語のドア歩数
                    let a_len = self.states[from].access.len();
                    let k = self.f_door_len;
                    let ops_total = a_len + 1 + 3 * k; // A(s) + 'dir' + F(=3K)

                    // 開始ラベルが含まれているか？
                    let has_initial = match obs.len() {
                        l if l == ops_total => false,
                        l if l == ops_total + 1 => true,
                        l => bail!(
                            "TransProbe: unexpected obs length {}, expected {} or {}",
                            l,
                            ops_total,
                            ops_total + 1
                        ),
                    };

                    // 到達先自然ラベルの位置
                    // [あり]  A(s) 後の現在室ラベルが obs[a_len]、'dir' 後の到達先が obs[a_len+1]
                    // [なし]  'dir' 後の到達先が obs[a_len]
                    let reached_idx = if has_initial { a_len + 1 } else { a_len };
                    if obs.len() <= reached_idx {
                        bail!("TransProbe: obs too short (reached_idx={})", reached_idx);
                    }
                    let reached_label = obs[reached_idx];

                    // 指紋は「到達先に対する F の最初のチョーク」から
                    let fp_start = reached_idx + 1;
                    if obs.len() <= fp_start {
                        bail!(
                            "TransProbe: empty fingerprint slice (fp_start={})",
                            fp_start
                        );
                    }
                    let fp: Vec<Label> = obs[fp_start..].to_vec();

                    // 既知 or 新規
                    let to_id = if let Some(&id) = self.fp2id.get(&fp) {
                        id
                    } else {
                        let mut access = self.states[from].access.clone();
                        access.push((b'0' + dir) as char);
                        let id = self.intern_fingerprint(fp, Some(access))?;
                        id
                    };

                    if self.states[to_id].label.is_none() {
                        self.states[to_id].label = Some(reached_label);
                    }
                    self.states[from].trans[dir as usize] = Some(to_id);
                }
            }
        }

        self.pending.clear();
        Ok(())
    }

    /// Build the final guess (rooms, starting_room, and per-door connections).
    fn build_guess(&self) -> Result<GuessMap> {
        if self.discovered() != self.n {
            bail!(
                "not all rooms discovered yet: have {}, need {}",
                self.discovered(),
                self.n
            );
        }
        // every state must have all 6 transitions known
        for (i, st) in self.states.iter().enumerate() {
            for d in 0..6 {
                if st.trans[d].is_none() {
                    bail!("state {} door {} not resolved yet", i, d);
                }
            }
        }

        // assemble GuessRoom list with labels and empty doors
        let mut rooms: Vec<GuessRoom> = Vec::with_capacity(self.n);
        for (i, st) in self.states.iter().enumerate() {
            let label = st
                .label
                .ok_or_else(|| anyhow!("room {} natural label not observed yet", i))?;
            rooms.push(GuessRoom {
                label,
                doors: [None, None, None, None, None, None],
            });
        }

        // Fill undirected connections. For each (s,i) -> t, find j with δ(t,j)=s.
        for s in 0..self.n {
            for i in 0..6 {
                let t = self.states[s].trans[i].unwrap();
                if rooms[s].doors[i].is_some() {
                    continue; // already connected
                }
                // find peer port j
                let mut peer_opt: Option<usize> = None;
                for j in 0..6 {
                    if self.states[t].trans[j] == Some(s) {
                        peer_opt = Some(j);
                        break;
                    }
                }
                let j = peer_opt.ok_or_else(|| {
                    anyhow!(
                        "no reciprocal port found: s={} door {} -> t={} has no back-edge",
                        s,
                        i,
                        t
                    )
                })?;

                // set both sides
                rooms[s].doors[i] = Some((t, j as u8));
                rooms[t].doors[j] = Some((s, i as u8));
            }
        }

        Ok(GuessMap {
            rooms,
            starting_room: 0,
        })
    }
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

    let mut solver = FingerprintSolver::new(cli.room_num);
    loop {
        let next_batch = solver.next_explore_batch();
        if next_batch.is_empty() {
            println!("No more routes to explore. Finishing session...");
            break;
        }
        println!("Sending batch of {} routes to explore...", next_batch.len());
        let result = session_guard.explore(&next_batch).await?;
        solver.apply_explore_results(&next_batch, &result.results)?;
    }
    let map = solver.build_guess()?;

    let guess_response = session_guard.guess(map.convert_to_api_guess_map()?).await?;
    println!("Guess response: {:?}", guess_response);

    if guess_response.correct {
        println!("🎉 Guess was CORRECT!");
    } else {
        println!("❌ Guess was incorrect.");
    }

    session_guard.mark_success();
    println!("Work completed successfully");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// ---------- モック環境 ----------
    /// 6方向すべてにドアがある（自己ループや多重はあり得る）という仕様に従う。
    #[derive(Clone)]
    struct MockRoom {
        label: Label,
        /// (to_room, peer_dir)
        doors: [(usize, Dir); 6],
    }

    #[derive(Clone)]
    struct MockWorld {
        rooms: Vec<MockRoom>,
        start: usize,
    }

    impl MockWorld {
        fn new(n: usize) -> Self {
            assert!(n >= 4);
            let mut rooms: Vec<MockRoom> = (0..n)
                .map(|i| MockRoom {
                    label: (i % 4) as u8,
                    doors: [(usize::MAX, 0); 6],
                })
                .collect();

            // 規則的につなぐ：0<->3 は (r) <-> (r+1)、1<->4 は (r) <-> (r+2)、2<->5 は自己ループ
            for r in 0..n {
                let a = r;
                let b = (r + 1) % n;
                rooms[a].doors[0] = (b, 3);
                rooms[b].doors[3] = (a, 0);
            }
            for r in 0..n {
                let a = r;
                let b = (r + 2) % n;
                rooms[a].doors[1] = (b, 4);
                rooms[b].doors[4] = (a, 1);
            }
            for r in 0..n {
                rooms[r].doors[2] = (r, 5);
                rooms[r].doors[5] = (r, 2);
            }

            Self { rooms, start: 0 }
        }

        /// ルートを実行して観測列を返す。
        /// include_initial = true: 開始ラベルを最初に1つ返す（x+1 仕様）。
        /// include_initial = false: 各操作（ドア or チョーク）ごとに1観測。`[d]` は d がそのまま返る（ユーザー注記）。
        fn run_plan(&self, route: &str, include_initial: bool) -> Vec<Label> {
            let mut cur = self.start;
            let mut obs: Vec<Label> = Vec::new();
            // プラン内での一時的な上書きラベル
            let mut override_label: HashMap<usize, Label> = HashMap::new();

            let mut read_label = |room: usize, override_label: &HashMap<usize, Label>| -> Label {
                if let Some(&v) = override_label.get(&room) {
                    v
                } else {
                    self.rooms[room].label
                }
            };

            if include_initial {
                obs.push(read_label(cur, &override_label));
            }

            let bytes = route.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                let ch = bytes[i] as char;
                if ch == '[' {
                    // チョーク: "[d]"
                    assert!(i + 2 < bytes.len() && bytes[i + 2] as char == ']');
                    let d = bytes[i + 1] - b'0';
                    assert!(d <= 3);
                    override_label.insert(cur, d);
                    obs.push(d);
                    i += 3;
                } else {
                    // ドア移動
                    let dir = bytes[i] - b'0';
                    assert!(dir <= 5);
                    let (to, _peer) = self.rooms[cur].doors[dir as usize];
                    cur = to;
                    let v = read_label(cur, &override_label);
                    obs.push(v);
                    i += 1;
                }
            }
            obs
        }

        fn expected_guess_map(&self) -> GuessMap {
            let mut rooms = Vec::with_capacity(self.rooms.len());
            for r in 0..self.rooms.len() {
                let mut gr = GuessRoom {
                    label: self.rooms[r].label,
                    doors: [None, None, None, None, None, None],
                };
                for d in 0..6usize {
                    let (to, peer) = self.rooms[r].doors[d];
                    gr.doors[d] = Some((to, peer));
                }
                rooms.push(gr);
            }
            GuessMap {
                rooms,
                starting_room: self.start,
            }
        }
    }

    /// guess が world と同値か（部屋番号の単純同型を許す）チェック
    fn assert_equivalent(world: &MockWorld, guess: &GuessMap) {
        let n = world.rooms.len();
        assert_eq!(guess.rooms.len(), n, "room count mismatch");
        // 同型写像 φ: guess→world
        let mut phi: Vec<Option<usize>> = vec![None; n];
        let mut psi: Vec<Option<usize>> = vec![None; n];
        let mut q = std::collections::VecDeque::new();

        let gs = guess.starting_room;
        let ws = world.start;
        phi[gs] = Some(ws);
        psi[ws] = Some(gs);
        q.push_back((gs, ws));

        while let Some((g, w)) = q.pop_front() {
            assert_eq!(
                guess.rooms[g].label, world.rooms[w].label,
                "label mismatch at g={} w={}", g, w
            );

            for d in 0..6usize {
                let (tg, jd_g) = guess.rooms[g].doors[d].expect("missing door in guess");
                let (tw, jd_w) = world.rooms[w].doors[d];
                assert_eq!(jd_g, jd_w, "peer port mismatch at g={},w={},d={}", g, w, d);

                match (phi[tg], psi[tw]) {
                    (None, None) => {
                        phi[tg] = Some(tw);
                        psi[tw] = Some(tg);
                        q.push_back((tg, tw));
                    }
                    (Some(mw), Some(mg)) => {
                        assert_eq!(mw, tw, "world index mismatch (phi)");
                        assert_eq!(mg, tg, "guess index mismatch (psi)");
                    }
                    (Some(mw), None) => {
                        assert_eq!(mw, tw, "world index mismatch (partial)");
                        psi[tw] = Some(tg);
                        q.push_back((tg, tw));
                    }
                    (None, Some(mg)) => {
                        assert_eq!(mg, tg, "guess index mismatch (partial)");
                        phi[tg] = Some(tw);
                        q.push_back((tg, tw));
                    }
                }
            }
        }
    }

    /// バッチを回して完了まで走らせる共通ヘルパ
    fn run_until_done(mut solver: FingerprintSolver, world: &MockWorld, include_initial: bool) -> GuessMap {
        for iter in 0..100 {
            let plans = solver.next_explore_batch();
            assert!(
                !plans.is_empty(),
                "iteration {} produced empty batch unexpectedly",
                iter
            );

            let results: Vec<Vec<Label>> = plans
                .iter()
                .map(|p| world.run_plan(p, include_initial))
                .collect();

            solver
                .apply_explore_results(&plans, &results)
                .expect("apply_explore_results failed");

            if let Ok(g) = solver.build_guess() {
                return g;
            }
        }
        panic!("did not converge within 100 batches");
    }

    #[test]
    fn solves_with_initial_label_semantics() {
        let n = 12;
        let world = MockWorld::new(n);
        let solver = FingerprintSolver::new(n);
        let guess = run_until_done(solver, &world, /*include_initial=*/ true);
        assert_equivalent(&world, &guess);
    }

    #[test]
    fn solves_without_initial_label_semantics() {
        let n = 12;
        let world = MockWorld::new(n);
        let solver = FingerprintSolver::new(n);
        let guess = run_until_done(solver, &world, /*include_initial=*/ false);
        assert_equivalent(&world, &guess);
    }

    /// 途中で探索を中断（= まだ state が埋まっていない状態で build_guess を呼ぶ）→ 失敗し、
    /// その後に再開して完了できることを確認
    #[test]
    fn interruption_then_resume() {
        let n = 12;
        let world = MockWorld::new(n);
        let mut solver = FingerprintSolver::new(n);

        // 1回だけ /explore 実行
        let plans1 = solver.next_explore_batch();
        let res1: Vec<Vec<Label>> = plans1
            .iter()
            .map(|p| world.run_plan(p, /*include_initial=*/ true))
            .collect();
        solver
            .apply_explore_results(&plans1, &res1)
            .expect("first apply failed");

        // まだ埋まり切らないはず
        assert!(solver.build_guess().is_err(), "should not be complete yet");

        // 再開：以後は完了するまで回す
        for _ in 0..100 {
            let plans = solver.next_explore_batch();
            let res: Vec<Vec<Label>> = plans
                .iter()
                .map(|p| world.run_plan(p, /*include_initial=*/ true))
                .collect();
            solver
                .apply_explore_results(&plans, &res)
                .expect("apply failed");

            if let Ok(guess) = solver.build_guess() {
                assert_equivalent(&world, &guess);
                return;
            }
        }
        panic!("did not finish after resume");
    }

    /// 未適用のバッチがある状態で next_explore_batch を呼ぶと「同じバッチを返す」ことを確認
    #[test]
    fn next_batch_replays_when_pending() {
        let n = 8;
        let mut solver = FingerprintSolver::new(n);
        let batch1 = solver.next_explore_batch();
        // apply する前にもう一度
        let batch2 = solver.next_explore_batch();
        assert_eq!(batch1, batch2, "pending batch must be replayed unchanged");
    }
}