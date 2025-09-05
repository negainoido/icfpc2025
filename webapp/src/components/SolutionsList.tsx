import React, { useEffect, useState } from 'react';

interface Solution {
  id: number;
  problem_id: number;
  problem_type: string | null;
  status: string | null;
  solver: string;
  score: number | null;
  ts: string;
}

interface ApiResponse<T> {
  success: boolean;
  data: T | null;
  message: string | null;
}

const SolutionsList: React.FC = () => {
  const [solutions, setSolutions] = useState<Solution[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const fetchSolutions = async () => {
      try {
        const response = await fetch('http://localhost:8080/api/solutions');
        if (!response.ok) {
          throw new Error(`HTTP error! status: ${response.status}`);
        }
        const result: ApiResponse<Solution[]> = await response.json();

        if (result.success && result.data) {
          setSolutions(result.data);
        } else {
          setError(result.message || 'Failed to fetch solutions');
        }
      } catch (err) {
        setError(err instanceof Error ? err.message : 'An error occurred');
      } finally {
        setLoading(false);
      }
    };

    fetchSolutions();
  }, []);

  const formatTimestamp = (timestamp: string) => {
    return new Date(timestamp).toLocaleString();
  };

  const getStatusColor = (status: string | null) => {
    if (!status) return '#666';

    switch (status.toLowerCase()) {
      case 'success':
      case 'solved':
        return '#28a745';
      case 'failed':
      case 'error':
        return '#dc3545';
      case 'pending':
      case 'running':
        return '#ffc107';
      default:
        return '#6c757d';
    }
  };

  if (loading) {
    return (
      <div style={{ padding: '20px', textAlign: 'center' }}>
        Loading solutions...
      </div>
    );
  }

  if (error) {
    return (
      <div style={{ padding: '20px', color: '#dc3545', textAlign: 'center' }}>
        Error: {error}
      </div>
    );
  }

  return (
    <div style={{ padding: '20px' }}>
      <h2 style={{ marginBottom: '20px' }}>Solutions ({solutions.length})</h2>

      {solutions.length === 0 ? (
        <div style={{ textAlign: 'center', color: '#666' }}>
          No solutions found
        </div>
      ) : (
        <div style={{ overflowX: 'auto' }}>
          <table
            style={{
              width: '100%',
              borderCollapse: 'collapse',
              backgroundColor: 'white',
              borderRadius: '8px',
              overflow: 'hidden',
              boxShadow: '0 2px 4px rgba(0,0,0,0.1)',
            }}
          >
            <thead>
              <tr style={{ backgroundColor: '#f8f9fa' }}>
                <th
                  style={{
                    padding: '12px',
                    textAlign: 'left',
                    borderBottom: '1px solid #dee2e6',
                  }}
                >
                  ID
                </th>
                <th
                  style={{
                    padding: '12px',
                    textAlign: 'left',
                    borderBottom: '1px solid #dee2e6',
                  }}
                >
                  Problem ID
                </th>
                <th
                  style={{
                    padding: '12px',
                    textAlign: 'left',
                    borderBottom: '1px solid #dee2e6',
                  }}
                >
                  Type
                </th>
                <th
                  style={{
                    padding: '12px',
                    textAlign: 'left',
                    borderBottom: '1px solid #dee2e6',
                  }}
                >
                  Status
                </th>
                <th
                  style={{
                    padding: '12px',
                    textAlign: 'left',
                    borderBottom: '1px solid #dee2e6',
                  }}
                >
                  Solver
                </th>
                <th
                  style={{
                    padding: '12px',
                    textAlign: 'left',
                    borderBottom: '1px solid #dee2e6',
                  }}
                >
                  Score
                </th>
                <th
                  style={{
                    padding: '12px',
                    textAlign: 'left',
                    borderBottom: '1px solid #dee2e6',
                  }}
                >
                  Timestamp
                </th>
              </tr>
            </thead>
            <tbody>
              {solutions.map((solution) => (
                <tr
                  key={solution.id}
                  style={{
                    borderBottom: '1px solid #dee2e6',
                    '&:hover': { backgroundColor: '#f8f9fa' },
                  }}
                >
                  <td style={{ padding: '12px' }}>{solution.id}</td>
                  <td style={{ padding: '12px' }}>{solution.problem_id}</td>
                  <td style={{ padding: '12px' }}>
                    {solution.problem_type || '-'}
                  </td>
                  <td style={{ padding: '12px' }}>
                    <span
                      style={{
                        color: getStatusColor(solution.status),
                        fontWeight: 'bold',
                      }}
                    >
                      {solution.status || '-'}
                    </span>
                  </td>
                  <td style={{ padding: '12px' }}>{solution.solver}</td>
                  <td style={{ padding: '12px' }}>
                    {solution.score !== null
                      ? solution.score.toLocaleString()
                      : '-'}
                  </td>
                  <td
                    style={{ padding: '12px', fontSize: '14px', color: '#666' }}
                  >
                    {formatTimestamp(solution.ts)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
};

export default SolutionsList;
