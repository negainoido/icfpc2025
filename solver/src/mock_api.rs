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
        Self {
            problem_name: String::new(),
            query_count: 0,
            graph: MockGraph::new(),
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
        
        // The correct solution for primus has 6 unique rooms
        let expected_room_count = 6;
        
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
        let paths = HashMap::new();
        let mut room_labels = HashMap::new();
        let mut connections = HashMap::new();

        // Define the primus problem structure with 6 unique vertices
        // Each room has a unique label (0-5)
        
        // Room labels - 6 unique rooms
        room_labels.insert(0, 0); // Starting room
        room_labels.insert(1, 1); 
        room_labels.insert(2, 2);
        room_labels.insert(3, 3);
        room_labels.insert(4, 4);
        room_labels.insert(5, 5);
        
        // Starting room (0) connections
        connections.insert((0, 0), 1); // door 0 -> room 1
        connections.insert((0, 1), 2); // door 1 -> room 2
        connections.insert((0, 2), 3); // door 2 -> room 3
        connections.insert((0, 3), 4); // door 3 -> room 4
        connections.insert((0, 4), 5); // door 4 -> room 5
        connections.insert((0, 5), 0); // door 5 -> self loop
        
        // Room 1 connections
        connections.insert((1, 0), 0); // back to start
        connections.insert((1, 1), 2); // to room 2
        connections.insert((1, 2), 3); // to room 3
        connections.insert((1, 3), 4); // to room 4
        connections.insert((1, 4), 5); // to room 5
        connections.insert((1, 5), 1); // self loop
        
        // Room 2 connections
        connections.insert((2, 0), 0); // to room 0
        connections.insert((2, 1), 1); // to room 1
        connections.insert((2, 2), 3); // to room 3
        connections.insert((2, 3), 4); // to room 4
        connections.insert((2, 4), 5); // to room 5
        connections.insert((2, 5), 2); // self loop
        
        // Room 3 connections
        connections.insert((3, 0), 0); // to room 0
        connections.insert((3, 1), 1); // to room 1
        connections.insert((3, 2), 2); // to room 2
        connections.insert((3, 3), 4); // to room 4
        connections.insert((3, 4), 5); // to room 5
        connections.insert((3, 5), 3); // self loop
        
        // Room 4 connections
        connections.insert((4, 0), 0); // to room 0
        connections.insert((4, 1), 1); // to room 1
        connections.insert((4, 2), 2); // to room 2
        connections.insert((4, 3), 3); // to room 3
        connections.insert((4, 4), 5); // to room 5
        connections.insert((4, 5), 4); // self loop
        
        // Room 5 connections
        connections.insert((5, 0), 0); // to room 0
        connections.insert((5, 1), 1); // to room 1
        connections.insert((5, 2), 2); // to room 2
        connections.insert((5, 3), 3); // to room 3
        connections.insert((5, 4), 4); // to room 4
        connections.insert((5, 5), 5); // self loop
        
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