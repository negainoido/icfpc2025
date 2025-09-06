// Session Management
export interface Session {
  id: number;
  session_id: string;
  user_name?: string;
  status: string;
  created_at: string;
  completed_at?: string;
}

// Select API Types
export interface SelectRequest {
  problemName: string;
  user_name?: string;
}

export interface SelectResponse {
  session_id: string;
  problemName: string;
}

// Explore API Types
export interface ExploreRequest {
  session_id: string;
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
  door: number; // Door number 0-5 (hexagonal room has 6 doors)
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
  map: Map;
}

export interface GuessResponse {
  session_id: string;
  correct: boolean;
}

// API Logs
export interface ApiLog {
  id: number;
  session_id: string;
  endpoint: string;
  request_body?: string;
  response_body?: string;
  response_status?: number;
  created_at: string;
}

// Session details with logs
export interface SessionDetail {
  session: Session;
  api_logs: ApiLog[];
}

// Sessions list response
export interface SessionsListResponse {
  sessions: Session[];
}
