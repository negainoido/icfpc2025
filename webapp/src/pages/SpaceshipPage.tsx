import React, { useState, useEffect } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useSpaceshipData } from '../hooks/useSpaceshipData';
import SpaceshipVisualization from '../components/SpaceshipVisualization';

const SpaceshipPage: React.FC = () => {
  const { problemNumber } = useParams<{ problemNumber: string }>();
  const navigate = useNavigate();
  const [selectedFile, setSelectedFile] = useState<string>(problemNumber ? `problem${problemNumber}` : 'problem1');
  const { points, loading, error } = useSpaceshipData(selectedFile);

  useEffect(() => {
    if (problemNumber) {
      setSelectedFile(`problem${problemNumber}`);
    }
  }, [problemNumber]);

  const problemFiles = Array.from({ length: 25 }, (_, i) => `problem${i + 1}`);

  return (
    <div style={{ padding: '20px', maxWidth: '1200px', margin: '0 auto' }}>
      <h1>Spaceship Point Cloud Visualization</h1>
      
      <div style={{ marginBottom: '20px' }}>
        <label htmlFor="file-select" style={{ marginRight: '10px' }}>
          Problem File:
        </label>
        <select
          id="file-select"
          value={selectedFile}
          onChange={(e) => {
            const newFile = e.target.value;
            setSelectedFile(newFile);
            const problemNum = newFile.replace('problem', '');
            navigate(`/spaceship/${problemNum}`);
          }}
          style={{
            padding: '8px 12px',
            borderRadius: '4px',
            border: '1px solid #ccc',
            fontSize: '14px',
          }}
        >
          {problemFiles.map((file) => (
            <option key={file} value={file}>
              {file}
            </option>
          ))}
        </select>
      </div>

      {loading && (
        <div style={{ 
          padding: '20px', 
          textAlign: 'center',
          background: '#f0f0f0',
          borderRadius: '4px',
          marginBottom: '20px'
        }}>
          Loading...
        </div>
      )}

      {error && (
        <div style={{ 
          padding: '20px', 
          background: '#ffebee',
          color: '#c62828',
          borderRadius: '4px',
          marginBottom: '20px'
        }}>
          Error: {error}
        </div>
      )}

      {!loading && !error && points.length > 0 && (
        <div>
          <div style={{ marginBottom: '10px', fontSize: '14px', color: '#666' }}>
            Showing {points.length} points from {selectedFile}.txt
          </div>
          <SpaceshipVisualization 
            points={points} 
            width={Math.min(window.innerWidth - 40, 1000)}
            height={600}
          />
        </div>
      )}

      {!loading && !error && points.length === 0 && selectedFile && (
        <div style={{ 
          padding: '20px', 
          textAlign: 'center',
          background: '#fff3e0',
          color: '#e65100',
          borderRadius: '4px'
        }}>
          No points found in the selected file.
        </div>
      )}

      <div style={{ marginTop: '20px', fontSize: '12px', color: '#888' }}>
        <h3>Instructions:</h3>
        <ul>
          <li>Use mouse wheel to zoom in/out</li>
          <li>Click and drag to pan around</li>
          <li>Hover over points to see coordinates in console</li>
        </ul>
      </div>
    </div>
  );
};

export default SpaceshipPage;