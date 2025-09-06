import React, { useState } from 'react';
import { useSession } from '../hooks/useSession';
import { api, ApiError } from '../services/api';

const PROBLEM_OPTIONS = [
  'library1',
  'library2',
  'library3',
  'library4',
  'library5',
  'library6',
  'library7',
  'library8',
  'library9',
  'library10',
];

export default function ProblemSelector() {
  const { state, dispatch } = useSession();
  const [selectedProblem, setSelectedProblem] = useState(PROBLEM_OPTIONS[0]);

  const handleSelect = async () => {
    if (!state.teamId) {
      dispatch({ type: 'SET_ERROR', payload: 'Team ID is required' });
      return;
    }

    try {
      dispatch({ type: 'SET_LOADING', payload: true });
      dispatch({ type: 'SET_ERROR', payload: null });

      const response = await api.select({
        id: state.teamId,
        problemName: selectedProblem,
      });

      dispatch({
        type: 'START_SESSION',
        payload: {
          sessionId: response.session_id,
          problemName: response.problemName,
        },
      });
    } catch (error) {
      if (error instanceof ApiError) {
        if (error.status === 409) {
          dispatch({ type: 'SET_ERROR', payload: 'A session is already active. Please complete or reset it first.' });
        } else {
          dispatch({ type: 'SET_ERROR', payload: error.message });
        }
      } else {
        dispatch({ type: 'SET_ERROR', payload: 'Failed to start session' });
      }
    } finally {
      dispatch({ type: 'SET_LOADING', payload: false });
    }
  };

  const handleTeamIdChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    dispatch({ type: 'SET_TEAM_ID', payload: e.target.value });
  };

  if (state.phase !== 'problem-selection' && state.phase !== 'idle') {
    return null;
  }

  return (
    <div style={{ padding: '20px', maxWidth: '500px', margin: '0 auto' }}>
      <h2>Select Problem</h2>
      
      <div style={{ marginBottom: '20px' }}>
        <label style={{ display: 'block', marginBottom: '5px' }}>
          Team ID (from /register):
        </label>
        <input
          type="text"
          value={state.teamId}
          onChange={handleTeamIdChange}
          placeholder="Enter your team ID"
          style={{
            width: '100%',
            padding: '8px',
            fontSize: '16px',
            border: '1px solid #ccc',
            borderRadius: '4px',
          }}
        />
      </div>

      <div style={{ marginBottom: '20px' }}>
        <label style={{ display: 'block', marginBottom: '5px' }}>
          Problem Name:
        </label>
        <select
          value={selectedProblem}
          onChange={(e) => setSelectedProblem(e.target.value)}
          style={{
            width: '100%',
            padding: '8px',
            fontSize: '16px',
            border: '1px solid #ccc',
            borderRadius: '4px',
          }}
        >
          {PROBLEM_OPTIONS.map(problem => (
            <option key={problem} value={problem}>
              {problem}
            </option>
          ))}
        </select>
      </div>

      {state.error && (
        <div style={{
          padding: '10px',
          backgroundColor: '#f8d7da',
          color: '#721c24',
          borderRadius: '4px',
          marginBottom: '20px',
        }}>
          {state.error}
        </div>
      )}

      <button
        onClick={handleSelect}
        disabled={state.isLoading || !state.teamId.trim()}
        style={{
          width: '100%',
          padding: '12px',
          fontSize: '16px',
          backgroundColor: state.isLoading || !state.teamId.trim() ? '#ccc' : '#007bff',
          color: 'white',
          border: 'none',
          borderRadius: '4px',
          cursor: state.isLoading || !state.teamId.trim() ? 'not-allowed' : 'pointer',
        }}
      >
        {state.isLoading ? 'Starting Session...' : 'Start Session'}
      </button>

      <div style={{ marginTop: '20px', fontSize: '14px', color: '#666' }}>
        <p><strong>Note:</strong> You need to register at the ICFPC website first to get your team ID.</p>
        <p>Only one session can be active at a time.</p>
      </div>
    </div>
  );
}