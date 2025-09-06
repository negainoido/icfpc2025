mod api;

use anyhow::{Result, Context};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use api::ApiClient;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::signal;

#[derive(Parser)]
#[command(name = "garasubo-solver")]
#[command(about = "ICFPC 2025 Problem Solver")]
struct Cli {
    problem_name: String,

    #[arg(long)]
    user_name: Option<String>,

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

    async fn start_session(&self, problem_name: String, user_name: Option<String>) -> Result<SelectResponse> {
        let response = self.api_client.select(problem_name.clone(), user_name.clone()).await
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
                    eprintln!("Warning: Failed to abort session {} on Ctrl+C: {:#}", session_id, e);
                } else {
                    println!("Session {} aborted successfully on Ctrl+C", session_id);
                }
            }
            std::process::exit(130); // Exit with SIGINT status
        }
    });

    match session_manager.start_session(cli.problem_name, cli.user_name).await {
        Ok(response) => {
            println!("Session started successfully!");
            println!("Session ID: {}", response.session_id);
            println!("Problem: {}", response.problem_name);

            // TODO: /explore の計画を建てる

            // TODO: /explore を叩く

            // TODO: /exploreの結果からマップの構築

            // TODO: /guess を叩く

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
