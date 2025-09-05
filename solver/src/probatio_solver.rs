// Specialized solver for the probatio (3-room) problem
use anyhow::Result;

pub fn solve_probatio(explorations: &[(String, Vec<i32>)]) -> Result<crate::Map> {
    println!("Solving probatio with {} explorations", explorations.len());
    
    // For probatio, we know it's a 3-room problem
    // Let's identify the rooms based on the exploration data
    
    // Room 0: Starting room with label from first exploration
    let start_label = explorations[0].1[0];
    
    // Find all unique labels we've seen at depth 1
    let mut room_labels = vec![start_label];
    let mut room1_label = None;
    let mut room2_label = None;
    
    for (plan, labels) in explorations {
        if labels.len() >= 2 && plan.len() >= 1 {
            let label = labels[1];
            if label != start_label {
                if room1_label.is_none() {
                    room1_label = Some(label);
                    room_labels.push(label);
                } else if room1_label != Some(label) && room2_label.is_none() {
                    room2_label = Some(label);
                    room_labels.push(label);
                }
            }
        }
    }
    
    println!("Identified rooms: {:?}", room_labels);
    
    // Now map which doors lead where from room 0
    let mut connections = Vec::new();
    let mut room0_doors = [None; 6];
    
    for (plan, labels) in explorations {
        if labels.len() >= 2 && plan.len() >= 1 {
            if let Some(door) = plan.chars().next().and_then(|c| c.to_digit(10)) {
                let door = door as usize;
                let dest_label = labels[1];
                
                let dest_room = if dest_label == start_label {
                    0
                } else if Some(dest_label) == room1_label {
                    1
                } else {
                    2
                };
                
                room0_doors[door] = Some(dest_room);
            }
        }
    }
    
    // Build connections from room 0
    for (door, dest) in room0_doors.iter().enumerate() {
        if let Some(dest_room) = dest {
            // Find which door in dest_room leads back to room 0
            let mut return_door = None;
            
            // Look for evidence of the return path
            for (plan, labels) in explorations {
                if plan.len() >= 2 && labels.len() >= 3 {
                    // Check if this path goes to dest_room and back
                    if let (Some(d1), Some(d2)) = (
                        plan.chars().nth(0).and_then(|c| c.to_digit(10)),
                        plan.chars().nth(1).and_then(|c| c.to_digit(10))
                    ) {
                        if d1 as usize == door && labels[2] == start_label {
                            return_door = Some(d2 as usize);
                            break;
                        }
                    }
                }
            }
            
            connections.push(crate::Connection {
                from: crate::DoorRef {
                    room: 0,
                    door,
                },
                to: crate::DoorRef {
                    room: *dest_room,
                    door: return_door.unwrap_or(door), // Use same door if we don't know
                },
            });
        }
    }
    
    // For room 1 and room 2, find their interconnections
    if room1_label.is_some() && room2_label.is_some() {
        // Look for paths that go from room 1 to room 2
        for (plan, labels) in explorations {
            if labels.len() >= 3 && plan.len() >= 2 {
                // Check if we go to room 1 then room 2
                if labels[1] == room1_label.unwrap() && labels[2] == room2_label.unwrap() {
                    if let Some(door) = plan.chars().nth(1).and_then(|c| c.to_digit(10)) {
                        // Door from room 1 to room 2
                        let mut found = false;
                        for conn in &connections {
                            if conn.from.room == 1 && conn.from.door == door as usize {
                                found = true;
                                break;
                            }
                        }
                        
                        if !found {
                            connections.push(crate::Connection {
                                from: crate::DoorRef {
                                    room: 1,
                                    door: door as usize,
                                },
                                to: crate::DoorRef {
                                    room: 2,
                                    door: 0, // We'll figure out the right door
                                },
                            });
                        }
                    }
                }
            }
        }
    }
    
    println!("Built {} connections", connections.len());
    
    Ok(crate::Map {
        rooms: room_labels,
        starting_room: 0,
        connections,
    })
}