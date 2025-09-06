export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  message?: string;
}

// Session Management
export interface Session {
  id: number;
  session_id: string;
  status: string;
  created_at: string;
  completed_at?: string;
}

// Select API Types
export interface SelectRequest {
  id: string;
  problemName: string;
}

export interface SelectResponse {
  session_id: string;
  problemName: string;
}

// Explore API Types
export interface ExploreRequest {
  session_id: string;
  id: string;
  plans: string[];
}

export interface ExploreResponse {
  session_id: string;
  results: number[][];
  queryCount: number;
}

// Guess API Types
export interface DoorLocation {
  room: number;
  door: number;
}

export interface Connection {
  from: DoorLocation;
  to: DoorLocation;
}

export interface Map {
  rooms: number[];
  startingRoom: number;
  connections: Connection[];
}

export interface GuessRequest {
  session_id: string;
  id: string;
  map: Map;
}

export interface GuessResponse {
  session_id: string;
  correct: boolean;
}
