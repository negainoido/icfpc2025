use crate::api;
use crate::api::{Connection, RoomDoor};
use serde::Serialize;
use std::collections::HashSet;

pub type Label = u8;
// 0..=3
pub type Dir = u8;

/// ============ 提出用の map 形（必要に応じてあなたの JSON へ変換してください） ==

#[derive(Clone, Debug, Serialize)]
pub struct GuessRoom {
    pub label: Label,
    /// doors[a] = Some((to_room, peer_port)) / None if absent
    pub doors: [Option<(usize, Dir)>; 6],
}

#[derive(Clone, Debug, Serialize)]
pub struct GuessMap {
    pub rooms: Vec<GuessRoom>,
    pub starting_room: usize,
}

impl GuessMap {
    pub fn convert_to_api_guess_map(&self) -> anyhow::Result<api::GuessMap> {
        let rooms = self.rooms.iter().map(|r| r.label as i32).collect();
        let starting_room = self.starting_room.into();
        let mut connections = vec![];

        let mut used = HashSet::new();

        // connectionsを重複がないように変換
        for (i, r) in self.rooms.iter().enumerate() {
            for (d, door) in r.doors.iter().enumerate() {
                if let Some((to_room, peer_port)) = door {
                    let from_room = i;
                    let from_door = d;
                    let to_room = *to_room;
                    let to_door = *peer_port as usize;
                    if !used.contains(&(from_room, from_door))
                        && !used.contains(&(to_room, to_door))
                    {
                        used.insert((from_room, from_door));
                        used.insert((to_room, to_door));
                        connections.push(Connection {
                            from: RoomDoor {
                                room: from_room,
                                door: from_door,
                            },
                            to: RoomDoor {
                                room: to_room,
                                door: to_door,
                            },
                        });
                    }
                } else {
                    return Err(anyhow::anyhow!("Undfined Door at Room {i}  Door {d}"));
                }
            }
        }

        Ok(api::GuessMap {
            rooms,
            starting_room,
            connections,
        })
    }
}
