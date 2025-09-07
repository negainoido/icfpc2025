import React, { useState, useEffect } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { api } from '../services/api';
import { Session, SessionDetail, Map } from '../types';

const SessionsPage = () => {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [currentSession, setCurrentSession] = useState<Session | null>(null);
  const [selectedSession, setSelectedSession] = useState<SessionDetail | null>(
    null
  );
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showDetailModal, setShowDetailModal] = useState(false);
  const navigate = useNavigate();

  useEffect(() => {
    loadSessions();
    loadCurrentSession();
  }, []);

  const loadSessions = async () => {
    try {
      setLoading(true);
      const response = await api.getSessions();
      setSessions(response.sessions);
    } catch (err) {
      console.error('Failed to load sessions:', err);
      setError('セッション一覧の取得に失敗しました');
    } finally {
      setLoading(false);
    }
  };

  const loadCurrentSession = async () => {
    try {
      const current = await api.getCurrentSession();
      setCurrentSession(current);
    } catch (err) {
      console.error('Failed to load current session:', err);
    }
  };

  const openSessionDetail = async (session: Session) => {
    try {
      const detail = await api.getSessionDetail(session.session_id);
      setSelectedSession(detail);
      setShowDetailModal(true);
    } catch (err) {
      console.error('Failed to load session detail:', err);
      setError('セッション詳細の取得に失敗しました');
    }
  };

  const closeDetailModal = () => {
    setShowDetailModal(false);
    setSelectedSession(null);
  };

  const handleAbortSession = async (sessionId: string) => {
    if (
      !window.confirm(
        'このセッションを中止しますか？この操作は元に戻せません。'
      )
    ) {
      return;
    }

    try {
      await api.abortSession(sessionId);
      await loadSessions();
      await loadCurrentSession();
      setError(null);
    } catch (err) {
      console.error('Failed to abort session:', err);
      setError('セッションの中止に失敗しました');
    }
  };

  const handleExportSession = async (sessionId: string) => {
    try {
      const exportData = await api.exportSession(sessionId);

      // JSON文字列に変換
      const jsonString = JSON.stringify(exportData, null, 2);

      // ファイル名を生成
      const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
      const filename = `session_${sessionId.substring(0, 8)}_${timestamp}.json`;

      // ダウンロード処理
      const blob = new Blob([jsonString], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = filename;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
    } catch (err) {
      console.error('Failed to export session:', err);
      setError('セッションのエクスポートに失敗しました');
    }
  };

  const isGuessRequestWithMap = (log: any) => {
    return log.endpoint === 'guess' && log.request_body;
  };

  const extractMapFromGuessLog = (log: any): Map | null => {
    try {
      if (!isGuessRequestWithMap(log)) return null;
      const requestData = JSON.parse(log.request_body);
      return requestData.map || null;
    } catch {
      return null;
    }
  };

  const handleVisualizeMap = (log: any) => {
    const map = extractMapFromGuessLog(log);
    if (map) {
      navigate('/visualize', { state: { map, sessionId: log.session_id, logId: log.id } });
    }
  };

  const getStatusBadgeColor = (status: string) => {
    switch (status) {
      case 'active':
        return '#28a745';
      case 'completed':
        return '#007bff';
      case 'failed':
        return '#dc3545';
      default:
        return '#6c757d';
    }
  };

  const formatDateTime = (dateString: string) => {
    return new Date(dateString).toLocaleString('ja-JP');
  };

  const getSessionDuration = (session: Session) => {
    if (!session.completed_at) {
      const now = new Date();
      const start = new Date(session.created_at);
      const diff = now.getTime() - start.getTime();
      const minutes = Math.floor(diff / 60000);
      const seconds = Math.floor((diff % 60000) / 1000);
      return `${minutes}:${seconds.toString().padStart(2, '0')} (進行中)`;
    }

    const start = new Date(session.created_at);
    const end = new Date(session.completed_at);
    const diff = end.getTime() - start.getTime();
    const minutes = Math.floor(diff / 60000);
    const seconds = Math.floor((diff % 60000) / 1000);
    return `${minutes}:${seconds.toString().padStart(2, '0')}`;
  };

  if (loading) {
    return (
      <div
        style={{
          minHeight: '100vh',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: '#f8f9fa',
        }}
      >
        <div style={{ textAlign: 'center', color: '#6c757d' }}>
          <div>セッション情報を読み込んでいます...</div>
        </div>
      </div>
    );
  }

  return (
    <div
      style={{
        minHeight: '100vh',
        backgroundColor: '#f8f9fa',
        padding: '20px',
      }}
    >
      <div
        style={{
          maxWidth: '1200px',
          margin: '0 auto',
        }}
      >
        {/* Header */}
        <div
          style={{
            backgroundColor: 'white',
            borderRadius: '8px',
            padding: '30px',
            marginBottom: '20px',
            boxShadow: '0 2px 4px rgba(0,0,0,0.1)',
          }}
        >
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              marginBottom: '20px',
            }}
          >
            <h1 style={{ margin: 0, color: '#343a40' }}>セッション管理</h1>
            <div style={{ display: 'flex', gap: '10px' }}>
              <Link
                to="/game"
                style={{
                  padding: '10px 20px',
                  backgroundColor: '#007bff',
                  color: 'white',
                  textDecoration: 'none',
                  borderRadius: '6px',
                  fontSize: '14px',
                }}
              >
                新しいゲーム開始
              </Link>
              <Link
                to="/"
                style={{
                  padding: '10px 20px',
                  backgroundColor: '#6c757d',
                  color: 'white',
                  textDecoration: 'none',
                  borderRadius: '6px',
                  fontSize: '14px',
                }}
              >
                ホームに戻る
              </Link>
            </div>
          </div>

          {/* Current Session Info */}
          {currentSession ? (
            <div
              style={{
                backgroundColor: '#d4edda',
                border: '1px solid #c3e6cb',
                borderRadius: '6px',
                padding: '15px',
              }}
            >
              <div
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'flex-start',
                }}
              >
                <div>
                  <h3 style={{ margin: '0 0 10px 0', color: '#155724' }}>
                    現在のアクティブセッション
                  </h3>
                  <div style={{ color: '#155724' }}>
                    <strong>セッションID:</strong> {currentSession.session_id}
                    <br />
                    {currentSession.user_name && (
                      <>
                        <strong>ユーザー名:</strong> {currentSession.user_name}
                        <br />
                      </>
                    )}
                    <strong>開始時刻:</strong>{' '}
                    {formatDateTime(currentSession.created_at)}
                    <br />
                    <strong>継続時間:</strong>{' '}
                    {getSessionDuration(currentSession)}
                  </div>
                </div>
                <button
                  onClick={() => handleAbortSession(currentSession.session_id)}
                  style={{
                    padding: '8px 16px',
                    backgroundColor: '#dc3545',
                    color: 'white',
                    border: 'none',
                    borderRadius: '4px',
                    cursor: 'pointer',
                    fontSize: '14px',
                    fontWeight: 'bold',
                  }}
                >
                  セッション中止
                </button>
              </div>
            </div>
          ) : (
            <div
              style={{
                backgroundColor: '#f8d7da',
                border: '1px solid #f5c6cb',
                borderRadius: '6px',
                padding: '15px',
              }}
            >
              <div style={{ color: '#721c24' }}>
                現在アクティブなセッションはありません
              </div>
            </div>
          )}
        </div>

        {/* Sessions List */}
        <div
          style={{
            backgroundColor: 'white',
            borderRadius: '8px',
            padding: '30px',
            boxShadow: '0 2px 4px rgba(0,0,0,0.1)',
          }}
        >
          <h2 style={{ margin: '0 0 20px 0', color: '#343a40' }}>
            セッション履歴
          </h2>

          {error && (
            <div
              style={{
                backgroundColor: '#f8d7da',
                border: '1px solid #f5c6cb',
                borderRadius: '6px',
                padding: '15px',
                marginBottom: '20px',
                color: '#721c24',
              }}
            >
              {error}
            </div>
          )}

          {sessions.length === 0 ? (
            <div
              style={{
                textAlign: 'center',
                color: '#6c757d',
                padding: '40px',
              }}
            >
              セッション履歴はありません
            </div>
          ) : (
            <div style={{ overflowX: 'auto' }}>
              <table
                style={{
                  width: '100%',
                  borderCollapse: 'collapse',
                  fontSize: '14px',
                }}
              >
                <thead>
                  <tr style={{ backgroundColor: '#f8f9fa' }}>
                    <th
                      style={{
                        padding: '12px',
                        textAlign: 'left',
                        borderBottom: '2px solid #dee2e6',
                      }}
                    >
                      セッションID
                    </th>
                    <th
                      style={{
                        padding: '12px',
                        textAlign: 'left',
                        borderBottom: '2px solid #dee2e6',
                      }}
                    >
                      ユーザー名
                    </th>
                    <th
                      style={{
                        padding: '12px',
                        textAlign: 'left',
                        borderBottom: '2px solid #dee2e6',
                      }}
                    >
                      ステータス
                    </th>
                    <th
                      style={{
                        padding: '12px',
                        textAlign: 'left',
                        borderBottom: '2px solid #dee2e6',
                      }}
                    >
                      開始時刻
                    </th>
                    <th
                      style={{
                        padding: '12px',
                        textAlign: 'left',
                        borderBottom: '2px solid #dee2e6',
                      }}
                    >
                      終了時刻
                    </th>
                    <th
                      style={{
                        padding: '12px',
                        textAlign: 'left',
                        borderBottom: '2px solid #dee2e6',
                      }}
                    >
                      継続時間
                    </th>
                    <th
                      style={{
                        padding: '12px',
                        textAlign: 'center',
                        borderBottom: '2px solid #dee2e6',
                      }}
                    >
                      アクション
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {sessions.map((session) => (
                    <tr
                      key={session.session_id}
                      style={{ borderBottom: '1px solid #dee2e6' }}
                    >
                      <td
                        style={{
                          padding: '12px',
                          fontFamily: 'monospace',
                          fontSize: '12px',
                        }}
                      >
                        {session.session_id.substring(0, 8)}...
                      </td>
                      <td style={{ padding: '12px' }}>
                        {session.user_name || '-'}
                      </td>
                      <td style={{ padding: '12px' }}>
                        <span
                          style={{
                            padding: '4px 8px',
                            borderRadius: '4px',
                            fontSize: '12px',
                            fontWeight: 'bold',
                            color: 'white',
                            backgroundColor: getStatusBadgeColor(
                              session.status
                            ),
                          }}
                        >
                          {session.status}
                        </span>
                      </td>
                      <td style={{ padding: '12px' }}>
                        {formatDateTime(session.created_at)}
                      </td>
                      <td style={{ padding: '12px' }}>
                        {session.completed_at
                          ? formatDateTime(session.completed_at)
                          : '-'}
                      </td>
                      <td style={{ padding: '12px' }}>
                        {getSessionDuration(session)}
                      </td>
                      <td style={{ padding: '12px', textAlign: 'center' }}>
                        <div
                          style={{
                            display: 'flex',
                            gap: '5px',
                            justifyContent: 'center',
                          }}
                        >
                          <button
                            onClick={() => openSessionDetail(session)}
                            style={{
                              padding: '6px 12px',
                              backgroundColor: '#17a2b8',
                              color: 'white',
                              border: 'none',
                              borderRadius: '4px',
                              cursor: 'pointer',
                              fontSize: '12px',
                            }}
                          >
                            詳細を見る
                          </button>
                          <button
                            onClick={() =>
                              handleExportSession(session.session_id)
                            }
                            style={{
                              padding: '6px 12px',
                              backgroundColor: '#28a745',
                              color: 'white',
                              border: 'none',
                              borderRadius: '4px',
                              cursor: 'pointer',
                              fontSize: '12px',
                            }}
                          >
                            JSON出力
                          </button>
                          {session.status === 'active' && (
                            <button
                              onClick={() =>
                                handleAbortSession(session.session_id)
                              }
                              style={{
                                padding: '6px 12px',
                                backgroundColor: '#dc3545',
                                color: 'white',
                                border: 'none',
                                borderRadius: '4px',
                                cursor: 'pointer',
                                fontSize: '12px',
                              }}
                            >
                              中止
                            </button>
                          )}
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>

      {/* Session Detail Modal */}
      {showDetailModal && selectedSession && (
        <div
          style={{
            position: 'fixed',
            top: 0,
            left: 0,
            right: 0,
            bottom: 0,
            backgroundColor: 'rgba(0,0,0,0.5)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            zIndex: 1000,
          }}
        >
          <div
            style={{
              backgroundColor: 'white',
              borderRadius: '8px',
              padding: '30px',
              maxWidth: '80%',
              maxHeight: '80%',
              overflow: 'auto',
              minWidth: '600px',
            }}
          >
            <div
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                marginBottom: '20px',
              }}
            >
              <h2 style={{ margin: 0, color: '#343a40' }}>セッション詳細</h2>
              <button
                onClick={closeDetailModal}
                style={{
                  background: 'none',
                  border: 'none',
                  fontSize: '24px',
                  cursor: 'pointer',
                  color: '#6c757d',
                }}
              >
                ×
              </button>
            </div>

            <div style={{ marginBottom: '20px' }}>
              <h3 style={{ color: '#343a40', marginBottom: '10px' }}>
                セッション情報
              </h3>
              <div
                style={{
                  backgroundColor: '#f8f9fa',
                  padding: '15px',
                  borderRadius: '6px',
                  fontSize: '14px',
                }}
              >
                <div>
                  <strong>セッションID:</strong>{' '}
                  {selectedSession.session.session_id}
                </div>
                {selectedSession.session.user_name && (
                  <div>
                    <strong>ユーザー名:</strong>{' '}
                    {selectedSession.session.user_name}
                  </div>
                )}
                <div>
                  <strong>ステータス:</strong>
                  <span
                    style={{
                      padding: '2px 6px',
                      borderRadius: '4px',
                      fontSize: '12px',
                      fontWeight: 'bold',
                      color: 'white',
                      backgroundColor: getStatusBadgeColor(
                        selectedSession.session.status
                      ),
                      marginLeft: '8px',
                    }}
                  >
                    {selectedSession.session.status}
                  </span>
                </div>
                <div>
                  <strong>開始時刻:</strong>{' '}
                  {formatDateTime(selectedSession.session.created_at)}
                </div>
                {selectedSession.session.completed_at && (
                  <div>
                    <strong>終了時刻:</strong>{' '}
                    {formatDateTime(selectedSession.session.completed_at)}
                  </div>
                )}
                <div>
                  <strong>継続時間:</strong>{' '}
                  {getSessionDuration(selectedSession.session)}
                </div>
              </div>
            </div>

            <div>
              <h3 style={{ color: '#343a40', marginBottom: '10px' }}>
                APIログ履歴 ({selectedSession.api_logs.length}件)
              </h3>
              {selectedSession.api_logs.length === 0 ? (
                <div
                  style={{
                    textAlign: 'center',
                    color: '#6c757d',
                    padding: '20px',
                  }}
                >
                  APIログはありません
                </div>
              ) : (
                <div style={{ maxHeight: '400px', overflowY: 'auto' }}>
                  {selectedSession.api_logs.map((log) => (
                    <div
                      key={log.id}
                      style={{
                        border: '1px solid #dee2e6',
                        borderRadius: '6px',
                        padding: '15px',
                        marginBottom: '10px',
                        fontSize: '14px',
                      }}
                    >
                      <div
                        style={{
                          display: 'flex',
                          justifyContent: 'space-between',
                          alignItems: 'center',
                          marginBottom: '10px',
                        }}
                      >
                        <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                          <div>
                            <strong
                              style={{
                                color: '#007bff',
                                textTransform: 'uppercase',
                              }}
                            >
                              {log.endpoint}
                            </strong>
                            <span
                              style={{
                                marginLeft: '10px',
                                padding: '2px 6px',
                                backgroundColor:
                                  log.response_status === 200
                                    ? '#28a745'
                                    : '#dc3545',
                                color: 'white',
                                borderRadius: '4px',
                                fontSize: '12px',
                              }}
                            >
                              {log.response_status || 'N/A'}
                            </span>
                          </div>
                          {isGuessRequestWithMap(log) && (
                            <button
                              onClick={() => handleVisualizeMap(log)}
                              style={{
                                padding: '4px 8px',
                                backgroundColor: '#17a2b8',
                                color: 'white',
                                border: 'none',
                                borderRadius: '4px',
                                cursor: 'pointer',
                                fontSize: '11px',
                                fontWeight: 'bold',
                              }}
                            >
                              🗺️ Visualize
                            </button>
                          )}
                        </div>
                        <div style={{ color: '#6c757d', fontSize: '12px' }}>
                          {formatDateTime(log.created_at)}
                        </div>
                      </div>

                      {log.request_body && (
                        <div style={{ marginBottom: '10px' }}>
                          <strong>Request:</strong>
                          <pre
                            style={{
                              backgroundColor: '#f8f9fa',
                              padding: '10px',
                              borderRadius: '4px',
                              fontSize: '12px',
                              fontFamily: 'monospace',
                              overflow: 'auto',
                              margin: '5px 0',
                            }}
                          >
                            {JSON.stringify(
                              JSON.parse(log.request_body),
                              null,
                              2
                            )}
                          </pre>
                        </div>
                      )}

                      {log.response_body && (
                        <div>
                          <strong>Response:</strong>
                          <pre
                            style={{
                              backgroundColor: '#f8f9fa',
                              padding: '10px',
                              borderRadius: '4px',
                              fontSize: '12px',
                              fontFamily: 'monospace',
                              overflow: 'auto',
                              margin: '5px 0',
                            }}
                          >
                            {JSON.stringify(
                              JSON.parse(log.response_body),
                              null,
                              2
                            )}
                          </pre>
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default SessionsPage;
