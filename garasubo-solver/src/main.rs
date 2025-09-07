mod api;
mod de_bruijn;
mod guess_map;

use anyhow::{Context, Result};
use api::{ApiClient, Connection as ApiConnection, GuessMap as ApiGuessMap, RoomDoor};
use clap::{Parser, Subcommand};
use garasubo_solver::session_manager::SessionManager;
use serde::{Deserialize, Serialize};
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
    let guess_map = guess_map::build_map_fixed_tail(
        &de_bruijn_seq,
        explore_response.results.as_slice(),
        &suffixes,
    )?;
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
