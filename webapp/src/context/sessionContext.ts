import { createContext } from 'react';
import { SessionState, SessionAction } from './sessionTypes';

export const SessionContext = createContext<{
  state: SessionState;
  dispatch: React.Dispatch<SessionAction>;
} | null>(null);