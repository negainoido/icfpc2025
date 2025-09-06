export type GamePhase = 'idle' | 'problem-selection' | 'exploring' | 'building-map' | 'completed';

export interface SessionState {
  sessionId: string | null;
  problemName: string | null;
  phase: GamePhase;
  explorationResults: number[][];
  queryCount: number;
  teamId: string;
  isLoading: boolean;
  error: string | null;
}

export type SessionAction =
  | { type: 'SET_TEAM_ID'; payload: string }
  | { type: 'START_SESSION'; payload: { sessionId: string; problemName: string } }
  | { type: 'ADD_EXPLORATION_RESULTS'; payload: { results: number[][]; queryCount: number } }
  | { type: 'SET_PHASE'; payload: GamePhase }
  | { type: 'SET_LOADING'; payload: boolean }
  | { type: 'SET_ERROR'; payload: string | null }
  | { type: 'RESET_SESSION' };

export const initialState: SessionState = {
  sessionId: null,
  problemName: null,
  phase: 'idle',
  explorationResults: [],
  queryCount: 0,
  teamId: '',
  isLoading: false,
  error: null,
};

export function sessionReducer(state: SessionState, action: SessionAction): SessionState {
  switch (action.type) {
    case 'SET_TEAM_ID':
      return { ...state, teamId: action.payload, phase: 'problem-selection' };
    case 'START_SESSION':
      return {
        ...state,
        sessionId: action.payload.sessionId,
        problemName: action.payload.problemName,
        phase: 'exploring',
        error: null,
      };
    case 'ADD_EXPLORATION_RESULTS':
      return {
        ...state,
        explorationResults: [...state.explorationResults, ...action.payload.results],
        queryCount: action.payload.queryCount,
      };
    case 'SET_PHASE':
      return { ...state, phase: action.payload };
    case 'SET_LOADING':
      return { ...state, isLoading: action.payload };
    case 'SET_ERROR':
      return { ...state, error: action.payload, isLoading: false };
    case 'RESET_SESSION':
      return { ...initialState, teamId: state.teamId };
    default:
      return state;
  }
}