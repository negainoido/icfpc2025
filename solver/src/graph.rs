use std::collections::{HashMap, HashSet, VecDeque};
use anyhow::{anyhow, Result};

const DOOR_COUNT: usize = 6;
const MAX_BATCH_SIZE: usize = 25;
const MAX_EXPLORATION_DEPTH: usize = 3;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct RoomState {
    pub label: i32,
    pub path: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct LibraryGraph {
    pub rooms: Vec<RoomNode>,
    pub starting_room: usize,
}

#[derive(Debug, Clone)]
pub struct RoomNode {
    pub label: i32,
    pub doors: [Option<(usize, usize)>; DOOR_COUNT], // (room_id, door_id)
}

impl LibraryGraph {
    pub fn new(starting_label: i32) -> Self {
        let mut rooms = vec![];
        let starting_room = RoomNode {
            label: starting_label,
            doors: [None; DOOR_COUNT],
        };
        rooms.push(starting_room);
        
        Self {
            rooms,
            starting_room: 0,
        }
    }
    
    pub fn from_explorations(explorations: &[(String, Vec<i32>)]) -> Result<Self> {
        if explorations.is_empty() {
            return Err(anyhow!("No explorations provided"));
        }
        
        println!("Building graph from {} explorations", explorations.len());
        
        // Build a state machine from the explorations
        let mut state_map: HashMap<Vec<i32>, usize> = HashMap::new();
        let mut graph = Self::new(explorations[0].1[0]);
        state_map.insert(vec![explorations[0].1[0]], 0);
        
        for (plan, labels) in explorations {
            if labels.is_empty() {
                continue;
            }
            
            let mut current_room = 0;
            let mut visited_path = vec![labels[0]];
            
            for (i, &label) in labels.iter().enumerate().skip(1) {
                visited_path.push(label);
                
                if i - 1 >= plan.len() {
                    break;
                }
                
                let door = match plan.chars().nth(i - 1).and_then(|c| c.to_digit(10)) {
                    Some(d) => d as usize,
                    None => continue,
                };
                
                if !state_map.contains_key(&visited_path) {
                    // New room discovered
                    let new_room_id = graph.rooms.len();
                    state_map.insert(visited_path.clone(), new_room_id);
                    
                    let new_room = RoomNode {
                        label,
                        doors: [None; DOOR_COUNT],
                    };
                    graph.rooms.push(new_room);
                    
                    // Connect rooms
                    if let Err(e) = graph.connect_rooms(current_room, door, new_room_id) {
                        println!("Warning: Failed to connect rooms: {}", e);
                    }
                    current_room = new_room_id;
                } else {
                    // Known room
                    let next_room = state_map[&visited_path];
                    if let Err(e) = graph.connect_rooms(current_room, door, next_room) {
                        println!("Warning: Failed to connect rooms: {}", e);
                    }
                    current_room = next_room;
                }
            }
        }
        
        println!("Graph built with {} rooms", graph.rooms.len());
        Ok(graph)
    }
    
    fn connect_rooms(&mut self, from_room: usize, from_door: usize, to_room: usize) -> Result<()> {
        if from_room >= self.rooms.len() {
            return Err(anyhow!("Invalid from_room index: {}", from_room));
        }
        if from_door >= DOOR_COUNT {
            return Err(anyhow!("Invalid from_door index: {}", from_door));
        }
        
        let to_door = self.find_available_door(to_room)
            .ok_or_else(|| anyhow!("No available doors in room {}", to_room))?;
        
        self.rooms[from_room].doors[from_door] = Some((to_room, to_door));
        
        if to_room < self.rooms.len() && to_door < DOOR_COUNT {
            self.rooms[to_room].doors[to_door] = Some((from_room, from_door));
        }
        
        Ok(())
    }
    
    fn find_available_door(&self, room_id: usize) -> Option<usize> {
        if room_id >= self.rooms.len() {
            return None;
        }
        
        for (door_id, door) in self.rooms[room_id].doors.iter().enumerate() {
            if door.is_none() {
                return Some(door_id);
            }
        }
        
        None
    }
    
    pub fn to_api_map(&self) -> crate::Map {
        let mut rooms = vec![];
        let mut connections = vec![];
        
        for room in &self.rooms {
            rooms.push(room.label);
        }
        
        // Build connections
        let mut processed_connections = HashSet::new();
        
        for (room_id, room) in self.rooms.iter().enumerate() {
            for (door_id, door_connection) in room.doors.iter().enumerate() {
                if let Some((target_room, target_door)) = door_connection {
                    let connection_key = if room_id < *target_room {
                        (room_id, door_id, *target_room, *target_door)
                    } else {
                        (*target_room, *target_door, room_id, door_id)
                    };
                    
                    if !processed_connections.contains(&connection_key) {
                        processed_connections.insert(connection_key);
                        connections.push(crate::Connection {
                            from: crate::DoorRef {
                                room: room_id,
                                door: door_id,
                            },
                            to: crate::DoorRef {
                                room: *target_room,
                                door: *target_door,
                            },
                        });
                    }
                }
            }
        }
        
        crate::Map {
            rooms,
            starting_room: self.starting_room,
            connections,
        }
    }
    
    pub fn simulate_exploration(&self, plan: &str) -> Vec<i32> {
        let mut labels = vec![];
        let mut current_room = self.starting_room;
        
        labels.push(self.rooms[current_room].label);
        
        for door_char in plan.chars() {
            if let Some(door_num) = door_char.to_digit(10) {
                let door_num = door_num as usize;
                
                if door_num < DOOR_COUNT {
                    if let Some((next_room, _)) = self.rooms[current_room].doors[door_num] {
                        current_room = next_room;
                        labels.push(self.rooms[current_room].label);
                    } else {
                        break; // Dead end
                    }
                }
            }
        }
        
        labels
    }
}

pub struct SmartExplorer {
    explorations: Vec<(String, Vec<i32>)>,
    known_states: HashMap<Vec<i32>, HashSet<usize>>,
    explored_prefixes: HashSet<String>,
}

impl SmartExplorer {
    pub fn new() -> Self {
        Self {
            explorations: vec![],
            known_states: HashMap::new(),
            explored_prefixes: HashSet::new(),
        }
    }
    
    pub fn add_exploration(&mut self, plan: &str, labels: &[i32]) {
        self.explorations.push((plan.to_string(), labels.to_vec()));
        
        // Add all prefixes of this plan to explored set
        for i in 1..=plan.len() {
            self.explored_prefixes.insert(plan[..i].to_string());
        }
        
        // Track which doors have been explored from each state
        let mut path = vec![];
        for (i, &label) in labels.iter().enumerate() {
            path.push(label);
            
            if i < plan.len() {
                if let Some(door) = plan.chars().nth(i).and_then(|c| c.to_digit(10)) {
                    self.known_states
                        .entry(path.clone())
                        .or_insert_with(HashSet::new)
                        .insert(door as usize);
                }
            }
        }
    }
    
    pub fn get_unexplored_plans(&self, max_plans: usize) -> Vec<String> {
        let mut plans = vec![];
        
        // First, try to explore all doors from known states
        for (state_path, explored_doors) in &self.known_states {
            for door in 0..DOOR_COUNT {
                if !explored_doors.contains(&door) {
                    // Build a plan to reach this state and then explore the door
                    if let Some(plan_to_state) = self.find_plan_to_state(state_path) {
                        let mut full_plan = plan_to_state;
                        full_plan.push_str(&door.to_string());
                        plans.push(full_plan);
                        
                        if plans.len() >= max_plans {
                            return plans;
                        }
                    }
                }
            }
        }
        
        // If we still need more plans, do breadth-first exploration
        let mut queue = VecDeque::new();
        queue.push_back(String::new());
        
        while let Some(current_plan) = queue.pop_front() {
            if current_plan.len() > 0 && current_plan.len() <= 6 {
                if !self.is_plan_explored(&current_plan) {
                    plans.push(current_plan.clone());
                    if plans.len() >= max_plans {
                        return plans;
                    }
                }
            }
            
            if current_plan.len() < 3 {
                for door in 0..DOOR_COUNT {
                    let mut next_plan = current_plan.clone();
                    next_plan.push_str(&door.to_string());
                    queue.push_back(next_plan);
                }
            }
        }
        
        plans
    }
    
    fn find_plan_to_state(&self, target_state: &[i32]) -> Option<String> {
        // Find an exploration that passes through this state
        for (plan, labels) in &self.explorations {
            for i in 0..=labels.len().saturating_sub(target_state.len()) {
                if &labels[i..i + target_state.len()] == target_state {
                    return Some(plan.chars().take(i + target_state.len().saturating_sub(1)).collect());
                }
            }
        }
        None
    }
    
    fn is_plan_explored(&self, plan: &str) -> bool {
        self.explored_prefixes.contains(plan)
    }
    
    pub fn build_graph(&self) -> Result<LibraryGraph> {
        LibraryGraph::from_explorations(&self.explorations)
    }
}