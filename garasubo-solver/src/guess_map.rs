use std::collections::{HashMap, HashSet};

/// /guess 用の Map 構造
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GuessMap {
    pub rooms: Vec<u8>,               // 各部屋の 2bit ラベル (0..3)
    pub starting_room: usize,         // 開始部屋 index
    pub connections: Vec<Connection>, // 無向辺（片側のみ列挙）
}

/// 接続 1 本
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Connection {
    pub from: Endpoint,
    pub to: Endpoint,
}

/// 端点
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Endpoint {
    pub room: usize,
    pub door: u8, // 0..5
}

/// エラー
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("plans.len()={} != results.len()={}", plans, results)]
    PlansResultsLenMismatch { plans: usize, results: usize },
    #[error("mismatch")]
    ResultLenMismatch {
        idx: usize,
        plan_len: usize,
        obs_len: usize,
    },
    #[error("bad digit plan")]
    BadDigitInPlan { idx: usize, ch: char },
    #[error("bad label value")]
    BadLabelValue {
        plan_idx: usize,
        pos: usize,
        value: i32,
    },
    #[error("bad room")]
    SuffixNotMatched { idx: usize, plan: String },
    #[error("bad door")]
    MissingStartingPrefix, // s="" の sR が見つからない
    #[error("bad door")]
    IncompleteCoverage {
        room_count: usize,
        missing_examples: Vec<String>,
    },
    #[error("bad door")]
    IncompleteDelta { room: usize, missing_doors: Vec<u8> },
    #[error("bad door")]
    PairingMismatch {
        room: usize,
        door: u8,
        to_room: usize,
    },
    #[error("bad door")]
    Internal(String),
}

/// 主要エントリ：固定サフィックス（1〜2 本）で探索した plans/results から map を構築
///
/// - `plans[i]` は "0".."5" の文字列
/// - `results[i]` は int ラベル列（長さは `plans[i].len() + 1`）
/// - `suffixes` は探索に使った R たち（通常は 1 本、`two_suffixes=true` の場合は 2 本）
///
/// ※ すべてのプレフィックス s について s∘R と s∘a∘R (a=0..5) が存在し、
///   全部屋を少なくとも 1 つの s がカバーしている前提で完全な map が組み上がります。
pub fn build_map_fixed_tail(
    plans: &[String],
    results: &[Vec<u8>],
    suffixes: &[String],
) -> Result<GuessMap, BuildError> {
    if plans.len() != results.len() {
        return Err(BuildError::PlansResultsLenMismatch {
            plans: plans.len(),
            results: results.len(),
        });
    }
    if suffixes.is_empty() {
        return Err(BuildError::Internal("suffixes must not be empty".into()));
    }

    // 入力検証 + int→u8 変換（0..3 のみ許可） + 全 plan 末尾サフィックスの割当
    let mut obs_u8: Vec<Vec<u8>> = Vec::with_capacity(results.len());
    let mut suffix_id_of_plan: Vec<Option<usize>> = vec![None; plans.len()];

    for (i, plan) in plans.iter().enumerate() {
        // 各文字が '0'..'5' であることを確認
        for ch in plan.chars() {
            if !('0'..='5').contains(&ch) {
                return Err(BuildError::BadDigitInPlan { idx: i, ch });
            }
        }
        // results 長さ = plan 長さ + 1
        let obs = &results[i];
        if obs.len() != plan.len() + 1 {
            return Err(BuildError::ResultLenMismatch {
                idx: i,
                plan_len: plan.len(),
                obs_len: obs.len(),
            });
        }
        // 0..3 であること
        let mut row = Vec::<u8>::with_capacity(obs.len());
        for (j, &v) in obs.iter().enumerate() {
            if !(0..=3).contains(&v) {
                return Err(BuildError::BadLabelValue {
                    plan_idx: i,
                    pos: j,
                    value: v as i32,
                });
            }
            row.push(v as u8);
        }
        obs_u8.push(row);

        // どのサフィックスで終わるかを記録
        let mut matched = None;
        for (sid, sfx) in suffixes.iter().enumerate() {
            if plan.ends_with(sfx) {
                matched = Some(sid);
                break;
            }
        }
        if matched.is_none() {
            return Err(BuildError::SuffixNotMatched {
                idx: i,
                plan: plan.clone(),
            });
        }
        suffix_id_of_plan[i] = matched;
    }

    // 以降、サフィックスごとに：
    //   - base plan: s∘R が存在する s
    //   - step plan: s∘a∘R が存在する (s,a)
    // を収集し、指紋（R に沿って観測されるラベル列 |R|+1）→ provisional node を作成
    // さらに同一 prefix を共有する（サフィックスだけが異なる）node 同士を union で同一部屋に統合する。

    // plan 文字列 → index の逆引き（サフィックス別）
    let mut index_of_plan_per_suffix: Vec<HashMap<&str, usize>> =
        Vec::with_capacity(suffixes.len());
    for sid in 0..suffixes.len() {
        index_of_plan_per_suffix.push(std::collections::HashMap::new());
    }
    for (i, plan) in plans.iter().enumerate() {
        let sid = suffix_id_of_plan[i].unwrap();
        index_of_plan_per_suffix[sid].insert(plan.as_str(), i);
    }

    // サフィックス別に、base 候補 s と child を持つ s を洗い出す
    let mut has_base: Vec<HashSet<String>> = vec![HashSet::new(); suffixes.len()];
    let mut has_child: Vec<HashSet<String>> = vec![HashSet::new(); suffixes.len()];

    for sid in 0..suffixes.len() {
        let sfx = &suffixes[sid];
        let sfx_len = sfx.len();
        for (i, plan) in plans.iter().enumerate() {
            if suffix_id_of_plan[i].unwrap() != sid {
                continue;
            }
            let rem = &plan[..plan.len() - sfx_len];
            // rem 自体が base 候補
            has_base[sid].insert(rem.to_string());
            // 親 rem[:-1] は child を持つ s 候補
            if rem.len() >= 1 {
                let parent = &rem[..rem.len() - 1];
                has_child[sid].insert(parent.to_string());
            }
        }
    }

    // s の集合（base かつ child を持つ）
    let mut base_s_per_suffix: Vec<Vec<String>> = Vec::with_capacity(suffixes.len());
    for sid in 0..suffixes.len() {
        let mut v: Vec<String> = has_base[sid]
            .intersection(&has_child[sid])
            .cloned()
            .collect();
        v.sort();
        base_s_per_suffix.push(v);
    }

    // 指紋（suffix_id, labels[..|R|+1]）→ provisional node id
    #[derive(Clone, PartialEq, Eq, Hash)]
    struct FingerKey {
        sid: usize,
        bytes: Vec<u8>,
    }
    let mut finger_to_node: HashMap<FingerKey, usize> = HashMap::new();
    let mut node_labels: Vec<u8> = Vec::new(); // node の先頭ラベル（部屋ラベル）

    // s→base node, (s,a)→step node（サフィックス別）を保持
    let mut base_node: Vec<HashMap<String, usize>> = vec![HashMap::new(); suffixes.len()];
    let mut step_node: Vec<HashMap<(String, u8), usize>> = vec![HashMap::new(); suffixes.len()];

    // 指紋から node を割り当てるヘルパ
    let mut get_or_create_node = |sid: usize, fp: Vec<u8>| -> usize {
        let key = FingerKey { sid, bytes: fp };
        if let Some(&id) = finger_to_node.get(&key) {
            return id;
        }
        let id = node_labels.len();
        let room_label = key.bytes[0]; // 先頭が部屋ラベル
        node_labels.push(room_label);
        finger_to_node.insert(key, id);
        id
    };

    // base/step を収集しながら provisional node を作成
    for sid in 0..suffixes.len() {
        let sfx = &suffixes[sid];
        let sfx_len = sfx.len();

        // sR と s a R のインデックスを取り、指紋を抽出して node にする
        for s in &base_s_per_suffix[sid] {
            // sR
            let plan_str = format!("{}{}", s, sfx);
            let &idx = index_of_plan_per_suffix[sid]
                .get(plan_str.as_str())
                .ok_or_else(|| {
                    BuildError::Internal(format!("inconsistent index map: missing {}", plan_str))
                })?;
            let fp = extract_fingerprint_for_base(&obs_u8[idx], s.len(), sfx_len);
            let node = get_or_create_node(sid, fp);
            base_node[sid].insert(s.clone(), node);

            // s a R
            for a in 0..6 {
                let plan_sa = format!("{}{}{}", s, (b'0' + a as u8) as char, sfx);
                if let Some(&jdx) = index_of_plan_per_suffix[sid].get(plan_sa.as_str()) {
                    let fp = extract_fingerprint_for_step(&obs_u8[jdx], s.len(), sfx_len);
                    let node2 = get_or_create_node(sid, fp);
                    step_node[sid].insert((s.clone(), a as u8), node2);
                }
            }
        }
    }

    // 同一 prefix でサフィックスだけが違う node を統合（Union-Find）
    let uf_size = node_labels.len();
    let mut uf = UnionFind::new(uf_size);

    // sR の統合
    let mut all_s: HashSet<String> = HashSet::new();
    for sid in 0..suffixes.len() {
        for s in base_node[sid].keys() {
            all_s.insert(s.clone());
        }
    }
    for s in &all_s {
        let mut first: Option<usize> = None;
        for sid in 0..suffixes.len() {
            if let Some(&nid) = base_node[sid].get(s) {
                if let Some(fid) = first {
                    uf.union(fid, nid);
                } else {
                    first = Some(nid);
                }
            }
        }
    }
    // s a R の統合
    for s in &all_s {
        for a in 0..6u8 {
            let mut first: Option<usize> = None;
            for sid in 0..suffixes.len() {
                if let Some(&nid) = step_node[sid].get(&(s.clone(), a)) {
                    if let Some(fid) = first {
                        uf.union(fid, nid);
                    } else {
                        first = Some(nid);
                    }
                }
            }
        }
    }

    // 代表 → 連番 room id へ
    let mut rep_to_room: HashMap<usize, usize> = HashMap::new();
    let mut rooms: Vec<u8> = Vec::new();

    for nid in 0..uf_size {
        let rep = uf.find(nid);
        if !rep_to_room.contains_key(&rep) {
            let rid = rep_to_room.len();
            rep_to_room.insert(rep, rid);
            // ラベル（先頭ラベル）は代表集合のどれでも同一のはず
            rooms.push(node_labels[nid]);
        }
    }

    // starting room は s="" の sR（どのサフィックスでも良い）から
    let mut starting_room_opt: Option<usize> = None;
    for sid in 0..suffixes.len() {
        if let Some(&nid) = base_node[sid].get("") {
            starting_room_opt = Some(*rep_to_room.get(&uf.find(nid)).unwrap());
            break;
        }
    }
    let starting_room = starting_room_opt.ok_or(BuildError::MissingStartingPrefix)?;

    // δ を作る（部屋 × 6 ドア → 次の部屋）
    let room_count = rep_to_room.len();
    let mut delta: Vec<[Option<usize>; 6]> = vec![[None; 6]; room_count];

    // s ごとに i を決め、各 a で j を埋める
    // s はどのサフィックスでも良いので、まず「代表 sid」を決めて走査
    let sid0 = 0usize.min(suffixes.len() - 1);
    let mut example_missing: Vec<String> = Vec::new();

    for s in &all_s {
        // i: base(s)
        let nid_i = match base_node[sid0].get(s).cloned().or_else(|| {
            // sid0 に無ければ他の sid から拾う
            for sid in 0..suffixes.len() {
                if let Some(&nid) = base_node[sid].get(s) {
                    return Some(nid);
                }
            }
            None
        }) {
            Some(nid) => nid,
            None => {
                example_missing.push(s.clone());
                continue;
            }
        };
        let i = *rep_to_room.get(&uf.find(nid_i)).unwrap();

        // 各 a
        for a in 0..6u8 {
            // j をどの sid からでも拾う
            let nid_j_opt = step_node[sid0].get(&(s.clone(), a)).cloned().or_else(|| {
                for sid in 0..suffixes.len() {
                    if let Some(&nid) = step_node[sid].get(&(s.clone(), a)) {
                        return Some(nid);
                    }
                }
                None
            });
            if let Some(nid_j) = nid_j_opt {
                let j = *rep_to_room.get(&uf.find(nid_j)).unwrap();
                if let Some(prev) = delta[i][a as usize] {
                    if prev != j {
                        return Err(BuildError::Internal(format!(
                            "conflicting delta at room {}, door {}: {} vs {}",
                            i, a, prev, j
                        )));
                    }
                } else {
                    delta[i][a as usize] = Some(j);
                }
            }
        }
    }

    // 一部 s が欠けると δ が埋まらない。安全のため検査
    for i in 0..room_count {
        let mut miss: Vec<u8> = Vec::new();
        for a in 0..6u8 {
            if delta[i][a as usize].is_none() {
                miss.push(a);
            }
        }
        if !miss.is_empty() {
            if !example_missing.is_empty() {
                return Err(BuildError::IncompleteCoverage {
                    room_count,
                    missing_examples: example_missing,
                });
            }
            return Err(BuildError::IncompleteDelta {
                room: i,
                missing_doors: miss,
            });
        }
    }

    // 無向 connections を構築（(i,a) と (j,b) の 1:1 ペアリング）
    let mut used = vec![[false; 6]; room_count];
    let mut connections: Vec<Connection> = Vec::with_capacity(3 * room_count);

    for i in 0..room_count {
        for a in 0..6u8 {
            if used[i][a as usize] {
                continue;
            }
            let j = delta[i][a as usize].unwrap();
            // j 側で未使用かつ δ[j][b]==i な b を探す（自己ループ/多重辺も許容）
            let mut found_b: Option<u8> = None;
            for b in 0..6u8 {
                if !used[j][b as usize] && delta[j][b as usize].unwrap() == i {
                    found_b = Some(b);
                    break;
                }
            }
            if let Some(b) = found_b {
                used[i][a as usize] = true;
                used[j][b as usize] = true;
                connections.push(Connection {
                    from: Endpoint { room: i, door: a },
                    to: Endpoint { room: j, door: b },
                });
            } else {
                return Err(BuildError::PairingMismatch {
                    room: i,
                    door: a,
                    to_room: j,
                });
            }
        }
    }
    // 想定: connections 本数は 3n（6n 有向 / 2）
    debug_assert_eq!(connections.len(), 3 * room_count);

    Ok(GuessMap {
        rooms,
        starting_room,
        connections,
    })
}

// 指紋抽出：sR（start = |s|, 長さ = |R|+1）
fn extract_fingerprint_for_base(obs: &[u8], s_len: usize, r_len: usize) -> Vec<u8> {
    let start = s_len;
    let end = start + r_len; // inclusive index は +1 減らしてスライス
    obs[start..=end].to_vec()
}

// 指紋抽出：s a R（start = |s|+1, 長さ = |R|+1）
fn extract_fingerprint_for_step(obs: &[u8], s_len: usize, r_len: usize) -> Vec<u8> {
    let start = s_len + 1;
    let end = start + r_len;
    obs[start..=end].to_vec()
}

/// 簡易 Union-Find
struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<u8>,
}
impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }
    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            let r = self.find(self.parent[x]);
            self.parent[x] = r;
        }
        self.parent[x]
    }
    fn union(&mut self, a: usize, b: usize) {
        let mut x = self.find(a);
        let mut y = self.find(b);
        if x == y {
            return;
        }
        if self.rank[x] < self.rank[y] {
            std::mem::swap(&mut x, &mut y);
        }
        self.parent[y] = x;
        if self.rank[x] == self.rank[y] {
            self.rank[x] += 1;
        }
    }
}
