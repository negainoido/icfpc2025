use crate::mock_api::MockApiClient;
use crate::solver::Solver;
use std::sync::Arc;

pub async fn test_room_discovery() {
    println!("\n=== Testing Room Discovery with Random Graphs ===\n");
    
    // Test multiple random seeds
    for seed in 0..10 {
        println!("Test #{}: seed={}", seed + 1, seed);
        
        // Create mock API with specific seed
        let mock_api = Arc::new(MockApiClient::new_with_seed("secundus", seed));
        let mut solver = Solver::new_with_max_tries(mock_api.clone(), 216, 3);
        
        // Run exploration
        solver.explore(12).await.unwrap();
        
        let num_rooms = solver.graph.rooms.len();
        println!("  Found {} rooms (expected 12)\n", num_rooms);
        
        if num_rooms > 12 {
            println!("  WARNING: Found MORE than expected!");
            // Print room labels
            let mut label_counts = [0; 4];
            for room in solver.graph.rooms.values() {
                if room.label < 4 {
                    label_counts[room.label as usize] += 1;
                }
            }
            println!("  Label distribution: 0={}, 1={}, 2={}, 3={}", 
                     label_counts[0], label_counts[1], label_counts[2], label_counts[3]);
        }
    }
}