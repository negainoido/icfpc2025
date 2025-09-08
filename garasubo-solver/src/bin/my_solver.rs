use clap::Parser;
use garasubo_solver::api::{ApiClient, Connection, GuessMap, RoomDoor};
use garasubo_solver::session_manager::SessionManager;
use std::cmp::PartialEq;
use std::collections::{HashMap, HashSet};
use tokio::signal;
use garasubo_solver::candidate_gen::{build_candidates, CandParams, Candidate, CandidateList};
use garasubo_solver::cover_walk::generate_cover_walk;
use garasubo_solver::pass2_ingest::apply_pass2_and_recluster;
use garasubo_solver::pass2_scheduler::{build_pass2_plan, PlanOutput, SchedulerParams};
use garasubo_solver::phase_c::{run_phase_c, MergeResult};
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

#[derive(Clone, Debug, Default, PartialEq)]
enum WorkingConnection {
    // 未探索
    #[default]
    Unknown,
    // labelのみ判明
    Seen(u8),
    // labelはわかっていて既知のノードのどれかにつながっている
    HalfKnown(u8),
    // nodeが判明
    Known {
        node_id: usize,
        // 対応する反対方向のedge
        reverse_edge: Option<usize>,
    },
}

struct KnownNodeConnection {
    node_id: usize,
}

struct KnownNode {
    id: usize,
    label: u8,
    edges: [Option<KnownNodeConnection>; 6],
    // startからの最短パス
    path: Vec<u8>,
}

impl KnownNode {
    fn new(id: usize, label: u8, path: Vec<u8>) -> Self {
        Self {
            id,
            label,
            edges: Default::default(),
            path,
        }
    }
}

#[derive(Clone, Debug)]
enum Action {
    // ドアを使って移動する
    Move(usize),
    // 炭でマーキングする
    Mark(usize),
}

enum Plan {
    // 最初のランダムウォーク
    Walk(Vec<u8>),
    // 炭を使ったマーキングにより部屋を識別するwalk
    MarkedWalk {
        plan: Vec<Action>,
        rewrite_target: HashSet<usize>,
        state_idx: usize,
    },
}

fn plan_to_string(plan: &Vec<Action>) -> String {
    let mut s = String::new();
    for p in plan {
        match p {
            Action::Move(i) => {
                s.push_str(&format!("{i}"));
            }
            Action::Mark(i) => {
                s.push_str(&format!("[{i}]"));
            }
        }
    }
    s
}

impl Plan {
    fn to_query_string(&self) -> String {
        match self {
            Plan::Walk(walk) => walk.iter().map(|i| ('0' as u8 + *i) as char).collect::<String>(),
            Plan::MarkedWalk { plan, .. } => plan_to_string(plan),
        }
    }
}

struct MySolver {
    size: usize,
    nodes: Vec<KnownNode>,
    label_count: [usize; 4],
    // exploreのクエリとその結果
    histories: Vec<(Vec<Action>, Vec<u8>)>,
    prev_query: Vec<Plan>,
    states: Vec<State>,
    cover_walk: Vec<u8>,
}

struct State {
    walk: Vec<u8>,
    y: Vec<u8>,
    // 訪れたノードでknownになったものの集合
    known_nodes: HashMap<usize, usize>,
    // 色ぬりかえに使ったノードID
    used_nodes: HashSet<usize>,
}

impl MySolver {
    fn new(size: usize) -> Self {
        Self {
            size,
            nodes: Vec::new(),
            label_count: [0; 4],
            histories: Vec::new(),
            prev_query: Vec::new(),
            states: Vec::new(),
            cover_walk: generate_cover_walk(size),
        }
    }

    fn initial_plan(&mut self) -> Vec<String> {
        let plans = vec![ self.cover_walk.iter().map(|i| ('0' as u8 + *i) as char).collect::<String>() ];

        self.prev_query = vec![ Plan::Walk(self.cover_walk.clone()) ];

        plans
    }

    fn next_plan(&mut self, results: &Vec<Vec<u8>>) -> Vec<String> {
        let plan_count = self.prev_query.len();
        // planが複数walkに対応していたとき用のidxカウンタ
        let mut result_idx = 0;
        let mut next_plan = vec![];
        for i in 0..plan_count {
            let query = &self.prev_query[i];

            match query {
                Plan::Walk(walk) => {
                    let n = self.size;
                    let y = results[result_idx].clone();
                    // 最初に登場したラベルのノードの位置をメモ
                    let mut memo = vec![None; 4];
                    let mut rewrite_target= HashSet::new();
                    let mut known_nodes = HashMap::new();
                    let mut used_nodes = HashSet::new();

                    // 既知のノードについては予め登録
                    if self.nodes.len() > 0 {
                        known_nodes.insert(0, 0);
                        let mut current_node_id = 0;
                        for (pos, w) in walk.iter().enumerate() {
                            let node = &self.nodes[current_node_id];
                            if let Some(edge) = &node.edges[*w as usize] {
                                known_nodes.insert(pos+1, edge.node_id);
                                println!("known node: {} label: {} id: {}", pos+1, edge.node_id, edge.node_id);
                                current_node_id = edge.node_id;
                            } else {
                                break;
                            }
                        }
                    }


                    for (i, &c) in y.iter().enumerate() {
                        if memo[c as usize] == None {
                            memo[c as usize] = Some(i);
                            rewrite_target.insert(i);
                            if let Some(node_id) = known_nodes.get(&i) {
                                used_nodes.insert(*node_id);
                            } else {
                                let label = c;
                                let node_id= self.nodes.len();
                                let node = KnownNode::new(node_id, label, vec![]);
                                self.nodes.push(node);
                                known_nodes.insert(i, node_id);
                                println!("x known node: {} label: {} id: {}", i, label, node_id);
                                used_nodes.insert(node_id);
                            }
                        }
                    }
                    // 最初に登場したラベルのノードを書き換えるwalkをつくる
                    let mut new_walk = vec![];
                    for (i, w) in walk.iter().enumerate() {
                        if rewrite_target.contains(&i) {
                            let label = y[i] as usize;
                            new_walk.push(Action::Mark((label+1) % 4));
                        }
                        new_walk.push(Action::Move(*w as usize));
                    }

                    // 次のクエリとして登録
                    next_plan.push(Plan::MarkedWalk {
                        plan: new_walk,
                        rewrite_target,
                        state_idx: self.states.len(),
                    });
                    self.states.push(State {
                        walk: walk.clone(),
                        y,
                        known_nodes,
                        used_nodes,
                    });


                    result_idx += 1;
                }
                Plan::MarkedWalk { plan, rewrite_target, state_idx } => {
                    println!("marked walk");
                    let y2 = &results[result_idx];
                    let state = &mut self.states[*state_idx];
                    let y = &state.y;
                    let mut y_idx = 0;
                    // ラベルが変わっていたときにどのノードと同一かとわかるか
                    let mut rewrite_memo = HashMap::new();
                    for (i, action) in plan.iter().enumerate() {
                        match action {
                            Action::Move(x) => {
                                if y[y_idx+1] != y2[i+1] {
                                    if let Some(idx) = rewrite_memo.get(&y[y_idx+1]) {
                                        println!("detect rewrite: {} to {} at {}", y[y_idx+1], y2[i+1], y_idx+1);
                                        println!("known node: {} {:?}", idx, rewrite_memo);
                                        let known_node_id = state.known_nodes[idx];
                                        if let Some(node_id) = state.known_nodes.get(&(y_idx+1)) {
                                            assert_eq!(*node_id, known_node_id, "rewrite target is not same as known node");
                                        } else {
                                            state.known_nodes.insert(y_idx+1, known_node_id);
                                        }
                                    } else {
                                        panic!("invalid rewrite");
                                    }
                                }
                                y_idx += 1;
                            }
                            Action::Mark(x) => {
                                // もともとの色
                                let label = y[y_idx];
                                rewrite_memo.insert(label, y_idx);
                                println!("found rewrite: {} to {} at {}", label, x, y_idx);
                                assert!(rewrite_target.contains(&y_idx), "invalid rewrite");
                                // y_idxは更新しない
                            }
                        }
                    }
                    // 次の計画を建てる
                    let mut new_target = HashMap::new();
                    for (i, label) in y.iter().enumerate() {
                        if new_target.contains_key(label) {
                            continue;
                        }
                        let node_id = if let Some(node_id) = state.known_nodes.get(&i) {
                            if state.used_nodes.contains(node_id) {
                                continue
                            }
                            *node_id
                        } else {
                            let node_id = self.nodes.len();
                            let node = KnownNode::new(node_id, *label, vec![]);
                            self.nodes.push(node);
                            state.known_nodes.insert(i, node_id);
                            println!("known node: {} label: {} id: {}", i, label, node_id);
                            node_id
                        };
                        new_target.insert(*label, i);
                        state.used_nodes.insert(node_id);
                    }
                    let new_rewrite_target = new_target.values().copied().collect::<HashSet<_>>();

                    if new_target.is_empty() {
                        println!("no new target");
                        // もう未知のノードがないならランダムウォーク
                        // まずはグラフを構築
                        for (i, &w) in state.walk.iter().enumerate() {
                            let w = w as usize;
                            let c = y[i];
                            let dest_id = self.nodes[state.known_nodes[&(i+1)]].id;
                            let node = &mut self.nodes[state.known_nodes[&i]];
                            match &node.edges[w] {
                                Some(edge) => {
                                    assert_eq!(edge.node_id, dest_id, "existing edge is not same as new edge");
                                },
                                None => {
                                    node.edges[w] = Some(KnownNodeConnection {
                                        node_id: dest_id,
                                    });
                                }
                            }
                        }
                        // 全ノードを探索するパスから始める
                        let base_walk = self.get_all_node_visit_path();
                        println!("base_walk_len: {:?}", base_walk);
                        // base walkの行き先を計算
                        let mut pos = 0;
                        for w in base_walk.iter() {
                            let w = *w as usize;
                            let node = &mut self.nodes[pos];
                            match &node.edges[w] {
                                Some(edge) => {
                                    pos = edge.node_id;
                                },
                                None => {
                                    panic!("invalid base walk at {} {}", i, pos);
                                }
                            }
                        }
                        let base_walk_dest = pos;
                        // 未訪問edgeを探す
                        let mut path = self.find_empty_edge_path(base_walk_dest);
                        if path.is_empty() {
                            println!("no empty edge");
                            continue;
                        }
                        println!("base_walk_len: {} path_len: {:?}", base_walk.len(), path.len());
                        let new_walk = base_walk.iter().map(|x| *x as u8);
                        let new_walk = new_walk.chain(path.iter().copied()).chain(self.cover_walk.iter().copied()).take(self.size * 6).collect::<Vec<_>>();
                        next_plan.push(Plan::Walk(new_walk));

                    } else {
                        println!("new marked walk planning: {:?}", new_rewrite_target);
                        let mut new_walk = vec![];
                        for (i, w) in state.walk.iter().enumerate() {
                            if new_rewrite_target.contains(&i) {
                                let label = y[i] as usize;
                                new_walk.push(Action::Mark((label+1) % 4));
                            }
                            new_walk.push(Action::Move(*w as usize));
                        }

                        // 次のクエリとして登録
                        next_plan.push(Plan::MarkedWalk {
                            plan: new_walk,
                            rewrite_target: new_rewrite_target,
                            state_idx: *state_idx,
                        });
                    }


                    result_idx += 1;
                }
            }
        }

        let queries = next_plan.iter().map(|p| p.to_query_string()).collect();
        self.prev_query = next_plan;
        queries
    }



    // すべてのノードを訪問するpathを得る（ノードの再訪問を許可）
    fn get_all_node_visit_path(&self) -> Vec<usize> {
        if self.nodes.is_empty() {
            return vec![];
        }

        // 開始ノード（通常はID=0）を探す
        let start_node_idx = 0; // もしID=0がなければ最初のノードを使用

        let mut visited = vec![false; self.nodes.len()];
        let mut path = vec![];
        let mut current_node_idx = start_node_idx;
        
        // すべてのノードを訪問するまでループ
        while visited.iter().any(|&v| !v) {
            visited[current_node_idx] = true;
            
            // 現在のノードから未訪問のノードを探す
            if let Some((door_idx, next_node_idx)) = self.find_unvisited_neighbor(current_node_idx, &visited) {
                path.push(door_idx);
                current_node_idx = next_node_idx;
            } else {
                // 現在のノードから未訪問のノードに直接行けない場合、
                // 他の未訪問ノードへのパスを探す
                if let Some((path_to_unvisited, target_node_idx)) = self.find_path_to_unvisited(current_node_idx, &visited) {
                    path.extend(path_to_unvisited);
                    current_node_idx = target_node_idx;
                } else {
                    // すべてのノードが訪問済みになった
                    break;
                }
            }
        }
        
        path
    }

    // 現在のノードから直接行ける未訪問のノードを探す
    fn find_unvisited_neighbor(&self, node_idx: usize, visited: &[bool]) -> Option<(usize, usize)> {
        let current_node = &self.nodes[node_idx];
        
        for door_idx in 0..6 {
            if let Some(connection) = &current_node.edges[door_idx] {
                if let Some(next_node_idx) = self.nodes.iter()
                    .position(|node| node.id == connection.node_id) {
                    
                    if !visited[next_node_idx] {
                        return Some((door_idx, next_node_idx));
                    }
                }
            }
        }
        None
    }

    // 現在のノードから未訪問のノードへのパスをBFSで探す
    fn find_path_to_unvisited(&self, start_node_idx: usize, visited: &[bool]) -> Option<(Vec<usize>, usize)> {
        use std::collections::VecDeque;
        
        let mut queue = VecDeque::new();
        let mut bfs_visited = vec![false; self.nodes.len()];
        let mut parent = vec![None; self.nodes.len()];
        let mut parent_door = vec![None; self.nodes.len()];
        
        queue.push_back(start_node_idx);
        bfs_visited[start_node_idx] = true;
        
        while let Some(current_idx) = queue.pop_front() {
            let current_node = &self.nodes[current_idx];
            
            for door_idx in 0..6 {
                if let Some(connection) = &current_node.edges[door_idx] {
                    if let Some(next_node_idx) = self.nodes.iter()
                        .position(|node| node.id == connection.node_id) {
                        
                        if !bfs_visited[next_node_idx] {
                            bfs_visited[next_node_idx] = true;
                            parent[next_node_idx] = Some(current_idx);
                            parent_door[next_node_idx] = Some(door_idx);
                            queue.push_back(next_node_idx);
                            
                            // 未訪問のノードを見つけた
                            if !visited[next_node_idx] {
                                // パスを再構築
                                let mut path = vec![];
                                let mut node_idx = next_node_idx;
                                
                                while let Some(p_idx) = parent[node_idx] {
                                    if let Some(door) = parent_door[node_idx] {
                                        path.push(door);
                                    }
                                    node_idx = p_idx;
                                }
                                
                                path.reverse();
                                return Some((path, next_node_idx));
                            }
                        }
                    }
                }
            }
        }
        
        None
    }

    // start nodeから最も近い未確定のedgeを探す
    fn find_empty_edge_path(&self, start_node_idx: usize) -> Vec<u8> {
        use std::collections::VecDeque;
        
        if self.nodes.is_empty() || start_node_idx >= self.nodes.len() {
            return vec![];
        }
        
        // BFSでノードを探索し、各ノードの未確定エッジをチェック
        let mut queue = VecDeque::new();
        let mut visited = vec![false; self.nodes.len()];
        let mut parent = vec![None; self.nodes.len()];
        let mut parent_door = vec![None; self.nodes.len()];
        
        queue.push_back(start_node_idx);
        visited[start_node_idx] = true;
        
        while let Some(current_idx) = queue.pop_front() {
            let current_node = &self.nodes[current_idx];
            
            // 現在のノードで未確定のエッジを探す
            for door_idx in 0..6 {
                if current_node.edges[door_idx].is_none() {
                    // パスを再構築して返す
                    let mut path = vec![];
                    let mut node_idx = current_idx;
                    
                    while let Some(p_idx) = parent[node_idx] {
                        if let Some(door) = parent_door[node_idx] {
                            path.push(door);
                        }
                        node_idx = p_idx;
                    }
                    
                    path.reverse();
                    path.push(door_idx as u8); // 未確定エッジへの最後の遷移
                    return path;
                }
            }
            
            // 隣接ノードをキューに追加
            for door_idx in 0..6 {
                if let Some(connection) = &current_node.edges[door_idx] {
                    if let Some(next_node_idx) = self.nodes.iter()
                        .position(|node| node.id == connection.node_id) {
                        
                        if !visited[next_node_idx] {
                            visited[next_node_idx] = true;
                            parent[next_node_idx] = Some(current_idx);
                            parent_door[next_node_idx] = Some(door_idx as u8);
                            queue.push_back(next_node_idx);
                        }
                    }
                }
            }
        }
        
        vec![] // 未確定のエッジが見つからない場合
    }

    fn build_map(&self) -> GuessMap {
        let rooms = self.nodes.iter().map(|node| {
            node.label as i32
        }).collect::<Vec<_>>();

        let mut connections = vec![];
        let mut used_edges = HashSet::new();
        for node in &self.nodes {
            let id = node.id;
            for door_idx in 0..6 {
                if used_edges.contains(&(id, door_idx)) {
                    continue;
                }
                if let Some(connection) = &node.edges[door_idx] {
                    let next_node_id = connection.node_id;
                    if next_node_id == id {
                        connections.push(Connection {
                            from: RoomDoor { room: id, door: door_idx },
                            to: RoomDoor { room: id, door: door_idx },
                        })
                    } else {
                        for door_jdx in 0..6 {
                            if used_edges.contains(&(next_node_id, door_jdx)) {
                                continue;
                            }
                            if let Some(connection) = &self.nodes[next_node_id].edges[door_jdx] {
                                if connection.node_id == id {
                                    connections.push(Connection {
                                        from: RoomDoor { room: id, door: door_idx },
                                        to: RoomDoor { room: next_node_id, door: door_jdx },
                                    });
                                    used_edges.insert((id, door_idx));
                                    used_edges.insert((next_node_id, door_jdx));
                                    break;
                                }
                            } else {
                                panic!("invalid edge at node {} {}", id, door_jdx);
                            }
                        }
                    }
                } else {
                    panic!("invalid edge at node {} {}", id, door_idx);
                }
            }
        }


        GuessMap {
            rooms,
            starting_room: 0,
            connections,
        }
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

    let mut solver = MySolver::new(cli.room_num);
    let initial_plan = solver.initial_plan();
    println!("initial plan: {:?}", initial_plan);
    let mut result = session_guard.explore(&initial_plan).await?;
    println!("first walk done: {:?}", result.results);
    loop {
        let next_plan = solver.next_plan(&result.results);
        if next_plan.is_empty() {
            break;
        }
        println!("next plan: {:?}", next_plan);
        result = session_guard.explore(&next_plan).await?;
        println!("next walk done {:?}", result.results);
    }

    let guess_response = session_guard.guess(solver.build_map()).await?;
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

    #[test]
    fn test_get_all_node_visit_path_empty() {
        let solver = MySolver::new(3);
        let path = solver.get_all_node_visit_path();
        assert_eq!(path, Vec::<usize>::new());
    }

    #[test]
    fn test_get_all_node_visit_path_single_node() {
        let mut solver = MySolver::new(3);
        
        // 単一ノードを追加
        let node = KnownNode::new(0, 1, vec![]);
        solver.nodes.push(node);
        
        let path = solver.get_all_node_visit_path();
        assert_eq!(path, Vec::<usize>::new());
    }

    #[test]
    fn test_get_all_node_visit_path_linear_graph() {
        let mut solver = MySolver::new(3);
        
        // 線形グラフを作成: 0 - 1 - 2
        let mut node0 = KnownNode::new(0, 1, vec![]);
        node0.edges[0] = Some(KnownNodeConnection {
            node_id: 1,
        });
        
        let mut node1 = KnownNode::new(1, 2, vec![0]);
        node1.edges[1] = Some(KnownNodeConnection {
            node_id: 0,
        });
        node1.edges[2] = Some(KnownNodeConnection {
            node_id: 2,
        });
        
        let mut node2 = KnownNode::new(2, 3, vec![0, 2]);
        node2.edges[3] = Some(KnownNodeConnection {
            node_id: 1,
        });
        
        solver.nodes.push(node0);
        solver.nodes.push(node1);
        solver.nodes.push(node2);
        
        let path = solver.get_all_node_visit_path();
        
        // パスが空でないことを確認
        assert!(!path.is_empty());
        
        // パスの各要素が0-5の範囲内であることを確認
        for &door in &path {
            assert!(door < 6, "Door number {} is out of range", door);
        }
        
        // パスの長さが合理的であることを確認（最大でもノード数-1の2倍程度）
        assert!(path.len() <= (solver.nodes.len() - 1) * 2);
    }

    #[test]
    fn test_get_all_node_visit_path_triangle_graph() {
        let mut solver = MySolver::new(3);
        
        // 三角形グラフを作成: 0 - 1 - 2 - 0
        let mut node0 = KnownNode::new(0, 1, vec![]);
        node0.edges[0] = Some(KnownNodeConnection {
            node_id: 1,
        });
        node0.edges[5] = Some(KnownNodeConnection {
            node_id: 2,
        });
        
        let mut node1 = KnownNode::new(1, 2, vec![0]);
        node1.edges[1] = Some(KnownNodeConnection {
            node_id: 0,
        });
        node1.edges[3] = Some(KnownNodeConnection {
            node_id: 2,
        });
        
        let mut node2 = KnownNode::new(2, 3, vec![0, 3]);
        node2.edges[2] = Some(KnownNodeConnection {
            node_id: 0,
        });
        node2.edges[4] = Some(KnownNodeConnection {
            node_id: 1,
        });
        
        solver.nodes.push(node0);
        solver.nodes.push(node1);
        solver.nodes.push(node2);
        
        let path = solver.get_all_node_visit_path();
        
        // パスが空でないことを確認
        assert!(!path.is_empty());
        
        // パスの各要素が0-5の範囲内であることを確認
        for &door in &path {
            assert!(door < 6, "Door number {} is out of range", door);
        }
        
        // 三角形グラフなので、最低2ステップは必要
        assert!(path.len() >= 2);
    }

    #[test]
    fn test_find_unvisited_neighbor() {
        let mut solver = MySolver::new(2);
        
        let mut node0 = KnownNode::new(0, 1, vec![]);
        node0.edges[0] = Some(KnownNodeConnection {
            node_id: 1,
        });
        
        let node1 = KnownNode::new(1, 2, vec![0]);
        
        solver.nodes.push(node0);
        solver.nodes.push(node1);
        
        let visited = vec![true, false]; // node0は訪問済み、node1は未訪問
        
        let result = solver.find_unvisited_neighbor(0, &visited);
        assert_eq!(result, Some((0, 1))); // ドア0でnode1に行ける
        
        let visited_all = vec![true, true]; // すべて訪問済み
        let result = solver.find_unvisited_neighbor(0, &visited_all);
        assert_eq!(result, None);
    }

    #[test]
    fn test_find_empty_edge_path_empty_graph() {
        let solver = MySolver::new(3);
        let path = solver.find_empty_edge_path(0);
        assert_eq!(path, Vec::<u8>::new());
    }

    #[test]
    fn test_find_empty_edge_path_invalid_start_node() {
        let mut solver = MySolver::new(3);
        let node = KnownNode::new(0, 1, vec![]);
        solver.nodes.push(node);
        
        let path = solver.find_empty_edge_path(10); // 存在しないノード
        assert_eq!(path, Vec::<u8>::new());
    }

    #[test]
    fn test_find_empty_edge_path_start_node_has_empty_edge() {
        let mut solver = MySolver::new(3);
        
        // ノード0を作成し、エッジ1だけを接続、他は未確定
        let mut node0 = KnownNode::new(0, 1, vec![]);
        node0.edges[1] = Some(KnownNodeConnection {
            node_id: 1,
        });
        // エッジ0,2,3,4,5は未確定（None）
        
        let node1 = KnownNode::new(1, 2, vec![1]);
        
        solver.nodes.push(node0);
        solver.nodes.push(node1);
        
        let path = solver.find_empty_edge_path(0);
        assert_eq!(path, vec![0u8]); // 最初の未確定エッジ（ドア0）に直接アクセス
    }

    #[test]
    fn test_find_empty_edge_path_multi_hop() {
        let mut solver = MySolver::new(3);
        
        // グラフ構造: 0 --[door0]--> 1 --[door2]--> 2
        // ノード2にのみ未確定エッジがある
        let mut node0 = KnownNode::new(0, 1, vec![]);
        node0.edges[0] = Some(KnownNodeConnection {
            node_id: 1,
        });
        // 他のエッジは全て確定済みと仮定
        for i in 1..6 {
            node0.edges[i] = Some(KnownNodeConnection {
                node_id: 0, // 自分自身への循環
            });
        }
        
        let mut node1 = KnownNode::new(1, 2, vec![0]);
        node1.edges[1] = Some(KnownNodeConnection {
            node_id: 0,
        });
        node1.edges[2] = Some(KnownNodeConnection {
            node_id: 2,
        });
        // 他のエッジは確定済み
        for i in [0, 3, 4, 5] {
            node1.edges[i] = Some(KnownNodeConnection {
                node_id: 1, // 自分自身への循環
            });
        }
        
        let mut node2 = KnownNode::new(2, 3, vec![0, 2]);
        node2.edges[3] = Some(KnownNodeConnection {
            node_id: 1,
        });
        // エッジ0,1,2,4,5は未確定（None）
        
        solver.nodes.push(node0);
        solver.nodes.push(node1);
        solver.nodes.push(node2);
        
        let path = solver.find_empty_edge_path(0);
        assert_eq!(path, vec![0u8, 2u8, 0u8]); // 0->1->2, そして2のドア0が未確定
    }

    #[test]
    fn test_find_empty_edge_path_no_empty_edges() {
        let mut solver = MySolver::new(2);
        
        // すべてのエッジが確定済みのグラフ
        let mut node0 = KnownNode::new(0, 1, vec![]);
        let mut node1 = KnownNode::new(1, 2, vec![0]);
        
        // すべてのエッジを確定済みにする
        for i in 0..6 {
            node0.edges[i] = Some(KnownNodeConnection {
                node_id: 1,
            });
            node1.edges[i] = Some(KnownNodeConnection {
                node_id: 0,
            });
        }
        
        solver.nodes.push(node0);
        solver.nodes.push(node1);
        
        let path = solver.find_empty_edge_path(0);
        assert_eq!(path, Vec::<u8>::new()); // 未確定エッジがないので空
    }
}
