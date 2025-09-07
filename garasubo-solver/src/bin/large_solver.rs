use clap::Parser;
use garasubo_solver::api::{ApiClient, GuessMap};
use garasubo_solver::chatgpt_solver::{Config, InteractiveSolver};
use garasubo_solver::session_manager::SessionManager;
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

    let mut config = Config::default();
    config.max_rooms = cli.room_num;
    let mut solver = InteractiveSolver::new(config);
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
