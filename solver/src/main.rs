mod api;
mod api_trait;
mod graph;
mod mock_api;
mod random_test;
mod solver;
mod test;

use anyhow::Result;
use api::ApiClient;
use api_trait::ApiClientTrait;
use clap::Parser;
use mock_api::MockApiClient;
use solver::Solver;
use std::sync::Arc;

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
    #[arg(
        long,
        default_value = "https://31pwr5t6ij.execute-api.eu-west-2.amazonaws.com"
    )]
    base_url: String,

    /// Random walk length for equivalence checking (defaults to 18 * problem_size)
    #[arg(long)]
    walk_length: Option<usize>,

    /// Maximum number of random walk attempts for equivalence checking
    #[arg(long, default_value = "3")]
    max_tries: usize,

    /// Use mock API instead of real API
    #[arg(long)]
    mock: bool,
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
            println!(
                "Warning: Unknown problem '{}', using default size of 10",
                problem_name
            );
            10
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Get team ID from args or environment variable
    let team_id = args.team_id.unwrap_or_else(|| {
        std::env::var("TEAM_ID")
            .expect("TEAM_ID not provided via --team-id or TEAM_ID environment variable")
    });

    println!("=== ICFPC 2025 Solver ===");
    println!("Team ID: {}", team_id);
    println!("Problem: {}", args.problem);
    println!("Base URL: {}", args.base_url);
    println!();

    // Create API client (mock or real based on flag)
    let api: Arc<dyn ApiClientTrait> = if args.mock {
        println!("Using MOCK API client");
        Arc::new(MockApiClient::new_with_problem(&args.problem))
    } else {
        println!("Using REAL API client");
        Arc::new(ApiClient::new(args.base_url.clone(), team_id.clone()))
    };

    // Select the problem
    println!("Selecting problem: {}", args.problem);
    api.select_problem(&args.problem).await?;

    // Get problem size
    let problem_size = get_problem_size(&args.problem);
    println!("Problem size: {} iterations", problem_size);

    // Calculate walk length as 18n (maximum allowed) or use provided value
    let walk_length = args.walk_length.unwrap_or(18 * problem_size);
    println!(
        "Random walk length: {} (max: 18 * {} = {})",
        walk_length,
        problem_size,
        18 * problem_size
    );

    // Create solver with calculated walk length and max_tries
    let mut solver = Solver::new_with_max_tries(api, walk_length, args.max_tries);

    // Run exploration
    println!("\n=== Starting Exploration ===\n");
    solver.explore(problem_size).await?;

    // Skip return door discovery - not needed for correct solution
    // solver.discover_return_doors().await?;

    // Output the graph
    solver.output_graph();

    // Export map for submission
    let submission_map = solver.get_submission_map();
    println!("\n=== Submission Map ===");
    println!("{}", serde_json::to_string_pretty(&submission_map)?);

    // Only submit if not using mock
    if !args.mock {
        println!("\n=== Submitting Solution ===");
        submit_solution(&args.base_url, &team_id, submission_map).await?;
    } else {
        println!("\n=== Mock mode: Checking solution ===");
        // Check solution with mock API
        let mock_api = MockApiClient::new_with_problem(&args.problem);
        match mock_api.check_solution(&submission_map) {
            Ok(correct) => {
                if correct {
                    println!("✅ Solution is CORRECT!");
                } else {
                    println!("❌ Solution is INCORRECT");
                }
            }
            Err(e) => {
                println!("Error checking solution: {}", e);
            }
        }
    }

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

    let response = client.post(&url).json(&request).send().await?;

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
