mod api;
mod de_bruijn;
mod guess_map;

use anyhow::{Context, Result};
use api::{
    ApiClient, Connection as ApiConnection, ExploreResponse, GuessMap as ApiGuessMap,
    GuessResponse, RoomDoor,
};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::signal;
use tokio::sync::Mutex;

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

#[derive(Subcommand)]
enum Commands {
    /// Select a problem and start a session
    Select {
        /// Problem name to select
        problem_name: String,
        /// User name (optional)
        #[arg(long)]
        user_name: Option<String>,
    },
}

#[derive(Serialize)]
struct SelectRequest {
    #[serde(rename = "problemName")]
    problem_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_name: Option<String>,
}

#[derive(Deserialize, Debug)]
struct SelectResponse {
    session_id: String,
    #[serde(rename = "problemName")]
    problem_name: String,
}

pub struct SessionGuard {
    api_client: ApiClient,
    session_id: String,
    should_abort: Arc<AtomicBool>,
}

impl SessionGuard {
    fn new(api_client: ApiClient, session_id: String) -> Self {
        Self {
            api_client,
            session_id,
            should_abort: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn mark_success(&self) {
        self.should_abort.store(false, Ordering::Relaxed);
    }

    pub async fn explore(&self, plans: &[String]) -> Result<ExploreResponse> {
        self.api_client.explore(&self.session_id, plans).await
    }

    pub async fn guess(&self, guess_map: ApiGuessMap) -> Result<GuessResponse> {
        self.api_client.guess(&self.session_id, guess_map).await
    }
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        if self.should_abort.load(Ordering::Relaxed) {
            let session_id = self.session_id.clone();
            let api_client = self.api_client.clone();

            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    if let Err(e) = api_client.abort_session(&session_id).await {
                        eprintln!(
                            "Warning: Failed to abort session {} during drop: {:#}",
                            session_id, e
                        );
                    } else {
                        println!("Session {} aborted successfully during drop", session_id);
                    }
                });
            });
        }
    }
}

struct SessionManager {
    api_client: ApiClient,
    current_session: Arc<Mutex<Option<String>>>,
}

impl SessionManager {
    fn new(api_client: ApiClient) -> Self {
        Self {
            api_client,
            current_session: Arc::new(Mutex::new(None)),
        }
    }

    async fn start_session(
        &self,
        problem_name: String,
        user_name: Option<String>,
    ) -> Result<SelectResponse> {
        let response = self
            .api_client
            .select(problem_name.clone(), user_name.clone())
            .await
            .with_context(|| format!("Failed to start session for problem '{}'", problem_name))?;

        let mut session = self.current_session.lock().await;
        *session = Some(response.session_id.clone());

        Ok(response)
    }

    async fn start_session_with_guard(
        &self,
        problem_name: String,
        user_name: Option<String>,
    ) -> Result<SessionGuard> {
        let response = self
            .api_client
            .select(problem_name.clone(), user_name.clone())
            .await
            .with_context(|| format!("Failed to start session for problem '{}'", problem_name))?;

        let mut session = self.current_session.lock().await;
        *session = Some(response.session_id.clone());

        Ok(SessionGuard::new(
            self.api_client.clone(),
            response.session_id,
        ))
    }

    async fn abort_current_session(&self) -> Result<()> {
        let session = self.current_session.lock().await;
        if let Some(ref session_id) = *session {
            if let Err(e) = self.api_client.abort_session(session_id).await {
                eprintln!("Warning: Failed to abort session {}: {:#}", session_id, e);
            } else {
                println!("Session {} aborted successfully", session_id);
            }
        }
        Ok(())
    }
}

fn convert_guess_map(guess_map: guess_map::GuessMap) -> ApiGuessMap {
    ApiGuessMap {
        rooms: guess_map.rooms.into_iter().map(|r| r as i32).collect(),
        starting_room: guess_map.starting_room as i32,
        connections: guess_map
            .connections
            .into_iter()
            .map(|conn| ApiConnection {
                from: RoomDoor {
                    room: conn.from.room as i32,
                    door: conn.from.door as i32,
                },
                to: RoomDoor {
                    room: conn.to.room as i32,
                    door: conn.to.door as i32,
                },
            })
            .collect(),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
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

    println!("Session started successfully!");
    println!("Session ID: {}", session_guard.session_id());
    println!("Problem: {}", cli.problem_name);

    let room_num = cli.room_num.unwrap_or(6);
    println!("Using room_num: {}", room_num);

    println!("Generating de Bruijn sequence for n={}...", room_num);
    let planner_config = de_bruijn::config_for_rooms(room_num);
    let de_bruijn_seq = de_bruijn::generate_explore_plans(&planner_config);

    println!("Sending explore request...");
    let explore_response = session_guard.explore(&de_bruijn_seq).await?;
    println!("Explore response: {:?}", explore_response);

    let suffixes = vec!["0000100020003000".to_string()];
    let guess_map =
        guess_map::build_map_fixed_tail(&de_bruijn_seq, &explore_response.results, &suffixes)?;
    println!("Generated guess map: {:?}", guess_map);

    println!("Sending guess request...");
    let guess_response = session_guard.guess(convert_guess_map(guess_map)).await?;
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
