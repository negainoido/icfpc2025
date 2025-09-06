import React, { useReducer, ReactNode } from 'react';
import { SessionContext } from './sessionContext';
import { initialState, sessionReducer } from './sessionTypes';

export function SessionProvider({ children }: { children: ReactNode }) {
  const [state, dispatch] = useReducer(sessionReducer, initialState);

  return (
    <SessionContext.Provider value={{ state, dispatch }}>
      {children}
    </SessionContext.Provider>
  );
}