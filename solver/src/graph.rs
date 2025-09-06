use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct Room {
    pub id: usize,
    pub label: u8,                          // 2-bit label (0-3)
    pub doors: [Option<(usize, usize)>; 6], // door_num -> (room_id, their_door_num)
}

impl Room {
    pub fn new(id: usize, label: u8) -> Self {
        Self {
            id,
            label,
            doors: [None; 6],
        }
    }

    pub fn connect_door(&mut self, door: usize, target_room: usize, target_door: usize) {
        self.doors[door] = Some((target_room, target_door));
    }
}

#[derive(Debug)]
pub struct Graph {
    pub rooms: HashMap<usize, Room>,
    pub starting_room: usize,
    next_id: usize,
    pub path_to_room: HashMap<usize, String>, // room_id -> path from start
}

impl Graph {
    pub fn new() -> Self {
        let mut graph = Self {
            rooms: HashMap::new(),
            starting_room: 0,
            next_id: 0,
            path_to_room: HashMap::new(),
        };

        // Add starting room
        graph.add_room(0); // Starting room always has label 0 based on problem examples
        graph.path_to_room.insert(0, String::new());

        graph
    }

    pub fn add_room(&mut self, label: u8) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.rooms.insert(id, Room::new(id, label));
        id
    }

    pub fn connect_rooms(&mut self, room1: usize, door1: usize, room2: usize, door2: usize) {
        if let Some(r1) = self.rooms.get_mut(&room1) {
            r1.connect_door(door1, room2, door2);
        }
        if let Some(r2) = self.rooms.get_mut(&room2) {
            r2.connect_door(door2, room1, door1);
        }
    }

    pub fn connect_one_way(&mut self, from_room: usize, from_door: usize, to_room: usize) {
        if let Some(room) = self.rooms.get_mut(&from_room) {
            room.doors[from_door] = Some((to_room, 0)); // We don't track the return door
        }
    }

    pub fn find_room_by_path(&self, path: &str) -> Option<usize> {
        if path.is_empty() {
            return Some(self.starting_room);
        }

        let mut current = self.starting_room;
        for door_char in path.chars() {
            let door = door_char.to_digit(10)? as usize;
            if door >= 6 {
                return None;
            }

            if let Some(room) = self.rooms.get(&current) {
                if let Some((next_room, _)) = room.doors[door] {
                    current = next_room;
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
        Some(current)
    }

    pub fn merge_rooms(&mut self, keep_id: usize, remove_id: usize) {
        if keep_id == remove_id {
            return;
        }

        // Get all connections from the room to be removed
        let remove_room = match self.rooms.get(&remove_id) {
            Some(r) => r.clone(),
            None => return,
        };

        // For each door of the removed room, update the connection
        for (door_num, connection) in remove_room.doors.iter().enumerate() {
            if let Some((target_room_id, target_door)) = connection {
                if *target_room_id != remove_id {
                    // Connect keep_room's door to the target
                    if let Some(keep_room) = self.rooms.get_mut(&keep_id) {
                        keep_room.doors[door_num] = Some((*target_room_id, *target_door));
                    }

                    // Update target room to point to keep_room instead of remove_room
                    if let Some(target_room) = self.rooms.get_mut(target_room_id) {
                        target_room.doors[*target_door] = Some((keep_id, door_num));
                    }
                }
            }
        }

        // Update path_to_room mappings
        let paths_to_update: Vec<(usize, String)> = self
            .path_to_room
            .iter()
            .filter(|(room_id, _)| **room_id == remove_id)
            .map(|(_, path)| (keep_id, path.clone()))
            .collect();

        for (room_id, path) in paths_to_update {
            self.path_to_room.insert(room_id, path);
        }

        // Remove the merged room
        self.rooms.remove(&remove_id);
        self.path_to_room.remove(&remove_id);
    }

    pub fn get_or_create_room_at_path(&mut self, path: &str, label: u8) -> usize {
        if let Some(room_id) = self.find_room_by_path(path) {
            return room_id;
        }

        // Create new room and connect it
        let new_room_id = self.add_room(label);
        self.path_to_room.insert(new_room_id, path.to_string());

        if !path.is_empty() {
            let parent_path = &path[0..path.len() - 1];
            let door = path.chars().last().unwrap().to_digit(10).unwrap() as usize;

            if let Some(parent_id) = self.find_room_by_path(parent_path) {
                // We don't know the return door yet, will be discovered later
                if let Some(parent) = self.rooms.get_mut(&parent_id) {
                    parent.doors[door] = Some((new_room_id, 0)); // Temporary, will be updated
                }
            }
        }

        new_room_id
    }

    pub fn export_for_submission(&self) -> serde_json::Value {
        use serde_json::json;

        let mut room_map = HashMap::new();

        // Ensure starting room is always index 0
        room_map.insert(self.starting_room, 0);
        let mut room_index = 1;

        // Create room index mapping for other rooms
        let mut sorted_room_ids: Vec<usize> = self.rooms.keys().cloned().collect();
        sorted_room_ids.sort();

        for room_id in sorted_room_ids {
            if room_id != self.starting_room {
                room_map.insert(room_id, room_index);
                room_index += 1;
            }
        }

        // Build rooms array in the correct order
        let mut rooms = vec![0u8; self.rooms.len()];
        for (room_id, room) in &self.rooms {
            let index = room_map[room_id];
            rooms[index] = room.label;
        }

        let mut connections = Vec::new();

        for (room_id, room) in &self.rooms {
            let from_index = room_map[room_id];

            for (door_num, connection) in room.doors.iter().enumerate() {
                if let Some((to_room_id, _to_door)) = connection {
                    // Skip if the target room doesn't exist
                    if !room_map.contains_key(to_room_id) {
                        continue;
                    }
                    let to_index = room_map[to_room_id];

                    // For one-way connections, we need to find the return door
                    // by checking the target room's connections back to this room
                    let mut return_door = 0;
                    if let Some(target_room) = self.rooms.get(to_room_id) {
                        for (d, conn) in target_room.doors.iter().enumerate() {
                            if let Some((back_room_id, _)) = conn {
                                if *back_room_id == *room_id {
                                    return_door = d;
                                    break;
                                }
                            }
                        }
                    }

                    connections.push(json!({
                        "from": {"room": from_index, "door": door_num},
                        "to": {"room": to_index, "door": return_door}
                    }));
                }
            }
        }

        json!({
            "rooms": rooms,
            "startingRoom": room_map[&self.starting_room],
            "connections": connections
        })
    }
}
