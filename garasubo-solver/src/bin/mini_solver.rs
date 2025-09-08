use clap::Parser;
use dotenvy::dotenv;
use garasubo_solver::api::{ApiClient, Connection, GuessMap, RoomDoor};
use garasubo_solver::session_manager::{SessionGuard, SessionManager};
use std::collections::HashSet;
use std::fmt;
use tokio::signal;

#[derive(Parser)]
#[command(name = "garasubo-solver")]
#[command(about = "ICFPC 2025 Problem Solver")]
struct Cli {
    problem_name: String,

    #[arg(long)]
    user_name: Option<String>,

    #[arg(long)]
    room_num: Option<usize>,

    #[arg(long, default_value = "https://negainoido.garasubo.com/api")]
    api_base_url: String,
}

type RoomId = usize;
type Port = u8; // 0..5

#[derive(Clone, Default)]
struct Prefix {
    fwd: Vec<Port>, // スタート → ここ
    rev: Vec<Port>, // ここ → スタート（常に known）
}

#[derive(Default)]
struct Room {
    id: RoomId,
    prefix: Prefix,
    base_label: Option<u8>, // 0..3, 未取得なら None
    // 6ポートの相手
    nbr: [Option<(RoomId, Port)>; 6],
}

impl Room {
    fn new(id: RoomId, prefix: Prefix) -> Room {
        Room {
            id,
            prefix,
            base_label: None,
            nbr: [None; 6],
        }
    }
}

#[derive(Clone)]
struct NewPrefix {
    parent: RoomId,
    a: Port,
    prefix: Prefix, // fwd: p+a, rev: [b]+p^-1（b は WaveA 後に確定）
    qid: usize,
    label: Option<u8>, // 観測した基本ラベル（任意の最適化）
}

struct World {
    rooms: Vec<Room>, // id = index
    next_qid: usize,
    // 未確定の半辺: (u_id, a) -> PairJobId
    frontier_pairs: Vec<(RoomId, Port)>,
    // 新規 prefix 候補
    new_prefixes: Vec<NewPrefix>, // parent, port a, child prefix
}

impl Default for World {
    fn default() -> Self {
        Self {
            rooms: Vec::new(),
            next_qid: 0,
            new_prefixes: Vec::new(),
            frontier_pairs: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub enum BuildGuessError {
    InvalidStartingRoom {
        starting_room: usize,
        room_count: usize,
    },
    MissingLabel {
        room: usize,
    },
    LabelOutOfRange {
        room: usize,
        value: u8,
    },
    MissingEdge {
        room: usize,
        door: u8,
    },
    OutOfRangePort {
        room: usize,
        door: u8,
        value: u8,
    },
    InvalidRoomId {
        room: usize,
        door: u8,
        target_room: usize,
        room_count: usize,
    },
    InconsistentEdge {
        room: usize,
        door: u8,
        peer_room: usize,
        peer_door: u8,
        found_back: Option<(usize, u8)>,
    },
}

impl fmt::Display for BuildGuessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use BuildGuessError::*;
        match self {
            InvalidStartingRoom {
                starting_room,
                room_count,
            } => write!(
                f,
                "starting_room={} が不正です（rooms={}）",
                starting_room, room_count
            ),
            MissingLabel { room } => write!(
                f,
                "room {} に base_label がありません（charcoal無しの基本ラベルを取得してください）",
                room
            ),
            LabelOutOfRange { room, value } => write!(
                f,
                "room {} のラベル {} が範囲外です（期待 0..3）",
                room, value
            ),
            MissingEdge { room, door } => write!(f, "room {} のポート {} が未接続です", room, door),
            OutOfRangePort { room, door, value } => write!(
                f,
                "room {} のポート {} に不正な相手側ポート {} が設定されています（期待 0..5）",
                room, door, value
            ),
            InvalidRoomId {
                room,
                door,
                target_room,
                room_count,
            } => write!(
                f,
                "room {} のポート {} の相手部屋ID {} が不正です（rooms={}）",
                room, door, target_room, room_count
            ),
            InconsistentEdge {
                room,
                door,
                peer_room,
                peer_door,
                found_back,
            } => write!(
                f,
                "非対称接続: ({}.{}) -> ({}.{}) に対し、戻りが一致しません（found_back={:?}）",
                room, door, peer_room, peer_door, found_back
            ),
        }
    }
}
impl std::error::Error for BuildGuessError {}

/// 辺の正規化キー（無向化）
/// (u,a)-(v,b) を (小さい方, 大きい方) の並びにします。
fn canonical_edge_key(u: i32, a: i32, v: i32, b: i32) -> ((i32, i32), (i32, i32)) {
    if (u, a) < (v, b) {
        ((u, a), (v, b))
    } else {
        ((v, b), (u, a))
    }
}

/// 厳格: すべてのラベル/辺が埋まっており、接続が対称であることを検証しつつ GuessMap を作る。
pub fn build_guess_map_strict(
    world: &World,
    starting_room: usize,
) -> Result<GuessMap, BuildGuessError> {
    let n = world.rooms.len();
    if starting_room >= n {
        return Err(BuildGuessError::InvalidStartingRoom {
            starting_room,
            room_count: n,
        });
    }

    // 1) rooms（基本ラベル）
    let mut rooms_vec: Vec<i32> = Vec::with_capacity(n);
    for (i, r) in world.rooms.iter().enumerate() {
        let lbl = r
            .base_label
            .ok_or(BuildGuessError::MissingLabel { room: i })?;
        if lbl > 3 {
            return Err(BuildGuessError::LabelOutOfRange {
                room: i,
                value: lbl,
            });
        }
        rooms_vec.push(lbl as i32);
    }

    // 2) connections（無向。自己ループ/多重辺にも対応）
    let mut seen = HashSet::<((i32, i32), (i32, i32))>::new();
    let mut connections: Vec<Connection> = Vec::new();

    for (u, r) in world.rooms.iter().enumerate() {
        for a in 0..6usize {
            let (v, b) = r.nbr[a].ok_or(BuildGuessError::MissingEdge {
                room: u,
                door: a as u8,
            })?;
            // 範囲チェック
            if v >= n {
                return Err(BuildGuessError::InvalidRoomId {
                    room: u,
                    door: a as u8,
                    target_room: v,
                    room_count: n,
                });
            }
            if b > 5 {
                return Err(BuildGuessError::OutOfRangePort {
                    room: u,
                    door: a as u8,
                    value: b,
                });
            }
            // 対称性チェック
            let back = world.rooms[v].nbr[b as usize];
            match back {
                Some((uu, aa)) if uu == u && aa == a as u8 => { /* OK */ }
                other => {
                    return Err(BuildGuessError::InconsistentEdge {
                        room: u,
                        door: a as u8,
                        peer_room: v,
                        peer_door: b,
                        found_back: other,
                    });
                }
            }

            // 無向化して重複抑止
            let key = canonical_edge_key(u as i32, a as i32, v as i32, b as i32);
            if seen.insert(key) {
                // 片側だけ Connection を作る
                let ((fr, fd), (tr, td)) = key;
                connections.push(Connection {
                    from: RoomDoor {
                        room: fr as usize,
                        door: fd as usize,
                    },
                    to: RoomDoor {
                        room: tr as usize,
                        door: td as usize,
                    },
                });
            }
        }
    }

    // ソートして安定化（任意）
    connections.sort_by_key(|c| (c.from.room, c.from.door, c.to.room, c.to.door));

    Ok(GuessMap {
        rooms: rooms_vec,
        starting_room: starting_room,
        connections,
    })
}

enum PlanKind {
    Pair { u: RoomId, a: Port, b: Port, x: u8 },
    Eq { p: RoomId, qid: usize, x: u8 }, // ← q を参照せず、一意IDで識別
    ObserveLabel { qid: usize },         // ← ラベル観測も qid
}

struct Plan {
    kind: PlanKind,
    s: String,
}

// --- Eq 集計を qid ベースに --- //
use std::collections::HashMap;

struct EqAgg {
    // (p_id, qid) -> 成功回数
    ok: HashMap<(RoomId, usize), u8>,
}

impl EqAgg {
    fn new() -> Self {
        Self { ok: HashMap::new() }
    }
    fn on_result(&mut self, p: RoomId, qid: usize, x: u8, last: u8) {
        if last == x {
            *self.ok.entry((p, qid)).or_insert(0) += 1;
        }
    }
    fn is_equal(&self, p: RoomId, qid: usize) -> bool {
        self.ok.get(&(p, qid)).copied().unwrap_or(0) >= 3
    }
}

// --- Pair 集計はそのままでもOK（u,a,b,x でユニーク） --- //
struct PairAgg {
    ok: HashMap<(RoomId, Port, Port), [bool; 4]>, // x=0..3 の成否
}
impl PairAgg {
    fn new() -> Self {
        Self { ok: HashMap::new() }
    }
    fn on_result(&mut self, u: RoomId, a: Port, b: Port, x: u8, last: u8) {
        let e = self.ok.entry((u, a, b)).or_insert([false; 4]);
        e[x as usize] = (last == x);
    }
    fn decide_b(&self, u: RoomId, a: Port) -> Option<Port> {
        (0..6u8).find(|&b| {
            let ok = self.ok.get(&(u, a, b)).copied().unwrap_or([false; 4]);
            ok.iter().filter(|&&t| t).count() >= 3
        })
    }
}

// door-steps の安全チェック
fn ensure_len_le_6n(plan_digits: usize, n: usize) {
    assert!(plan_digits <= 6 * n, "plan too long");
}

// プレフィックス/ポートを文字列化
fn enc_ports(ports: &[Port]) -> String {
    ports.iter().map(|&d| (b'0' + d) as char).collect()
}
fn enc_charcoal(x: u8) -> String {
    format!("[{}]", x)
}

fn make_pair_plan(u: RoomId, p: &Prefix, a: Port, b: Port, x: u8, n: usize) -> Plan {
    let s = format!("{}[{}]{}{}", enc_ports(&p.fwd), x, a, b);
    ensure_len_le_6n(p.fwd.len() + 2, n);
    Plan {
        kind: PlanKind::Pair { u, a, b, x },
        s,
    }
}

fn make_eq_plan(p_room: &Room, qid: usize, q: &Prefix, x: u8, n: usize) -> Plan {
    let s = format!(
        "{}[{}]{}{}",
        enc_ports(&p_room.prefix.fwd),
        x,
        enc_ports(&p_room.prefix.rev),
        enc_ports(&q.fwd)
    );
    ensure_len_le_6n(
        p_room.prefix.fwd.len() + p_room.prefix.rev.len() + q.fwd.len(),
        n,
    );
    Plan {
        kind: PlanKind::Eq {
            p: p_room.id,
            qid,
            x,
        },
        s,
    }
}

fn make_label_plan(qid: usize, q: &Prefix, n: usize) -> Plan {
    let s = enc_ports(&q.fwd);
    ensure_len_le_6n(q.fwd.len(), n);
    Plan {
        kind: PlanKind::ObserveLabel { qid },
        s,
    }
}

fn hash_prefix(q: &Prefix) -> u64 {
    /* fwd と rev を軽くハッシュ */
    0
}

#[derive(Debug)]
enum EdgeSetError {
    Conflict {
        at_room: RoomId,
        at_port: Port,
        want: (RoomId, Port),
        have: (RoomId, Port),
    },
}

fn set_edge(world: &mut World, u: RoomId, a: Port, v: RoomId, b: Port) -> Result<(), EdgeSetError> {
    // u側
    if let Some((vv, bb)) = world.rooms[u].nbr[a as usize] {
        if vv != v || bb != b {
            return Err(EdgeSetError::Conflict {
                at_room: u,
                at_port: a,
                want: (v, b),
                have: (vv, bb),
            });
        }
    } else {
        world.rooms[u].nbr[a as usize] = Some((v, b));
    }
    // v側
    if let Some((uu, aa)) = world.rooms[v].nbr[b as usize] {
        if uu != u || aa != a {
            return Err(EdgeSetError::Conflict {
                at_room: v,
                at_port: b,
                want: (u, a),
                have: (uu, aa),
            });
        }
    } else {
        world.rooms[v].nbr[b as usize] = Some((u, a));
    }
    Ok(())
}

struct Batch {
    plans: Vec<Plan>,
    // flush 後の突合用メタデータを並行で保持
}

impl Batch {
    fn new() -> Self {
        Self { plans: vec![] }
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.plans.is_empty()
    }
    fn push(&mut self, p: Plan) {
        self.plans.push(p);
    }
    async fn flush(self, session: &SessionGuard) -> anyhow::Result<Vec<(Plan, Vec<u8>)>> {
        // client.explore(plans.map(|p| p.s))
        // -> results: Vec<Vec<u8>>
        // ここで self.plans の順序と results を1対1で消費
        let result = session
            .explore(&self.plans.iter().map(|p| p.s.clone()).collect::<Vec<_>>())
            .await?;
        Ok(self.plans.into_iter().zip(result.results).collect())
    }
}

/// 開始部屋の基礎ラベルを 1 本の観測（空プラン）で確定
async fn bootstrap_start(
    world: &mut World,
    session: &SessionGuard,
    _n_max: usize,
) -> anyhow::Result<()> {
    // 空プラン "" で現在地（開始部屋）のラベルが 1 要素として返る
    let resp = session.explore(&vec![String::new()]).await?;
    anyhow::ensure!(resp.results.len() == 1, "unexpected /explore result shape");
    let obs = &resp.results[0];
    anyhow::ensure!(!obs.is_empty(), "empty observation for start");
    let label = *obs.last().unwrap(); // 0..3
    world.rooms[0].base_label = Some(label);
    Ok(())
}

async fn solve(n: usize, session: &SessionGuard) -> anyhow::Result<World> {
    let mut world = World::default();
    // Start room を ID=0 で作成
    world.rooms.push(Room::new(
        0,
        Prefix {
            fwd: vec![],
            rev: vec![],
        },
    ));

    // まず開始部屋の基礎ラベルを確定
    bootstrap_start(&mut world, session, n).await?;

    // BFS フロンティア
    let mut front: Vec<RoomId> = vec![0];

    while !front.is_empty() {
        // ---------- Wave A: pair まとめ打ち ----------
        let mut batch = Batch::new();
        for &u in &front {
            for a in 0..6u8 {
                if world.rooms[u].nbr[a as usize].is_some() {
                    continue; // 既に確定済み
                }
                for b in 0..6u8 {
                    for &x in &[0u8, 1, 2] {
                        batch.push(make_pair_plan(u, &world.rooms[u].prefix, a, b, x, n));
                    }
                }
            }
        }

        if batch.is_empty() {
            // このフロンティアから生やす半辺がない ⇒ 完了
            break;
        }

        let results = batch.flush(session).await?;

        // Pair の集計
        let mut pair_agg = PairAgg::new();
        for (plan, obs) in results {
            let last = *obs.last().ok_or_else(|| anyhow::anyhow!("empty obs"))?;
            match plan.kind {
                PlanKind::Pair { u, a, b, x } => {
                    pair_agg.on_result(u, a, b, x, last);
                }
                _ => unreachable!(),
            }
        }

        // 決定した (u,a)->b から子 prefix を生やす（qid を付与）
        world.new_prefixes.clear();
        for &u in &front {
            for a in 0..6u8 {
                if world.rooms[u].nbr[a as usize].is_some() {
                    continue;
                }
                if let Some(b) = pair_agg.decide_b(u, a) {
                    let qid = world.next_qid;
                    world.next_qid += 1;

                    // q = p + a, q^-1 = [b] + p^-1
                    let mut qf = world.rooms[u].prefix.fwd.clone();
                    qf.push(a);
                    let mut qr = vec![b];
                    qr.extend_from_slice(&world.rooms[u].prefix.rev);
                    let child = Prefix { fwd: qf, rev: qr };

                    world.new_prefixes.push(NewPrefix {
                        parent: u,
                        a,
                        prefix: child,
                        qid,
                        label: None,
                    });
                }
            }
        }

        if world.new_prefixes.is_empty() {
            // 生やせる子が無い ⇒ 収束
            break;
        }

        // ---------- Wave B-1: 新規 prefix の基礎ラベル観測（候補絞り用） ----------
        let mut qid_to_index = std::collections::HashMap::<usize, usize>::new();
        let mut batch = Batch::new();
        for (i, np) in world.new_prefixes.iter().enumerate() {
            qid_to_index.insert(np.qid, i);
            batch.push(make_label_plan(np.qid, &np.prefix, n));
        }

        let label_results = batch.flush(session).await?;
        for (plan, obs) in label_results {
            let last = *obs.last().unwrap();
            match plan.kind {
                PlanKind::ObserveLabel { qid } => {
                    if let Some(&idx) = qid_to_index.get(&qid) {
                        world.new_prefixes[idx].label = Some(last);
                    }
                }
                _ => unreachable!(),
            }
        }

        // ---------- Wave B-2: eq まとめ打ち ----------
        let candidates: Vec<RoomId> = (0..world.rooms.len()).collect();
        let mut batch = Batch::new();
        for np in &world.new_prefixes {
            for &s in &candidates {
                // 基本ラベルが既知なら一致するものだけに絞る
                if let (Some(sl), Some(ql)) = (world.rooms[s].base_label, np.label) {
                    if sl != ql {
                        continue;
                    }
                }
                for &x in &[0u8, 1, 2] {
                    batch.push(make_eq_plan(&world.rooms[s], np.qid, &np.prefix, x, n));
                }
            }
        }

        let eq_results = batch.flush(session).await?;
        let mut eq_agg = EqAgg::new();
        for (plan, obs) in eq_results {
            let last = *obs.last().unwrap();
            if let PlanKind::Eq { p, qid, x } = plan.kind {
                eq_agg.on_result(p, qid, x, last);
            } else {
                unreachable!();
            }
        }

        // ---------- 決着：ID に統合してエッジを張る（上書き禁止） ----------
        let mut next_front: Vec<RoomId> = vec![];
        let new_prefixes = world.new_prefixes.drain(..).collect::<Vec<_>>();
        for np in new_prefixes.iter() {
            // 既知の誰かと同一？
            let mut same: Option<RoomId> = None;
            for s in 0..world.rooms.len() {
                if eq_agg.is_equal(s, np.qid) {
                    same = Some(s);
                    break;
                }
            }

            let vid = if let Some(s) = same {
                s
            } else {
                let id = world.rooms.len();
                let mut new_room = Room::new(id, np.prefix.clone());
                if let Some(lbl) = np.label {
                    new_room.base_label = Some(lbl);
                }
                world.rooms.push(new_room);
                next_front.push(id);
                id
            };

            // 接続: (np.parent, np.a) <-> (vid, b) ; b は q.rev[0]
            let b = np.prefix.rev[0];
            if let Err(e) = set_edge(&mut world, np.parent, np.a, vid, b) {
                anyhow::bail!(
                    "edge conflict while setting ({}.{}) <-> ({}.{}) : {:?}",
                    np.parent,
                    np.a,
                    vid,
                    b,
                    e
                );
            }
        }

        front = next_front;
    }

    Ok(world)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
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

    let result = solve(cli.room_num.unwrap_or(12), &session_guard).await?;

    session_guard
        .guess(build_guess_map_strict(&result, 0)?)
        .await?;

    Ok(())
}
