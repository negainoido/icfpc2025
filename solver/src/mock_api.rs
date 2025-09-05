use anyhow::Result;
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use crate::api_trait::ApiClientTrait;
use serde_json;

#[derive(Debug, Clone)]
pub struct MockApiClient {
    problem_name: String,
    query_count: u32,
    graph: MockGraph,
}

#[derive(Debug, Clone)]
struct MockGraph {
    // Maps path to the actual room it leads to
    paths: HashMap<String, usize>,
    // Maps room to its label
    room_labels: HashMap<usize, u8>,
    // Maps (room, door) to destination room
    connections: HashMap<(usize, usize), usize>,
}

impl MockApiClient {
    pub fn new() -> Self {
        Self::new_with_problem("primus")
    }
    
    pub fn new_with_problem(problem: &str) -> Self {
        Self {
            problem_name: problem.to_string(),
            query_count: 0,
            graph: MockGraph::new_for_problem(problem),
        }
    }

    pub fn check_solution(&self, submission: &serde_json::Value) -> Result<bool> {
        // Extract rooms and connections from submission
        let submitted_rooms = submission["rooms"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("Invalid submission: missing rooms"))?;
        
        let submitted_connections = submission["connections"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("Invalid submission: missing connections"))?;
        
        let starting_room = submission["startingRoom"]
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("Invalid submission: missing startingRoom"))? as usize;
        
        // Expected room count based on problem
        let expected_room_count = match self.problem_name.as_str() {
            "probatio" => 3,
            "primus" => 6,
            "secundus" => 12,
            "tertius" => 18,
            "quartus" => 24,
            "quintus" => 30,
            _ => 6,
        };
        
        if submitted_rooms.len() != expected_room_count {
            println!("[MOCK] Solution verification FAILED: Expected {} rooms, got {}", 
                     expected_room_count, submitted_rooms.len());
            return Ok(false);
        }
        
        // Build the submitted graph structure
        let mut submitted_graph: HashMap<(usize, usize), usize> = HashMap::new();
        
        for conn in submitted_connections {
            let from_room = conn["from"]["room"].as_u64().unwrap() as usize;
            let from_door = conn["from"]["door"].as_u64().unwrap() as usize;
            let to_room = conn["to"]["room"].as_u64().unwrap() as usize;
            
            submitted_graph.insert((from_room, from_door), to_room);
        }
        
        // Build submitted room labels
        let mut submitted_labels = HashMap::new();
        for (idx, label) in submitted_rooms.iter().enumerate() {
            submitted_labels.insert(idx, label.as_u64().unwrap() as u8);
        }
        
        // Check if the graphs are topologically equivalent
        // Test random paths to verify they produce the same label sequences
        println!("[MOCK] Checking solution by comparing random paths...");
        
        use rand::Rng;
        let mut rng = rand::thread_rng();
        
        // Test with many random paths of various lengths
        let num_tests = 100;
        let max_path_length = 20;
        
        for test_num in 0..num_tests {
            // Generate a random path
            let path_length = rng.gen_range(0..=max_path_length);
            let mut path = String::new();
            for _ in 0..path_length {
                let door = rng.gen_range(0..6);
                path.push_str(&door.to_string());
            }
            
            // Show progress for longer tests
            if test_num % 20 == 0 {
                println!("  Testing path {}/{} (length {}): '{}'", 
                         test_num + 1, num_tests, path.len(), 
                         if path.len() > 10 { &path[..10] } else { &path });
            }
            // Execute path on original graph
            let original_labels = self.execute_path(&path);
            
            // Execute path on submitted graph
            let mut submitted_result = Vec::new();
            let mut current_room = starting_room;
            submitted_result.push(submitted_labels[&current_room]);
            
            for c in path.chars() {
                if let Some(door) = c.to_digit(10) {
                    let door = door as usize;
                    if let Some(&next_room) = submitted_graph.get(&(current_room, door)) {
                        current_room = next_room;
                        submitted_result.push(submitted_labels[&current_room]);
                    } else {
                        println!("[MOCK] Solution verification FAILED: Missing connection for room {} door {}", 
                                 current_room, door);
                        return Ok(false);
                    }
                }
            }
            
            if original_labels != submitted_result {
                println!("[MOCK] Solution verification FAILED: Path '{}' produces different labels", path);
                println!("  Expected: {:?}", original_labels);
                println!("  Got:      {:?}", submitted_result);
                return Ok(false);
            }
        }
        
        println!("[MOCK] Solution verification PASSED: All tested paths match!");
        Ok(true)
    }

    fn execute_path(&self, path: &str) -> Vec<u8> {
        let mut result = Vec::new();
        let mut current_room = 0; // Always start from room 0
        
        // Record starting room label
        result.push(self.graph.room_labels[&current_room]);
        
        // Follow the path
        for c in path.chars() {
            if let Some(door) = c.to_digit(10) {
                let door = door as usize;
                if let Some(&next_room) = self.graph.connections.get(&(current_room, door)) {
                    current_room = next_room;
                    result.push(self.graph.room_labels[&current_room]);
                } else {
                    // Invalid door - shouldn't happen in mock
                    break;
                }
            }
        }
        
        result
    }
}

impl MockGraph {
    fn new() -> Self {
        Self::new_for_problem("primus")
    }
    
    fn new_for_problem(problem: &str) -> Self {
        use rand::{Rng, SeedableRng};
        // Use a fixed seed for consistent graph generation
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        
        let paths = HashMap::new();
        let mut room_labels = HashMap::new();
        let mut connections = HashMap::new();

        let num_rooms = match problem {
            "probatio" => 3,
            "primus" => 6,
            "secundus" => 12,
            "tertius" => 18,
            "quartus" => 24,
            "quintus" => 30,
            _ => 6, // Default to 6
        };
        
        // Generate room labels
        // For probatio: 3 rooms with unique labels (0-2)
        // For primus: 6 rooms with unique labels (0-5)
        // For larger problems: use 4 labels (0,1,2,3) cycling
        let num_labels = match num_rooms {
            3 => 3,   // probatio - all unique
            6 => 6,   // primus - all unique
            _ => 4,   // larger problems use 4 labels
        };
        
        for i in 0..num_rooms {
            let label = if num_rooms <= 6 {
                i as u8  // Unique labels for small problems
            } else {
                (i % num_labels) as u8  // Cycle through labels for larger problems
            };
            room_labels.insert(i, label);
        }
        
        // Generate connections for each room's 6 doors
        if num_rooms == 12 {
            // For secundus, create a specific structure that ensures all 12 rooms are distinct
            // Create 3 groups of 4 rooms (one for each label set)
            for room in 0..num_rooms {
                for door in 0..6 {
                    // Create a structured pattern that ensures distinctness
                    let target = match door {
                        0 => (room + 1) % num_rooms,  // Next room in sequence
                        1 => (room + num_rooms - 1) % num_rooms,  // Previous room
                        2 => (room + 3) % num_rooms,  // Jump by 3
                        3 => (room + 5) % num_rooms,  // Jump by 5
                        4 => (room + 7) % num_rooms,  // Jump by 7
                        5 => room,  // Self loop
                        _ => 0,
                    };
                    connections.insert((room, door), target);
                }
            }
        } else {
            // For other sizes, use random generation
            for room in 0..num_rooms {
                // Ensure each room has at least one outgoing connection to the next room
                let next_room = (room + 1) % num_rooms;
                connections.insert((room, 0), next_room);
                
                // Random connections for other doors
                for door in 1..6 {
                    let target = rng.gen_range(0..num_rooms);
                    connections.insert((room, door), target);
                }
            }
        }
        
        Self {
            paths,
            room_labels,
            connections,
        }
    }
}

#[async_trait]
impl ApiClientTrait for MockApiClient {
    async fn select_problem(&self, problem_name: &str) -> Result<()> {
        println!("[MOCK] Selected problem: {}", problem_name);
        Ok(())
    }

    async fn explore(&self, plans: Vec<String>) -> Result<(Vec<Vec<u8>>, u32)> {
        println!("[MOCK] Calling explore with {} plans:", plans.len());
        for (i, plan) in plans.iter().enumerate() {
            println!("  Plan {}: '{}'", i + 1, plan);
        }
        
        let mut results = Vec::new();
        
        for plan in &plans {
            let path_result = self.execute_path(plan);
            println!("  Result for '{}': {:?}", plan, path_result);
            results.push(path_result);
        }
        
        let query_count = self.query_count + results.len() as u32;
        println!("[MOCK] Response: {} results, total query count: {}", 
                 results.len(), query_count);
        
        Ok((results, query_count))
    }
}