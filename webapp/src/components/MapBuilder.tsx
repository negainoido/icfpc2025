import React, { useState } from 'react';
import { useSession } from '../hooks/useSession';
import { Connection, Map } from '../types';

interface MapBuilderState {
  rooms: number[];
  startingRoom: number;
  connections: Connection[];
}

export default function MapBuilder() {
  const { state, dispatch } = useSession();
  const [map, setMap] = useState<MapBuilderState>({
    rooms: [],
    startingRoom: 0,
    connections: [],
  });

  const [newRoom, setNewRoom] = useState('');
  const [newConnection, setNewConnection] = useState({
    fromRoom: '',
    fromDoor: '',
    toRoom: '',
    toDoor: '',
  });

  const handleAddRoom = () => {
    const roomNum = parseInt(newRoom);
    if (!isNaN(roomNum) && !map.rooms.includes(roomNum)) {
      setMap(prev => ({
        ...prev,
        rooms: [...prev.rooms, roomNum].sort((a, b) => a - b),
      }));
      setNewRoom('');
    }
  };

  const handleRemoveRoom = (roomNum: number) => {
    setMap(prev => ({
      ...prev,
      rooms: prev.rooms.filter(r => r !== roomNum),
      connections: prev.connections.filter(
        c => c.from.room !== roomNum && c.to.room !== roomNum
      ),
      startingRoom: prev.startingRoom === roomNum ? (prev.rooms[0] || 0) : prev.startingRoom,
    }));
  };

  const handleAddConnection = () => {
    const fromRoom = parseInt(newConnection.fromRoom);
    const fromDoor = parseInt(newConnection.fromDoor);
    const toRoom = parseInt(newConnection.toRoom);
    const toDoor = parseInt(newConnection.toDoor);

    if (!isNaN(fromRoom) && !isNaN(fromDoor) && !isNaN(toRoom) && !isNaN(toDoor)) {
      if (map.rooms.includes(fromRoom) && map.rooms.includes(toRoom)) {
        const connection: Connection = {
          from: { room: fromRoom, door: fromDoor },
          to: { room: toRoom, door: toDoor },
        };

        setMap(prev => ({
          ...prev,
          connections: [...prev.connections, connection],
        }));

        setNewConnection({
          fromRoom: '',
          fromDoor: '',
          toRoom: '',
          toDoor: '',
        });
      }
    }
  };

  const handleRemoveConnection = (index: number) => {
    setMap(prev => ({
      ...prev,
      connections: prev.connections.filter((_, i) => i !== index),
    }));
  };

  const handleSubmitMap = () => {
    if (map.rooms.length === 0) {
      dispatch({ type: 'SET_ERROR', payload: 'Map must have at least one room' });
      return;
    }

    if (!map.rooms.includes(map.startingRoom)) {
      dispatch({ type: 'SET_ERROR', payload: 'Starting room must be in the rooms list' });
      return;
    }

    const finalMap: Map = {
      rooms: map.rooms,
      startingRoom: map.startingRoom,
      connections: map.connections,
    };

    // Store the map in session state and move to guess phase
    dispatch({ type: 'SET_PHASE', payload: 'completed' });
    
    // Trigger the guess API call
    // This could be moved to a separate component or handled differently
    console.log('Final map:', finalMap);
  };

  const handleBackToExplore = () => {
    dispatch({ type: 'SET_PHASE', payload: 'exploring' });
  };

  if (state.phase !== 'building-map') {
    return null;
  }

  return (
    <div style={{ padding: '20px', maxWidth: '1000px', margin: '0 auto' }}>
      <h2>Build Library Map</h2>
      
      <div style={{ marginBottom: '20px', padding: '10px', backgroundColor: '#e7f3ff', borderRadius: '4px' }}>
        <p><strong>Session:</strong> {state.sessionId}</p>
        <p><strong>Problem:</strong> {state.problemName}</p>
        <p><strong>Exploration Results:</strong> {state.explorationResults.length} sets of data</p>
      </div>

      <div style={{ display: 'flex', gap: '20px' }}>
        {/* Left Panel - Map Construction */}
        <div style={{ flex: 1 }}>
          <h3>Rooms</h3>
          
          <div style={{ marginBottom: '20px' }}>
            <div style={{ display: 'flex', gap: '10px', marginBottom: '10px' }}>
              <input
                type="number"
                value={newRoom}
                onChange={(e) => setNewRoom(e.target.value)}
                placeholder="Room number"
                style={{
                  flex: 1,
                  padding: '8px',
                  border: '1px solid #ccc',
                  borderRadius: '4px',
                }}
              />
              <button
                onClick={handleAddRoom}
                disabled={!newRoom.trim()}
                style={{
                  padding: '8px 16px',
                  backgroundColor: !newRoom.trim() ? '#ccc' : '#28a745',
                  color: 'white',
                  border: 'none',
                  borderRadius: '4px',
                  cursor: !newRoom.trim() ? 'not-allowed' : 'pointer',
                }}
              >
                Add Room
              </button>
            </div>

            <div style={{ marginBottom: '10px' }}>
              <label>Starting Room: </label>
              <select
                value={map.startingRoom}
                onChange={(e) => setMap(prev => ({ ...prev, startingRoom: parseInt(e.target.value) }))}
                style={{
                  padding: '4px',
                  border: '1px solid #ccc',
                  borderRadius: '4px',
                }}
              >
                {map.rooms.map(room => (
                  <option key={room} value={room}>{room}</option>
                ))}
              </select>
            </div>

            <div>
              <strong>Current Rooms:</strong>
              {map.rooms.length === 0 ? (
                <p style={{ color: '#666', fontStyle: 'italic' }}>No rooms added yet</p>
              ) : (
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: '5px', marginTop: '5px' }}>
                  {map.rooms.map(room => (
                    <span
                      key={room}
                      style={{
                        display: 'inline-flex',
                        alignItems: 'center',
                        gap: '5px',
                        padding: '4px 8px',
                        backgroundColor: room === map.startingRoom ? '#d4edda' : '#f8f9fa',
                        border: room === map.startingRoom ? '2px solid #28a745' : '1px solid #dee2e6',
                        borderRadius: '4px',
                      }}
                    >
                      {room} {room === map.startingRoom && '(start)'}
                      <button
                        onClick={() => handleRemoveRoom(room)}
                        style={{
                          background: 'none',
                          border: 'none',
                          color: '#dc3545',
                          cursor: 'pointer',
                          fontSize: '12px',
                        }}
                      >
                        ×
                      </button>
                    </span>
                  ))}
                </div>
              )}
            </div>
          </div>

          <h3>Connections</h3>
          
          <div style={{ marginBottom: '20px' }}>
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: '10px', marginBottom: '10px' }}>
              <input
                type="number"
                value={newConnection.fromRoom}
                onChange={(e) => setNewConnection(prev => ({ ...prev, fromRoom: e.target.value }))}
                placeholder="From room"
                style={{ padding: '8px', border: '1px solid #ccc', borderRadius: '4px' }}
              />
              <input
                type="number"
                value={newConnection.fromDoor}
                onChange={(e) => setNewConnection(prev => ({ ...prev, fromDoor: e.target.value }))}
                placeholder="From door"
                style={{ padding: '8px', border: '1px solid #ccc', borderRadius: '4px' }}
              />
              <input
                type="number"
                value={newConnection.toRoom}
                onChange={(e) => setNewConnection(prev => ({ ...prev, toRoom: e.target.value }))}
                placeholder="To room"
                style={{ padding: '8px', border: '1px solid #ccc', borderRadius: '4px' }}
              />
              <input
                type="number"
                value={newConnection.toDoor}
                onChange={(e) => setNewConnection(prev => ({ ...prev, toDoor: e.target.value }))}
                placeholder="To door"
                style={{ padding: '8px', border: '1px solid #ccc', borderRadius: '4px' }}
              />
            </div>
            <button
              onClick={handleAddConnection}
              disabled={!newConnection.fromRoom || !newConnection.fromDoor || !newConnection.toRoom || !newConnection.toDoor}
              style={{
                width: '100%',
                padding: '8px 16px',
                backgroundColor: (!newConnection.fromRoom || !newConnection.fromDoor || !newConnection.toRoom || !newConnection.toDoor) ? '#ccc' : '#007bff',
                color: 'white',
                border: 'none',
                borderRadius: '4px',
                cursor: (!newConnection.fromRoom || !newConnection.fromDoor || !newConnection.toRoom || !newConnection.toDoor) ? 'not-allowed' : 'pointer',
              }}
            >
              Add Connection
            </button>
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
              onClick={handleBackToExplore}
              style={{
                flex: 1,
                padding: '12px',
                fontSize: '16px',
                backgroundColor: '#6c757d',
                color: 'white',
                border: 'none',
                borderRadius: '4px',
                cursor: 'pointer',
              }}
            >
              Back to Explore
            </button>
            
            <button
              onClick={handleSubmitMap}
              disabled={map.rooms.length === 0}
              style={{
                flex: 2,
                padding: '12px',
                fontSize: '16px',
                backgroundColor: map.rooms.length === 0 ? '#ccc' : '#28a745',
                color: 'white',
                border: 'none',
                borderRadius: '4px',
                cursor: map.rooms.length === 0 ? 'not-allowed' : 'pointer',
              }}
            >
              Submit Map & Guess
            </button>
          </div>
        </div>

        {/* Right Panel - Current Map & Exploration Data */}
        <div style={{ flex: 1 }}>
          <h3>Current Map</h3>
          <pre style={{
            backgroundColor: '#f8f9fa',
            padding: '15px',
            borderRadius: '4px',
            fontSize: '12px',
            overflow: 'auto',
            marginBottom: '20px',
          }}>
            {JSON.stringify(map, null, 2)}
          </pre>

          <h3>Connections ({map.connections.length})</h3>
          {map.connections.length === 0 ? (
            <p style={{ color: '#666', fontStyle: 'italic' }}>No connections added yet</p>
          ) : (
            <div style={{ maxHeight: '200px', overflowY: 'auto' }}>
              {map.connections.map((conn, index) => (
                <div
                  key={index}
                  style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                    padding: '8px',
                    backgroundColor: '#f8f9fa',
                    marginBottom: '5px',
                    borderRadius: '4px',
                    fontSize: '14px',
                  }}
                >
                  <span>
                    Room {conn.from.room} Door {conn.from.door} ↔ Room {conn.to.room} Door {conn.to.door}
                  </span>
                  <button
                    onClick={() => handleRemoveConnection(index)}
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
                </div>
              ))}
            </div>
          )}

          <h3>Exploration Data</h3>
          <div style={{ maxHeight: '200px', overflowY: 'auto' }}>
            {state.explorationResults.map((result, index) => (
              <div
                key={index}
                style={{
                  padding: '8px',
                  backgroundColor: '#f8f9fa',
                  marginBottom: '5px',
                  borderRadius: '4px',
                  fontSize: '12px',
                }}
              >
                <strong>Result {index + 1}:</strong>
                <div style={{ fontFamily: 'monospace', marginTop: '4px' }}>
                  [{result.join(', ')}]
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}