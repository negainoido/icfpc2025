use std::collections::{HashMap, HashSet};
use anyhow::{anyhow, Result};
use serde::{Serialize, Deserialize};

const DOOR_COUNT: usize = 6;

#[derive(Debug, Clone)]
pub struct SimpleGraph {
    nodes: Vec<Node>,
    starting_node: usize,
}

#[derive(Debug, Clone)]
struct Node {
    id: usize,
    label: i32,
    edges: HashMap<usize, (usize, usize)>, // door -> (node_id, their_door)
}

impl SimpleGraph {
    pub fn build_from_explorations(explorations: &[(String, Vec<i32>)]) -> Result<crate::Map> {
        if explorations.is_empty() {
            return Err(anyhow!("No explorations to build from"));
        }

        // Simple approach: track transitions
        // state = (current_position_in_sequence, door_taken) -> next_position
        let mut transitions: HashMap<(Vec<i32>, usize), Vec<i32>> = HashMap::new();
        
        for (plan, labels) in explorations {
            if labels.len() <= 1 {
                continue;
            }
            
            for i in 0..plan.len().min(labels.len() - 1) {
                let from_state = labels[0..=i].to_vec();
                let to_state = labels[0..=i+1].to_vec();
                
                if let Some(door) = plan.chars().nth(i).and_then(|c| c.to_digit(10)) {
                    transitions.insert((from_state, door as usize), to_state);
                }
            }
        }
        
        // Build a simple graph from transitions
        let mut room_map: HashMap<Vec<i32>, usize> = HashMap::new();
        let mut rooms = Vec::new();
        let mut connections = Vec::new();
        
        // Starting room
        let start_label = explorations[0].1[0];
        room_map.insert(vec![start_label], 0);
        rooms.push(start_label);
        
        // Process all transitions
        for ((from_state, door), to_state) in &transitions {
            // Get or create from room
            let from_room = if let Some(&id) = room_map.get(from_state) {
                id
            } else {
                let id = rooms.len();
                room_map.insert(from_state.clone(), id);
                rooms.push(*from_state.last().unwrap_or(&0));
                id
            };
            
            // Get or create to room  
            let to_room = if let Some(&id) = room_map.get(to_state) {
                id
            } else {
                let id = rooms.len();
                room_map.insert(to_state.clone(), id);
                rooms.push(*to_state.last().unwrap_or(&0));
                id
            };
            
            // Add connection if not self-loop or duplicate
            let mut found = false;
            for conn in &connections {
                if let crate::Connection { from, to } = conn {
                    if (from.room == from_room && from.door == *door && to.room == to_room) ||
                       (to.room == from_room && to.door == *door && from.room == to_room) {
                        found = true;
                        break;
                    }
                }
            }
            
            if !found && from_room != to_room {
                // Find available door in destination room
                let mut used_doors = HashSet::new();
                for conn in &connections {
                    if let crate::Connection { from, to } = conn {
                        if to.room == to_room {
                            used_doors.insert(to.door);
                        }
                        if from.room == to_room {
                            used_doors.insert(from.door);
                        }
                    }
                }
                
                let to_door = (0..DOOR_COUNT).find(|d| !used_doors.contains(d)).unwrap_or(0);
                
                connections.push(crate::Connection {
                    from: crate::DoorRef {
                        room: from_room,
                        door: *door,
                    },
                    to: crate::DoorRef {
                        room: to_room,
                        door: to_door,
                    },
                });
            }
        }
        
        println!("Simple graph: {} rooms, {} connections", rooms.len(), connections.len());
        
        Ok(crate::Map {
            rooms,
            starting_room: 0,
            connections,
        })
    }
}