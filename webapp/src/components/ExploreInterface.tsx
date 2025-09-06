import React, { useState } from 'react';
import { useSession } from '../hooks/useSession';
import { api, ApiError } from '../services/api';

export default function ExploreInterface() {
  const { state, dispatch } = useSession();
  const [plans, setPlans] = useState<string[]>(['']);
  const [currentPlan, setCurrentPlan] = useState('');

  const handleAddPlan = () => {
    if (currentPlan.trim()) {
      setPlans([...plans.filter(p => p.trim()), currentPlan.trim()]);
      setCurrentPlan('');
    }
  };

  const handleRemovePlan = (index: number) => {
    setPlans(plans.filter((_, i) => i !== index));
  };

  const handleExplore = async () => {
    if (!state.sessionId || !state.teamId) {
      dispatch({ type: 'SET_ERROR', payload: 'No active session' });
      return;
    }

    const validPlans = plans.filter(p => p.trim());
    if (validPlans.length === 0) {
      dispatch({ type: 'SET_ERROR', payload: 'At least one plan is required' });
      return;
    }

    try {
      dispatch({ type: 'SET_LOADING', payload: true });
      dispatch({ type: 'SET_ERROR', payload: null });

      const response = await api.explore({
        session_id: state.sessionId,
        id: state.teamId,
        plans: validPlans,
      });

      dispatch({
        type: 'ADD_EXPLORATION_RESULTS',
        payload: {
          results: response.results,
          queryCount: response.queryCount,
        },
      });
    } catch (error) {
      if (error instanceof ApiError) {
        dispatch({ type: 'SET_ERROR', payload: error.message });
      } else {
        dispatch({ type: 'SET_ERROR', payload: 'Failed to explore' });
      }
    } finally {
      dispatch({ type: 'SET_LOADING', payload: false });
    }
  };

  const handleFinishExploring = () => {
    dispatch({ type: 'SET_PHASE', payload: 'building-map' });
  };

  if (state.phase !== 'exploring') {
    return null;
  }

  return (
    <div style={{ padding: '20px', maxWidth: '800px', margin: '0 auto' }}>
      <h2>Explore Library</h2>
      
      <div style={{ marginBottom: '20px', padding: '10px', backgroundColor: '#e7f3ff', borderRadius: '4px' }}>
        <p><strong>Session:</strong> {state.sessionId}</p>
        <p><strong>Problem:</strong> {state.problemName}</p>
        <p><strong>Query Count:</strong> {state.queryCount}</p>
      </div>

      <div style={{ display: 'flex', gap: '20px' }}>
        {/* Left Panel - Plan Input */}
        <div style={{ flex: 1 }}>
          <h3>Exploration Plans</h3>
          
          <div style={{ marginBottom: '10px' }}>
            <input
              type="text"
              value={currentPlan}
              onChange={(e) => setCurrentPlan(e.target.value)}
              placeholder="Enter plan (e.g., NESW)"
              style={{
                width: '100%',
                padding: '8px',
                fontSize: '16px',
                border: '1px solid #ccc',
                borderRadius: '4px',
              }}
              onKeyPress={(e) => {
                if (e.key === 'Enter') {
                  handleAddPlan();
                }
              }}
            />
            <button
              onClick={handleAddPlan}
              disabled={!currentPlan.trim()}
              style={{
                marginTop: '5px',
                padding: '8px 16px',
                backgroundColor: !currentPlan.trim() ? '#ccc' : '#28a745',
                color: 'white',
                border: 'none',
                borderRadius: '4px',
                cursor: !currentPlan.trim() ? 'not-allowed' : 'pointer',
              }}
            >
              Add Plan
            </button>
          </div>

          <div style={{ marginBottom: '20px' }}>
            <h4>Current Plans:</h4>
            {plans.filter(p => p.trim()).length === 0 ? (
              <p style={{ color: '#666', fontStyle: 'italic' }}>No plans added yet</p>
            ) : (
              <ul style={{ listStyle: 'none', padding: 0 }}>
                {plans.filter(p => p.trim()).map((plan, index) => (
                  <li key={index} style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                    padding: '8px',
                    backgroundColor: '#f8f9fa',
                    marginBottom: '5px',
                    borderRadius: '4px',
                  }}>
                    <code>{plan}</code>
                    <button
                      onClick={() => handleRemovePlan(index)}
                      style={{
                        padding: '4px 8px',
                        backgroundColor: '#dc3545',
                        color: 'white',
                        border: 'none',
                        borderRadius: '4px',
                        cursor: 'pointer',
                        fontSize: '12px',
                      }}
                    >
                      Remove
                    </button>
                  </li>
                ))}
              </ul>
            )}
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

          <div style={{ display: 'flex', gap: '10px' }}>
            <button
              onClick={handleExplore}
              disabled={state.isLoading || plans.filter(p => p.trim()).length === 0}
              style={{
                flex: 1,
                padding: '12px',
                fontSize: '16px',
                backgroundColor: state.isLoading || plans.filter(p => p.trim()).length === 0 ? '#ccc' : '#007bff',
                color: 'white',
                border: 'none',
                borderRadius: '4px',
                cursor: state.isLoading || plans.filter(p => p.trim()).length === 0 ? 'not-allowed' : 'pointer',
              }}
            >
              {state.isLoading ? 'Exploring...' : 'Explore'}
            </button>
            
            <button
              onClick={handleFinishExploring}
              disabled={state.explorationResults.length === 0}
              style={{
                flex: 1,
                padding: '12px',
                fontSize: '16px',
                backgroundColor: state.explorationResults.length === 0 ? '#ccc' : '#28a745',
                color: 'white',
                border: 'none',
                borderRadius: '4px',
                cursor: state.explorationResults.length === 0 ? 'not-allowed' : 'pointer',
              }}
            >
              Build Map
            </button>
          </div>
        </div>

        {/* Right Panel - Results */}
        <div style={{ flex: 1 }}>
          <h3>Exploration Results</h3>
          
          {state.explorationResults.length === 0 ? (
            <p style={{ color: '#666', fontStyle: 'italic' }}>No exploration results yet</p>
          ) : (
            <div style={{ maxHeight: '400px', overflowY: 'auto' }}>
              {state.explorationResults.map((result, index) => (
                <div key={index} style={{
                  padding: '10px',
                  backgroundColor: '#f8f9fa',
                  marginBottom: '10px',
                  borderRadius: '4px',
                  fontSize: '14px',
                }}>
                  <strong>Result {index + 1}:</strong>
                  <pre style={{ margin: '5px 0', fontFamily: 'monospace' }}>
                    {JSON.stringify(result, null, 2)}
                  </pre>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>

      <div style={{ marginTop: '20px', fontSize: '14px', color: '#666' }}>
        <p><strong>Instructions:</strong></p>
        <ul>
          <li>Enter plans using direction letters (N, E, S, W)</li>
          <li>Each plan represents a sequence of moves through the library</li>
          <li>Results show room labels encountered during exploration</li>
          <li>Use multiple exploration rounds to gather enough information</li>
        </ul>
      </div>
    </div>
  );
}