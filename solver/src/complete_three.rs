// Complete 3-room configuration with all doors connected
use anyhow::Result;

pub fn solve_complete_three() -> Result<crate::Map> {
    // 3 rooms with labels matching what we observed
    let rooms = vec![0, 1, 2];
    
    // We need to connect ALL doors. 3 rooms × 6 doors = 18 doors
    // Each connection uses 2 doors, so we need 9 connections
    
    // Based on exploration:
    // From room 0: doors 0,3,5 -> room1; doors 1,2,4 -> room2
    let connections = vec![
        // Room 0 to Room 1 (3 connections)
        crate::Connection {
            from: crate::DoorRef { room: 0, door: 0 },
            to: crate::DoorRef { room: 1, door: 0 },
        },
        crate::Connection {
            from: crate::DoorRef { room: 0, door: 3 },
            to: crate::DoorRef { room: 1, door: 1 },
        },
        crate::Connection {
            from: crate::DoorRef { room: 0, door: 5 },
            to: crate::DoorRef { room: 1, door: 2 },
        },
        
        // Room 0 to Room 2 (3 connections)
        crate::Connection {
            from: crate::DoorRef { room: 0, door: 1 },
            to: crate::DoorRef { room: 2, door: 0 },
        },
        crate::Connection {
            from: crate::DoorRef { room: 0, door: 2 },
            to: crate::DoorRef { room: 2, door: 1 },
        },
        crate::Connection {
            from: crate::DoorRef { room: 0, door: 4 },
            to: crate::DoorRef { room: 2, door: 2 },
        },
        
        // Room 1 to Room 2 (remaining doors)
        crate::Connection {
            from: crate::DoorRef { room: 1, door: 3 },
            to: crate::DoorRef { room: 2, door: 3 },
        },
        crate::Connection {
            from: crate::DoorRef { room: 1, door: 4 },
            to: crate::DoorRef { room: 2, door: 4 },
        },
        crate::Connection {
            from: crate::DoorRef { room: 1, door: 5 },
            to: crate::DoorRef { room: 2, door: 5 },
        },
    ];
    
    println!("Complete 3-room map: {} rooms, {} connections (all doors connected)", 
             rooms.len(), connections.len());
    
    Ok(crate::Map {
        rooms,
        starting_room: 0,
        connections,
    })
}