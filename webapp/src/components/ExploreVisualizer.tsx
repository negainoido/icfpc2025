import React, { useState, useEffect, useMemo } from 'react';
import { MapStruct, ExploreStep, ExploreState } from '../types';
import {
  simulateExploreSteps,
  getExploreStepDescription,
} from '../utils/explore';

interface Props {
  map: MapStruct;
  steps: ExploreStep[];
  onStateChange: (state: ExploreState | null) => void;
}

export default function ExploreVisualizer({
  map,
  steps,
  onStateChange,
}: Props) {
  const [currentStepIndex, setCurrentStepIndex] = useState(0);
  const [isPlaying, setIsPlaying] = useState(false);
  const [playbackSpeed, setPlaybackSpeed] = useState(1000); // milliseconds

  // Simulate all states once when steps change
  const allStates = useMemo(() => {
    try {
      return simulateExploreSteps(map, steps);
    } catch (error) {
      console.error('Error simulating explore steps:', error);
      return [];
    }
  }, [map, steps]);

  // Current state
  const currentState = allStates[currentStepIndex] || null;

  // Update parent with current state
  useEffect(() => {
    onStateChange(currentState);
  }, [currentState, onStateChange]);

  // Auto-play functionality
  useEffect(() => {
    if (!isPlaying || currentStepIndex >= allStates.length - 1) {
      setIsPlaying(false);
      return;
    }

    const timeout = setTimeout(() => {
      setCurrentStepIndex((prev) => Math.min(prev + 1, allStates.length - 1));
    }, playbackSpeed);

    return () => clearTimeout(timeout);
  }, [isPlaying, currentStepIndex, allStates.length, playbackSpeed]);

  const handlePrevious = () => {
    setCurrentStepIndex((prev) => Math.max(0, prev - 1));
    setIsPlaying(false);
  };

  const handleNext = () => {
    setCurrentStepIndex((prev) => Math.min(prev + 1, allStates.length - 1));
    setIsPlaying(false);
  };

  const handlePlay = () => {
    if (currentStepIndex >= allStates.length - 1) {
      setCurrentStepIndex(0);
    }
    setIsPlaying(true);
  };

  const handlePause = () => {
    setIsPlaying(false);
  };

  const handleReset = () => {
    setCurrentStepIndex(0);
    setIsPlaying(false);
  };

  const handleStepClick = (stepIndex: number) => {
    setCurrentStepIndex(stepIndex);
    setIsPlaying(false);
  };

  if (allStates.length === 0) {
    return (
      <div
        style={{
          padding: '12px',
          backgroundColor: '#f8d7da',
          border: '1px solid #f5c6cb',
          borderRadius: '4px',
          color: '#721c24',
        }}
      >
        <strong>Error:</strong> Could not simulate the explore steps. Please
        check your map and explore string.
      </div>
    );
  }

  const currentStep = steps[currentStepIndex - 1]; // -1 because states include initial state
  const isAtStart = currentStepIndex === 0;
  const isAtEnd = currentStepIndex >= allStates.length - 1;

  return (
    <div>
      {/* Control Panel */}
      <div
        style={{
          padding: '10px',
          backgroundColor: '#f8f9fa',
          borderRadius: '8px',
          border: '1px solid #dee2e6',
          marginBottom: '20px',
        }}
      >
        <div
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
            marginBottom: '15px',
          }}
        >
          <h3 style={{ margin: 0, color: '#343a40' }}>Explore Simulation</h3>
          <div style={{ display: 'flex', gap: '10px', alignItems: 'center' }}>
            <label style={{ fontSize: '14px', color: '#495057' }}>
              Speed:
              <select
                value={playbackSpeed}
                onChange={(e) => setPlaybackSpeed(parseInt(e.target.value))}
                style={{
                  marginLeft: '5px',
                  padding: '4px',
                  borderRadius: '3px',
                  border: '1px solid #ced4da',
                }}
              >
                <option value={2000}>Slow (2s)</option>
                <option value={1000}>Normal (1s)</option>
                <option value={500}>Fast (0.5s)</option>
                <option value={200}>Very Fast (0.2s)</option>
              </select>
            </label>
          </div>
        </div>

        {/* Playback Controls */}
        <div
          style={{
            display: 'flex',
            gap: '10px',
            alignItems: 'center',
            marginBottom: '15px',
          }}
        >
          <button
            onClick={handleReset}
            style={{
              padding: '8px 12px',
              backgroundColor: '#6c757d',
              color: 'white',
              border: 'none',
              borderRadius: '4px',
              cursor: 'pointer',
              fontSize: '14px',
            }}
          >
            Reset
          </button>
          <button
            onClick={handlePrevious}
            disabled={isAtStart}
            style={{
              padding: '8px 12px',
              backgroundColor: isAtStart ? '#e9ecef' : '#007bff',
              color: isAtStart ? '#6c757d' : 'white',
              border: 'none',
              borderRadius: '4px',
              cursor: isAtStart ? 'not-allowed' : 'pointer',
              fontSize: '14px',
            }}
          >
            Previous
          </button>
          {isPlaying ? (
            <button
              onClick={handlePause}
              style={{
                padding: '8px 12px',
                backgroundColor: '#dc3545',
                color: 'white',
                border: 'none',
                borderRadius: '4px',
                cursor: 'pointer',
                fontSize: '14px',
              }}
            >
              Pause
            </button>
          ) : (
            <button
              onClick={handlePlay}
              disabled={isAtEnd}
              style={{
                padding: '8px 12px',
                backgroundColor: isAtEnd ? '#e9ecef' : '#28a745',
                color: isAtEnd ? '#6c757d' : 'white',
                border: 'none',
                borderRadius: '4px',
                cursor: isAtEnd ? 'not-allowed' : 'pointer',
                fontSize: '14px',
              }}
            >
              Play
            </button>
          )}
          <button
            onClick={handleNext}
            disabled={isAtEnd}
            style={{
              padding: '8px 12px',
              backgroundColor: isAtEnd ? '#e9ecef' : '#007bff',
              color: isAtEnd ? '#6c757d' : 'white',
              border: 'none',
              borderRadius: '4px',
              cursor: isAtEnd ? 'not-allowed' : 'pointer',
              fontSize: '14px',
            }}
          >
            Next
          </button>
        </div>

        {/* Progress Bar */}
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '10px',
            marginBottom: '10px',
          }}
        >
          <span style={{ fontSize: '14px', color: '#495057' }}>Step:</span>
          <div
            style={{
              flex: 1,
              height: '20px',
              backgroundColor: '#e9ecef',
              borderRadius: '10px',
              position: 'relative',
              overflow: 'hidden',
            }}
          >
            <div
              style={{
                height: '100%',
                backgroundColor: '#007bff',
                width: `${(currentStepIndex / Math.max(1, allStates.length - 1)) * 100}%`,
                transition: 'width 0.3s ease',
              }}
            />
          </div>
          <span style={{ fontSize: '14px', color: '#495057' }}>
            {currentStepIndex} / {allStates.length - 1}
          </span>
        </div>
      </div>

      {/* Current State Information */}
      <div
        style={{
          padding: '10px',
          backgroundColor: 'white',
          borderRadius: '8px',
          border: '1px solid #dee2e6',
          marginBottom: '20px',
        }}
      >
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: '1fr 1fr',
            gap: '20px',
          }}
        >
          {/* Current Step Info */}
          <div>
            <h4 style={{ margin: '0 0 10px 0', color: '#343a40' }}>
              Current Step
            </h4>
            {currentStep ? (
              <div
                style={{
                  padding: '10px',
                  backgroundColor: '#f8f9fa',
                  borderRadius: '4px',
                  fontSize: '14px',
                }}
              >
                <div style={{ color: '#495057', marginBottom: '5px' }}>
                  Step {currentStepIndex}:
                </div>
                <div
                  style={{
                    color: currentStep.type === 'move' ? '#007bff' : '#28a745',
                    fontWeight: 'bold',
                  }}
                >
                  {getExploreStepDescription(currentStep)}
                </div>
              </div>
            ) : (
              <div
                style={{
                  padding: '10px',
                  backgroundColor: '#f8f9fa',
                  borderRadius: '4px',
                  fontSize: '14px',
                  color: '#6c757d',
                }}
              >
                Initial state (no step executed)
              </div>
            )}
          </div>

          {/* Current Room Info */}
          <div>
            <h4 style={{ margin: '0 0 10px 0', color: '#343a40' }}>
              Current Room
            </h4>
            <div
              style={{
                padding: '10px',
                backgroundColor: '#f8f9fa',
                borderRadius: '4px',
                fontSize: '14px',
              }}
            >
              <div style={{ marginBottom: '5px' }}>
                <strong>Room:</strong> {currentState?.currentRoom}
              </div>
              <div style={{ marginBottom: '5px' }}>
                <strong>Original Label:</strong>{' '}
                {map.rooms[currentState?.currentRoom || 0]}
              </div>
              {currentState?.chalkMarks.has(currentState.currentRoom) && (
                <div style={{ color: '#28a745', fontWeight: 'bold' }}>
                  <strong>Chalk Mark:</strong>{' '}
                  {currentState.chalkMarks.get(currentState.currentRoom)}
                </div>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Observed Labels */}
      <div
        style={{
          padding: '10px',
          backgroundColor: 'white',
          borderRadius: '8px',
          border: '1px solid #dee2e6',
        }}
      >
        <h4 style={{ margin: '0 0 10px 0', color: '#343a40' }}>
          Observed Labels
        </h4>
        <div
          style={{
            display: 'flex',
            flexWrap: 'wrap',
            gap: '8px',
          }}
        >
          {currentState?.observedLabels.map((label, index) => (
            <div
              key={index}
              style={{
                padding: '6px 12px',
                backgroundColor:
                  index === currentState.observedLabels.length - 1
                    ? '#007bff'
                    : '#e9ecef',
                color:
                  index === currentState.observedLabels.length - 1
                    ? 'white'
                    : '#495057',
                borderRadius: '15px',
                fontSize: '12px',
                fontWeight: 'bold',
              }}
            >
              {label}
            </div>
          )) || []}
        </div>
        <div
          style={{
            marginTop: '10px',
            fontSize: '12px',
            color: '#6c757d',
          }}
        >
          Expected API response: [{currentState?.observedLabels.join(', ')}]
        </div>
      </div>

      {/* Step Timeline */}
      <div
        style={{
          marginTop: '20px',
          padding: '10px',
          backgroundColor: 'white',
          borderRadius: '8px',
          border: '1px solid #dee2e6',
        }}
      >
        <h4 style={{ margin: '0 0 10px 0', color: '#343a40' }}>
          Step Timeline
        </h4>
        <div
          style={{
            display: 'flex',
            gap: '4px',
            flexWrap: 'wrap',
          }}
        >
          {allStates.map((_, index) => {
            const isCurrent = index === currentStepIndex;
            const isPast = index < currentStepIndex;

            return (
              <button
                key={index}
                onClick={() => handleStepClick(index)}
                style={{
                  width: '30px',
                  height: '30px',
                  backgroundColor: isCurrent
                    ? '#007bff'
                    : isPast
                      ? '#28a745'
                      : '#e9ecef',
                  color: isCurrent || isPast ? 'white' : '#495057',
                  border: 'none',
                  borderRadius: '4px',
                  cursor: 'pointer',
                  fontSize: '12px',
                  fontWeight: 'bold',
                }}
                title={index === 0 ? 'Initial state' : `Step ${index}`}
              >
                {index}
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
