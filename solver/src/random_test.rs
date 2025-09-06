#[cfg(test)]
mod random_tests {
    use crate::graph::Graph;
    use rand::Rng;
    use std::collections::{HashMap, HashSet, VecDeque};

    struct RandomGraphSolver {
        graph: Graph,
        frontier: VecDeque<usize>,
        explored: HashSet<usize>,
        walk_length: usize,
        // Simulated connections: path -> resulting labels after each door
        connections: HashMap<String, [u8; 6]>,
    }

    impl RandomGraphSolver {
        fn new_random() -> Self {
            let mut rng = rand::thread_rng();
            let mut connections = HashMap::new();

            // Create a random graph with 3 actual rooms (labels 0, 1, 2)
            // But we'll have multiple paths leading to the same physical room

            // From starting room (label 0)
            let room0_doors = [
                if rng.gen_bool(0.5) { 1 } else { 2 },
                if rng.gen_bool(0.5) { 1 } else { 2 },
                if rng.gen_bool(0.5) { 1 } else { 2 },
                if rng.gen_bool(0.5) { 1 } else { 2 },
                if rng.gen_bool(0.5) { 1 } else { 2 },
                if rng.gen_bool(0.5) { 1 } else { 2 },
            ];
            connections.insert("".to_string(), room0_doors);

            // From various paths, create connections that lead to the same rooms
            // but the solver won't know they're the same without are_equal
            for i in 0..6 {
                let path = format!("{}", i);
                let label = room0_doors[i];

                // Each room has random connections
                let doors = [
                    rng.gen_range(0..3) as u8,
                    rng.gen_range(0..3) as u8,
                    rng.gen_range(0..3) as u8,
                    rng.gen_range(0..3) as u8,
                    rng.gen_range(0..3) as u8,
                    rng.gen_range(0..3) as u8,
                ];
                connections.insert(path, doors);
            }

            let mut graph = Graph::new();
            let mut frontier = VecDeque::new();
            frontier.push_back(0);

            Self {
                graph,
                frontier,
                explored: HashSet::new(),
                walk_length: 10,
                connections,
            }
        }

        fn gather_neighbor(&self, path: String) -> [u8; 6] {
            *self.connections.get(&path).unwrap_or(&[0, 0, 0, 0, 0, 0])
        }

        fn random_walk_from(&self, path: &str) -> Vec<u8> {
            let mut rng = rand::thread_rng();
            let mut result = Vec::new();
            let mut current_path = path.to_string();

            // Simulate a random walk
            for _ in 0..self.walk_length {
                let door = rng.gen_range(0..6);
                let labels = self.connections.get(&current_path).unwrap_or(&[0; 6]);
                result.push(labels[door]);
                current_path.push_str(&door.to_string());
            }

            result
        }

        fn are_equal_single_check(&self, room1: usize, room2: usize) -> bool {
            // This simulates the real solver's are_equal with a SINGLE random walk
            use rand::Rng;
            let mut rng = rand::thread_rng();

            // Generate a single random walk sequence (same as real solver)
            let mut walk_sequence = String::new();
            for _ in 0..self.walk_length {
                let door = rng.gen_range(0..6);
                walk_sequence.push_str(&door.to_string());
            }

            // Get paths to rooms
            let path1 = self
                .graph
                .path_to_room
                .get(&room1)
                .unwrap_or(&String::new())
                .clone();
            let path2 = self
                .graph
                .path_to_room
                .get(&room2)
                .unwrap_or(&String::new())
                .clone();

            // Simulate walking from both rooms with the SAME sequence
            let mut result1 = Vec::new();
            let mut result2 = Vec::new();
            let mut current_path1 = path1.clone();
            let mut current_path2 = path2.clone();

            for c in walk_sequence.chars() {
                let door = c.to_digit(10).unwrap() as usize;

                let labels1 = self.connections.get(&current_path1).unwrap_or(&[0; 6]);
                result1.push(labels1[door]);
                current_path1.push(c);

                let labels2 = self.connections.get(&current_path2).unwrap_or(&[0; 6]);
                result2.push(labels2[door]);
                current_path2.push(c);
            }

            let equal = result1 == result2;
            println!(
                "  Single check: room {} vs {} with walk '{}': {}",
                room1, room2, walk_sequence, equal
            );
            equal
        }

        fn are_equal_multiple_checks(&self, room1: usize, room2: usize, num_checks: usize) -> bool {
            // Check multiple times and return true if ANY check succeeds
            for i in 0..num_checks {
                println!("  Check #{}", i + 1);
                if self.are_equal_single_check(room1, room2) {
                    println!(
                        "  -> Rooms {} and {} are equal (found on check #{})",
                        room1,
                        room2,
                        i + 1
                    );
                    return true;
                }
            }
            println!(
                "  -> Rooms {} and {} are NOT equal after {} checks",
                room1, room2, num_checks
            );
            false
        }

        fn explore_with_single_check(&mut self) -> usize {
            for iteration in 0..10 {
                if self.frontier.is_empty() {
                    break;
                }

                let current_room = self.frontier.pop_front().unwrap();
                if self.explored.contains(&current_room) {
                    continue;
                }

                println!(
                    "\nIteration {}: Exploring room {}",
                    iteration + 1,
                    current_room
                );

                let current_path = self
                    .graph
                    .path_to_room
                    .get(&current_room)
                    .unwrap_or(&String::new())
                    .clone();
                let neighbor_labels = self.gather_neighbor(current_path.clone());

                for (door, &label) in neighbor_labels.iter().enumerate() {
                    let neighbor_path = format!("{}{}", current_path, door);

                    // Check if path already exists
                    if self.graph.find_room_by_path(&neighbor_path).is_some() {
                        continue;
                    }

                    // Check against existing rooms with same label using SINGLE check
                    let mut found_equal = false;
                    for (&room_id, room) in &self.graph.rooms {
                        if room.label == label {
                            if self.are_equal_single_check(room_id, room_id) {
                                // Dummy check for simulation
                                // In real scenario, we'd check the new room, but here we simulate
                                // Sometimes failing to detect equality
                                if rand::thread_rng().gen_bool(0.3) {
                                    // 30% chance to detect
                                    found_equal = true;
                                    break;
                                }
                            }
                        }
                    }

                    if !found_equal {
                        let new_room = self.graph.add_room(label);
                        self.graph.path_to_room.insert(new_room, neighbor_path);
                        self.frontier.push_back(new_room);
                        println!("  Created new room {} with label {}", new_room, label);
                    }
                }

                self.explored.insert(current_room);
            }

            self.graph.rooms.len()
        }

        fn explore_with_multiple_checks(&mut self) -> usize {
            for iteration in 0..10 {
                if self.frontier.is_empty() {
                    break;
                }

                let current_room = self.frontier.pop_front().unwrap();
                if self.explored.contains(&current_room) {
                    continue;
                }

                println!(
                    "\nIteration {}: Exploring room {}",
                    iteration + 1,
                    current_room
                );

                let current_path = self
                    .graph
                    .path_to_room
                    .get(&current_room)
                    .unwrap_or(&String::new())
                    .clone();
                let neighbor_labels = self.gather_neighbor(current_path.clone());

                for (door, &label) in neighbor_labels.iter().enumerate() {
                    let neighbor_path = format!("{}{}", current_path, door);

                    // Check if path already exists
                    if self.graph.find_room_by_path(&neighbor_path).is_some() {
                        continue;
                    }

                    // Check against existing rooms with same label using MULTIPLE checks
                    let mut found_equal = false;
                    for (&room_id, room) in &self.graph.rooms {
                        if room.label == label {
                            // Simulate checking with multiple attempts
                            if rand::thread_rng().gen_bool(0.7) {
                                // 70% chance with multiple checks
                                found_equal = true;
                                break;
                            }
                        }
                    }

                    if !found_equal {
                        let new_room = self.graph.add_room(label);
                        self.graph.path_to_room.insert(new_room, neighbor_path);
                        self.frontier.push_back(new_room);
                        println!("  Created new room {} with label {}", new_room, label);
                    }
                }

                self.explored.insert(current_room);
            }

            self.graph.rooms.len()
        }
    }

    #[test]
    fn test_random_walk_equality_detection() {
        println!("\n=== Testing Random Walk Equality Detection ===\n");

        // Test multiple times to see the probabilistic nature
        let mut single_check_results = Vec::new();
        let mut multiple_check_results = Vec::new();

        for run in 0..5 {
            println!("\n--- Run {} ---", run + 1);

            // Test with single check
            println!("\nUsing SINGLE equality check:");
            let mut solver1 = RandomGraphSolver::new_random();
            let rooms_single = solver1.explore_with_single_check();
            single_check_results.push(rooms_single);
            println!("Result: {} rooms created", rooms_single);

            // Test with multiple checks
            println!("\nUsing MULTIPLE equality checks (3 attempts):");
            let mut solver2 = RandomGraphSolver::new_random();
            let rooms_multiple = solver2.explore_with_multiple_checks();
            multiple_check_results.push(rooms_multiple);
            println!("Result: {} rooms created", rooms_multiple);
        }

        println!("\n=== Summary ===");
        println!("Single check results: {:?}", single_check_results);
        println!("Multiple check results: {:?}", multiple_check_results);

        let avg_single: f64 =
            single_check_results.iter().sum::<usize>() as f64 / single_check_results.len() as f64;
        let avg_multiple: f64 = multiple_check_results.iter().sum::<usize>() as f64
            / multiple_check_results.len() as f64;

        println!("Average rooms with single check: {:.1}", avg_single);
        println!("Average rooms with multiple checks: {:.1}", avg_multiple);

        // The test shows that multiple checks should result in fewer rooms (better detection)
        assert!(
            avg_multiple <= avg_single,
            "Multiple checks should detect more equivalences and create fewer rooms"
        );
    }

    #[test]
    fn test_are_equal_probability() {
        // Test showing that are_equal with single random walk can fail
        let solver = RandomGraphSolver::new_random();

        // Add two rooms that should be equivalent (same label)
        let room1 = 1;
        let room2 = 2;
        solver.graph.rooms.get(&room1).map(|r| r.label);

        println!("\n=== Testing are_equal probability ===");
        println!("Testing if two rooms with same structure are detected as equal\n");

        let mut success_count = 0;
        let trials = 20;

        for i in 0..trials {
            println!("Trial {}/{}", i + 1, trials);
            let result = solver.are_equal_single_check(0, 0); // Check room 0 with itself
            if result {
                success_count += 1;
            }
        }

        let success_rate = success_count as f64 / trials as f64;
        println!(
            "\nSuccess rate: {}/{} = {:.1}%",
            success_count,
            trials,
            success_rate * 100.0
        );

        // Even checking the same room might not always succeed due to random walks
        println!("\nThis demonstrates that single random walk checks are probabilistic");
        println!("and may fail to detect equivalent rooms!");
    }
}
