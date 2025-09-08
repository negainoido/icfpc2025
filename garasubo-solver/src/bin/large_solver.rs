use clap::Parser;
use garasubo_solver::api::ApiClient;
use garasubo_solver::session_manager::SessionManager;
use std::cmp::Ordering;
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
use std::collections::{HashMap, HashSet, VecDeque};

/* =========================
 *   Union-Find
 * ========================= */
#[derive(Clone, Debug)]
struct UF {
    p: Vec<usize>,
    sz: Vec<usize>,
}
impl UF {
    fn new() -> Self {
        Self {
            p: Vec::new(),
            sz: Vec::new(),
        }
    }
    fn add(&mut self) -> usize {
        let id = self.p.len();
        self.p.push(id);
        self.sz.push(1);
        id
    }
    fn find(&mut self, x: usize) -> usize {
        if self.p[x] != x {
            let r = self.find(self.p[x]);
            self.p[x] = r;
        }
        self.p[x]
    }
    fn same(&mut self, a: usize, b: usize) -> bool {
        self.find(a) == self.find(b)
    }
    fn unite(&mut self, a: usize, b: usize) -> usize {
        let mut a = self.find(a);
        let mut b = self.find(b);
        if a == b {
            return a;
        }
        if self.sz[a] < self.sz[b] {
            std::mem::swap(&mut a, &mut b);
        }
        self.p[b] = a;
        self.sz[a] += self.sz[b];
        a
    }
}

/* =========================
 *   Solver impl
 * ========================= */

#[derive(Clone, Debug)]
enum PlanKind {
    /// plan = F only (s0の初回指紋)
    InitF,
    /// plan = A[from] + dir + F
    TransProbe { from: usize, dir: Dir },
    /// plan = A[a] + [mark] + back(A[a]) + A[b]
    EqCheck { a: usize, b: usize, mark: Label },
}

#[derive(Clone, Debug)]
struct PendingPlan {
    route: String,
    meta: PlanKind,
}

#[derive(Clone, Debug)]
struct State {
    /// 生成時のアクセス語（start からの扉列、チョーク無し）
    access: String,
    /// 自然ラベル（初回到達時に確定）
    label: Option<Label>,
    /// δ(s, i) = to（未確定は None）。to は「生成ノード id」（代表ではない）
    trans: [Option<usize>; 6],
    /// BFS 木の親（最初に到達したときの from と dir）
    parent: Option<usize>,
    parent_dir: Option<Dir>,
    /// 親へ戻る自ポート j（δ(s, j) == parent の j）。確定後に Eq 検査の復路に使う
    back_to_parent: Option<Dir>,
}

impl State {
    fn new(access: String) -> Self {
        Self {
            access,
            label: None,
            trans: [None, None, None, None, None, None],
            parent: None,
            parent_dir: None,
            back_to_parent: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FingerprintSolver {
    /// known number of rooms (n)
    n: usize,
    /// per-plan door-step budget (6n)
    budget: usize,

    /// fingerprint program body F（「[d]→door→[d]」×K）
    f_body: String,
    f_door_len: usize,

    /// 全ノード（物理部屋の複製を含む）
    states: Vec<State>,

    /// Union-Find（物理部屋の同一化）
    uf: UF,

    /// 代表ノードが保持する“統合済み遷移”（union 時にマージ）
    /// キーは代表 id（uf.find(i) == i）
    rep_trans: HashMap<usize, [Option<usize>; 6]>,
    /// 代表ノードの自然ラベル
    rep_label: HashMap<usize, Label>,
    /// 代表ノードの代表アクセス語（最小辞書順）
    rep_access: HashMap<usize, String>,

    /// バッチ保留
    pending: Vec<PendingPlan>,
    scheduled_init_f: bool,

    /// Eq 候補（a, b の順序は小さい代表 id から）
    eq_candidates: HashSet<(usize, usize)>,
}

impl FingerprintSolver {
    pub fn new(n: usize) -> Self {
        let budget = 6 * n;
        // K は「|A最大| + 1 + K <= 6n」を満たす大きめを選ぶ
        let safety_margin = 8usize;
        let max_depth_bound = n.saturating_sub(1);
        let k = {
            let k_raw = budget.saturating_sub(1 + max_depth_bound + safety_margin);
            std::cmp::max(20, k_raw)
        };
        let f_body = build_fingerprint_body(k, 0xC0FFEE_u64);

        let mut uf = UF::new();
        uf.add(); // s0
        let mut rep_trans = HashMap::new();
        rep_trans.insert(0, [None, None, None, None, None, None]);
        let mut rep_access = HashMap::new();
        rep_access.insert(0, String::new());

        Self {
            n,
            budget,
            f_body,
            f_door_len: k,
            states: vec![State::new(String::new())],
            uf,
            rep_trans,
            rep_label: HashMap::new(),
            rep_access,
            pending: Vec::new(),
            scheduled_init_f: false,
            eq_candidates: HashSet::new(),
        }
    }

    fn rep_of(&mut self, x: usize) -> usize {
        self.uf.find(x)
    }

    fn rep_count(&mut self) -> usize {
        (0..self.states.len())
            .filter(|&i| self.uf.find(i) == i)
            .count()
    }

    fn ensure_rep_slot(&mut self, r: usize) {
        self.rep_trans
            .entry(r)
            .or_insert([None, None, None, None, None, None]);
        self.rep_access
            .entry(r)
            .or_insert_with(|| self.states[r].access.clone());
        if let Some(lbl) = self.states[r].label {
            self.rep_label.entry(r).or_insert(lbl);
        }
    }

    fn add_state(&mut self, access: String) -> usize {
        let id = self.states.len();
        self.states.push(State::new(access));
        let r = self.uf.add();
        debug_assert_eq!(r, id);
        self.ensure_rep_slot(id);
        id
    }

    /// union 時に、代表の遷移・ラベル・アクセス語をマージ
    fn union_merge(&mut self, a: usize, b: usize) -> usize {
        let ra = self.uf.find(a);
        let rb = self.uf.find(b);
        if ra == rb {
            return ra;
        }
        let r = self.uf.unite(ra, rb);
        let o = if r == ra { rb } else { ra };

        // ラベル
        let la = self.rep_label.get(&ra).copied().or(self.states[ra].label);
        let lb = self.rep_label.get(&rb).copied().or(self.states[rb].label);
        let lbl = match (la, lb) {
            (Some(x), Some(y)) => {
                if x != y {
                    panic!("label conflict on union: {} vs {}", x, y);
                }
                x
            }
            (Some(x), None) => x,
            (None, Some(y)) => y,
            (None, None) => {
                // どちらも未設定なら据え置き
                0
            }
        };
        if la.is_some() || lb.is_some() {
            self.rep_label.insert(r, lbl);
        }

        // アクセス語：辞書順で最小を代表に
        let acc_r = self
            .rep_access
            .get(&ra)
            .cloned()
            .unwrap_or_else(|| self.states[ra].access.clone());
        let acc_o = self
            .rep_access
            .get(&rb)
            .cloned()
            .unwrap_or_else(|| self.states[rb].access.clone());
        let best = match acc_r.cmp(&acc_o) {
            Ordering::Less | Ordering::Equal => acc_r,
            Ordering::Greater => acc_o,
        };
        self.rep_access.insert(r, best);

        // 遷移マージ（代表配列を統合、隣接も代表化）
        let mut tr = *self.rep_trans.get(&ra).unwrap_or(&[None; 6]);
        let to = *self.rep_trans.get(&rb).unwrap_or(&[None; 6]);

        for d in 0..6 {
            match (tr[d], to[d]) {
                (None, None) => {}
                (Some(x), None) => tr[d] = Some(self.uf.find(x)),
                (None, Some(y)) => tr[d] = Some(self.uf.find(y)),
                (Some(x), Some(y)) => {
                    let rx = self.uf.find(x);
                    let ry = self.uf.find(y);
                    if rx != ry {
                        let rxy = self.union_merge(rx, ry);
                        tr[d] = Some(rxy);
                    } else {
                        tr[d] = Some(rx);
                    }
                }
            }
        }
        self.rep_trans.insert(r, tr);
        // 古い代表のエントリは放置（find で隠れる）

        r
    }

    /// 代表 r のポート d が未確定なら、代表配列を参照して None かどうか判定
    fn port_unresolved(&mut self, r: usize, d: usize) -> bool {
        let r = self.uf.find(r);
        self.ensure_rep_slot(r);
        self.rep_trans[&r][d].is_none()
    }

    /// 代表 r のポート d に to を記録（to は“生成 id”）
    fn set_port(&mut self, r: usize, d: usize, to: usize) {
        let r = self.uf.find(r);
        self.ensure_rep_slot(r);
        self.rep_trans.get_mut(&r).unwrap()[d] = Some(self.uf.find(to));
    }

    /// s の親への逆ポートが分かったらセット
    fn maybe_set_back_to_parent(&mut self, s: usize) {
        if self.states[s].back_to_parent.is_some() {
            return;
        }
        if let Some(p) = self.states[s].parent {
            let rp = self.uf.find(p);
            // s の遷移のどれかが親代表に向いていれば、それが逆ポート
            for j in 0..6 {
                if let Some(t) = self.states[s].trans[j] {
                    if self.uf.find(t) == rp {
                        self.states[s].back_to_parent = Some(j as u8);
                        break;
                    }
                }
            }
        }
    }

    /// a から start に戻る「逆経路語」を作る（全ステップで back_to_parent が分かっている場合のみ）
    fn make_backword_to_start(&self, mut a: usize) -> Option<String> {
        let mut v: Vec<u8> = Vec::new();
        loop {
            if a == 0 {
                break;
            }
            let st = &self.states[a];
            let j = st.back_to_parent?;
            v.push(j);
            a = st.parent?;
        }
        let s: String = v.into_iter().map(|d| (b'0' + d) as char).collect();
        Some(s)
    }

    /// Eq 候補を（代表 id の昇順で）登録
    fn add_eq_candidate(&mut self, a: usize, b: usize) {
        let mut ra = self.uf.find(a);
        let mut rb = self.uf.find(b);
        if ra == rb {
            return;
        }
        if ra > rb {
            std::mem::swap(&mut ra, &mut rb);
        }
        self.eq_candidates.insert((ra, rb));
    }
}

/* ---------- 指紋語 ---------- */

/// F: for j in 0..k-1: "[j%4]" + door + "[(3*j+1)%4]"
fn build_fingerprint_body(k: usize, seed: u64) -> String {
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
        let door = ((x >> 60) % 6) as u8;
        s.push((b'0' + door) as char);

        // post-chalk
        let c2 = (((3 * j + 1) % 4) as u8) & 0b11;
        s.push('[');
        s.push((b'0' + c2) as char);
        s.push(']');
    }
    s
}

impl Solver for FingerprintSolver {
    fn next_explore_batch(&mut self) -> Vec<String> {
        if !self.pending.is_empty() {
            return self.pending.iter().map(|p| p.route.clone()).collect();
        }

        if !self.scheduled_init_f {
            // s0 の初回指紋＋s0 の 6 ポート即展開
            self.pending.push(PendingPlan {
                route: self.f_body.clone(),
                meta: PlanKind::InitF,
            });
            self.scheduled_init_f = true;
            for d in 0u8..=5u8 {
                self.pending.push(PendingPlan {
                    route: format!("{}{}{}", "", (b'0' + d) as char, self.f_body),
                    meta: PlanKind::TransProbe { from: 0, dir: d },
                });
            }
        } else {
            // 1) 代表ノードの未確定ポートをすべて投げる
            let reps: Vec<usize> = (0..self.states.len())
                .filter(|&i| self.uf.find(i) == i)
                .collect();
            for &r in &reps {
                let acc = self.rep_access.get(&r).cloned().unwrap_or_default();
                for d in 0u8..=5u8 {
                    if self.port_unresolved(r, d as usize) {
                        let route = format!("{}{}{}", acc, (b'0' + d) as char, self.f_body);
                        self.pending.push(PendingPlan {
                            route,
                            meta: PlanKind::TransProbe { from: r, dir: d },
                        });
                    }
                }
            }

            // 2) 逆経路が分かった Eq 候補をまとめて投げる
            for &(a, b) in &self.eq_candidates.clone() {
                // a の逆経路が全部わかっているときだけ
                if let Some(back) = self.make_backword_to_start(a) {
                    // mark は b の自然ラベル+1（自然ラベルが未確定ならスキップ）
                    let lb = match self.rep_label.get(&b).copied().or(self.states[b].label) {
                        Some(x) => x,
                        None => continue,
                    };
                    let mark: Label = (lb + 1) & 3;
                    let acc_a = self
                        .rep_access
                        .get(&a)
                        .cloned()
                        .unwrap_or_else(|| self.states[a].access.clone());
                    let acc_b = self
                        .rep_access
                        .get(&b)
                        .cloned()
                        .unwrap_or_else(|| self.states[b].access.clone());
                    let route = format!("{}[{}]{}{}", acc_a, (b'0' + mark) as char, back, acc_b);
                    self.pending.push(PendingPlan {
                        route,
                        meta: PlanKind::EqCheck { a, b, mark },
                    });
                }
            }
        }

        self.pending.iter().map(|p| p.route.clone()).collect()
    }

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
                    // 観測長: 3K or 3K+1
                    let k = self.f_door_len;
                    let ops_f = 3 * k;
                    let has_initial = match obs.len() {
                        l if l == ops_f => false,
                        l if l == ops_f + 1 => true,
                        l => bail!(
                            "InitF: unexpected obs length {}, expected {} or {}",
                            l,
                            ops_f,
                            ops_f + 1
                        ),
                    };
                    // s0 の自然ラベル（あればセット）
                    if has_initial && self.states[0].label.is_none() {
                        self.states[0].label = Some(obs[0]);
                        self.rep_label.insert(0, obs[0]);
                    }
                    // s0 の代表スロット初期化
                    self.ensure_rep_slot(0);
                }

                PlanKind::TransProbe { from, dir } => {
                    let from = self.uf.find(from);
                    let a_len = self.rep_access.get(&from).map(|s| s.len()).unwrap_or(0);
                    let k = self.f_door_len;
                    let ops_total = a_len + 1 + 3 * k;

                    let has_initial = match obs.len() {
                        l if l == ops_total => false,
                        l if l == ops_total + 1 => true,
                        l => bail!(
                            "TransProbe: unexpected obs length {}, expected {} or {} (from={}, dir={})",
                            l, ops_total, ops_total + 1, from, dir
                        ),
                    };

                    // 到達先の自然ラベル
                    let reached_idx = if has_initial { a_len + 1 } else { a_len };
                    let reached_label = obs[reached_idx];

                    // 新ノード作成（※ 指紋による早期同一視はしない）
                    let mut access = self.rep_access.get(&from).cloned().unwrap_or_default();
                    access.push((b'0' + dir) as char);
                    let to_id = self.add_state(access);

                    // 親情報
                    if self.states[to_id].parent.is_none() {
                        self.states[to_id].parent = Some(from);
                        self.states[to_id].parent_dir = Some(dir);
                    }

                    // ラベル設定
                    if self.states[to_id].label.is_none() {
                        self.states[to_id].label = Some(reached_label);
                    }

                    // from の代表遷移に反映
                    self.states[from].trans[dir as usize] = Some(to_id); // raw
                    self.set_port(from, dir as usize, to_id);

                    // “自分に戻る可能性”がありそう＝ラベルが同じなら Eq 候補（誤判定は EqCheck で回避）
                    if let (Some(la), Some(lb)) = (
                        self.rep_label
                            .get(&from)
                            .copied()
                            .or(self.states[from].label),
                        self.states[to_id].label,
                    ) {
                        if la == lb {
                            self.add_eq_candidate(from, to_id);
                        }
                    }
                }

                PlanKind::EqCheck { a, b, mark } => {
                    // 末尾の観測が mark なら a==b
                    if let Some(&last) = obs.last() {
                        if last == mark {
                            let rep = self.union_merge(a, b);
                            // 統合が波及したので代表の遷移がある程度埋まる
                        }
                    }
                }
            }
        }

        // 子→親の逆ポートが分かったかを確認
        for i in 0..self.states.len() {
            self.maybe_set_back_to_parent(i);
        }

        self.pending.clear();
        Ok(())
    }

    fn build_guess(&self) -> Result<GuessMap> {
        // ※ UF を可変参照しないため find を使わず“代表＝index==parent[index]”判定はできない。
        // ここでは conservative に: 代表集合を集め、必要条件をチェック
        let mut parent = vec![0usize; self.uf.p.len()];
        for i in 0..self.uf.p.len() {
            // 経路圧縮無しの find
            let mut x = i;
            while self.uf.p[x] != x {
                x = self.uf.p[x];
            }
            parent[i] = x;
        }
        let mut reps: Vec<usize> = Vec::new();
        for i in 0..parent.len() {
            if parent[i] == i {
                reps.push(i);
            }
        }
        if reps.len() != self.n {
            bail!("representative count {} != n={}", reps.len(), self.n);
        }

        // 各代表の遷移が埋まっているか
        for &r in &reps {
            let tr = self
                .rep_trans
                .get(&r)
                .ok_or_else(|| anyhow!("rep {} has no trans", r))?;
            for d in 0..6usize {
                if tr[d].is_none() {
                    bail!("rep {} door {} unresolved", r, d);
                }
            }
        }

        // GuessMap 構築
        // 代表 id → 連番 id への割り当て
        let mut idmap: HashMap<usize, usize> = HashMap::new();
        for (idx, &r) in reps.iter().enumerate() {
            idmap.insert(r, idx);
        }

        let mut rooms: Vec<GuessRoom> = Vec::with_capacity(self.n);
        for &r in &reps {
            let label = self
                .rep_label
                .get(&r)
                .copied()
                .or(self.states[r].label)
                .ok_or_else(|| anyhow!("rep {} label unknown", r))?;
            rooms.push(GuessRoom {
                label,
                doors: [None, None, None, None, None, None],
            });
        }

        // 双方向対応の復元： (r,i)->t があるとき、t 側の j を探す
        for &r in &reps {
            let tr = self.rep_trans.get(&r).unwrap();
            for i in 0..6usize {
                let t = tr[i].unwrap();
                let rr = idmap[&r];
                let tt = idmap[&t];
                if rooms[rr].doors[i].is_some() {
                    continue;
                }
                // peer port j
                let tj = self.rep_trans.get(&t).unwrap();
                let mut peer: Option<usize> = None;
                for j in 0..6usize {
                    if tj[j].map(|x| idmap[&x]) == Some(rr) {
                        peer = Some(j);
                        break;
                    }
                }
                let j =
                    peer.ok_or_else(|| anyhow!("peer port not found for rep {} door {}", r, i))?;
                rooms[rr].doors[i] = Some((tt, j as u8));
                rooms[tt].doors[j] = Some((rr, i as u8));
            }
        }

        Ok(GuessMap {
            rooms,
            starting_room: idmap[&self.uf.p[0]],
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

    /// --- モックワールド（テスト用） ---
    /// ラベル: r%4。ポート:
    /// 0<->3 は +1/-1、1<->4 は +2/-2、2<->5 は自己ループ。
    #[derive(Clone)]
    struct MockRoom {
        label: Label,
        doors: [(usize, Dir); 6], // (to, peer)
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

        /// ルート実行。include_initial=true で先頭に開始ラベル。
        /// `[d]` はそのまま d を返し、カレント部屋の表示ラベルを d に上書き（プラン中のみ）。
        fn run_plan(&self, route: &str, include_initial: bool) -> Vec<Label> {
            let mut cur = self.start;
            let mut obs: Vec<Label> = Vec::new();
            let mut override_label: HashMap<usize, Label> = HashMap::new();
            let read_label = |room: usize, ov: &HashMap<usize, Label>, base: Label| -> Label {
                ov.get(&room).copied().unwrap_or(base)
            };
            if include_initial {
                obs.push(read_label(cur, &override_label, self.rooms[cur].label));
            }
            let bytes = route.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                let ch = bytes[i] as char;
                if ch == '[' {
                    let d = bytes[i + 1] - b'0';
                    assert!(d <= 3 && bytes[i + 2] as char == ']');
                    override_label.insert(cur, d);
                    obs.push(d);
                    i += 3;
                } else {
                    let dir = (bytes[i] - b'0') as usize;
                    assert!(dir <= 5);
                    let (to, _peer) = self.rooms[cur].doors[dir];
                    cur = to;
                    let v = read_label(cur, &override_label, self.rooms[cur].label);
                    obs.push(v);
                    i += 1;
                }
            }
            obs
        }

        fn to_guess(&self) -> GuessMap {
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

    /// isomorphic 判定（starting_room を保持）
    fn assert_iso(world: &MockWorld, guess: &GuessMap) {
        let n = world.rooms.len();
        assert_eq!(guess.rooms.len(), n, "room count mismatch");
        // s0 の対応先を world.start に固定
        let gs0 = guess.starting_room;
        let mut map_g2w = vec![None; n];
        let mut map_w2g = vec![None; n];
        map_g2w[gs0] = Some(world.start);
        map_w2g[world.start] = Some(gs0);
        let mut q = VecDeque::new();
        q.push_back(gs0);

        while let Some(g) = q.pop_front() {
            let w = map_g2w[g].unwrap();
            // label
            assert_eq!(
                guess.rooms[g].label, world.rooms[w].label,
                "label mismatch g={} w={}",
                g, w
            );
            // doors
            for d in 0..6usize {
                let (tg, jd_g) = guess.rooms[g].doors[d].expect("missing door in guess");
                let (tw, jd_w) = world.rooms[w].doors[d];
                assert_eq!(jd_g, jd_w, "peer mismatch at g={},w={},d={}", g, w, d);
                match (map_g2w[tg], map_w2g[tw]) {
                    (Some(mw), Some(mg)) => {
                        assert_eq!(mw, tw, "world idx mismatch");
                        assert_eq!(mg, tg, "guess idx mismatch");
                    }
                    (None, None) => {
                        map_g2w[tg] = Some(tw);
                        map_w2g[tw] = Some(tg);
                        q.push_back(tg);
                    }
                    (Some(mw), None) => {
                        assert_eq!(mw, tw, "world idx mismatch(partial)");
                        map_w2g[tw] = Some(tg);
                        q.push_back(tg);
                    }
                    (None, Some(mg)) => {
                        assert_eq!(mg, tg, "guess idx mismatch(partial)");
                        map_g2w[tg] = Some(tw);
                        q.push_back(tg);
                    }
                }
            }
        }
    }

    /// 共通ドライバ
    fn run_until_done(
        mut solver: FingerprintSolver,
        world: &MockWorld,
        include_initial: bool,
    ) -> GuessMap {
        for iter in 0..100 {
            let plans = solver.next_explore_batch();
            assert!(
                !plans.is_empty(),
                "iteration {} produced empty batch unexpectedly",
                iter + 1
            );
            let results: Vec<Vec<Label>> = plans
                .iter()
                .map(|p| world.run_plan(p, include_initial))
                .collect();
            solver
                .apply_explore_results(&plans, &results)
                .expect("apply failed");
            if let Ok(g) = solver.build_guess() {
                return g;
            }
        }
        panic!("did not converge within 100 batches");
    }

    #[test]
    fn next_batch_replays_when_pending() {
        let n = 8;
        let mut solver = FingerprintSolver::new(n);
        let batch1 = solver.next_explore_batch();
        let batch2 = solver.next_explore_batch();
        assert_eq!(batch1, batch2, "pending batch must be replayed unchanged");
    }

    #[test]
    fn solves_with_initial_label_semantics() {
        let n = 12;
        let world = MockWorld::new(n);
        let solver = FingerprintSolver::new(n);
        let guess = run_until_done(solver, &world, true);
        assert_iso(&world, &guess);
    }

    #[test]
    fn solves_without_initial_label_semantics() {
        let n = 12;
        let world = MockWorld::new(n);
        let solver = FingerprintSolver::new(n);
        let guess = run_until_done(solver, &world, false);
        assert_iso(&world, &guess);
    }

    #[test]
    fn interruption_then_resume() {
        let n = 12;
        let world = MockWorld::new(n);
        let mut solver = FingerprintSolver::new(n);

        // 1 回だけ実行
        let plans1 = solver.next_explore_batch();
        let res1: Vec<Vec<Label>> = plans1.iter().map(|p| world.run_plan(p, true)).collect();
        solver
            .apply_explore_results(&plans1, &res1)
            .expect("first apply failed");

        // まだ完了していないはず
        assert!(solver.build_guess().is_err(), "should not be complete yet");

        // 再開して完了まで
        let guess = run_until_done(solver, &world, true);
        assert_iso(&world, &guess);
    }
}
