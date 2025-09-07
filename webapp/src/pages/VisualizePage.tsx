import React, { useState, useEffect } from 'react';
import { useLocation, Link } from 'react-router-dom';
import {MapStruct, ApiLog, ExploreState, ExploreStep} from '../types';
import { api } from '../services/api';
import MapInput from '../components/MapInput';
import MapVisualizer from '../components/MapVisualizer';
import ExploreInput from "../components/ExploreInput.tsx";
import ExploreVisualizer from "../components/ExploreVisualizer.tsx";

export default function VisualizePage() {
  const [map, setMap] = useState<MapStruct | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [sessionInfo, setSessionInfo] = useState<{
    sessionId?: string;
    logId?: number;
  } | null>(null);
  const [loading, setLoading] = useState(false);
  const location = useLocation();
  const [exploreSteps, setExploreSteps] = useState<ExploreStep[]>([]);
  const [exploreState, setExploreState] = useState<ExploreState | null>(null);

  // Helper function to extract map from the last guess request
  const extractMapFromLastGuessRequest = (apiLogs: ApiLog[]): MapStruct | null => {
    const guessLogs = apiLogs
      .filter((log) => log.endpoint === 'guess' && log.request_body)
      .sort(
        (a, b) =>
          new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
      );

    if (guessLogs.length === 0) return null;

    try {
      const requestData = JSON.parse(guessLogs[0].request_body!);
      return requestData.map || null;
    } catch {
      return null;
    }
  };

  // Handle map data passed from session history or fetch from session
  useEffect(() => {
    const state = location.state as any;

    // If map is directly provided (from detailed log view)
    if (state?.map) {
      setMap(state.map);
      setError(null);
      if (state.sessionId && state.logId) {
        setSessionInfo({ sessionId: state.sessionId, logId: state.logId });
      }
      return;
    }

    // If only sessionId is provided, fetch session details and extract map
    if (state?.sessionId && !state?.logId) {
      const fetchSessionMap = async () => {
        try {
          setLoading(true);
          setError(null);
          const sessionDetail = await api.getSessionDetail(state.sessionId);
          const extractedMap = extractMapFromLastGuessRequest(
            sessionDetail.api_logs
          );

          if (extractedMap) {
            setMap(extractedMap);
            setSessionInfo({ sessionId: state.sessionId });
          } else {
            setError('このセッションにはguessリクエストが見つかりませんでした');
          }
        } catch (err) {
          console.error('Failed to fetch session details:', err);
          setError('セッションデータの取得に失敗しました');
        } finally {
          setLoading(false);
        }
      };

      fetchSessionMap();
    }
  }, [location.state]);

  const handleMapLoad = (loadedMap: MapStruct) => {
    setMap(loadedMap);
    setError(null);
    setSessionInfo(null); // Clear session info when manually loading a new map
  };

  const handleError = (errorMessage: string) => {
    setError(errorMessage);
    setMap(null);
  };

  const handleReset = () => {
    setMap(null);
    setError(null);
  };

  const handleExploreLoad = (newSteps: ExploreStep[]) => {
      setExploreSteps(newSteps);
  };
  const handleExploreStateChange = (state: ExploreState | null) => {
      console.log('Explore state changed:', state);
      setExploreState(state);
  }

  return (
    <div
      style={{
        minHeight: '100vh',
        backgroundColor: '#f8f9fa',
        padding: '12px',
      }}
    >
      <div style={{ margin: '0 auto' }}>
        <div
          style={{
            backgroundColor: 'white',
            borderRadius: '8px',
            padding: '12px',
            marginBottom: '12px',
            boxShadow: '0 4px 6px rgba(0, 0, 0, 0.1)',
          }}
        >
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              marginBottom: '12px',
            }}
          >
            <h1 style={{ margin: 0, color: '#343a40', flexGrow: 1 }}>
              Map Visualizer
            </h1>
            <Link
              to="/sessions"
              style={{
                padding: '8px 16px',
                backgroundColor: '#6c757d',
                color: 'white',
                textDecoration: 'none',
                borderRadius: '4px',
                fontSize: '14px',
              }}
            >
              ← Back to Sessions
            </Link>
          </div>

          {sessionInfo && (
            <div
              style={{
                backgroundColor: '#d1ecf1',
                border: '1px solid #bee5eb',
                borderRadius: '4px',
                padding: '12px',
                marginBottom: '15px',
              }}
            >
              <div style={{ color: '#0c5460', fontSize: '14px' }}>
                <strong>🗂️ From Session History:</strong> Session ID:{' '}
                {sessionInfo.sessionId?.substring(0, 8)}...
                {sessionInfo.logId
                  ? ` (Log #${sessionInfo.logId})`
                  : ' (Latest guess request)'}
              </div>
            </div>
          )}

          <p style={{ color: '#6c757d', marginBottom: '20px' }}>
            {sessionInfo
              ? 'Viewing map from a guess request in session history.'
              : 'Upload or paste a Map JSON to visualize the library layout. Each room will be displayed as a hexagon with doors on each side.'}
          </p>

          <div
            style={{
              display: 'flex',
              gap: '10px',
              alignItems: 'center',
              marginBottom: '20px',
            }}
          >
            <button
              onClick={handleReset}
              style={{
                padding: '8px 16px',
                backgroundColor: '#6c757d',
                color: 'white',
                border: 'none',
                borderRadius: '4px',
                cursor: 'pointer',
              }}
            >
              Reset
            </button>
            <span style={{ fontSize: '14px', color: '#6c757d' }}>
              {map
                ? `Loaded map with ${map.rooms.length} rooms and ${map.connections.length} connections`
                : 'No map loaded'}
            </span>
          </div>
        </div>

        {/* Visualization - Top Panel */}
        <div
          style={{
            backgroundColor: 'white',
            borderRadius: '8px',
            padding: '12px',
            marginBottom: '12px',
            boxShadow: '0 4px 6px rgba(0, 0, 0, 0.1)',
            height: '600px',
          }}
        >
          {loading ? (
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                height: '100%',
                color: '#6c757d',
                fontSize: '18px',
              }}
            >
              <div style={{ textAlign: 'center' }}>
                <div style={{ marginBottom: '10px' }}>
                  📊 Loading session data...
                </div>
                <div style={{ fontSize: '14px' }}>
                  Extracting map from guess requests
                </div>
              </div>
            </div>
          ) : map ? (
            <MapVisualizer 
              map={map} 
              exploreState={exploreState}
              highlightCurrentRoom={exploreState?.currentRoom}
              pathHistory={exploreState?.pathHistory}
              chalkMarks={exploreState?.chalkMarks}
            />
          ) : (
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                height: '100%',
                color: '#6c757d',
                fontSize: '18px',
              }}
            >
              Load a map to see the visualization
            </div>
          )}
        </div>

        {/* Explore Controls Panel */}
        {map && (
          <div
            style={{
              backgroundColor: 'white',
              borderRadius: '8px',
              padding: '12px',
              marginBottom: '12px',
              boxShadow: '0 4px 6px rgba(0, 0, 0, 0.1)',
            }}
          >
            {exploreSteps.length > 0 ? (
              <ExploreVisualizer
                map={map}
                steps={exploreSteps}
                onStateChange={handleExploreStateChange}
              />
            ) : (
              <div
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  padding: '40px',
                  color: '#6c757d',
                  fontSize: '16px',
                }}
              >
                Enter an explore string below to start the simulation
              </div>
            )}
          </div>
        )}

        {/* Input Panel - Bottom */}
        <div
          style={{
            backgroundColor: 'white',
            borderRadius: '8px',
            padding: '12px',
            boxShadow: '0 4px 6px rgba(0, 0, 0, 0.1)',
          }}
        >
            <ExploreInput
                onExploreLoad={handleExploreLoad}
                onError={handleError}
                roomCount={map?.rooms.length || 0}
            />
        <MapInput onMapLoad={handleMapLoad} onError={handleError} />

          {error && (
            <div
              style={{
                marginTop: '15px',
                padding: '12px',
                backgroundColor: '#f8d7da',
                color: '#721c24',
                borderRadius: '4px',
                border: '1px solid #f5c6cb',
              }}
            >
              <strong>Error:</strong> {error}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
