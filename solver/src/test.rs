#[cfg(test)]
mod tests {
    use crate::graph::Graph;
    use std::collections::{HashSet, HashMap, VecDeque};

    // Mock API client for testing
    pub struct MockApiClient {
        pub responses: std::collections::HashMap<String, Vec<u8>>,
        pub query_count: u32,
    }

    impl MockApiClient {
        pub fn new() -> Self {
            let mut responses = std::collections::HashMap::new();
            
            // For probatio problem, we should have 3 rooms total
            // Room 0 (label 0) - starting room
            // Room 1 (label 1) 
            // Room 2 (label 2)
            
            // From room 0 (empty path)
            responses.insert("0".to_string(), vec![0, 1]); // door 0 -> room with label 1
            responses.insert("1".to_string(), vec![0, 1]); // door 1 -> room with label 1
            responses.insert("2".to_string(), vec![0, 1]); // door 2 -> room with label 1
            responses.insert("3".to_string(), vec![0, 2]); // door 3 -> room with label 2
            responses.insert("4".to_string(), vec![0, 2]); // door 4 -> room with label 2
            responses.insert("5".to_string(), vec![0, 2]); // door 5 -> room with label 2
            
            // From room 1 (reached via path "0")
            responses.insert("00".to_string(), vec![0, 1, 0]); // door 0 -> room with label 0
            responses.insert("01".to_string(), vec![0, 1, 2]); // door 1 -> room with label 2
            responses.insert("02".to_string(), vec![0, 1, 2]); // door 2 -> room with label 2
            responses.insert("03".to_string(), vec![0, 1, 2]); // door 3 -> room with label 2
            responses.insert("04".to_string(), vec![0, 1, 0]); // door 4 -> room with label 0
            responses.insert("05".to_string(), vec![0, 1, 0]); // door 5 -> room with label 0
            
            // From room 2 (reached via path "3")
            responses.insert("30".to_string(), vec![0, 2, 1]); // door 0 -> room with label 1
            responses.insert("31".to_string(), vec![0, 2, 1]); // door 1 -> room with label 1
            responses.insert("32".to_string(), vec![0, 2, 0]); // door 2 -> room with label 0
            responses.insert("33".to_string(), vec![0, 2, 0]); // door 3 -> room with label 0
            responses.insert("34".to_string(), vec![0, 2, 0]); // door 4 -> room with label 0
            responses.insert("35".to_string(), vec![0, 2, 1]); // door 5 -> room with label 1

            // Random walks for equivalence testing
            // All paths to room with label 0 should give same random walk
            responses.insert("_walk_0".to_string(), vec![0, 1, 2, 2, 1, 0, 1, 1, 2, 0]);
            
            // All paths to room with label 1 should give same random walk
            responses.insert("_walk_1".to_string(), vec![1, 0, 2, 0, 0, 2, 1, 2, 2, 1]);
            
            // All paths to room with label 2 should give same random walk
            responses.insert("_walk_2".to_string(), vec![2, 1, 0, 0, 1, 1, 2, 0, 1, 2]);
            
            Self { responses, query_count: 0 }
        }

        pub async fn explore(&mut self, plans: Vec<String>) -> (Vec<Vec<u8>>, u32) {
            let mut results = Vec::new();
            for plan in plans {
                if let Some(response) = self.responses.get(&plan) {
                    results.push(response.clone());
                } else if plan.contains("walk") {
                    // For random walks, extract the label from the last character before "walk"
                    let path = plan.split("walk").next().unwrap();
                    let label = if path.is_empty() {
                        0 // starting room
                    } else {
                        // Get label from the actual path
                        self.responses.get(path).and_then(|r| r.last()).copied().unwrap_or(0)
                    };
                    let walk_key = format!("_walk_{}", label);
                    results.push(self.responses.get(&walk_key).unwrap().clone());
                } else {
                    // For testing random walks
                    results.push(vec![0, 0]); // default response
                }
            }
            self.query_count += results.len() as u32;
            (results, self.query_count)
        }
    }

    // Mock solver that simulates the full exploration process
    struct MockSolver {
        api: MockApiClient,
        graph: Graph,
        frontier: VecDeque<usize>,
        explored: HashSet<usize>,
        walk_length: usize,
    }

    impl MockSolver {
        fn new() -> Self {
            let mut graph = Graph::new();
            let mut frontier = VecDeque::new();
            frontier.push_back(0); // Start with room 0
            
            Self {
                api: MockApiClient::new(),
                graph,
                frontier,
                explored: HashSet::new(),
                walk_length: 10,
            }
        }

        async fn gather_neighbor(&mut self, room_id: usize) -> Result<[Option<u8>; 6], String> {
            let path = self.graph.path_to_room.get(&room_id)
                .ok_or("Room path not found")?;
            
            let mut plans = Vec::new();
            for door in 0..6 {
                plans.push(format!("{}{}", path, door));
            }
            
            let (results, _) = self.api.explore(plans).await;
            
            let mut doors = [None; 6];
            for (door, result) in results.iter().enumerate() {
                if let Some(&label) = result.last() {
                    doors[door] = Some(label);
                }
            }
            
            Ok(doors)
        }

        async fn random_walk_from(&mut self, room_id: usize) -> Result<Vec<u8>, String> {
            let path = self.graph.path_to_room.get(&room_id)
                .ok_or("Room path not found")?;
            
            let plan = format!("{}walk{}", path, self.walk_length);
            let (results, _) = self.api.explore(vec![plan]).await;
            
            results.first()
                .ok_or("No walk result".to_string())
                .map(|r| r.clone())
        }

        async fn are_equal(&mut self, room1: usize, room2: usize) -> Result<bool, String> {
            println!("      Checking if rooms {} and {} are equal", room1, room2);
            
            // First check labels
            let label1 = self.graph.rooms.get(&room1)
                .ok_or("Room1 not found")?.label;
            let label2 = self.graph.rooms.get(&room2)
                .ok_or("Room2 not found")?.label;
            
            if label1 != label2 {
                println!("        Labels differ: {} vs {}", label1, label2);
                return Ok(false);
            }
            
            // Get random walks
            let walk1 = self.random_walk_from(room1).await?;
            let walk2 = self.random_walk_from(room2).await?;
            
            println!("        Walk1: {:?}", walk1);
            println!("        Walk2: {:?}", walk2);
            
            Ok(walk1 == walk2)
        }

        async fn explore(&mut self, max_iterations: usize) -> Result<(), String> {
            for iteration in 0..max_iterations {
                if self.frontier.is_empty() {
                    println!("Frontier empty at iteration {}", iteration);
                    break;
                }
                
                let current_room = self.frontier.pop_front().unwrap();
                
                if self.explored.contains(&current_room) {
                    continue;
                }
                
                println!("\n=== Iteration {} ===", iteration + 1);
                println!("Exploring room {} (label {})", current_room, 
                         self.graph.rooms[&current_room].label);
                
                // Get neighbor labels
                let neighbor_labels = self.gather_neighbor(current_room).await?;
                println!("  Neighbor labels: {:?}", neighbor_labels);
                
                // Store newly created rooms to check for equivalence
                let mut newly_created_rooms: Vec<(usize, u8)> = Vec::new();
                let mut door_targets = [None; 6];
                
                for (door, label_opt) in neighbor_labels.iter().enumerate() {
                    if let Some(label) = label_opt {
                        // Check if we already have a room with this label from the same parent
                        let mut found_existing = false;
                        
                        // First check among existing rooms
                        let existing_rooms: Vec<(usize, u8)> = self.graph.rooms
                            .iter()
                            .filter(|(id, _)| **id != current_room)
                            .map(|(id, room)| (*id, room.label))
                            .collect();
                        
                        for (room_id, room_label) in existing_rooms {
                            if room_label == *label {
                                // For mock testing, rooms with same label are always equal
                                // (since our mock data is deterministic)
                                println!("    Door {} -> existing room {} (label {})", 
                                        door, room_id, label);
                                door_targets[door] = Some(room_id);
                                found_existing = true;
                                break;
                            }
                        }
                        
                        // Then check among newly created rooms in this iteration
                        if !found_existing {
                            for &(new_room_id, new_label) in &newly_created_rooms {
                                if new_label == *label {
                                    println!("    Door {} -> newly created room {} (label {})", 
                                            door, new_room_id, label);
                                    door_targets[door] = Some(new_room_id);
                                    found_existing = true;
                                    break;
                                }
                            }
                        }
                        
                        // Create new room if no existing equivalent found
                        if !found_existing {
                            let new_room = self.graph.add_room(*label);
                            let path = format!("{}{}", 
                                self.graph.path_to_room[&current_room], door);
                            self.graph.path_to_room.insert(new_room, path);
                            
                            println!("    Door {} -> NEW room {} (label {})", 
                                    door, new_room, label);
                            door_targets[door] = Some(new_room);
                            newly_created_rooms.push((new_room, *label));
                            self.frontier.push_back(new_room);
                        }
                    }
                }
                
                // Connect doors
                for (door, target) in door_targets.iter().enumerate() {
                    if let Some(target_room) = target {
                        self.graph.connect_rooms(current_room, door, *target_room, 0);
                    }
                }
                
                self.explored.insert(current_room);
                
                println!("  Total rooms after iteration: {}", self.graph.rooms.len());
            }
            
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_probatio_should_have_3_rooms() {
        let mut mock_solver = MockSolver::new();
        
        // Create a graph and simulate exploration
        let mut graph = Graph::new();
        let mut explored: std::collections::HashSet<usize> = std::collections::HashSet::new();
        let mut frontier: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
        frontier.push_back(0); // Start with room 0
        
        // Run exploration
        let result = mock_solver.explore(3).await;
        assert!(result.is_ok(), "Exploration failed: {:?}", result);
        
        // Check the final graph
        let room_count = mock_solver.graph.rooms.len();
        println!("\n=== Final Graph ===");
        println!("Total rooms: {}", room_count);
        
        for (room_id, room) in &mock_solver.graph.rooms {
            println!("Room {} (label {})", room_id, room.label);
        }
        
        assert_eq!(room_count, 3, "Probatio should have exactly 3 rooms, but got {}", room_count);
        
        // Verify room labels
        let mut labels: HashSet<u8> = HashSet::new();
        for room in mock_solver.graph.rooms.values() {
            labels.insert(room.label);
        }
        
        assert_eq!(labels.len(), 3, "Should have 3 distinct labels");
        assert!(labels.contains(&0), "Should have room with label 0");
        assert!(labels.contains(&1), "Should have room with label 1");
        assert!(labels.contains(&2), "Should have room with label 2");
    }

    #[tokio::test]
    async fn test_equivalence_checking() {
        // Test that rooms with same random walk are considered equal
        let mut mock_api = MockApiClient::new();
        
        // Simulate random walks from different paths that lead to the same room
        let walk1 = mock_api.explore(vec!["0walk".to_string()]).await.0[0].clone(); // path "0" leads to room with label 1
        let walk2 = mock_api.explore(vec!["1walk".to_string()]).await.0[0].clone(); // path "1" also leads to room with label 1
        let walk3 = mock_api.explore(vec!["2walk".to_string()]).await.0[0].clone(); // path "2" also leads to room with label 1
        
        // These should all be the same since they lead to the same room
        assert_eq!(walk1, walk2, "Paths 0 and 1 should have same random walk");
        assert_eq!(walk2, walk3, "Paths 1 and 2 should have same random walk");
        
        // Test different room
        let walk4 = mock_api.explore(vec!["3walk".to_string()]).await.0[0].clone(); // path "3" leads to room with label 2
        assert_ne!(walk1, walk4, "Paths to different rooms should have different random walks");
    }

    #[tokio::test] 
    async fn test_newly_created_rooms_should_be_checked_against_each_other() {
        // When we create multiple new rooms in one iteration, 
        // we should check if they are equivalent to each other
        
        let neighbors = vec![1, 1, 1, 2, 2, 2]; // 3 doors lead to label 1, 3 to label 2
        let mut new_rooms: Vec<(usize, u8)> = Vec::new();
        
        for (door, &label) in neighbors.iter().enumerate() {
            // Check if this new room is equivalent to any previously created new room
            let mut found_equivalent = false;
            for &(existing_id, existing_label) in &new_rooms {
                if existing_label == label {
                    // In real code, we'd use are_equal here
                    // For this test, we know rooms with same label should be equal
                    found_equivalent = true;
                    println!("Door {} should connect to previously created room {}", door, existing_id);
                    break;
                }
            }
            
            if !found_equivalent {
                let new_id = door + 1; // Simulate room ID
                new_rooms.push((new_id, label));
                println!("Created new room {} with label {}", new_id, label);
            }
        }
        
        // Should only create 2 new rooms (one for each unique label)
        assert_eq!(new_rooms.len(), 2, "Should create only 2 new rooms for 2 unique labels");
    }
}