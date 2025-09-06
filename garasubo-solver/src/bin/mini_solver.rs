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

    #[arg(long, default_value = "https://negainoido.garasubo.com")]
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

#[derive(Default)]
struct World {
    rooms: Vec<Room>, // id = index
    // 未確定の半辺: (u_id, a) -> PairJobId
    frontier_pairs: Vec<(RoomId, Port)>,
    // 新規 prefix 候補
    new_prefixes: Vec<(RoomId, Port, Prefix)>, // parent, port a, child prefix
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
                    from: RoomDoor { room: fr, door: fd },
                    to: RoomDoor { room: tr, door: td },
                });
            }
        }
    }

    // ソートして安定化（任意）
    connections.sort_by_key(|c| (c.from.room, c.from.door, c.to.room, c.to.door));

    Ok(GuessMap {
        rooms: rooms_vec,
        starting_room: starting_room as i32,
        connections,
    })
}

enum PlanKind {
    Pair { u: RoomId, a: Port, b: Port, x: u8 },
    Eq { p: RoomId, q_prefix: Prefix, x: u8 }, // p は既知ID
    ObserveLabel { q_prefix: Prefix, sink: ObserveSink },
}

struct Plan {
    kind: PlanKind,
    s: String, // エンコード済み
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

fn make_pair_plan(u_id: RoomId, p: &Prefix, a: Port, b: Port, x: u8, n_max: usize) -> Plan {
    // p [x] a b
    let s = format!(
        "{}{}{}",
        enc_ports(&p.fwd),
        enc_charcoal(x),
        enc_ports(&[a, b])
    );
    ensure_len_le_6n(p.fwd.len() + 2, n_max);
    Plan {
        kind: PlanKind::Pair { u: u_id, a, b, x },
        s,
    }
}

fn make_eq_plan(p_room: &Room, q: &Prefix, x: u8, n: usize) -> Plan {
    let s = format!(
        "{}{}{}{}",
        enc_ports(&p_room.prefix.fwd),
        enc_charcoal(x),
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
            q_prefix: q.clone(),
            x,
        },
        s,
    }
}

fn make_label_plan(q: &Prefix, sink: ObserveSink, n_max: usize) -> Plan {
    // 単に q.fwd を歩いて観測（0 歩可）
    let s = enc_ports(&q.fwd);
    ensure_len_le_6n(q.fwd.len(), n_max);
    Plan {
        kind: PlanKind::ObserveLabel {
            q_prefix: q.clone(),
            sink,
        },
        s,
    }
}

struct PairAgg {
    // (u,a) ごとの「b×x」成功表
    ok: std::collections::HashMap<(RoomId, Port, Port), [bool; 4]>, // x=0..3
}

impl PairAgg {
    fn new() -> PairAgg {
        PairAgg {
            ok: std::collections::HashMap::new(),
        }
    }
}

impl PairAgg {
    fn on_result(&mut self, u: RoomId, a: Port, b: Port, x: u8, last: u8) {
        let key = (u, a, b);
        let e = self.ok.entry(key).or_insert([false; 4]);
        e[x as usize] = (last == x);
    }
    fn decide_b(&self, u: RoomId, a: Port) -> Option<Port> {
        (0..6u8).find(|&b| {
            let ok = self.ok.get(&(u, a, b)).copied().unwrap_or([false; 4]);
            // 3値で送るなら、3つの x が true か
            ok.iter().filter(|&&t| t).count() >= 3
        })
    }
}

struct EqAgg {
    // (p_id, q_hash) -> 成功回数
    ok: std::collections::HashMap<(RoomId, u64), usize>,
}

impl EqAgg {
    fn new() -> EqAgg {
        EqAgg {
            ok: std::collections::HashMap::new(),
        }
    }
}

fn hash_prefix(q: &Prefix) -> u64 {
    /* fwd と rev を軽くハッシュ */
    0
}

impl EqAgg {
    fn on_result(&mut self, p: RoomId, q: &Prefix, x: u8, last: u8) {
        let k = (p, hash_prefix(q));
        let e = self.ok.entry(k).or_default();
        if last == x {
            *e += 1;
        }
    }
    fn is_equal(&self, p: RoomId, q: &Prefix) -> bool {
        self.ok.get(&(p, hash_prefix(q))).copied().unwrap_or(0) >= 3
    }
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

#[derive(Clone)]
enum ObserveSink {
    /// 観測値を start に入れる
    StartBaseLabel,
    /// 観測値を new_prefixes[i] に入れる（i は実装側で割当）
    NewPrefix(usize),
}

/// 開始部屋の基礎ラベルを 1 本の観測で確定
async fn bootstrap_start(
    world: &mut World,
    session: &SessionGuard,
    n_max: usize,
) -> anyhow::Result<()> {
    let start = &world.rooms[0];
    let mut batch = Batch::new();
    batch.push(make_label_plan(
        &start.prefix,
        ObserveSink::StartBaseLabel,
        n_max,
    ));
    let results = batch.flush(session).await?;

    let (_plan, obs) = &results[0];
    anyhow::ensure!(!obs.is_empty(), "empty observation");
    let label = *obs.last().unwrap(); // 0..3
    world.rooms[0].base_label = Some(label);

    Ok(())
    // 【代替案】空プランが禁止の場合:
    // - 最初の Wave A の Pair プランの "観測列[0]" を拾って start の基礎ラベルに入れる。
    //   (Pair は p=[].fwd なので obs[0] が開始部屋)
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

    // 既知集合 S（Eq 候補）は常に 0..rooms.len()-1
    while !front.is_empty() {
        // ---------- Wave A: pair まとめ打ち ----------
        let mut batch = Batch::new();
        for &u in &front {
            for a in 0..6u8 {
                if world.rooms[u].nbr[a as usize].is_some() {
                    continue;
                }
                for b in 0..6u8 {
                    for &x in &[0u8, 1, 2] {
                        batch.push(make_pair_plan(u, &world.rooms[u].prefix, a, b, x, n));
                    }
                }
            }
        }

        if batch.is_empty() {
            // front の全半辺が既に確定している ⇒ 新規は出ない
            break;
        }

        let results = batch.flush(session).await?;

        // Pair の集計
        let mut pair_agg = PairAgg::new();
        for (plan, obs) in results {
            let last = *obs.last().expect("non-empty obs");
            match plan.kind {
                PlanKind::Pair { u, a, b, x } => {
                    pair_agg.on_result(u, a, b, x, last);
                }
                PlanKind::Eq { .. } | PlanKind::ObserveLabel { .. } => unreachable!(),
            }
        }

        // 決定した (u,a)->b から子 prefix を生やす
        let mut new_prefixes: Vec<(RoomId, Port, Prefix)> = vec![];
        for &u in &front {
            for a in 0..6u8 {
                if world.rooms[u].nbr[a as usize].is_some() {
                    continue;
                }
                if let Some(b) = pair_agg.decide_b(u, a) {
                    // q = p + a, q^-1 = [b] + p^-1
                    let mut qf = world.rooms[u].prefix.fwd.clone();
                    qf.push(a);
                    let mut qr = vec![b];
                    qr.extend_from_slice(&world.rooms[u].prefix.rev);
                    let child = Prefix { fwd: qf, rev: qr };
                    new_prefixes.push((u, a, child));
                } else {
                    // b が決定できなかった場合は次ウェーブで 4 色目を投げ直す等の救済（省略）
                }
            }
        }

        // ---------- Wave B: label と eq ----------
        // まず新規 prefix の基礎ラベルを観測（空きフィルタ用）
        let mut batch = Batch::new();
        for (i, (_, _, q)) in new_prefixes.iter().enumerate() {
            batch.push(make_label_plan(q, ObserveSink::NewPrefix(i), n));
        }
        let label_results = if !batch.is_empty() {
            Some(batch.flush(session).await?)
        } else {
            None
        };

        let mut q_label: Vec<u8> = vec![0; new_prefixes.len()];
        if let Some(results) = label_results {
            for (plan, obs) in results {
                let last = *obs.last().unwrap();
                if let PlanKind::ObserveLabel { sink, .. } = plan.kind {
                    match sink {
                        ObserveSink::NewPrefix(i) => q_label[i] = last,
                        ObserveSink::StartBaseLabel => unreachable!(),
                    }
                }
            }
        }

        // 候補集合 S を準備
        let S: Vec<RoomId> = (0..world.rooms.len()).collect();

        // eq をまとめて投げる
        let mut batch = Batch::new();
        for (i, (_, _, q)) in new_prefixes.iter().enumerate() {
            // 簡易フィルタ: 基礎ラベル一致のみ残す
            for &s in &S {
                let sbl = world.rooms[s].base_label;
                if let Some(sbl) = sbl {
                    if sbl != q_label[i] {
                        continue;
                    }
                }
                for &x in &[0u8, 1, 2] {
                    batch.push(make_eq_plan(&world.rooms[s], q, x, n));
                }
            }
        }
        let results = if !batch.is_empty() {
            Some(batch.flush(session).await?)
        } else {
            None
        };

        // eq の集計
        let mut eq_agg = EqAgg::new();
        if let Some(results) = results {
            for (plan, obs) in results {
                let last = *obs.last().unwrap();
                if let PlanKind::Eq { p, q_prefix, x } = plan.kind {
                    eq_agg.on_result(p, &q_prefix, x, last);
                }
            }
        }

        // new_prefixes を ID に確定し、辺を両側に張る
        let mut next_front: Vec<RoomId> = vec![];
        for (idx, (u, a, q)) in new_prefixes.into_iter().enumerate() {
            // 既知の誰かと一致？
            let mut same: Option<RoomId> = None;
            for &s in &S {
                if eq_agg.is_equal(s, &q) {
                    same = Some(s);
                    break;
                }
            }
            let vid = if let Some(s) = same {
                s
            } else {
                let id = world.rooms.len();
                let mut new_room = Room::new(id, q.clone());
                new_room.base_label = Some(q_label[idx]);
                world.rooms.push(new_room);
                next_front.push(id);
                id
            };

            // 接続: (u,a) <-> (vid, b) ; b は q.rev[0]
            let b = q.rev[0];
            world.rooms[u].nbr[a as usize] = Some((vid, b));
            world.rooms[vid].nbr[b as usize] = Some((u, a));
        }

        front = next_front;
    }

    // 念のため、残っている半辺がないかチェック
    // （あれば、front を「まだ未確定の頂点」に再設定して追加ラウンドを回す設計でもOK）
    // assert!(!has_unpaired_halfedges(&world));

    Ok(world)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    let cli = Cli::parse();

    let session_manager = SessionManager::new(ApiClient::new(cli.api_base_url));

    let session_manager_for_signal = session_manager.current_session.clone();
    let api_client_for_signal = ApiClient::new("https://negainoido.garasubo.com".to_string());

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
