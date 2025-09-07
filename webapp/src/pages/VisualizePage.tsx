import React, { useState, useEffect } from 'react';
import { useLocation, Link } from 'react-router-dom';
import {
  MapStruct,
  ApiLog,
  ExploreState,
  ExploreStep,
  SessionDetail,
} from '../types';
import { api } from '../services/api';
import MapInput from '../components/MapInput';
import MapVisualizer from '../components/MapVisualizer';
import ExploreInput from '../components/ExploreInput.tsx';
import ExploreVisualizer from '../components/ExploreVisualizer.tsx';
import { parseExploreString, predictObservedLabels } from '../utils/explore';

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
  const [exploreOptions, setExploreOptions] = useState<
    {
      key: string;
      logId: number;
      planIndex: number;
      plan: string;
      label: string;
    }[]
  >([]);
  const [selectedExploreKey, setSelectedExploreKey] = useState<string | null>(
    null
  );
  const [exploreResponses, setExploreResponses] = useState<
    Record<number, { results: number[][]; queryCount?: number }>
  >({});

  // Helper function to extract map from the last guess request
  const extractMapFromLastGuessRequest = (
    apiLogs: ApiLog[]
  ): MapStruct | null => {
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

  // Extract explore plans from session detail
  const extractExploreOptionsFromSessionDetail = (detail: SessionDetail) => {
    const options: {
      key: string;
      logId: number;
      planIndex: number;
      plan: string;
      label: string;
    }[] = [];
    const responses: Record<
      number,
      { results: number[][]; queryCount?: number }
    > = {};

    detail.api_logs
      .filter((log) => log.endpoint === 'explore' && log.request_body)
      .sort(
        (a, b) =>
          new Date(a.created_at).getTime() - new Date(b.created_at).getTime()
      )
      .forEach((log) => {
        try {
          const req = JSON.parse(log.request_body || '{}') as {
            plans?: string[];
          };
          const plans = req.plans || [];
          plans.forEach((plan, idx) => {
            const shortPlan =
              plan.length > 40 ? `${plan.slice(0, 37)}...` : plan;
            const ts = new Date(log.created_at).toLocaleString('ja-JP');
            options.push({
              key: `${log.id}-${idx}`,
              logId: log.id,
              planIndex: idx,
              plan,
              label: `#${log.id} @ ${ts} (plan ${idx + 1}): ${shortPlan}`,
            });
          });
          if (log.response_body) {
            try {
              const resp = JSON.parse(log.response_body) as {
                results?: number[][];
                queryCount?: number;
              };
              if (Array.isArray(resp.results)) {
                responses[log.id] = {
                  results: resp.results,
                  queryCount: resp.queryCount,
                };
              }
            } catch (e) {
              // ignore malformed response
            }
          }
        } catch (e) {
          // ignore malformed
        }
      });

    setExploreOptions(options);
    setExploreResponses(responses);
    if (options.length > 0) {
      setSelectedExploreKey(options[0].key);
    } else {
      setSelectedExploreKey(null);
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
      // If we know the session, also fetch explore options
      if (state.sessionId) {
        (async () => {
          try {
            const sessionDetail = await api.getSessionDetail(state.sessionId);
            extractExploreOptionsFromSessionDetail(sessionDetail);
          } catch (err) {
            // ignore explore options failure
          }
        })();
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

          // Also extract explore plans
          extractExploreOptionsFromSessionDetail(sessionDetail);
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
  };

  const handleLoadSelectedExplore = () => {
    if (!selectedExploreKey) return;
    const option = exploreOptions.find((o) => o.key === selectedExploreKey);
    if (!option) return;
    try {
      const steps = parseExploreString(option.plan);
      setExploreSteps(steps);
      setError(null);
    } catch (e) {
      setError((e as Error).message);
    }
  };

  const selectedOption = selectedExploreKey
    ? exploreOptions.find((o) => o.key === selectedExploreKey) || null
    : null;
  const actualLabels = selectedOption
    ? exploreResponses[selectedOption.logId]?.results?.[
        selectedOption.planIndex
      ] || null
    : null;
  const expectedLabels =
    map && exploreSteps.length > 0
      ? predictObservedLabels(map, exploreSteps)
      : null;

  const renderLabelsRow = (
    labels: number[] | null,
    compareTo?: number[] | null
  ) => {
    if (!labels) return <span style={{ color: '#6c757d' }}>N/A</span>;
    const maxLen = Math.max(labels.length, compareTo?.length || 0);
    const items = [] as JSX.Element[];
    for (let i = 0; i < maxLen; i++) {
      const v = labels[i];
      const other = compareTo ? compareTo[i] : undefined;
      const match = other === undefined ? true : v === other;
      const bg =
        other === undefined ? '#e9ecef' : match ? '#d4edda' : '#f8d7da';
      const color =
        other === undefined ? '#495057' : match ? '#155724' : '#721c24';
      items.push(
        <span
          key={i}
          title={`index ${i}${other !== undefined ? `, expected ${other}` : ''}`}
          style={{
            display: 'inline-block',
            minWidth: 22,
            padding: '2px 6px',
            marginRight: 4,
            borderRadius: 4,
            backgroundColor: bg,
            color,
            textAlign: 'center',
            fontFamily: 'monospace',
          }}
        >
          {v === undefined ? '—' : v}
        </span>
      );
    }
    return <div style={{ display: 'flex', flexWrap: 'wrap' }}>{items}</div>;
  };

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
            {sessionInfo && (
              <div
                style={{
                  display: 'flex',
                  gap: '8px',
                  alignItems: 'center',
                  marginBottom: '12px',
                }}
              >
                <span style={{ fontSize: '14px', color: '#495057' }}>
                  Explore選択:
                </span>
                <select
                  value={selectedExploreKey ?? ''}
                  onChange={(e) =>
                    setSelectedExploreKey(e.target.value || null)
                  }
                  style={{ padding: '6px 8px', minWidth: '320px' }}
                >
                  {exploreOptions.length === 0 ? (
                    <option value="" disabled>
                      セッション内にExploreが見つかりません
                    </option>
                  ) : (
                    exploreOptions.map((opt) => (
                      <option key={opt.key} value={opt.key}>
                        {opt.label}
                      </option>
                    ))
                  )}
                </select>
                <button
                  onClick={handleLoadSelectedExplore}
                  disabled={!selectedExploreKey}
                  style={{
                    padding: '6px 12px',
                    backgroundColor: '#007bff',
                    color: 'white',
                    border: 'none',
                    borderRadius: '4px',
                    cursor: selectedExploreKey ? 'pointer' : 'not-allowed',
                    fontSize: '12px',
                  }}
                >
                  選択を読み込む
                </button>
              </div>
            )}
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
                下の入力欄からExplore文字列を入力、または上でExploreを選択して読み込み
              </div>
            )}

            {/* Expected vs Actual response comparison */}
            {selectedOption && (
              <div
                style={{
                  marginTop: '12px',
                  borderTop: '1px solid #dee2e6',
                  paddingTop: '12px',
                }}
              >
                <div style={{ marginBottom: 6, color: '#343a40' }}>
                  <strong>選択中のExplore:</strong>{' '}
                  <code>{selectedOption.plan}</code>
                </div>
                <div
                  style={{
                    display: 'grid',
                    gridTemplateColumns: '160px 1fr',
                    rowGap: 8,
                    columnGap: 10,
                  }}
                >
                  <div style={{ color: '#495057' }}>Expected (from map):</div>
                  <div>{renderLabelsRow(expectedLabels, actualLabels)}</div>
                  <div style={{ color: '#495057' }}>Actual (API):</div>
                  <div>{renderLabelsRow(actualLabels, expectedLabels)}</div>
                </div>
                {expectedLabels && actualLabels && (
                  <div style={{ marginTop: 8 }}>
                    {JSON.stringify(expectedLabels) ===
                    JSON.stringify(actualLabels) ? (
                      <span
                        style={{
                          color: '#155724',
                          background: '#d4edda',
                          padding: '4px 8px',
                          borderRadius: 4,
                        }}
                      >
                        ✅ 完全一致しました
                      </span>
                    ) : (
                      <span
                        style={{
                          color: '#721c24',
                          background: '#f8d7da',
                          padding: '4px 8px',
                          borderRadius: 4,
                        }}
                      >
                        ⚠️ 期待値とAPIレスポンスが異なります
                      </span>
                    )}
                  </div>
                )}
                {/* Raw API response snippet */}
                {actualLabels && (
                  <div style={{ marginTop: 10 }}>
                    <div style={{ color: '#495057', marginBottom: 4 }}>
                      APIレスポンス（該当プラン）:
                    </div>
                    <pre
                      style={{
                        backgroundColor: '#f8f9fa',
                        padding: '8px',
                        borderRadius: '4px',
                        border: '1px solid #e9ecef',
                        maxHeight: 160,
                        overflow: 'auto',
                        margin: 0,
                      }}
                    >
                      {JSON.stringify(actualLabels)}
                    </pre>
                    {selectedOption &&
                      exploreResponses[selectedOption.logId]?.queryCount !==
                        undefined && (
                        <div style={{ marginTop: 6, color: '#6c757d' }}>
                          queryCount:{' '}
                          {exploreResponses[selectedOption.logId]?.queryCount}
                        </div>
                      )}
                  </div>
                )}
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
