import React, { useState } from 'react';
import { useSession } from '../hooks/useSession';
import { api, ApiError } from '../services/api';

const PROBLEM_OPTIONS = [
  'probatio',
  'primus',
  'secundus',
  'tertius',
  'quartus',
  'quintus',
];

export default function ProblemSelector() {
  const { state, dispatch } = useSession();
  const [selectedProblem, setSelectedProblem] = useState(PROBLEM_OPTIONS[0]);
  const [userName, setUserName] = useState('');

  const handleSelect = async () => {
    try {
      dispatch({ type: 'SET_LOADING', payload: true });
      dispatch({ type: 'SET_ERROR', payload: null });

      const response = await api.select({
        problemName: selectedProblem,
        user_name: userName.trim() || undefined,
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

  if (state.phase !== 'problem-selection') {
    return null;
  }

  return (
    <div style={{ padding: '20px', maxWidth: '500px', margin: '0 auto' }}>
      <h2>Select Problem</h2>
      
      <div style={{ marginBottom: '20px' }}>
        <label style={{ display: 'block', marginBottom: '5px' }}>
          User Name (optional):
        </label>
        <input
          type="text"
          value={userName}
          onChange={(e) => setUserName(e.target.value)}
          placeholder="Enter your name (optional)"
          style={{
            width: '100%',
            padding: '8px',
            fontSize: '16px',
            border: '1px solid #ccc',
            borderRadius: '4px',
            marginBottom: '15px',
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
        disabled={state.isLoading}
        style={{
          width: '100%',
          padding: '12px',
          fontSize: '16px',
          backgroundColor: state.isLoading ? '#ccc' : '#007bff',
          color: 'white',
          border: 'none',
          borderRadius: '4px',
          cursor: state.isLoading ? 'not-allowed' : 'pointer',
        }}
      >
        {state.isLoading ? 'Starting Session...' : 'Start Session'}
      </button>

      <div style={{ marginTop: '20px', fontSize: '14px', color: '#666' }}>
        <p><strong>Note:</strong> Team authentication is handled automatically by the server.</p>
        <p>Only one session can be active at a time.</p>
      </div>
    </div>
  );
}