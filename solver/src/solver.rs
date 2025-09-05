use crate::api::ApiClient;
use crate::graph::Graph;
use anyhow::Result;
use std::collections::{HashSet, VecDeque};

pub struct Solver {
    api: ApiClient,
    graph: Graph,
    explored: HashSet<usize>,
    frontier: VecDeque<usize>,
    walk_length: usize,
}

impl Solver {
    pub fn new(api: ApiClient, walk_length: usize) -> Self {
        Self {
            api,
            graph: Graph::new(),
            explored: HashSet::new(),
            frontier: VecDeque::new(),
            walk_length,
        }
    }

    pub fn get_path_to_room(&self, room_id: usize) -> String {
        self.graph.path_to_room.get(&room_id).cloned().unwrap_or_default()
    }

    pub async fn gather_neighbor(&self, route: String) -> Result<Vec<u8>> {
        // Send 6 plans to explore each adjacent door (0-5)
        let mut plans = Vec::new();
        for door in 0..6 {
            let mut plan = route.clone();
            plan.push_str(&door.to_string());
            plans.push(plan);
        }

        let (results, _) = self.api.explore(plans).await?;
        
        // Extract the last label from each result (the neighbor's label)
        let neighbor_labels: Vec<u8> = results
            .iter()
            .map(|result| *result.last().unwrap_or(&0))
            .collect();

        Ok(neighbor_labels)
    }


    pub async fn are_equal(&self, v1: usize, v2: usize) -> Result<bool> {
        use rand::Rng;
        
        // Generate a single random walk sequence
        let mut rng = rand::thread_rng();
        let mut walk_sequence = String::new();
        for _ in 0..self.walk_length {
            let door = rng.gen_range(0..6);
            walk_sequence.push_str(&door.to_string());
        }
        
        // Execute the SAME walk from both vertices
        let path1 = self.get_path_to_room(v1);
        let path2 = self.get_path_to_room(v2);
        
        let plan1 = format!("{}{}", path1, walk_sequence);
        let plan2 = format!("{}{}", path2, walk_sequence);
        
        let (results, _) = self.api.explore(vec![plan1, plan2]).await?;
        
        // Compare the resulting label sequences
        Ok(results[0] == results[1])
    }

    pub async fn explore(&mut self, problem_size: usize) -> Result<()> {
        // Initialize with start vertex (already added in Graph::new())
        self.frontier.push_back(self.graph.starting_room);

        println!("Starting exploration with graph size: {}", problem_size);

        for iteration in 0..problem_size {
            println!("\n=== Iteration {}/{} ===", iteration + 1, problem_size);

            if let Some(current_room) = self.frontier.pop_front() {
                if self.explored.contains(&current_room) {
                    continue;
                }

                let current_path = self.get_path_to_room(current_room);
                println!("Exploring room {} at path: '{}'", current_room, current_path);
                
                // Gather neighbors
                let neighbor_labels = self.gather_neighbor(current_path.clone()).await?;
                println!("Found neighbors with labels: {:?}", neighbor_labels);

                // Process each neighbor
                for (door_num, &label) in neighbor_labels.iter().enumerate() {
                    let neighbor_path = format!("{}{}", current_path, door_num);
                    
                    // Check if we already have a room at this path
                    if let Some(existing_room_id) = self.graph.find_room_by_path(&neighbor_path) {
                        // Room already exists in graph, just ensure connection
                        println!("Door {} leads to existing room {} at path '{}'", door_num, existing_room_id, neighbor_path);
                        // Connection already exists from initial creation
                        continue;
                    }
                    
                    // Check if this neighbor is equal to any existing room with the same label
                    let mut found_match = false;
                    let existing_rooms_with_label: Vec<usize> = self.graph.rooms
                        .iter()
                        .filter(|(_, room)| room.label == label)
                        .map(|(id, _)| *id)
                        .collect();
                    
                    for existing_room_id in existing_rooms_with_label {
                        println!("Checking if neighbor at door {} is room {} (both have label {})",
                                 door_num, existing_room_id, label);
                        
                        // Create a temporary room to test equivalence
                        let temp_room_id = self.graph.add_room(label);
                        self.graph.path_to_room.insert(temp_room_id, neighbor_path.clone());
                        
                        if self.are_equal(temp_room_id, existing_room_id).await? {
                            println!("Door {} leads to existing room {}", door_num, existing_room_id);
                            // Connect to the existing room, finding the correct return door
                            self.graph.connect_rooms(current_room, door_num, existing_room_id, 0);
                            found_match = true;
                            
                            // Remove the temporary room
                            self.graph.rooms.remove(&temp_room_id);
                            self.graph.path_to_room.remove(&temp_room_id);
                            break;
                        } else {
                            // Remove the temporary room
                            self.graph.rooms.remove(&temp_room_id);
                            self.graph.path_to_room.remove(&temp_room_id);
                        }
                    }
                    
                    if !found_match {
                        // Create new room
                        let new_room_id = self.graph.add_room(label);
                        self.graph.path_to_room.insert(new_room_id, neighbor_path.clone());
                        self.graph.connect_rooms(current_room, door_num, new_room_id, 0);
                        
                        // Add to frontier if not explored
                        if !self.explored.contains(&new_room_id) {
                            self.frontier.push_back(new_room_id);
                        }
                        
                        println!("Created new room {} with label {} at path '{}'", new_room_id, label, neighbor_path);
                    }
                }


                self.explored.insert(current_room);
                
                // Print current graph status
                println!("Graph now has {} rooms", self.graph.rooms.len());
            } else {
                println!("No more rooms to explore");
                break;
            }
        }

        // Discover return doors - simplified version
        println!("\nDiscovering return doors...");
        self.discover_simple_return_doors().await?;

        Ok(())
    }

    async fn discover_simple_return_doors(&mut self) -> Result<()> {
        // For probatio (3 rooms), we need to discover the correct return doors
        // We'll test each connection to find the correct return door
        let room_ids: Vec<usize> = self.graph.rooms.keys().cloned().collect();
        
        for room_id in room_ids {
            let room = self.graph.rooms[&room_id].clone();
            
            for (door, connection) in room.doors.iter().enumerate() {
                if let Some((target_room_id, _current_return_door)) = connection {
                    // We have a connection but need to find the correct return door
                    // Test each door of the target room to see which one leads back
                    let target_path = self.get_path_to_room(*target_room_id);
                    
                    for return_door in 0..6 {
                        let test_path = format!("{}{}", target_path, return_door);
                        let (results, _) = self.api.explore(vec![test_path.clone()]).await?;
                        
                        if let Some(result) = results.get(0) {
                            if result.len() >= 2 {
                                let final_label = result[result.len() - 1];
                                // Check if this leads back to our original room
                                if final_label == room.label {
                                    // This might be the return door - verify by checking path length
                                    if result.len() == target_path.len() + 2 {
                                        // Correct return door found
                                        self.graph.connect_rooms(room_id, door, *target_room_id, return_door);
                                        println!("Connected room {} door {} <-> room {} door {}", 
                                                room_id, door, target_room_id, return_door);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }

    async fn discover_return_doors(&mut self) -> Result<()> {
        // For each room and door, find where it leads and what door it connects to
        let room_ids: Vec<usize> = self.graph.rooms.keys().cloned().collect();
        
        for room_id in room_ids {
            let path = self.get_path_to_room(room_id);
            
            for door in 0..6 {
                let test_path = format!("{}{}", path, door);
                
                // Explore this single step
                let (results, _) = self.api.explore(vec![test_path.clone()]).await?;
                
                if let Some(result) = results.get(0) {
                    if result.len() >= 2 {
                        // We moved to another room
                        let target_label = result[result.len() - 1];
                        
                        // Find the room with this label that we reach from this door
                        // We need to test each candidate room
                        let candidates: Vec<usize> = self.graph.rooms
                            .iter()
                            .filter(|(_, room)| room.label == target_label)
                            .map(|(id, _)| *id)
                            .collect();
                        
                        for candidate_id in candidates {
                            let candidate_path = self.get_path_to_room(candidate_id);
                            
                            // Check if we can reach the original room from this candidate
                            for return_door in 0..6 {
                                let return_test = format!("{}{}", candidate_path, return_door);
                                let (return_results, _) = self.api.explore(vec![return_test]).await?;
                                
                                if let Some(return_result) = return_results.get(0) {
                                    if return_result.len() >= 2 {
                                        let return_label = return_result[return_result.len() - 1];
                                        if return_label == self.graph.rooms[&room_id].label {
                                            // This might be the return door
                                            // Verify by checking if paths match
                                            let full_test = format!("{}{}{}", path, door, return_door);
                                            let (verify_results, _) = self.api.explore(vec![full_test]).await?;
                                            
                                            if let Some(verify_result) = verify_results.get(0) {
                                                // Check if we're back at the original room
                                                if verify_result.len() == result.len() + 1 &&
                                                   verify_result[verify_result.len() - 1] == self.graph.rooms[&room_id].label {
                                                    // Found the connection!
                                                    self.graph.connect_rooms(room_id, door, candidate_id, return_door);
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }

    pub fn output_graph(&self) {
        println!("\n=== Final Graph ===");
        println!("Total rooms: {}", self.graph.rooms.len());
        
        for (id, room) in &self.graph.rooms {
            println!("\nRoom {} (label: {})", id, room.label);
            for (door, connection) in room.doors.iter().enumerate() {
                if let Some((target_room, target_door)) = connection {
                    println!("  Door {} -> Room {} (via their door {})", 
                             door, target_room, target_door);
                }
            }
        }
    }

    pub fn get_submission_map(&self) -> serde_json::Value {
        self.graph.export_for_submission()
    }
}