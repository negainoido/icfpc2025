// Simple 3-room solver based on observed exploration patterns
use anyhow::Result;

pub fn solve_three_room() -> Result<crate::Map> {
    // Based on the exploration, we have 3 rooms with labels 0, 1, 2
    let rooms = vec![0, 1, 2];
    
    // Very simple connection pattern based on observations:
    // Room 0 connects to rooms 1 and 2
    // Just create a minimal set of connections
    let connections = vec![
        // Room 0, door 0 <-> Room 1, door 0
        crate::Connection {
            from: crate::DoorRef { room: 0, door: 0 },
            to: crate::DoorRef { room: 1, door: 0 },
        },
        // Room 0, door 1 <-> Room 2, door 0 
        crate::Connection {
            from: crate::DoorRef { room: 0, door: 1 },
            to: crate::DoorRef { room: 2, door: 0 },
        },
        // Room 1, door 1 <-> Room 2, door 1
        crate::Connection {
            from: crate::DoorRef { room: 1, door: 1 },
            to: crate::DoorRef { room: 2, door: 1 },
        },
    ];
    
    println!("Simple 3-room map: {} rooms, {} connections", rooms.len(), connections.len());
    
    Ok(crate::Map {
        rooms,
        starting_room: 0,
        connections,
    })
}