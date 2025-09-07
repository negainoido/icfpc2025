// Session Management
export interface Session {
  id: number;
  session_id: string;
  user_name?: string;
  problem_name?: string;
  status: string;
  score?: number;
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

export interface MapStruct {
  rooms: number[];
  startingRoom: number;
  connections: Connection[];
}

export interface GuessRequest {
  session_id: string;
  map: MapStruct;
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

// Explore Visualization Types
export interface MoveStep {
  type: 'move';
  door: number; // 0-5
}

export interface ChalkStep {
  type: 'chalk';
  label: number; // 0-3
}

export type ExploreStep = MoveStep | ChalkStep;

// Path segment for visualization
export interface PathSegment {
  from: number;
  to: number;
  door: number;
}

// Explore execution state
export interface ExploreState {
  currentRoom: number;
  currentPosition: { x: number; y: number };
  pathHistory: PathSegment[];
  chalkMarks: Map<number, number>; // roomIndex -> chalkLabel
  observedLabels: number[]; // observed labels history
  stepIndex: number;
  totalSteps: number;
}

// Props for visualization components with explore state
export interface ExploreVisualizationProps {
  exploreState: ExploreState | null;
  highlightCurrentRoom?: number;
  pathHistory?: PathSegment[];
  chalkMarks?: Map<number, number>;
}
