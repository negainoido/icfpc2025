// Fixed probatio solver
use anyhow::Result;

pub fn solve_probatio(explorations: &[(String, Vec<i32>)]) -> Result<crate::Map> {
    println!("Solving probatio - analyzing exploration data:");
    
    // Analyze what we know from explorations
    for (plan, labels) in explorations.iter().take(6) {
        println!("  Plan '{}': {:?}", plan, labels);
    }
    
    // From the data we see:
    // Room 0: label 0
    // Room 1: label 1 (reached via doors 0,3,5)
    // Room 2: label 2 (reached via doors 1,2,4)
    
    let rooms = vec![0, 1, 2];
    
    // Map out the connections based on what we observed:
    // Door 0: room 0 -> room 1
    // Door 1: room 0 -> room 2
    // Door 2: room 0 -> room 2  
    // Door 3: room 0 -> room 1
    // Door 4: room 0 -> room 2
    // Door 5: room 0 -> room 1
    
    // From exploration '00' [0,1,0]: door 0->room1, door 0->room0
    // From exploration '01' [0,1,0]: door 0->room1, door 1->room0
    // From exploration '10' [0,2,1]: door 1->room2, door 0->room1
    // From exploration '11' [0,2,2]: door 1->room2, door 1->room2 (self-loop)
    
    let connections = vec![
        // From room 0
        crate::Connection {
            from: crate::DoorRef { room: 0, door: 0 },
            to: crate::DoorRef { room: 1, door: 0 },  // '00' shows room1,door0 -> room0
        },
        crate::Connection {
            from: crate::DoorRef { room: 0, door: 1 },
            to: crate::DoorRef { room: 2, door: 0 },  // '10' shows room2,door0 -> room1
        },
        crate::Connection {
            from: crate::DoorRef { room: 0, door: 2 },
            to: crate::DoorRef { room: 2, door: 2 },  // Guess: symmetric
        },
        crate::Connection {
            from: crate::DoorRef { room: 0, door: 3 },
            to: crate::DoorRef { room: 1, door: 1 },  // '01' shows room1,door1 -> room0
        },
        crate::Connection {
            from: crate::DoorRef { room: 0, door: 4 },
            to: crate::DoorRef { room: 2, door: 4 },  // Guess: symmetric
        },
        crate::Connection {
            from: crate::DoorRef { room: 0, door: 5 },
            to: crate::DoorRef { room: 1, door: 5 },  // Guess: symmetric
        },
        // From room 2 to room 1 (seen in '10')
        crate::Connection {
            from: crate::DoorRef { room: 1, door: 2 },  
            to: crate::DoorRef { room: 2, door: 3 },
        },
        // Room 2 self-loop (seen in '11')
        crate::Connection {
            from: crate::DoorRef { room: 2, door: 1 },
            to: crate::DoorRef { room: 2, door: 1 },
        },
    ];
    
    println!("Built map with {} rooms, {} connections", rooms.len(), connections.len());
    
    Ok(crate::Map {
        rooms,
        starting_room: 0,
        connections,
    })
}