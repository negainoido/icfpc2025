import React, { useState } from 'react';
import { ExploreStep } from '../types';
import {
  parseExploreString,
  validateExploreString,
  validateStepLimit,
  countDoorSteps,
  getExploreStepDescription,
} from '../utils/explore';

interface Props {
  onExploreLoad: (steps: ExploreStep[]) => void;
  onError: (errorMessage: string) => void;
  roomCount?: number;
}

export default function ExploreInput({
  onExploreLoad,
  onError,
  roomCount = 0,
}: Props) {
  const [exploreString, setExploreString] = useState('');
  const [parsedSteps, setParsedSteps] = useState<ExploreStep[]>([]);
  const [isValid, setIsValid] = useState(true);

  const handleInputChange = (value: string) => {
    setExploreString(value);

    if (!value.trim()) {
      setParsedSteps([]);
      setIsValid(true);
      return;
    }

    // Validate syntax
    const validation = validateExploreString(value);
    if (!validation.valid) {
      setIsValid(false);
      setParsedSteps([]);
      return;
    }

    try {
      const steps = parseExploreString(value);
      setParsedSteps(steps);

      // Validate step limit if room count is provided
      if (roomCount > 0) {
        const stepLimitValidation = validateStepLimit(steps, roomCount);
        if (!stepLimitValidation.valid) {
          setIsValid(false);
          onError(stepLimitValidation.error || 'Step limit exceeded');
          return;
        }
      }

      setIsValid(true);
    } catch (error) {
      setIsValid(false);
      setParsedSteps([]);
      onError((error as Error).message);
    }
  };

  const handleLoadExplore = () => {
    if (parsedSteps.length === 0) {
      onError('Please enter a valid explore string');
      return;
    }

    if (!isValid) {
      onError('Please fix the errors in your explore string');
      return;
    }

    onExploreLoad(parsedSteps);
  };

  const handleClear = () => {
    setExploreString('');
    setParsedSteps([]);
    setIsValid(true);
  };

  const doorSteps = countDoorSteps(parsedSteps);
  const stepLimit = roomCount * 6;

  return (
    <div>
      <h3 style={{ marginBottom: '15px', color: '#343a40' }}>
        Explore String Input
      </h3>

      <div style={{ marginBottom: '15px' }}>
        <label
          htmlFor="exploreString"
          style={{
            display: 'block',
            marginBottom: '5px',
            fontWeight: 'bold',
            color: '#495057',
          }}
        >
          Explore String:
        </label>
        <input
          id="exploreString"
          type="text"
          value={exploreString}
          onChange={(e) => handleInputChange(e.target.value)}
          placeholder="Enter explore string (e.g., 2[3]12[0])"
          style={{
            width: '100%',
            padding: '10px',
            border: `2px solid ${isValid ? '#ced4da' : '#dc3545'}`,
            borderRadius: '4px',
            fontSize: '14px',
            fontFamily: 'monospace',
            backgroundColor: isValid ? 'white' : '#fff5f5',
          }}
        />
        <div
          style={{
            fontSize: '12px',
            color: '#6c757d',
            marginTop: '5px',
          }}
        >
          Format: 0-5 for doors, [0-3] for chalk marks (e.g., "2[3]12[0]")
        </div>
      </div>

      {parsedSteps.length > 0 && (
        <div
          style={{
            marginBottom: '15px',
            padding: '10px',
            backgroundColor: '#f8f9fa',
            borderRadius: '4px',
            border: '1px solid #dee2e6',
          }}
        >
          <div
            style={{
              marginBottom: '10px',
              fontWeight: 'bold',
              fontSize: '14px',
            }}
          >
            Parsed Steps ({parsedSteps.length}):
          </div>
          <div
            style={{
              fontSize: '12px',
              maxHeight: '120px',
              overflowY: 'auto',
              lineHeight: '1.4',
            }}
          >
            {parsedSteps.map((step, index) => (
              <div key={index} style={{ marginBottom: '2px' }}>
                <span style={{ color: '#6c757d' }}>{index + 1}.</span>{' '}
                <span
                  style={{
                    color: step.type === 'move' ? '#007bff' : '#28a745',
                    fontWeight: 'bold',
                  }}
                >
                  {getExploreStepDescription(step)}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}

      {parsedSteps.length > 0 && (
        <div
          style={{
            marginBottom: '15px',
            fontSize: '14px',
            display: 'flex',
            gap: '20px',
            flexWrap: 'wrap',
          }}
        >
          <span>
            <strong>Total steps:</strong> {parsedSteps.length}
          </span>
          <span>
            <strong>Door steps:</strong> {doorSteps}
            {roomCount > 0 && (
              <span
                style={{
                  color: doorSteps > stepLimit ? '#dc3545' : '#28a745',
                  marginLeft: '5px',
                }}
              >
                (limit: {stepLimit})
              </span>
            )}
          </span>
          <span>
            <strong>Chalk marks:</strong> {parsedSteps.length - doorSteps}
          </span>
        </div>
      )}

      <div style={{ display: 'flex', gap: '10px' }}>
        <button
          onClick={handleLoadExplore}
          disabled={!isValid || parsedSteps.length === 0}
          style={{
            padding: '10px 20px',
            backgroundColor:
              isValid && parsedSteps.length > 0 ? '#007bff' : '#6c757d',
            color: 'white',
            border: 'none',
            borderRadius: '4px',
            cursor:
              isValid && parsedSteps.length > 0 ? 'pointer' : 'not-allowed',
            fontSize: '14px',
            fontWeight: 'bold',
          }}
        >
          Load Explore
        </button>
        <button
          onClick={handleClear}
          style={{
            padding: '10px 20px',
            backgroundColor: '#6c757d',
            color: 'white',
            border: 'none',
            borderRadius: '4px',
            cursor: 'pointer',
            fontSize: '14px',
          }}
        >
          Clear
        </button>
      </div>

      <div
        style={{
          marginTop: '15px',
          fontSize: '12px',
          color: '#6c757d',
          lineHeight: '1.4',
        }}
      >
        <strong>Instructions:</strong>
        <ul style={{ marginTop: '5px', paddingLeft: '20px' }}>
          <li>Use digits 0-5 to specify door movements</li>
          <li>Use [0-3] to mark current room with chalk</li>
          <li>
            Example: "2[3]12[0]" means: door 2 → mark 3 → door 1 → door 2 → mark
            0
          </li>
          <li>
            Each plan is limited to {roomCount > 0 ? stepLimit : '6n'} door
            steps
            {roomCount > 0 && ` (${roomCount} rooms × 6)`}
          </li>
        </ul>
      </div>
    </div>
  );
}
