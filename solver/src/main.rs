mod graph;
mod simple_graph;
mod probatio_solver;
mod three_room;
mod complete_three;

use anyhow::Result;
use clap::{Parser, Subcommand};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use std::collections::HashMap;

use crate::graph::SmartExplorer;

const DOOR_COUNT: usize = 6;
const MAX_INITIAL_PLANS: usize = 30;
const MAX_ITERATIONS: usize = 50;
const DEFAULT_BATCH_SIZE: usize = 25;

const API_URL: &str = "https://31pwr5t6ij.execute-api.eu-west-2.amazonaws.com";

#[derive(Debug, Serialize, Deserialize)]
struct RegisterRequest {
    name: String,
    pl: String,
    email: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct RegisterResponse {
    id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SelectRequest {
    id: String,
    #[serde(rename = "problemName")]
    problem_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SelectResponse {
    #[serde(rename = "problemName")]
    problem_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExploreRequest {
    id: String,
    plans: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExploreResponse {
    results: Vec<Vec<i32>>,
    #[serde(rename = "queryCount")]
    query_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Connection {
    from: DoorRef,
    to: DoorRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DoorRef {
    room: usize,
    door: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct Map {
    rooms: Vec<i32>,
    #[serde(rename = "startingRoom")]
    starting_room: usize,
    connections: Vec<Connection>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GuessRequest {
    id: String,
    map: Map,
}

#[derive(Debug, Serialize, Deserialize)]
struct GuessResponse {
    correct: bool,
}

#[derive(Parser)]
#[command(name = "icfpc_solver")]
#[command(about = "Solver for ICFPC 2025 Labyrinthine Library problem")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Register {
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "Rust")]
        pl: String,
        #[arg(long)]
        email: String,
    },
    Solve {
        #[arg(long)]
        id: String,
        #[arg(long, default_value = "probatio")]
        problem: String,
        #[arg(long, default_value_t = 100)]
        max_queries: usize,
    },
    Test {
        #[arg(long)]
        id: String,
    },
}

struct LibrarySolver {
    client: Client,
    team_id: String,
}

impl LibrarySolver {
    fn new(team_id: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        
        Self { client, team_id }
    }
    
    fn select_problem(&self, problem_name: &str) -> Result<String> {
        let request = SelectRequest {
            id: self.team_id.clone(),
            problem_name: problem_name.to_string(),
        };
        
        let response = self
            .client
            .post(format!("{}/select", API_URL))
            .json(&request)
            .send()?
            .json::<SelectResponse>()?;
        
        println!("Selected problem: {}", response.problem_name);
        Ok(response.problem_name)
    }
    
    fn explore(&self, plans: Vec<String>) -> Result<ExploreResponse> {
        let request = ExploreRequest {
            id: self.team_id.clone(),
            plans,
        };
        
        let response = self
            .client
            .post(format!("{}/explore", API_URL))
            .json(&request)
            .send()?
            .json::<ExploreResponse>()?;
        
        Ok(response)
    }
    
    fn submit_guess(&self, map: Map) -> Result<bool> {
        let request = GuessRequest {
            id: self.team_id.clone(),
            map,
        };
        
        let response = self
            .client
            .post(format!("{}/guess", API_URL))
            .json(&request)
            .send()?;
        
        let text = response.text()?;
        println!("API Response: {}", text);
        
        match serde_json::from_str::<GuessResponse>(&text) {
            Ok(resp) => Ok(resp.correct),
            Err(e) => {
                println!("Failed to parse response: {}", e);
                Err(anyhow::anyhow!("Failed to parse response: {}", e))
            }
        }
    }
    
    fn solve(&self, problem_name: &str, max_queries: usize) -> Result<()> {
        println!("Solving problem: {}", problem_name);
        self.select_problem(problem_name)?;
        
        let mut explorer = SmartExplorer::new();
        let mut all_explorations = Vec::new();
        let mut query_count = 0;
        
        // Phase 1: Initial exploration - explore all doors from starting room
        println!("Phase 1: Initial exploration");
        let mut initial_plans = vec![];
        
        // Explore each door from the starting room
        for door in 0..DOOR_COUNT {
            initial_plans.push(door.to_string());
        }
        
        // Add some depth-2 explorations
        for door1 in 0..DOOR_COUNT {
            for door2 in 0..DOOR_COUNT {
                if initial_plans.len() < MAX_INITIAL_PLANS {
                    initial_plans.push(format!("{}{}", door1, door2));
                }
            }
        }
        
        let response = self.explore(initial_plans.clone())?;
        query_count = response.query_count;
        
        for (i, labels) in response.results.iter().enumerate() {
            explorer.add_exploration(&initial_plans[i], labels);
            all_explorations.push((initial_plans[i].clone(), labels.clone()));
        }
        
        println!("Initial exploration complete. Query count: {}", query_count);
        
        // Phase 2: Smart exploration based on unexplored paths
        println!("Phase 2: Smart exploration");
        let mut iteration = 0;
        
        while query_count < max_queries as i32 && iteration < MAX_ITERATIONS {
            iteration += 1;
            
            // Get unexplored plans
            let batch_plans = explorer.get_unexplored_plans(DEFAULT_BATCH_SIZE);
            
            if batch_plans.is_empty() {
                println!("No more unexplored plans found");
                break;
            }
            
            println!("Iteration {}: Exploring {} new plans", iteration, batch_plans.len());
            
            let response = self.explore(batch_plans.clone())?;
            query_count = response.query_count;
            
            for (i, labels) in response.results.iter().enumerate() {
                explorer.add_exploration(&batch_plans[i], labels);
                all_explorations.push((batch_plans[i].clone(), labels.clone()));
            }
            
            println!("Query count: {}", query_count);
            
            // Try to reconstruct the map with simple approach first
            match simple_graph::SimpleGraph::build_from_explorations(&all_explorations) {
                Ok(map) => {
                    println!("Simple approach: Attempting to submit map with {} rooms, {} connections", map.rooms.len(), map.connections.len());
                    
                    match self.submit_guess(map) {
                        Ok(true) => {
                            println!("✓ Correct map! Total queries: {}", query_count);
                            return Ok(());
                        }
                        Ok(false) => {
                            println!("✗ Incorrect map from simple approach");
                        }
                        Err(e) => {
                            println!("Error submitting guess: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("Simple approach failed: {}", e);
                }
            }
            
            // Fallback to original approach
            match explorer.build_graph() {
                Ok(graph) => {
                    let map = graph.to_api_map();
                    println!("Original approach: Attempting to submit map with {} rooms, {} connections", map.rooms.len(), map.connections.len());
                    
                    match self.submit_guess(map) {
                        Ok(true) => {
                            println!("✓ Correct map! Total queries: {}", query_count);
                            return Ok(());
                        }
                        Ok(false) => {
                            println!("✗ Incorrect map from original approach");
                        }
                        Err(e) => {
                            println!("Error submitting guess: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("Original approach failed: {}", e);
                }
            }
            
            // Early termination if we're making too many queries without progress
            if query_count > (max_queries / 2) as i32 && iteration > 20 {
                println!("Many queries without success, trying final reconstruction");
                break;
            }
        }
        
        // Final attempt - try both approaches
        println!("Final reconstruction attempt");
        
        // Try simple approach first
        match simple_graph::SimpleGraph::build_from_explorations(&all_explorations) {
            Ok(map) => {
                println!("Simple approach final: {} rooms, {} connections", map.rooms.len(), map.connections.len());
                
                if self.submit_guess(map)? {
                    println!("✓ Correct map! Total queries: {}", query_count);
                    return Ok(());
                }
            }
            Err(e) => {
                println!("Simple approach failed: {}", e);
            }
        }
        
        // Try original approach
        match explorer.build_graph() {
            Ok(graph) => {
                let map = graph.to_api_map();
                println!("Original approach final: {} rooms, {} connections", map.rooms.len(), map.connections.len());
                
                if self.submit_guess(map)? {
                    println!("✓ Correct map! Total queries: {}", query_count);
                } else {
                    println!("✗ Failed to find correct map with either approach");
                }
            }
            Err(e) => {
                println!("Could not reconstruct map: {}", e);
            }
        }
        
        Ok(())
    }
}

fn register(name: String, pl: String, email: String) -> Result<String> {
    let client = Client::new();
    let request = RegisterRequest { name, pl, email };
    
    let response = client
        .post(format!("{}/register", API_URL))
        .json(&request)
        .send()?
        .json::<RegisterResponse>()?;
    
    println!("Registered successfully! Team ID: {}", response.id);
    Ok(response.id)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Register { name, pl, email } => {
            let id = register(name, pl, email)?;
            println!("Save this ID for future use: {}", id);
        }
        Commands::Solve { id, problem, max_queries } => {
            let solver = LibrarySolver::new(id);
            solver.solve(&problem, max_queries)?;
        }
        Commands::Test { id } => {
            println!("Running minimal test with probatio problem...");
            let solver = LibrarySolver::new(id);
            
            // Try a minimal exploration for 3-room problem
            solver.select_problem("probatio")?;
            
            // Minimal exploration: just explore each door
            let plans = vec!["0", "1", "2", "3", "4", "5", "00", "01", "10", "11"];
            let response = solver.explore(plans.iter().map(|s| s.to_string()).collect())?;
            
            println!("Minimal exploration results:");
            for (plan, labels) in plans.iter().zip(&response.results) {
                println!("  Plan '{}': {:?}", plan, labels);
            }
            
            // Try to build minimal map
            let explorations: Vec<(String, Vec<i32>)> = plans.iter()
                .zip(&response.results)
                .map(|(p, l)| (p.to_string(), l.clone()))
                .collect();
                
            // Try complete 3-room configuration first
            println!("\n--- Trying complete 3-room configuration (all doors connected) ---");
            match complete_three::solve_complete_three() {
                Ok(map) => {
                    println!("Map: {} rooms, {} connections", map.rooms.len(), map.connections.len());
                    match solver.submit_guess(map) {
                        Ok(correct) => {
                            if correct {
                                println!("✓ Success with complete 3-room!");
                                return Ok(());
                            } else {
                                println!("✗ Incorrect");
                            }
                        }
                        Err(e) => {
                            println!("Error: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("Failed: {}", e);
                }
            }
            
            // Try simple 3-room configuration
            println!("\n--- Trying simple 3-room configuration ---");
            match three_room::solve_three_room() {
                Ok(map) => {
                    println!("Map: rooms={:?}, connections={:?}", map.rooms, map.connections);
                    match solver.submit_guess(map) {
                        Ok(correct) => {
                            if correct {
                                println!("✓ Success with simple 3-room!");
                                return Ok(());
                            } else {
                                println!("✗ Incorrect");
                            }
                        }
                        Err(e) => {
                            println!("Error: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("Failed: {}", e);
                }
            }
            
            // Try probatio-specific solver
            println!("\n--- Trying probatio-specific solver ---");
            match probatio_solver::solve_probatio(&explorations) {
                Ok(map) => {
                    println!("Map: rooms={:?}, connections={:?}", map.rooms, map.connections);
                    match solver.submit_guess(map) {
                        Ok(correct) => {
                            if correct {
                                println!("✓ Success with probatio solver!");
                                return Ok(());
                            } else {
                                println!("✗ Incorrect");
                            }
                        }
                        Err(e) => {
                            println!("Error: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("Failed: {}", e);
                }
            }
        }
    }
    
    Ok(())
}