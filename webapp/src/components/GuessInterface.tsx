import React, { useState } from 'react';
import { useSession } from '../hooks/useSession';
import { api, ApiError } from '../services/api';
import { Map } from '../types';

interface Props {
  map: Map;
}

export default function GuessInterface({ map }: Props) {
  const { state, dispatch } = useSession();
  const [result, setResult] = useState<{ correct: boolean } | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const handleSubmitGuess = async () => {
    if (!state.sessionId || !state.teamId) {
      dispatch({ type: 'SET_ERROR', payload: 'No active session' });
      return;
    }

    try {
      setIsSubmitting(true);
      dispatch({ type: 'SET_ERROR', payload: null });

      const response = await api.guess({
        session_id: state.sessionId,
        id: state.teamId,
        map,
      });

      setResult({ correct: response.correct });
      dispatch({ type: 'SET_PHASE', payload: 'completed' });
    } catch (error) {
      if (error instanceof ApiError) {
        dispatch({ type: 'SET_ERROR', payload: error.message });
      } else {
        dispatch({ type: 'SET_ERROR', payload: 'Failed to submit guess' });
      }
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleStartNew = () => {
    setResult(null);
    dispatch({ type: 'RESET_SESSION' });
  };

  if (state.phase !== 'completed') {
    return null;
  }

  return (
    <div style={{ padding: '20px', maxWidth: '800px', margin: '0 auto' }}>
      <h2>Submit Final Guess</h2>
      
      <div style={{ marginBottom: '20px', padding: '10px', backgroundColor: '#e7f3ff', borderRadius: '4px' }}>
        <p><strong>Session:</strong> {state.sessionId}</p>
        <p><strong>Problem:</strong> {state.problemName}</p>
        <p><strong>Total Explorations:</strong> {state.explorationResults.length}</p>
        <p><strong>Final Query Count:</strong> {state.queryCount}</p>
      </div>

      {result ? (
        // Show result after submission
        <div style={{ textAlign: 'center' }}>
          <div
            style={{
              padding: '20px',
              borderRadius: '8px',
              marginBottom: '20px',
              backgroundColor: result.correct ? '#d4edda' : '#f8d7da',
              border: `2px solid ${result.correct ? '#28a745' : '#dc3545'}`,
            }}
          >
            <h3 style={{ 
              color: result.correct ? '#155724' : '#721c24',
              margin: '0 0 10px 0'
            }}>
              {result.correct ? '🎉 Correct!' : '❌ Incorrect'}
            </h3>
            <p style={{ 
              color: result.correct ? '#155724' : '#721c24',
              margin: 0
            }}>
              {result.correct 
                ? 'Congratulations! Your map was correct.'
                : 'Your map was incorrect. Better luck next time!'
              }
            </p>
          </div>

          <button
            onClick={handleStartNew}
            style={{
              padding: '12px 24px',
              fontSize: '16px',
              backgroundColor: '#007bff',
              color: 'white',
              border: 'none',
              borderRadius: '4px',
              cursor: 'pointer',
            }}
          >
            Start New Session
          </button>
        </div>
      ) : (
        // Show map confirmation and submit button
        <div>
          <h3>Final Map</h3>
          <div style={{
            backgroundColor: '#f8f9fa',
            padding: '20px',
            borderRadius: '4px',
            marginBottom: '20px',
          }}>
            <div style={{ marginBottom: '15px' }}>
              <strong>Rooms:</strong> [{map.rooms.join(', ')}]
            </div>
            <div style={{ marginBottom: '15px' }}>
              <strong>Starting Room:</strong> {map.startingRoom}
            </div>
            <div>
              <strong>Connections:</strong>
              {map.connections.length === 0 ? (
                <span style={{ color: '#666', fontStyle: 'italic' }}> None</span>
              ) : (
                <ul style={{ marginTop: '5px', paddingLeft: '20px' }}>
                  {map.connections.map((conn, index) => (
                    <li key={index} style={{ marginBottom: '5px' }}>
                      Room {conn.from.room} Door {conn.from.door} ↔ Room {conn.to.room} Door {conn.to.door}
                    </li>
                  ))}
                </ul>
              )}
            </div>
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

          <div style={{ textAlign: 'center' }}>
            <p style={{ marginBottom: '20px', color: '#666' }}>
              <strong>Warning:</strong> Once you submit your guess, the session will be terminated.
              Make sure your map is complete and correct.
            </p>

            <button
              onClick={handleSubmitGuess}
              disabled={isSubmitting}
              style={{
                padding: '12px 24px',
                fontSize: '18px',
                backgroundColor: isSubmitting ? '#ccc' : '#dc3545',
                color: 'white',
                border: 'none',
                borderRadius: '4px',
                cursor: isSubmitting ? 'not-allowed' : 'pointer',
                minWidth: '200px',
              }}
            >
              {isSubmitting ? 'Submitting...' : 'Submit Final Guess'}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}