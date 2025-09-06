use crate::api_trait::ApiClientTrait;
use crate::graph::Graph;
use anyhow::Result;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

pub struct Solver {
    api: Arc<dyn ApiClientTrait>,
    pub graph: Graph,
    pub frontier: VecDeque<(usize, String)>, // (room_id, path_to_room)
    pub explored: HashSet<usize>,
    walk_length: usize,
    max_tries: usize,
}

impl Solver {
    pub fn new(api: Arc<dyn ApiClientTrait>, walk_length: usize) -> Self {
        Self::new_with_max_tries(api, walk_length, 3)
    }

    pub fn new_with_max_tries(
        api: Arc<dyn ApiClientTrait>,
        walk_length: usize,
        max_tries: usize,
    ) -> Self {
        Self {
            api,
            graph: Graph::new(),
            frontier: VecDeque::new(),
            explored: HashSet::new(),
            walk_length,
            max_tries,
        }
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

    pub async fn are_equal(&self, path1: &str, path2: &str) -> Result<bool> {
        use rand::Rng;

        // Try multiple random walks to verify equivalence
        // Two rooms are equal only if ALL random walks produce the same labels

        for attempt in 0..self.max_tries {
            // Generate a single random walk sequence
            let mut rng = rand::thread_rng();
            let mut walk_sequence = String::new();
            for _ in 0..self.walk_length {
                let door = rng.gen_range(0..6);
                walk_sequence.push_str(&door.to_string());
            }

            // Print comparison info
            println!("      Attempt {}/{}:", attempt + 1, self.max_tries);
            println!("        Path 1: '{}'", path1);
            println!("        Path 2: '{}'", path2);
            println!(
                "        Random walk: '{}...' (length {})",
                &walk_sequence[..walk_sequence.len().min(20)],
                walk_sequence.len()
            );

            // Execute the SAME walk from both vertices
            let plan1 = format!("{}{}", path1, walk_sequence);
            let plan2 = format!("{}{}", path2, walk_sequence);

            // Debug: verify what we're sending
            println!(
                "        Plan 1 starts with: '{}'",
                &plan1[..plan1.len().min(10)]
            );
            println!(
                "        Plan 2 starts with: '{}'",
                &plan2[..plan2.len().min(10)]
            );

            let (results, _) = self.api.explore(vec![plan1.clone(), plan2.clone()]).await?;

            // Skip the path portion and compare only the random walk labels
            // Note: results contain labels for all rooms visited, including starting room
            // So for a path of length n, we have n+1 labels (starting + n transitions)
            let path1_labels = path1.len() + 1; // +1 for starting room
            let path2_labels = path2.len() + 1; // +1 for starting room

            // Extract only the random walk portion from each result
            let walk1 = &results[0][path1_labels..];
            let walk2 = &results[1][path2_labels..];

            // Compare the random walk sequences
            let equal = walk1 == walk2;

            // Print the actual label sequences (truncated for readability)
            let display_len = 30.min(walk1.len());
            println!(
                "        Walk from path 1: {:?}{}",
                &walk1[..display_len],
                if walk1.len() > display_len { "..." } else { "" }
            );
            println!(
                "        Walk from path 2: {:?}{}",
                &walk2[..display_len],
                if walk2.len() > display_len { "..." } else { "" }
            );

            if !equal {
                // Found them different, return false immediately
                println!("        Result: DIFFERENT");
                return Ok(false);
            } else {
                println!("        Result: SAME");
            }
        }

        // All attempts showed they're equal, so they're likely the same room
        println!("      Verdict: Paths '{}' and '{}' are EQUAL", path1, path2);
        Ok(true)
    }

    pub async fn explore(&mut self, problem_size: usize) -> Result<()> {
        // Initialize with start vertex and empty path
        self.frontier
            .push_back((self.graph.starting_room, String::new()));

        println!("Starting exploration with problem size: {}", problem_size);

        for iteration in 0..problem_size {
            println!("\n=== Iteration {}/{} ===", iteration + 1, problem_size);

            if let Some((current_room, current_path)) = self.frontier.pop_front() {
                if self.explored.contains(&current_room) {
                    continue;
                }

                println!(
                    "Exploring room {} (label {}) at path: '{}'",
                    current_room, self.graph.rooms[&current_room].label, current_path
                );

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
                    let existing_rooms: Vec<usize> = self
                        .graph
                        .rooms
                        .iter()
                        .filter(|(_, room)| room.label == label)
                        .map(|(id, _)| *id)
                        .collect();

                    // First create a temporary room to test equivalence
                    let temp_room_id = self.graph.add_room(label);
                    self.graph
                        .path_to_room
                        .insert(temp_room_id, neighbor_path.clone());

                    // Check if this room is equivalent to any existing room with same label
                    for &existing_room in &existing_rooms {
                        println!(
                            "    Checking if door {} is equivalent to existing room {} (both label {})",
                            door_num, existing_room, label
                        );

                        // Get path to existing room (from path_to_room for now)
                        let existing_path = self
                            .graph
                            .path_to_room
                            .get(&existing_room)
                            .cloned()
                            .unwrap_or_default();

                        if self
                            .are_equal(&neighbor_path, &existing_path)
                            .await
                            .unwrap_or(false)
                        {
                            println!(
                                "    Door {} -> existing room {} (equivalent, label {})",
                                door_num, existing_room, label
                            );

                            // Check if the existing room already has a connection back to current_room
                            // If so, establish a proper bidirectional connection
                            let mut return_door = None;
                            if let Some(target_room) = self.graph.rooms.get(&existing_room) {
                                for (d, conn) in target_room.doors.iter().enumerate() {
                                    if let Some((connected_room, _)) = conn {
                                        if *connected_room == current_room {
                                            return_door = Some(d);
                                            break;
                                        }
                                    }
                                }
                            }

                            if let Some(return_door_num) = return_door {
                                // Establish bidirectional connection
                                self.graph.connect_rooms(
                                    current_room,
                                    door_num,
                                    existing_room,
                                    return_door_num,
                                );
                                println!(
                                    "      Established bidirectional: Room {} door {} <-> Room {} door {}",
                                    current_room, door_num, existing_room, return_door_num
                                );
                            } else {
                                // Just one-way for now, will be completed when the other room is explored
                                self.graph
                                    .connect_one_way(current_room, door_num, existing_room);
                            }

                            found_existing = true;

                            // Remove the temporary room
                            self.graph.rooms.remove(&temp_room_id);
                            self.graph.path_to_room.remove(&temp_room_id);
                            break;
                        }
                    }

                    if !found_existing {
                        // Keep the new room
                        println!(
                            "    Door {} -> NEW room {} (label {})",
                            door_num, temp_room_id, label
                        );
                        self.graph
                            .connect_one_way(current_room, door_num, temp_room_id);
                        self.frontier.push_back((temp_room_id, neighbor_path));
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

    pub fn output_graph(&self) {
        println!("\n=== Final Graph ===");
        println!("Rooms: {}", self.graph.rooms.len());

        for (room_id, room) in &self.graph.rooms {
            println!("Room {} (label {})", room_id, room.label);
            for (door, connection) in room.doors.iter().enumerate() {
                if let Some((target_room, target_door)) = connection {
                    println!(
                        "  Door {} -> Room {} (door {})",
                        door, target_room, target_door
                    );
                }
            }
        }
    }

    pub fn get_submission_map(&self) -> serde_json::Value {
        self.graph.export_for_submission()
    }
}
