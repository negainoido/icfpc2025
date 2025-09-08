use clap::Parser;
use garasubo_solver::api::{ApiClient, Connection};
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
    // 対応する反対方向のedge
    reverse_edge: Option<usize>,
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
    // ただのランダムウォーク
    Walk(Vec<u8>),
    // 炭を使ったマーキングにより部屋を識別するwalk
    MarkedWalk {
        plan: Vec<Action>,
        rewrite_target: HashSet<usize>,
        state_idx: usize,
    },
}

impl Plan {
    fn to_query_string(&self) -> String {
        match self {
            Plan::Walk(walk) => walk.iter().map(|i| ('0' as u8 + *i) as char).collect::<String>(),
            Plan::MarkedWalk { plan_output, .. } => plan_output.plan.clone(),
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
}

struct State {
    walk: Vec<u8>,
    y: Vec<u8>,
    // 訪れたノードでknownになったものの集合
    known_nodes: HashMap<usize, usize>
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
        }
    }

    fn initial_plan(&mut self) -> Vec<String> {
        let walk = generate_cover_walk(self.size);

        let plans = vec![ walk.iter().map(|i| ('0' as u8 + *i) as char).collect::<String>() ];

        self.prev_query = vec![ Plan::Walk(walk) ];

        plans
    }

    fn next_plan(&mut self, results: Vec<Vec<u8>>) -> Vec<String> {
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
                    // 最初に登場したラベルのノードをメモ
                    let mut memo = vec![None; 4];
                    let mut rewrite_target= HashSet::new();
                    let mut known_nodes = HashMap::new();
                    for (i, &c) in y.iter().enumerate() {
                        if memo[c as usize] == None {
                            memo[c as usize] = Some(i);
                            rewrite_target.insert(i);
                        }
                    }
                    // known nodeとして追加
                    for (i, &v) in memo.iter().enumerate() {
                        if let Some(v) = v {
                            let label = i as u8;
                            let path = walk.iter().take(v).copied().collect::<Vec<_>>() ;
                            let node = KnownNode::new(i, label, path);
                            self.nodes.push(node);
                            known_nodes.insert(i, self.nodes.len()-1);
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
                    });


                    result_idx += 1;
                }
                Plan::MarkedWalk { plan, rewrite_target, state_idx } => {
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
                                    if let Some(idx) = rewrite_memo.get(&y[y_idx]) {
                                        let known_node_id = state.known_nodes[idx];
                                        state.known_nodes.insert(*idx, known_node_id);
                                    }
                                }
                                y_idx += 1;
                            }
                            Action::Mark(x) => {
                                let label = y[y_idx+1];
                                rewrite_memo.insert(label, y_idx);
                                // y_idxは更新しない
                            }
                        }
                    }
                    // 次の計画を建てる
                    let mut new_target = HashMap::new();
                    for (i, label) in y.iter().enumerate() {
                        if state.known_nodes.contains_key(&i) || new_target.contains_key(label) {
                            continue;
                        }
                        new_target.insert(*label, i);
                    }
                    let new_rewrite_target = new_target.values().copied().collect::<HashSet<_>>();

                    if new_target.is_empty() {
                        // もう未知のノードがないならランダムウォーク
                        // まずはグラフを構築
                        for (i, &c) in y.iter().enumerate() {
                            let w = state.walk[i] as usize;
                            let dest_id = self.nodes[state.known_nodes[&(i+1)]].id;
                            let node = &mut self.nodes[state.known_nodes[&i]];
                            match &node.edges[w] {
                                Some(edge) => {
                                    assert_eq!(edge.node_id, dest_id);
                                },
                                None => {
                                    node.edges[w] = Some(KnownNodeConnection {
                                        node_id: dest_id,
                                        reverse_edge: None,
                                    });
                                }
                            }
                        }

                    } else {
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

    fn plan_to_string(&self, plan: &Vec<Action>) -> String {
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
    let initial_result = session_guard.explore(&initial_plan).await?;
    loop {}

    let guess_response = session_guard.guess(todo!()).await?;
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
