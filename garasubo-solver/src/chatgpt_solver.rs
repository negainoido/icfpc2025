use crate::solver::{Dir, GuessMap, GuessRoom, Label};
use anyhow::{anyhow, Context, Result};
use std::collections::{HashMap, HashSet};

// 0..=5
pub type StateId = usize;

/// ============ ルート・観測の最小表現（内部用） ===============================

#[derive(Clone, Debug, PartialEq, Eq)]
enum Tok {
    Move(Dir),
    Ink(Label),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Plan(Vec<Tok>);

impl Plan {
    fn from_moves(ms: impl IntoIterator<Item = Dir>) -> Self {
        Self(ms.into_iter().map(Tok::Move).collect())
    }
    fn push_move(&mut self, d: Dir) {
        self.0.push(Tok::Move(d));
    }
    #[allow(dead_code)]
    fn push_ink(&mut self, lab: Label) {
        self.0.push(Tok::Ink(lab));
    }
    fn moves_len(&self) -> usize {
        self.0.iter().filter(|t| matches!(t, Tok::Move(_))).count()
    }
    fn concat(mut self, rhs: &Plan) -> Self {
        self.0.extend(rhs.0.iter().cloned());
        self
    }
}

/// 観測：labels[0] は開始時ラベル
#[derive(Clone, Debug)]
struct Obs {
    labels: Vec<Label>,
}
impl Obs {
    pub fn tail_after_alpha<'a>(&'a self, alpha_moves: usize) -> &'a [Label] {
        // α 読了直後のラベルは labels[alpha_moves]
        let idx = alpha_moves;
        if idx <= self.labels.len() {
            &self.labels[idx..]
        } else {
            &[]
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct FKey(Vec<Label>);

/// ============ 内部で管理するドラフト ========================================

#[derive(Clone, Debug, Default)]
struct Transitions {
    to: [Option<StateId>; 6],
}
#[derive(Clone, Debug)]
struct MapDraft {
    labels: Vec<Label>,
    alpha: Vec<Vec<Dir>>,    // 各状態への代表アクセス α（Move のみ）
    trans: Vec<Transitions>, // δ
}
impl MapDraft {
    fn new(start_label: Label) -> Self {
        Self {
            labels: vec![start_label],
            alpha: vec![vec![]],
            trans: vec![Transitions::default()],
        }
    }
}

/// ============ 設定 ===========================================================

#[derive(Clone, Debug)]
pub struct Config {
    pub f_init_max_len: usize,   // 初期 F 候補の長さ上限（例: 2）
    pub f_refine_max_len: usize, // F 延長語の長さ上限（例: 2）
    pub door_order: [Dir; 6],    // 扉優先順
    pub max_rooms: usize,        // 例: 90
}
impl Default for Config {
    fn default() -> Self {
        Self {
            f_init_max_len: 2,
            f_refine_max_len: 2,
            door_order: [0, 1, 2, 3, 4, 5],
            max_rooms: 90,
        }
    }
}

/// ============ フェーズ管理 ===================================================

#[derive(Clone, Debug)]
enum Phase {
    /// 初期 F 候補の評価（A×C を一括打ち）
    BuildFInit {
        accesses: Vec<Vec<Dir>>,   // 代表 α（例: ε, 0, 1）
        candidates: Vec<Vec<Dir>>, // Σ^{≤k}
        // 直前のバッチ： (access_idx, cand_idx) ごとの計測を期待
        pending: Vec<(usize, usize)>,
    },
    /// F の延長（衝突ペアを割る ext を探索：長さ ext_len）
    BuildFRefine {
        accesses: Vec<Vec<Dir>>,
        current_f: Plan,
        colliding_pairs: Vec<(usize, usize)>, // 代表 α の index ペア
        ext_len: usize,
        // 直前のバッチ： (pair_idx, which_row(0/1), ext_index)
        pending: Vec<(usize, u8, usize)>,
    },
    /// 確定した F を使って start の ID 取得（ε·F を 1 本）
    IdentifyStart {
        f: Plan,
        // pending は 1 本だけ
        pending: bool,
    },
    /// BFS 層ごとに α_s·a·F をまとめ打ち
    Enumerate {
        f: Plan,
        draft: MapDraft,
        key2id: HashMap<FKey, StateId>,
        frontier: Vec<StateId>, // 今層に展開する状態
        // 直前のバッチ： (s,a)
        pending: Vec<(StateId, Dir)>,
    },
    /// すべて埋まった
    Done { draft: MapDraft },
}

/// ============ 制御構造体（公開インターフェース） =============================

pub struct InteractiveSolver {
    cfg: Config,
    phase: Phase,
    // スコア計算用一時領域
    f_init_scores: HashMap<usize, HashSet<FKey>>, // cand_idx -> set of keys
}

impl InteractiveSolver {
    pub fn new(cfg: Config) -> Self {
        // 代表アクセス α は小さめで OK（ε, 0, 1）
        let accesses = vec![vec![], vec![0], vec![1]];
        let candidates = enumerate_words_upto(cfg.f_init_max_len);

        Self {
            cfg,
            phase: Phase::BuildFInit {
                accesses,
                candidates,
                pending: vec![],
            },
            f_init_scores: HashMap::new(),
        }
    }

    /// --- 外部に出す：次の /explore に投げる文字列セット ---
    ///
    /// 返り値が `vec![]` のとき、探索は完了しています（/guess へ）。
    pub fn next_explore_batch(&mut self) -> Vec<String> {
        match &mut self.phase {
            Phase::BuildFInit {
                accesses,
                candidates,
                pending,
            } => {
                // A×C をすべて 1 バッチで
                let mut plans = vec![];
                pending.clear();
                for (ai, a) in accesses.iter().enumerate() {
                    for (ci, c) in candidates.iter().enumerate() {
                        let plan = Plan::from_moves(a.clone()).concat(&Plan::from_moves(c.clone()));
                        plans.push(encode_plan(&plan));
                        pending.push((ai, ci));
                    }
                }
                plans
            }
            Phase::BuildFRefine {
                accesses,
                current_f,
                colliding_pairs,
                ext_len,
                pending,
            } => {
                // いまの ext_len で ext 候補を全列挙
                let exts = enumerate_words_exact(*ext_len);
                let mut plans = vec![];
                pending.clear();
                for (pi, &(i, j)) in colliding_pairs.iter().enumerate() {
                    for (ei, ext) in exts.iter().enumerate() {
                        let f2 = current_f.clone().concat(&Plan::from_moves(ext.clone()));
                        // row i
                        let p_i = Plan::from_moves(accesses[i].clone()).concat(&f2);
                        plans.push(encode_plan(&p_i));
                        pending.push((pi, 0, ei));
                        // row j
                        let p_j = Plan::from_moves(accesses[j].clone()).concat(&f2);
                        plans.push(encode_plan(&p_j));
                        pending.push((pi, 1, ei));
                    }
                }
                plans
            }
            Phase::IdentifyStart { f, pending } => {
                if *pending {
                    // すでに送信済みで結果待ち
                    return vec![];
                }
                *pending = true;
                let plan = encode_plan(&f.clone());
                vec![plan]
            }
            Phase::Enumerate {
                f,
                draft,
                frontier,
                pending,
                ..
            } => {
                // 未探索が残っている限り、空バッチを返さない。
                loop {
                    pending.clear();
                    // 1) frontier が空なら、未探索を持つ全状態で再構築
                    if frontier.is_empty() {
                        let mut rest = vec![];
                        for s in 0..draft.trans.len() {
                            if draft.trans[s].to.iter().any(|x| x.is_none()) {
                                rest.push(s);
                            }
                        }
                        if rest.is_empty() {
                            // もう未探索は無い → 完了
                            self.phase = Phase::Done {
                                draft: draft.clone(),
                            };
                            return vec![];
                        }
                        *frontier = rest;
                    }

                    // 2) frontier からクエリ組み立て
                    let mut plans = vec![];
                    for &s in frontier.iter() {
                        let alpha = &draft.alpha[s];
                        for &a in &self.cfg.door_order {
                            if draft.trans[s].to[a as usize].is_some() {
                                continue;
                            }
                            let mut p = Plan::from_moves(alpha.clone());
                            p.push_move(a);
                            let p = p.concat(&f);
                            plans.push(encode_plan(&p));
                            pending.push((s, a));
                        }
                    }
                    if !plans.is_empty() {
                        // 1 本でも作れたら返す
                        return plans;
                    }

                    // 3) この frontier では打つものが無かった → 破棄して再試行
                    frontier.clear();
                }
            }
            Phase::Done { .. } => vec![],
        }
    }

    /// --- 外部に出す：直前のリクエスト（と同順の結果）で内部状態を更新 ---
    ///
    /// - `sent_routes`: 直前の `next_explore_batch()` が返したルート群（同順）
    /// - `obs_labels`: それぞれに対応するラベル列（labels[0] が開始時ラベル）
    pub fn apply_explore_results(
        &mut self,
        sent_routes: &[String],
        obs_labels: &[Vec<Label>],
    ) -> Result<()> {
        if sent_routes.is_empty() && obs_labels.is_empty() {
            // 何も送っていない/何も返ってない → 何もしない
            return Ok(());
        }
        anyhow::ensure!(
            sent_routes.len() == obs_labels.len(),
            "mismatched lens: sent={}, recv={}",
            sent_routes.len(),
            obs_labels.len()
        );

        match &mut self.phase {
            Phase::BuildFInit {
                accesses,
                candidates,
                pending,
            } => {
                let mut tails_by_cand: HashMap<usize, HashSet<FKey>> = HashMap::new();

                for ((ai, ci), obs) in pending.iter().cloned().zip(obs_labels.iter()) {
                    let alpha_moves = accesses[ai].len();
                    let tail = tail_after_alpha_from_str(&sent_routes, &obs, alpha_moves)
                        .context("tail extraction (FInit)")?;
                    tails_by_cand.entry(ci).or_default().insert(FKey(tail));
                }

                // スコア最大の候補を採用
                let (best_ci, _best_score) = candidates
                    .iter()
                    .enumerate()
                    .map(|(ci, _)| {
                        let score = tails_by_cand.get(&ci).map(|s| s.len()).unwrap_or(0);
                        (ci, score)
                    })
                    .max_by_key(|&(_, sc)| sc)
                    .ok_or_else(|| anyhow!("no candidates"))?;

                let best_f = Plan::from_moves(candidates[best_ci].clone());
                // 衝突（A 内で tail が同一のペア）が残っていないかチェック
                let mut tails_for_best: Vec<FKey> = vec![FKey(vec![]); accesses.len()];
                // もう一度 piecemeal に取りに行かないといけないが、今回のバッチ内に
                // ちょうど best_ci の結果が含まれているので再利用する
                for ((ai, ci), obs) in pending.iter().cloned().zip(obs_labels.iter()) {
                    if ci != best_ci {
                        continue;
                    }
                    let alpha_moves = accesses[ai].len();
                    let tail = tail_after_alpha_from_str(&sent_routes, &obs, alpha_moves)?;
                    tails_for_best[ai] = FKey(tail);
                }
                let mut pairs = vec![];
                for i in 0..tails_for_best.len() {
                    for j in (i + 1)..tails_for_best.len() {
                        if !tails_for_best[i].0.is_empty() && tails_for_best[i] == tails_for_best[j]
                        {
                            pairs.push((i, j));
                        }
                    }
                }

                if pairs.is_empty() {
                    // 十分に分離できた → start 識別へ
                    self.phase = Phase::IdentifyStart {
                        f: best_f,
                        pending: false,
                    };
                } else {
                    // まだ衝突 → ext_len=1 から延長探索へ
                    self.phase = Phase::BuildFRefine {
                        accesses: accesses.clone(),
                        current_f: best_f,
                        colliding_pairs: pairs,
                        ext_len: 1,
                        pending: vec![],
                    };
                }
                Ok(())
            }

            Phase::BuildFRefine {
                accesses,
                current_f,
                colliding_pairs,
                ext_len,
                pending,
            } => {
                let exts = enumerate_words_exact(*ext_len);
                // (pair_idx -> (i,j)) ごとに、ext ごとの 2 行を比較
                // 受信順は pending と同じ。
                // 集計: ext_index -> (i_tail, j_tail) のうち 1 組でも不一致なら採用
                let mut pair_ext_split: HashSet<usize> = HashSet::new(); // ext_idx が割った
                                                                         // tail 保存： (pair_idx, which_row(0/1), ext_idx) -> tail
                let mut tails: HashMap<(usize, u8, usize), FKey> = HashMap::new();

                for (tag, obs) in pending.iter().cloned().zip(obs_labels.iter()) {
                    let (pi, which, ei) = tag;
                    let (i, j) = colliding_pairs[pi];
                    let a_idx = if which == 0 { i } else { j };
                    let alpha_moves = accesses[a_idx].len();
                    let tail = tail_after_alpha_from_str(&sent_routes, &obs, alpha_moves)?;
                    tails.insert((pi, which, ei), FKey(tail));
                }

                // どれか ext が割れば OK
                let mut chosen_ext: Option<Vec<Dir>> = None;
                'outer: for (ei, ext) in exts.iter().enumerate() {
                    for (pi, _) in colliding_pairs.iter().enumerate() {
                        let ti = tails.get(&(pi, 0, ei));
                        let tj = tails.get(&(pi, 1, ei));
                        if let (Some(ti), Some(tj)) = (ti, tj) {
                            if ti != tj {
                                pair_ext_split.insert(ei);
                                chosen_ext = Some(ext.clone());
                                break 'outer;
                            }
                        }
                    }
                }

                if let Some(ext) = chosen_ext {
                    // F を延長したら、そのまま IdentifyStart へ直行
                    *current_f = current_f.clone().concat(&Plan::from_moves(ext));
                    self.phase = Phase::IdentifyStart {
                        f: current_f.clone(),
                        pending: false,
                    };
                    Ok(())
                } else {
                    // この長さでは割れなかった → ext_len を伸ばす
                    if *ext_len >= self.cfg.f_refine_max_len {
                        // これ以上は延ばさない方針：現 F で続行
                        self.phase = Phase::IdentifyStart {
                            f: current_f.clone(),
                            pending: false,
                        };
                        Ok(())
                    } else {
                        *ext_len += 1;
                        // 次の next_explore_batch() で ext_len を伸ばして再投下
                        Ok(())
                    }
                }
            }

            Phase::IdentifyStart { f, pending } => {
                // ε·F の結果 1 本だけ
                anyhow::ensure!(sent_routes.len() == 1, "IdentifyStart expects single route");
                let labels = &obs_labels[0];
                anyhow::ensure!(!labels.is_empty(), "empty labels");
                let tail = tail_after_alpha_from_str_single(&sent_routes[0], labels, 0)?;
                let start_label = tail[0];
                let mut draft = MapDraft::new(start_label);
                let mut key2id: HashMap<FKey, StateId> = HashMap::new();
                key2id.insert(FKey(tail), 0);

                *pending = false;

                self.phase = Phase::Enumerate {
                    f: f.clone(),
                    draft,
                    key2id,
                    frontier: vec![0],
                    pending: vec![],
                };
                Ok(())
            }

            Phase::Enumerate {
                f,
                draft,
                key2id,
                frontier,
                pending,
            } => {
                // pending と obs_labels は 1 対 1
                anyhow::ensure!(
                    pending.len() == obs_labels.len(),
                    "enumerate: pending={}, recv={}",
                    pending.len(),
                    obs_labels.len()
                );

                // 今層で新規に見つかった状態を next_frontier に積む
                let mut next_frontier = vec![];

                for (((s, a), labels), route) in pending
                    .iter()
                    .cloned()
                    .zip(obs_labels.iter())
                    .zip(sent_routes.iter())
                {
                    // α_s の長さ + 1（Move a）だけ進んだ時点の tail
                    // α は「α_s の Move 数 + Move(a) の 1」のみ
                    let alpha_len = draft.alpha[s].len() + 1;
                    let tail = tail_after_alpha_from_str_single(route, &labels, alpha_len)
                        .with_context(|| format!("tail extraction at s={}, a={}", s, a))?;

                    let key = FKey(tail.clone());
                    let lab = *tail.first().ok_or_else(|| anyhow!("empty tail"))?;
                    // 未知の状態なら登録
                    let t = if let Some(&sid) = key2id.get(&key) {
                        sid
                    } else {
                        let sid = draft.labels.len();
                        anyhow::ensure!(
                            sid <= self.cfg.max_rooms,
                            "room overflow (>{})",
                            self.cfg.max_rooms
                        );
                        key2id.insert(key, sid);
                        draft.labels.push(lab);
                        let mut alpha_t = draft.alpha[s].clone();
                        alpha_t.push(a);
                        draft.alpha.push(alpha_t);
                        draft.trans.push(Transitions::default());
                        next_frontier.push(sid);
                        sid
                    };
                    draft.trans[s].to[a as usize] = Some(t);
                }

                // frontier を更新：基本は「今回新規に見つかった状態」
                // ただし頑健性のため、まだ穴が残っている既知状態も追加しておく。
                let mut still_open = vec![];
                for s in 0..draft.trans.len() {
                    if draft.trans[s].to.iter().any(|x| x.is_none()) {
                        still_open.push(s);
                    }
                }
                // next_frontier を優先的に前段に置き、重複は避ける
                let mut merged: Vec<StateId> = vec![];
                let mut seen = std::collections::HashSet::new();
                for s in next_frontier.into_iter().chain(still_open.into_iter()) {
                    if seen.insert(s) {
                        merged.push(s);
                    }
                }
                *frontier = merged;
                pending.clear();
                Ok(())
            }

            Phase::Done { .. } => Ok(()),
        }
    }

    /// --- 外部に出す：推定完了後の map を生成（未完なら Err） ---
    pub fn build_guess(&self) -> Result<GuessMap> {
        let draft = match &self.phase {
            Phase::Done { draft } => draft.clone(),
            Phase::Enumerate { draft, .. } => {
                // すべて埋まっていれば Done でなくても出してよい
                let all_filled = draft
                    .trans
                    .iter()
                    .all(|tr| tr.to.iter().all(|x| x.is_some()));
                if !all_filled {
                    return Err(anyhow!("map is not complete yet"));
                }
                draft.clone()
            }
            _ => return Err(anyhow!("exploration not finished")),
        };

        // ポート対を復元
        let pairs = recover_port_pairs(&draft);

        // 提出形（例）に整形
        let mut rooms: Vec<GuessRoom> = draft
            .labels
            .iter()
            .map(|&lab| GuessRoom {
                label: lab,
                doors: [None, None, None, None, None, None],
            })
            .collect();

        for (s, a, t, b) in pairs {
            rooms[s].doors[a as usize] = Some((t, b));
            rooms[t].doors[b as usize] = Some((s, a));
        }

        Ok(GuessMap {
            rooms,
            starting_room: 0,
        })
    }
}

/// ============ ユーティリティ ==================================================

fn enumerate_words_upto(k: usize) -> Vec<Vec<Dir>> {
    let mut out = vec![];
    for len in 1..=k {
        out.extend(enumerate_words_exact(len));
    }
    out
}
fn enumerate_words_exact(k: usize) -> Vec<Vec<Dir>> {
    if k == 0 {
        return vec![vec![]];
    }
    let mut res = vec![vec![]];
    for _ in 0..k {
        let mut next = vec![];
        for w in &res {
            for d in 0u8..6 {
                let mut w2 = w.clone();
                w2.push(d);
                next.push(w2);
            }
        }
        res = next;
    }
    res
}

/// Plan → 文字列（例: "012[1]30"）
fn encode_plan(p: &Plan) -> String {
    let mut s = String::new();
    for t in &p.0 {
        match *t {
            Tok::Move(d) => s.push(char::from(b'0' + d)),
            Tok::Ink(l) => {
                s.push('[');
                s.push(char::from(b'0' + l));
                s.push(']');
            }
        }
    }
    s
}

/// 直近の `next_explore_batch()` が返した順序と同じと仮定し、
/// α の Move 数 `alpha_moves` の直後からの tail を抽出して FKey として返す。
///
/// 注意：実運用ではサーバの仕様に合わせて、`labels` の意味づけ（長さ等）を確認してください。
fn tail_after_alpha_from_str(
    _sent_routes: &[String],
    labels: &[Label],
    alpha_moves: usize,
) -> Result<Vec<Label>> {
    let idx = alpha_moves;
    if idx > labels.len() {
        return Err(anyhow!("labels too short: need >= {}", idx));
    }
    Ok(labels[idx..].to_vec())
}

/// 単一ルート用（引数に route 文字列も受け取れる形を用意）
fn tail_after_alpha_from_str_single(
    _route: &str,
    labels: &[Label],
    alpha_moves: usize,
) -> Result<Vec<Label>> {
    tail_after_alpha_from_str(&[], labels, alpha_moves)
}

/// δ から相手ポートを復元して（s,a）<->（t,b）のペア列を返す
fn recover_port_pairs(draft: &MapDraft) -> Vec<(StateId, Dir, StateId, Dir)> {
    let n = draft.trans.len();
    let mut used = vec![[false; 6]; n];
    let mut pairs = vec![];

    for s in 0..n {
        for a in 0..6usize {
            if used[s][a] {
                continue;
            }
            if let Some(t) = draft.trans[s].to[a] {
                // 逆像 δ(t,b)=s となる b を探す
                let mut found = None;
                for b in 0..6usize {
                    if let Some(tt) = draft.trans[t].to[b] {
                        if tt == s && !used[t][b] {
                            found = Some(b as Dir);
                            break;
                        }
                    }
                }
                if let Some(b) = found {
                    used[s][a] = true;
                    used[t][b as usize] = true;
                    pairs.push((s, a as Dir, t, b));
                } else {
                    // 理論上ここには来ない（問題仕様では無向ポートで対称）。
                    // 復元できない場合はダミーで埋めない（Guess 側で None のままになる）。
                }
            }
        }
    }
    pairs
}
