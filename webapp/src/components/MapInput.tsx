import React, { useState } from 'react';
import { MapStruct } from '../types';

interface Props {
  onMapLoad: (map: MapStruct) => void;
  onError: (error: string) => void;
}

export default function MapInput({ onMapLoad, onError }: Props) {
  const [jsonText, setJsonText] = useState('');
  const [isLoading, setIsLoading] = useState(false);

  const sampleMap = {
    rooms: [0, 1, 2, 1, 3],
    startingRoom: 0,
    connections: [
      {
        from: { room: 0, door: 0 },
        to: { room: 1, door: 3 },
      },
      {
        from: { room: 0, door: 1 },
        to: { room: 2, door: 2 },
      },
      {
        from: { room: 1, door: 0 },
        to: { room: 3, door: 5 },
      },
    ],
  };

  const handleLoadSample = () => {
    const sampleJson = JSON.stringify(sampleMap, null, 2);
    setJsonText(sampleJson);
    onMapLoad(sampleMap);
  };

  const validateMap = (raw: unknown): MapStruct => {
    if (!raw || typeof raw !== 'object') {
      throw new Error('Map must be a JSON object');
    }

    // Normalize: accept wrappers like { map: {...} } (e.g., guess request payloads)
    const data = (raw as any).map && typeof (raw as any).map === 'object' ? (raw as any).map : (raw as any);

    if (!Array.isArray(data.rooms)) {
      throw new Error('Map must have a "rooms" array');
    }

    if (typeof data.startingRoom !== 'number') {
      throw new Error('Map must have a "startingRoom" number');
    }

    if (!Array.isArray(data.connections)) {
      throw new Error('Map must have a "connections" array');
    }

    // Validate room values are numbers
    for (const room of data.rooms) {
      if (typeof room !== 'number' || room < 0 || room > 3) {
        throw new Error('Room values must be numbers between 0 and 3');
      }
    }

    // Validate starting room exists
    if (!data.rooms.some((_, index) => index === data.startingRoom)) {
      throw new Error('Starting room index must exist in rooms array');
    }

    // Validate connections
    for (const conn of data.connections) {
      if (!conn.from || !conn.to) {
        throw new Error('Connections must have "from" and "to" objects');
      }

      if (
        typeof conn.from.room !== 'number' ||
        typeof conn.from.door !== 'number'
      ) {
        throw new Error('Connection "from" must have room and door numbers');
      }

      if (
        typeof conn.to.room !== 'number' ||
        typeof conn.to.door !== 'number'
      ) {
        throw new Error('Connection "to" must have room and door numbers');
      }

      // Validate door numbers are 0-5
      if (
        conn.from.door < 0 ||
        conn.from.door > 5 ||
        conn.to.door < 0 ||
        conn.to.door > 5
      ) {
        throw new Error('Door numbers must be between 0 and 5');
      }

      // Validate room indices exist
      if (conn.from.room < 0 || conn.from.room >= data.rooms.length) {
        throw new Error(
          `Connection references non-existent room ${conn.from.room}`
        );
      }
      if (conn.to.room < 0 || conn.to.room >= data.rooms.length) {
        throw new Error(
          `Connection references non-existent room ${conn.to.room}`
        );
      }
    }

    return data as MapStruct;
  };

  const handleLoadJson = () => {
    if (!jsonText.trim()) {
      onError('Please enter JSON data');
      return;
    }

    try {
      setIsLoading(true);
      const data = JSON.parse(jsonText);
      const validatedMap = validateMap(data);
      onMapLoad(validatedMap);
    } catch (error) {
      onError(error instanceof Error ? error.message : 'Invalid JSON format');
    } finally {
      setIsLoading(false);
    }
  };

  const handleFileUpload = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = (e) => {
      const text = e.target?.result as string;
      setJsonText(text);
      try {
        const data = JSON.parse(text);
        const validatedMap = validateMap(data);
        onMapLoad(validatedMap);
      } catch (error) {
        onError(error instanceof Error ? error.message : 'Invalid file format');
      }
    };
    reader.readAsText(file);
  };

  return (
    <div>
      <h3 style={{ marginBottom: '15px' }}>Load Map Data</h3>

      {/* File Upload */}
      <div style={{ marginBottom: '20px' }}>
        <label
          style={{
            display: 'block',
            marginBottom: '8px',
            fontSize: '14px',
            fontWeight: 'bold',
            color: '#495057',
          }}
        >
          Upload JSON File:
        </label>
        <input
          type="file"
          accept=".json"
          onChange={handleFileUpload}
          style={{
            width: '100%',
            padding: '8px',
            border: '1px solid #ced4da',
            borderRadius: '4px',
          }}
        />
      </div>

      {/* Sample Data Button */}
      <div style={{ marginBottom: '20px' }}>
        <button
          onClick={handleLoadSample}
          style={{
            padding: '10px 16px',
            backgroundColor: '#28a745',
            color: 'white',
            border: 'none',
            borderRadius: '4px',
            cursor: 'pointer',
            fontSize: '14px',
          }}
        >
          Load Sample Map
        </button>
      </div>

      {/* Text Input */}
      <div style={{ marginBottom: '20px' }}>
        <label
          style={{
            display: 'block',
            marginBottom: '8px',
            fontSize: '14px',
            fontWeight: 'bold',
            color: '#495057',
          }}
        >
          Or paste JSON data:
        </label>
        <textarea
          value={jsonText}
          onChange={(e) => setJsonText(e.target.value)}
          placeholder={`Paste your Map JSON here...\n\nExample format:\n{\n  "rooms": [0, 1, 2],\n  "startingRoom": 0,\n  "connections": [\n    {\n      "from": {"room": 0, "door": 0},\n      "to": {"room": 1, "door": 3}\n    }\n  ]\n}`}
          rows={12}
          style={{
            width: '100%',
            padding: '12px',
            border: '1px solid #ced4da',
            borderRadius: '4px',
            fontFamily: 'monospace',
            fontSize: '12px',
            resize: 'vertical',
          }}
        />
      </div>

      {/* Load Button */}
      <button
        onClick={handleLoadJson}
        disabled={isLoading || !jsonText.trim()}
        style={{
          width: '100%',
          padding: '12px',
          backgroundColor:
            isLoading || !jsonText.trim() ? '#6c757d' : '#007bff',
          color: 'white',
          border: 'none',
          borderRadius: '4px',
          cursor: isLoading || !jsonText.trim() ? 'not-allowed' : 'pointer',
          fontSize: '16px',
          fontWeight: 'bold',
        }}
      >
        {isLoading ? 'Loading...' : 'Load Map'}
      </button>

      {/* Format Help */}
      <div
        style={{
          marginTop: '20px',
          padding: '12px',
          backgroundColor: '#e7f3ff',
          borderRadius: '4px',
          fontSize: '12px',
          color: '#495057',
        }}
      >
        <strong>Map Format:</strong>
        <ul
          style={{ marginTop: '8px', paddingLeft: '20px', margin: '8px 0 0 0' }}
        >
          <li>
            <strong>rooms:</strong> Array of room label values (0-3)
          </li>
          <li>
            <strong>startingRoom:</strong> Index of the starting room
          </li>
          <li>
            <strong>connections:</strong> Array of door connections
          </li>
          <li>
            <strong>door numbers:</strong> 0-5 (hexagon has 6 doors)
          </li>
        </ul>
      </div>
    </div>
  );
}
