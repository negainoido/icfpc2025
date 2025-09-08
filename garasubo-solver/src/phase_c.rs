// phase_c.rs
//
// Phase C: 合法マージ（Union-Find + ロールバック + 伝播クロージャ）
//
// 入力：
//   - W: 扉列（0..=5, 長さ L）
//   - Y: ラベル列（0..=3, 長さ L+1）
//   - CandidateList: Phase B の出力（スコア降順）
//   - target_n: 既知の部屋数 n
//
// 出力：
//   - MergeResult：time->cluster の写像、cluster ごとの label と δ(door) の一部（観測から導出）など
//
// 使い方は本ファイル末尾のサンプル参照。

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use crate::candidate_gen::{Candidate, CandidateList};

/// マージ後の最終出力
#[derive(Debug, Clone)]
pub struct MergeResult {
    /// 各時刻 t (0..=L) -> 圧縮後 cluster_id (0..k-1)
    pub time_to_cluster: Vec<usize>,
    /// cluster_id -> ラベル (0..3)
    pub cluster_labels: Vec<u8>,
    /// cluster_id -> [Option<cluster_id>; 6] （観測から得られた遷移のみ）
    pub delta_by_cluster: Vec<[Option<usize>; 6]>,
    /// 代表インデックス（各 cluster_id の代表となる時刻 t）
    pub cluster_representatives: Vec<usize>,
    /// いくつに縮約できたか（cluster 数 = cluster_labels.len()）
    pub cluster_count: usize,
    /// 統計情報
    pub stats: MergeStats,
}

#[derive(Debug, Clone, Default)]
pub struct MergeStats {
    pub attempted_candidates: usize,
    pub accepted_merges: usize,   // 候補受け入れ回数（1 回で複数 Union が起きることがある）
    pub rejected_merges: usize,
    pub final_components: usize,
}

/// 内部状態（Union-Find + δ + ラベル）
struct MergeState {
    n_nodes: usize,                // = L+1
    parent: Vec<usize>,
    size: Vec<usize>,
    label: Vec<u8>,                // 各代表ノードのラベル（非代表インデックスにも持たせるが参照は rep() 越し）
    delta: Vec<[Option<usize>; 6]>,// δ[node][door] = Some(next_node) or None
    components: usize,             // 現在の連結成分（クラスター）数
    changes: Vec<Change>,          // ロールバックログ
}

#[derive(Debug, Clone, Copy)]
enum Change {
    Parent { v: usize, prev: usize },
    Size   { v: usize, prev: usize },
    Delta  { node: usize, door: u8, prev: Option<usize> },
    Comps  { prev: usize },
}

impl MergeState {
    fn new_from_walk(w: &[u8], y: &[u8]) -> Self {
        assert_eq!(y.len(), w.len() + 1);
        let l = w.len();
        let n = l + 1;
        let mut parent = vec![0usize; n];
        let mut size   = vec![1usize; n];
        for i in 0..n { parent[i] = i; }

        // ラベルと δ を初期化
        let mut delta = vec![[None; 6]; n];
        for t in 0..l {
            let d = w[t] as usize;
            delta[t][d] = Some(t + 1);
        }

        Self {
            n_nodes: n,
            parent,
            size,
            label: y.to_vec(),
            delta,
            components: n,
            changes: Vec::new(),
        }
    }

    #[inline]
    fn rep(&self, mut x: usize) -> usize {
        // ロールバック可能 DSU なのでパス圧縮なし
        while self.parent[x] != x {
            x = self.parent[x];
        }
        x
    }

    fn snapshot(&self) -> usize { self.changes.len() }

    fn rollback(&mut self, mark: usize) {
        while self.changes.len() > mark {
            match self.changes.pop().unwrap() {
                Change::Parent { v, prev } => self.parent[v] = prev,
                Change::Size   { v, prev } => self.size[v] = prev,
                Change::Delta  { node, door, prev } => self.delta[node][door as usize] = prev,
                Change::Comps  { prev } => self.components = prev,
            }
        }
    }

    fn set_delta(&mut self, node: usize, door: u8, val: Option<usize>) {
        let d = door as usize;
        let prev = self.delta[node][d];
        if prev == val { return; }
        self.changes.push(Change::Delta { node, door, prev });
        self.delta[node][d] = val;
    }

    /// union-by-size（x, y は代表想定でなくてもよい）
    /// 成功時は (new_root, absorbed_root) を返す。
    fn unite(&mut self, x: usize, y: usize) -> Option<(usize, usize)> {
        let mut rx = self.rep(x);
        let mut ry = self.rep(y);
        if rx == ry { return None; }

        if self.size[rx] < self.size[ry] {
            std::mem::swap(&mut rx, &mut ry);
        }
        // components
        let prev_c = self.components;
        self.changes.push(Change::Comps { prev: prev_c });
        self.components = self.components.saturating_sub(1);

        // parent
        self.changes.push(Change::Parent { v: ry, prev: self.parent[ry] });
        self.parent[ry] = rx;

        // size
        let prev_size = self.size[rx];
        self.changes.push(Change::Size { v: rx, prev: prev_size });
        self.size[rx] = prev_size + self.size[ry];

        // ラベルは一致前提、保持側（rx）の値をそのまま使う
        Some((rx, ry))
    }

    /// 伝播クロージャ付きの原子的マージ
    /// - a,b を同一クラスターに“できる”なら true（確定）
    /// - ラベル衝突等の矛盾があれば false（完全ロールバック）
    fn closure_merge(&mut self, a: usize, b: usize) -> bool {
        let mark = self.snapshot();
        let mut stack: Vec<(usize, usize)> = vec![(self.rep(a), self.rep(b))];

        while let Some((x0, y0)) = stack.pop() {
            let mut x = self.rep(x0);
            let mut y = self.rep(y0);
            if x == y { continue; }

            // ラベル一致の確認
            if self.label[x] != self.label[y] {
                self.rollback(mark);
                return false;
            }

            // 事前に δ をコピー（union で代表が変わっても参照できるように）
            let dx = self.delta[x];
            let dy = self.delta[y];

            // 結合
            let (root, absorbed) = match self.unite(x, y) {
                Some(rs) => rs,
                None => continue,
            };

            // δ の統合と連鎖約束
            for door in 0..6u8 {
                let px = dx[door as usize].map(|p| self.rep(p));
                let py = dy[door as usize].map(|p| self.rep(p));

                match (px, py) {
                    (Some(u), Some(v)) => {
                        if u != v {
                            // 「同じ部屋の同じ扉の先」は同一であるべき
                            stack.push((u, v));
                        }
                        // root の δ[door] は何かしら入れておく（未設定なら u）
                        if self.delta[root][door as usize].is_none() {
                            self.set_delta(root, door, Some(u));
                        } else {
                            // 既存と矛盾するならそれもマージ要求として積む
                            let cur = self.delta[root][door as usize].unwrap();
                            let cur_rep = self.rep(cur);
                            if cur_rep != u {
                                stack.push((cur_rep, u));
                            }
                        }
                    }
                    (Some(u), None) | (None, Some(u)) => {
                        let urep = self.rep(u);
                        if let Some(cur) = self.delta[root][door as usize] {
                            let cur_rep = self.rep(cur);
                            if cur_rep != urep {
                                // 既存と異なる先が来た → どちらかに統一される必要がある
                                stack.push((cur_rep, urep));
                            }
                        } else {
                            self.set_delta(root, door, Some(urep));
                        }
                    }
                    (None, None) => { /* 何も観測されていない */ }
                }
            }
        }
        // すべて矛盾なく閉じた
        true
    }

    /// 現在の代表数（O(n)）
    fn count_components(&self) -> usize {
        let mut seen = HashSet::new();
        for i in 0..self.n_nodes {
            seen.insert(self.rep(i));
        }
        seen.len()
    }

    /// 結果を圧縮して出力：time->cluster, cluster->label, cluster->δ
    fn export(&self, w: &[u8], y: &[u8]) -> MergeResult {
        let n = self.n_nodes;
        assert_eq!(y.len(), n);

        // 代表 -> cluster_id
        let mut rep_to_id: HashMap<usize, usize> = HashMap::new();
        let mut time_to_cluster = vec![0usize; n];
        let mut reps: Vec<usize> = Vec::new();

        for t in 0..n {
            let r = self.rep(t);
            let rep_to_id_len = rep_to_id.len();
            let id = *rep_to_id.entry(r).or_insert_with(|| {
                let new_id = rep_to_id_len;
                reps.push(r);
                new_id
            });
            time_to_cluster[t] = id;
        }
        let k = rep_to_id.len();

        // cluster_labels
        let mut cluster_labels = vec![0u8; k];
        for (rid, &r) in reps.iter().enumerate() {
            cluster_labels[rid] = self.label[r];
        }

        // δ は「元のウォーク」をスキャンして最終代表で集計（安全）
        let mut delta_by_cluster = vec![[None; 6]; k];
        for t in 0..(n - 1) {
            let c = time_to_cluster[t];
            let d = w[t] as usize;
            let nxt = time_to_cluster[t + 1];
            if let Some(cur) = delta_by_cluster[c][d] {
                // 矛盾していないか点検（closure が効いていれば一致する）
                debug_assert_eq!(cur, nxt, "delta mismatch on cluster={}, door={}", c, d);
            } else {
                delta_by_cluster[c][d] = Some(nxt);
            }
        }

        MergeResult {
            time_to_cluster,
            cluster_labels,
            delta_by_cluster,
            cluster_representatives: reps,
            cluster_count: k,
            stats: MergeStats::default(), // 後で上書き
        }
    }
}

/// API：候補に基づく貪欲マージを実行
pub fn run_phase_c(
    w: &[u8],
    y: &[u8],
    cand: &CandidateList,
    target_n: usize,
) -> MergeResult {
    let mut st = MergeState::new_from_walk(w, y);
    let mut attempted = 0usize;
    let mut accepted = 0usize;
    let mut rejected = 0usize;

    // スコア降順（CandidateList は既に降順だが念のため安定化）
    let mut list = cand.list.clone();
    list.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.a.cmp(&b.a))
            .then_with(|| a.b.cmp(&b.b))
    });

    let mut comps = st.count_components();
    for c in list {
        if comps <= target_n { break; }
        attempted += 1;

        let a = c.a as usize;
        let b = c.b as usize;

        // 既に同じ代表ならスキップ
        if st.rep(a) == st.rep(b) { continue; }

        // 原子的マージを試す
        if st.closure_merge(a, b) {
            accepted += 1;
            comps = st.count_components();
        } else {
            rejected += 1;
        }
    }

    // 出力
    let mut out = st.export(w, y);
    out.stats = MergeStats {
        attempted_candidates: attempted,
        accepted_merges: accepted,
        rejected_merges: rejected,
        final_components: comps,
    };
    out
}

// ======================== 追加: 複数ラン対応の内部関数群 ========================

impl MergeState {
    /// フラット化された複数ランから初期状態を構築
    /// - w_flat: Σ L_i
    /// - y_flat: Σ (L_i+1)
    /// - breaks: 各ランの y 開始インデックス（例: [0, y1.len(), y1.len()+y2.len(), ...] ではなく [0, off2, off3, ...]）
    ///   ※ 最終ランの「終端」は y_flat.len() で自明なので breaks には含めない前提
    fn new_from_flat_runs(w_flat: &[u8], y_flat: &[u8], breaks: &[usize]) -> Self {
        // 入力検証（軽め）
        assert!(!y_flat.is_empty(), "y_flat must be non-empty");
        // 各ランの y 長の総和 - ラン数  == w_flat.len()
        // ここでは breaks[i] はラン i の開始 y オフセット（i=0 は 0 のはず）
        assert!(breaks.first().copied().unwrap_or(0) == 0, "breaks[0] must be 0");
        let mut sum_l = 0usize;
        let mut prev = 0usize;
        for &b in breaks.iter().skip(1) {
            assert!(b > prev && b <= y_flat.len(), "invalid breaks");
            sum_l += (b - prev) - 1;
            prev = b;
        }
        // 最終ラン
        sum_l += (y_flat.len() - prev) - 1;
        assert_eq!(sum_l, w_flat.len(), "Σ(L_i) must equal w_flat.len()");

        // ベース配列
        let n = y_flat.len();
        let mut parent = vec![0usize; n];
        let mut size   = vec![1usize; n];
        for i in 0..n { parent[i] = i; }

        let mut delta = vec![[None; 6]; n];

        // 各ランに対して δ を張る（境界は跨がない）
        let mut w_cursor = 0usize;
        for (i, &start) in breaks.iter().enumerate() {
            let end = if i + 1 < breaks.len() { breaks[i + 1] } else { y_flat.len() };
            assert!(end > start, "empty run detected");
            let li = end - start - 1; // このランのステップ数
            for k in 0..li {
                let t  = start + k;
                let d  = w_flat[w_cursor + k] as usize;
                assert!(d < 6, "door must be 0..=5");
                delta[t][d] = Some(t + 1);
            }
            w_cursor += li;
        }
        debug_assert_eq!(w_cursor, w_flat.len());

        Self {
            n_nodes: n,
            parent,
            size,
            label: y_flat.to_vec(),
            delta,
            components: n,
            changes: Vec::new(),
        }
    }

    /// W を使わず、代表ノードに蓄積された δ からクラスタ δ を構成する
    fn export_multi(&self) -> MergeResult {
        let n = self.n_nodes;

        // 代表 -> cluster_id
        use std::collections::HashMap;
        let mut rep_to_id: HashMap<usize, usize> = HashMap::new();
        let mut time_to_cluster = vec![0usize; n];
        let mut reps: Vec<usize> = Vec::new();

        for t in 0..n {
            let r = self.rep(t);
            let rep_to_id_len = rep_to_id.len();
            let id = *rep_to_id.entry(r).or_insert_with(|| {
                let nid = rep_to_id_len;
                reps.push(r);
                nid
            });
            time_to_cluster[t] = id;
        }
        let k = reps.len();

        // ラベル
        let mut cluster_labels = vec![0u8; k];
        for (rid, &r) in reps.iter().enumerate() {
            cluster_labels[rid] = self.label[r];
        }

        // クラスタ δ：代表の δ から構築
        let mut delta_by_cluster = vec![[None; 6]; k];
        for (rid, &r) in reps.iter().enumerate() {
            for door in 0..6usize {
                if let Some(next) = self.delta[r][door] {
                    let cnext = time_to_cluster[self.rep(next)];
                    if let Some(cur) = delta_by_cluster[rid][door] {
                        debug_assert_eq!(cur, cnext, "delta mismatch on cluster={}, door={}", rid, door);
                    } else {
                        delta_by_cluster[rid][door] = Some(cnext);
                    }
                }
            }
        }

        MergeResult {
            time_to_cluster,
            cluster_labels,
            delta_by_cluster,
            cluster_representatives: reps,
            cluster_count: k,
            stats: MergeStats::default(),
        }
    }
}

/// 複数ラン（フラット）の内部版：
/// - `w_flat`: Σ L_i
/// - `y_flat`: Σ (L_i + 1)
/// - `breaks`: 各ランの y 開始インデックス（先頭は必ず 0）
/// - `cand`: フラット時刻空間での候補（Phase B マルチラン版で作成）
/// 既存の run_phase_c と同じロジックで貪欲マージし、export は export_multi を使う。
pub fn run_phase_c_internal_from_flat(
    w_flat: &[u8],
    y_flat: &[u8],
    breaks: &[usize],
    cand: &crate::candidate_gen::CandidateList,
    target_n: usize,
) -> MergeResult {
    use std::cmp::Ordering;

    let mut st = MergeState::new_from_flat_runs(w_flat, y_flat, breaks);

    let mut attempted = 0usize;
    let mut accepted  = 0usize;
    let mut rejected  = 0usize;

    // スコア降順で安定ソート（CandidateList は既に降順が多いが念のため）
    let mut list = cand.list.clone();
    list.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.a.cmp(&b.a))
            .then_with(|| a.b.cmp(&b.b))
    });

    let mut comps = st.count_components();
    for c in list {
        if comps <= target_n { break; }
        attempted += 1;
        let a = c.a as usize;
        let b = c.b as usize;
        if st.rep(a) == st.rep(b) { continue; }
        if st.closure_merge(a, b) {
            accepted += 1;
            comps = st.count_components();
        } else {
            rejected += 1;
        }
    }

    // 出力（W ではなく state の δ を用いた multi 版）
    let mut out = st.export_multi();
    out.stats = MergeStats {
        attempted_candidates: attempted,
        accepted_merges: accepted,
        rejected_merges: rejected,
        final_components: comps,
    };
    out
}

#[test]
fn multi_runs_no_cross_boundary_edges() {
    use crate::candidate_gen::{CandidateList, Candidate, Hits};

    // ラン1: y=[0,0,0], w=[0,0]
    // ラン2: y=[1,1,1], w=[1,1]
    let y1 = vec![0u8,0,0];
    let w1 = vec![0u8,0];
    let y2 = vec![1u8,1,1];
    let w2 = vec![1u8,1];

    // フラット
    let w_flat = [w1.as_slice(), w2.as_slice()].concat();
    let y_flat = [y1.as_slice(), y2.as_slice()].concat();
    let breaks = vec![0usize, y1.len()]; // ラン開始の y オフセット

    // 候補なし（マージしない）
    let cand = CandidateList {
        list: Vec::<Candidate>::new(),
        stats: crate::candidate_gen::CandStats::default(),
    };

    let m = run_phase_c_internal_from_flat(&w_flat, &y_flat, &breaks, &cand, y_flat.len());
    // ラン1の最終時刻 = 2（y1.len()-1）
    let c_last_run1 = m.time_to_cluster[2];
    // そのクラスタからは何の δ も張られていないはず（境界跨ぎ無し）
    assert!(m.delta_by_cluster[c_last_run1].iter().all(|x| x.is_none()));
}
