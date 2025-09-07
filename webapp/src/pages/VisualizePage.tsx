import React, { useState } from 'react';
import { Map } from '../types';
import MapInput from '../components/MapInput';
import MapVisualizer from '../components/MapVisualizer';

export default function VisualizePage() {
  const [map, setMap] = useState<Map | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleMapLoad = (loadedMap: Map) => {
    setMap(loadedMap);
    setError(null);
  };

  const handleError = (errorMessage: string) => {
    setError(errorMessage);
    setMap(null);
  };

  const handleReset = () => {
    setMap(null);
    setError(null);
  };

  return (
    <div style={{ minHeight: '100vh', backgroundColor: '#f8f9fa', padding: '20px' }}>
      <div style={{ margin: '0 auto' }}>
        <div
          style={{
            backgroundColor: 'white',
            borderRadius: '8px',
            padding: '20px',
            marginBottom: '20px',
            boxShadow: '0 4px 6px rgba(0, 0, 0, 0.1)',
          }}
        >
          <h1 style={{ marginBottom: '20px', color: '#343a40' }}>Map Visualizer</h1>
          <p style={{ color: '#6c757d', marginBottom: '20px' }}>
            Upload or paste a Map JSON to visualize the library layout. Each room will be displayed as a hexagon with doors on each side.
          </p>
          
          <div style={{ display: 'flex', gap: '10px', alignItems: 'center', marginBottom: '20px' }}>
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
              {map ? `Loaded map with ${map.rooms.length} rooms and ${map.connections.length} connections` : 'No map loaded'}
            </span>
          </div>
        </div>

        <div style={{ display: 'flex', gap: '20px', minHeight: '600px' }}>
          {/* Left Panel - Input */}
          <div
            style={{
              flex: '0 0 400px',
              backgroundColor: 'white',
              borderRadius: '8px',
              padding: '20px',
              boxShadow: '0 4px 6px rgba(0, 0, 0, 0.1)',
              height: 'fit-content',
            }}
          >
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

          {/* Right Panel - Visualization */}
          <div
            style={{
              flex: 1,
              backgroundColor: 'white',
              borderRadius: '8px',
              padding: '20px',
              boxShadow: '0 4px 6px rgba(0, 0, 0, 0.1)',
              minHeight: '600px',
            }}
          >
            {map ? (
              <MapVisualizer map={map} />
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
        </div>
      </div>
    </div>
  );
}