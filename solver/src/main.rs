mod api;
mod graph;
mod solver;
mod test;
mod random_test;

use anyhow::Result;
use clap::Parser;
use solver::Solver;
use api::ApiClient;

#[derive(Parser, Debug)]
#[command(name = "ICFPC 2025 Solver")]
#[command(about = "Solver for ICFPC 2025 library exploration problem", long_about = None)]
struct Args {
    /// Team ID (defaults to TEAM_ID environment variable if not provided)
    #[arg(short, long)]
    team_id: Option<String>,

    /// Problem name
    #[arg(short, long)]
    problem: String,

    /// Base URL for the API
    #[arg(long, default_value = "https://31pwr5t6ij.execute-api.eu-west-2.amazonaws.com")]
    base_url: String,


    /// Random walk length for equivalence checking
    #[arg(long, default_value = "10")]
    walk_length: usize,
}

// Problem size configuration as per the issue
fn get_problem_size(problem_name: &str) -> usize {
    match problem_name {
        "probatio" => 3,
        "primus" => 6,
        "secundus" => 12,
        "tertius" => 18,
        "quartus" => 24,
        "quintus" => 30,
        _ => {
            println!("Warning: Unknown problem '{}', using default size of 10", problem_name);
            10
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Get team ID from args or environment variable
    let team_id = args.team_id.unwrap_or_else(|| {
        std::env::var("TEAM_ID").expect("TEAM_ID not provided via --team-id or TEAM_ID environment variable")
    });

    println!("=== ICFPC 2025 Solver ===");
    println!("Team ID: {}", team_id);
    println!("Problem: {}", args.problem);
    println!("Base URL: {}", args.base_url);
    println!();

    // Create API client
    let api = ApiClient::new(args.base_url.clone(), team_id.clone());

    // Select the problem
    println!("Selecting problem: {}", args.problem);
    api.select_problem(&args.problem).await?;

    // Create solver
    let mut solver = Solver::new(api, args.walk_length);

    // Get problem size
    let problem_size = get_problem_size(&args.problem);
    println!("Problem size: {} iterations", problem_size);

    // Run exploration
    println!("\n=== Starting Exploration ===\n");
    solver.explore(problem_size).await?;
    
    // Discover return doors
    solver.discover_return_doors().await?;

    // Output the graph
    solver.output_graph();

    // Export map for submission
    let submission_map = solver.get_submission_map();
    println!("\n=== Submission Map ===");
    println!("{}", serde_json::to_string_pretty(&submission_map)?);

    println!("\n=== Submitting Solution ===");
    submit_solution(&args.base_url, &team_id, submission_map).await?;

    Ok(())
}

async fn submit_solution(base_url: &str, team_id: &str, map: serde_json::Value) -> Result<()> {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize)]
    struct GuessRequest {
        id: String,
        map: serde_json::Value,
    }

    #[derive(Deserialize)]
    struct GuessResponse {
        correct: Option<bool>,
        error: Option<String>,
    }

    let client = reqwest::Client::new();
    let url = format!("{}/guess", base_url);
    
    let request = GuessRequest {
        id: team_id.to_string(),
        map,
    };

    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "Submit failed with status {}: {}",
            status,
            text
        ));
    }

    let guess_response: GuessResponse = response.json().await?;
    
    if let Some(error) = guess_response.error {
        println!("Submission error: {}", error);
        return Err(anyhow::anyhow!("Submission error: {}", error));
    }

    if let Some(correct) = guess_response.correct {
        if correct {
            println!("✅ Solution is CORRECT!");
        } else {
            println!("❌ Solution is INCORRECT");
        }
    }

    Ok(())
}