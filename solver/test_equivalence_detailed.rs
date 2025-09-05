use icfpc_solver::mock_api::MockApiClient;
use icfpc_solver::solver::Solver;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    println!("\n=== Testing Room Equivalence Detection ===\n");
    
    // Create a mock with a specific seed
    let mock_api = Arc::new(MockApiClient::new_with_seed("secundus", 42));
    
    // Create solver with more tries to ensure accurate equivalence checking
    let mut solver = Solver::new_with_max_tries(mock_api.clone(), 216, 10);
    
    // Run exploration
    println!("Starting exploration for secundus (12 rooms expected)...\n");
    solver.explore(12).await.unwrap();
    
    let num_rooms = solver.graph.rooms.len();
    println!("\n=== Results ===");
    println!("Rooms discovered: {}", num_rooms);
    
    if num_rooms != 12 {
        println!("WARNING: Expected 12 rooms, found {}", num_rooms);
        
        // Print room details
        println!("\n=== Room Details ===");
        let mut room_list: Vec<_> = solver.graph.rooms.iter().collect();
        room_list.sort_by_key(|(id, _)| **id);
        
        for (id, room) in room_list.iter().take(20) {
            let path = solver.graph.path_to_room.get(id).cloned().unwrap_or_default();
            println!("Room {}: label={}, path='{}'", id, room.label, path);
        }
        
        // Count label distribution
        let mut label_counts = [0; 4];
        for room in solver.graph.rooms.values() {
            if room.label < 4 {
                label_counts[room.label as usize] += 1;
            }
        }
        println!("\n=== Label Distribution ===");
        for (label, count) in label_counts.iter().enumerate() {
            println!("Label {}: {} rooms", label, count);
        }
    }
    
    // Test equivalence checking explicitly
    println!("\n=== Testing Equivalence Checking ===");
    
    // Find two rooms with the same label
    let mut same_label_pairs = Vec::new();
    for (id1, room1) in &solver.graph.rooms {
        for (id2, room2) in &solver.graph.rooms {
            if id1 < id2 && room1.label == room2.label {
                same_label_pairs.push((*id1, *id2, room1.label));
                if same_label_pairs.len() >= 3 {
                    break;
                }
            }
        }
        if same_label_pairs.len() >= 3 {
            break;
        }
    }
    
    for (id1, id2, label) in same_label_pairs {
        println!("\nChecking rooms {} and {} (both label {}):", id1, id2, label);
        let equal = solver.are_equal(id1, id2).await.unwrap();
        println!("Result: {}", if equal { "EQUAL" } else { "DIFFERENT" });
    }
}