use crate::api_trait::ApiClientTrait;
use crate::graph::Graph;
use anyhow::Result;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

pub struct Solver {
    api: Arc<dyn ApiClientTrait>,
    pub graph: Graph,
    pub frontier: VecDeque<usize>,
    pub explored: HashSet<usize>,
    walk_length: usize,
}

impl Solver {
    pub fn new(api: Arc<dyn ApiClientTrait>, walk_length: usize) -> Self {
        Self {
            api,
            graph: Graph::new(),
            frontier: VecDeque::new(),
            explored: HashSet::new(),
            walk_length,
        }
    }

    fn get_path_to_room(&self, room_id: usize) -> String {
        self.graph.path_to_room.get(&room_id).cloned().unwrap_or_default()
    }

    pub async fn gather_neighbor(&self, path: String) -> Result<[u8; 6]> {
        let mut plans = Vec::new();
        for door in 0..6 {
            plans.push(format!("{}{}", path, door));
        }
        
        let (results, _) = self.api.explore(plans).await?;
        
        let mut neighbor_labels = [0u8; 6];
        for (i, result) in results.iter().enumerate() {
            if let Some(&last_label) = result.last() {
                neighbor_labels[i] = last_label;
            }
        }
        
        Ok(neighbor_labels)
    }

    pub async fn are_equal(&self, v1: usize, v2: usize) -> Result<bool> {
        use rand::Rng;
        
        // Try multiple random walks (up to 3 times) to increase detection probability
        const MAX_TRIES: usize = 3;
        
        for attempt in 0..MAX_TRIES {
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
            
            let (results, _) = self.api.explore(vec![plan1.clone(), plan2.clone()]).await?;
            
            // Skip the path portion and compare only the random walk labels
            let path1_len = path1.len();
            let path2_len = path2.len();
            
            // Extract only the random walk portion from each result
            let walk1 = &results[0][path1_len..];
            let walk2 = &results[1][path2_len..];
            
            // Compare the random walk sequences
            let equal = walk1 == walk2;
            println!("  Attempt {}/{}: Comparing room {} and room {}: equal = {}", 
                     attempt + 1, MAX_TRIES, v1, v2, equal);
            
            if equal {
                // Found them equal, return true immediately
                return Ok(true);
            }
        }
        
        // After all attempts, they're not equal
        println!("  Rooms {} and {} are NOT equal after {} attempts", v1, v2, MAX_TRIES);
        Ok(false)
    }

    pub async fn explore(&mut self, problem_size: usize) -> Result<()> {
        // Initialize with start vertex
        self.frontier.push_back(self.graph.starting_room);

        println!("Starting exploration with problem size: {}", problem_size);

        for iteration in 0..problem_size {
            println!("\n=== Iteration {}/{} ===", iteration + 1, problem_size);

            if let Some(current_room) = self.frontier.pop_front() {
                if self.explored.contains(&current_room) {
                    continue;
                }

                let current_path = self.get_path_to_room(current_room);
                println!("Exploring room {} (label {}) at path: '{}'", 
                         current_room, self.graph.rooms[&current_room].label, current_path);
                
                // Gather neighbors
                let neighbor_labels = self.gather_neighbor(current_path.clone()).await?;
                println!("  Neighbor labels: {:?}", neighbor_labels);

                // Process each door
                for (door_num, &label) in neighbor_labels.iter().enumerate() {
                    let neighbor_path = format!("{}{}", current_path, door_num);
                    
                    // Check if this door is already connected from the current room
                    if let Some(room) = self.graph.rooms.get(&current_room) {
                        if room.doors[door_num].is_some() {
                            println!("    Door {} -> already connected", door_num);
                            continue;
                        }
                    }
                    
                    // Look for existing rooms with the same label and check equivalence
                    let mut found_existing = false;
                    let existing_rooms: Vec<usize> = self.graph.rooms
                        .iter()
                        .filter(|(_, room)| room.label == label)
                        .map(|(id, _)| *id)
                        .collect();
                    
                    // First create a temporary room to test equivalence
                    let temp_room_id = self.graph.add_room(label);
                    self.graph.path_to_room.insert(temp_room_id, neighbor_path.clone());
                    
                    // Check if this room is equivalent to any existing room with same label
                    for &existing_room in &existing_rooms {
                        println!("    Checking if door {} is equivalent to existing room {} (both label {})", 
                                door_num, existing_room, label);
                        
                        if self.are_equal(temp_room_id, existing_room).await.unwrap_or(false) {
                            println!("    Door {} -> existing room {} (equivalent, label {})", 
                                    door_num, existing_room, label);
                            self.graph.connect_one_way(current_room, door_num, existing_room);
                            found_existing = true;
                            
                            // Remove the temporary room
                            self.graph.rooms.remove(&temp_room_id);
                            self.graph.path_to_room.remove(&temp_room_id);
                            break;
                        }
                    }
                    
                    if !found_existing {
                        // Keep the new room
                        println!("    Door {} -> NEW room {} (label {})", 
                                door_num, temp_room_id, label);
                        self.graph.connect_one_way(current_room, door_num, temp_room_id);
                        self.frontier.push_back(temp_room_id);
                    }
                }
                
                self.explored.insert(current_room);
                println!("  Total rooms: {}", self.graph.rooms.len());
            } else {
                println!("Frontier empty, exploration complete");
                break;
            }
        }

        println!("\n=== Exploration Complete ===");
        println!("Total rooms discovered: {}", self.graph.rooms.len());
        println!("Total rooms explored: {}", self.explored.len());

        Ok(())
    }

    pub async fn discover_return_doors(&mut self) -> Result<()> {
        println!("\n=== Discovering Return Doors ===");
        
        // Build a list of connections that need return door discovery
        let mut connections_to_check = Vec::new();
        
        for (&room_id, room) in &self.graph.rooms {
            for (door_num, connection) in room.doors.iter().enumerate() {
                if let Some((target_room_id, target_door)) = connection {
                    // If the return door is unknown (0), we need to discover it
                    if *target_door == 0 {
                        connections_to_check.push((room_id, door_num, *target_room_id));
                    }
                }
            }
        }
        
        // For each connection, find the return door
        for (from_room, from_door, to_room) in connections_to_check {
            let to_path = self.get_path_to_room(to_room);
            
            // Explore all doors from the target room
            let mut plans = Vec::new();
            for door in 0..6 {
                plans.push(format!("{}{}", to_path, door));
            }
            
            let (results, _) = self.api.explore(plans).await?;
            
            // Find which door leads back to from_room
            // We need to check if the final room reached is equivalent to from_room
            for (door, result) in results.iter().enumerate() {
                // Create a temp room for this path result
                let temp_room_id = self.graph.rooms.len() + 100 + door;
                let test_path = format!("{}{}", to_path, door);
                self.graph.path_to_room.insert(temp_room_id, test_path);
                
                // Check if this leads back to from_room using are_equal
                if self.are_equal(temp_room_id, from_room).await.unwrap_or(false) {
                    self.graph.connect_rooms(from_room, from_door, to_room, door);
                    println!("  Room {} door {} <-> Room {} door {}", 
                            from_room, from_door, to_room, door);
                    self.graph.path_to_room.remove(&temp_room_id);
                    break;
                }
                
                self.graph.path_to_room.remove(&temp_room_id);
            }
        }
        
        Ok(())
    }

    pub fn output_graph(&self) {
        println!("\n=== Final Graph ===");
        println!("Rooms: {}", self.graph.rooms.len());
        
        for (room_id, room) in &self.graph.rooms {
            println!("Room {} (label {})", room_id, room.label);
            for (door, connection) in room.doors.iter().enumerate() {
                if let Some((target_room, target_door)) = connection {
                    println!("  Door {} -> Room {} (door {})", door, target_room, target_door);
                }
            }
        }
    }

    pub fn get_submission_map(&self) -> serde_json::Value {
        self.graph.export_for_submission()
    }
}