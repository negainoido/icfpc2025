import React from 'react';
import { useSession } from '../hooks/useSession';
import ProblemSelector from '../components/ProblemSelector';
import ExploreInterface from '../components/ExploreInterface';
import MapBuilder from '../components/MapBuilder';
import GuessInterface from '../components/GuessInterface';
import { Map } from '../types';
import { api } from '../services/api';

export default function GamePage() {
  const { state, dispatch } = useSession();

  // Mock map for GuessInterface - in a real implementation, this would come from MapBuilder
  const mockMap: Map = {
    rooms: [1, 2, 3],
    startingRoom: 1,
    connections: [
      { from: { room: 1, door: 1 }, to: { room: 2, door: 1 } },
      { from: { room: 2, door: 2 }, to: { room: 3, door: 1 } },
    ],
  };

  const getPhaseTitle = () => {
    switch (state.phase) {
      case 'problem-selection':
        return 'Select a Problem';
      case 'exploring':
        return 'Explore the Library';
      case 'building-map':
        return 'Build Your Map';
      case 'completed':
        return 'Submit Your Guess';
      default:
        return 'ICFPC 2025 Library Explorer';
    }
  };

  const getPhaseDescription = () => {
    switch (state.phase) {
      case 'problem-selection':
        return 'Choose a library problem to solve. Team authentication is handled automatically by the server.';
      case 'exploring':
        return 'Send exploration plans to gather information about the library structure. Each plan returns room labels you encounter.';
      case 'building-map':
        return 'Use your exploration results to construct the complete library map with rooms and connections.';
      case 'completed':
        return 'Review your final map and submit your guess. The session will be terminated after submission.';
      default:
        return '';
    }
  };

  const handleReset = async () => {
    if (!state.sessionId) {
      dispatch({ type: 'RESET_SESSION' });
      return;
    }

    if (!window.confirm('現在のセッションを中止しますか？この操作は元に戻せません。')) {
      return;
    }

    try {
      dispatch({ type: 'SET_LOADING', payload: true });
      await api.abortSession(state.sessionId);
      dispatch({ type: 'RESET_SESSION' });
      alert('セッションを正常に中止しました');
    } catch (err) {
      console.error('Failed to abort session:', err);
      dispatch({ type: 'SET_ERROR', payload: 'セッションの中止に失敗しました' });
    }
  };

  return (
    <div style={{ minHeight: '100vh', backgroundColor: '#f8f9fa' }}>
      <div style={{
        backgroundColor: 'white',
        borderBottom: '1px solid #dee2e6',
        padding: '15px 20px',
        marginBottom: '20px',
      }}>
        <div style={{ maxWidth: '1000px', margin: '0 auto' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <div>
              <h1 style={{ margin: '0 0 5px 0', fontSize: '24px', color: '#343a40' }}>
                {getPhaseTitle()}
              </h1>
              <p style={{ margin: 0, color: '#6c757d', fontSize: '14px' }}>
                {getPhaseDescription()}
              </p>
            </div>
            
            {state.sessionId && (
              <button
                onClick={handleReset}
                disabled={state.isLoading}
                style={{
                  padding: '8px 16px',
                  backgroundColor: state.isLoading ? '#6c757d' : '#dc3545',
                  color: 'white',
                  border: 'none',
                  borderRadius: '4px',
                  cursor: state.isLoading ? 'not-allowed' : 'pointer',
                  fontSize: '14px',
                }}
              >
                {state.isLoading ? 'Aborting...' : 'Reset Session'}
              </button>
            )}
          </div>
        </div>
      </div>

      <div style={{ maxWidth: '1200px', margin: '0 auto', padding: '0 20px' }}>
        {/* Error Display */}
        {state.error && (
          <div style={{
            backgroundColor: '#f8d7da',
            border: '1px solid #f5c6cb',
            borderRadius: '6px',
            padding: '15px',
            marginBottom: '20px',
            color: '#721c24'
          }}>
            {state.error}
          </div>
        )}

        {/* Phase Progress Indicator */}
        <div style={{
          display: 'flex',
          justifyContent: 'center',
          marginBottom: '30px',
          padding: '20px',
          backgroundColor: 'white',
          borderRadius: '8px',
          boxShadow: '0 2px 4px rgba(0,0,0,0.1)',
        }}>
          {[
            { key: 'problem-selection', label: 'Select Problem', icon: '📋' },
            { key: 'exploring', label: 'Explore', icon: '🔍' },
            { key: 'building-map', label: 'Build Map', icon: '🗺️' },
            { key: 'completed', label: 'Submit Guess', icon: '🎯' },
          ].map((phase, index, array) => (
            <React.Fragment key={phase.key}>
              <div style={{ textAlign: 'center' }}>
                <div
                  style={{
                    width: '50px',
                    height: '50px',
                    borderRadius: '50%',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    fontSize: '20px',
                    backgroundColor: 
                      state.phase === phase.key ? '#007bff' :
                      ['problem-selection', 'exploring', 'building-map', 'completed'].indexOf(state.phase) > 
                      ['problem-selection', 'exploring', 'building-map', 'completed'].indexOf(phase.key) ? '#28a745' : '#e9ecef',
                    color: 
                      state.phase === phase.key || 
                      ['problem-selection', 'exploring', 'building-map', 'completed'].indexOf(state.phase) > 
                      ['problem-selection', 'exploring', 'building-map', 'completed'].indexOf(phase.key) ? 'white' : '#6c757d',
                    marginBottom: '8px',
                  }}
                >
                  {phase.icon}
                </div>
                <div style={{
                  fontSize: '12px',
                  fontWeight: state.phase === phase.key ? 'bold' : 'normal',
                  color: state.phase === phase.key ? '#007bff' : '#6c757d',
                }}>
                  {phase.label}
                </div>
              </div>
              
              {index < array.length - 1 && (
                <div style={{
                  flex: 1,
                  height: '2px',
                  backgroundColor: 
                    ['problem-selection', 'exploring', 'building-map', 'completed'].indexOf(state.phase) > index ? '#28a745' : '#e9ecef',
                  alignSelf: 'center',
                  margin: '0 20px',
                  maxWidth: '100px',
                }} />
              )}
            </React.Fragment>
          ))}
        </div>

        {/* Main Content Area */}
        <div style={{
          backgroundColor: 'white',
          borderRadius: '8px',
          boxShadow: '0 2px 4px rgba(0,0,0,0.1)',
          minHeight: '500px',
        }}>
          <ProblemSelector />
          <ExploreInterface />
          <MapBuilder />
          <GuessInterface map={mockMap} />
        </div>
      </div>

      {/* Footer */}
      <div style={{
        textAlign: 'center',
        padding: '30px 20px',
        color: '#6c757d',
        fontSize: '14px',
        marginTop: '40px',
      }}>
        <p>
          ICFPC 2025 Library Explorer | 
          <a href="https://icfpcontest2025.github.io/" target="_blank" rel="noopener noreferrer" style={{ color: '#007bff', marginLeft: '5px' }}>
            Official Website
          </a>
        </p>
      </div>
    </div>
  );
}