mod api;
mod de_bruijn;

use anyhow::{Context, Result};
use api::ApiClient;
use clap::{Parser, Subcommand};
use de_bruijn::generate_de_bruijn_sequence;
use serde::{Deserialize, Serialize};
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
    room_num: Option<u64>,

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

    match session_manager
        .start_session(cli.problem_name, cli.user_name)
        .await
    {
        Ok(response) => {
            println!("Session started successfully!");
            println!("Session ID: {}", response.session_id);
            println!("Problem: {}", response.problem_name);

            let room_num = cli.room_num.unwrap_or(6);
            println!("Using room_num: {}", room_num);

            println!("Generating de Bruijn sequence for n={}...", room_num);
            let de_bruijn_seq = generate_de_bruijn_sequence(room_num as usize);
            println!(
                "Generated de Bruijn sequence (length {}): {}",
                de_bruijn_seq.len(),
                de_bruijn_seq
            );

            println!("Sending explore request...");
            match session_manager
                .api_client
                .explore(&response.session_id, vec![de_bruijn_seq])
                .await
            {
                Ok(explore_response) => {
                    println!("Explore response: {:?}", explore_response);
                }
                Err(e) => {
                    eprintln!("Failed to explore: {:#}", e);
                    if let Err(abort_error) = session_manager.abort_current_session().await {
                        eprintln!("Additional error during cleanup: {:#}", abort_error);
                    }
                    std::process::exit(1);
                }
            }

            // TODO: /exploreの結果からマップの構築

            // TODO: /guess を叩く

            // 本来は/guessを叩けば自動的にsessionは終了するが、今はそこまで実装できていないのでsessionをabortさせておく
            session_manager.abort_current_session().await?;

            println!("Work completed successfully");
        }
        Err(e) => {
            eprintln!("Fatal error during session startup: {:#}", e);
            if let Err(abort_error) = session_manager.abort_current_session().await {
                eprintln!("Additional error during cleanup: {:#}", abort_error);
            }
            std::process::exit(1);
        }
    }

    Ok(())
}
