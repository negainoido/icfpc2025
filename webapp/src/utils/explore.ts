import { MapStruct, ExploreStep, ExploreState, PathSegment, Connection } from '../types';

export function parseExploreString(input: string): ExploreStep[] {
  const steps: ExploreStep[] = [];
  let i = 0;

  while (i < input.length) {
    const char = input[i];

    if (char === '[') {
      // Find the closing bracket
      const closeIndex = input.indexOf(']', i);
      if (closeIndex === -1) {
        throw new Error(`Missing closing bracket for chalk mark at position ${i}`);
      }

      const labelStr = input.slice(i + 1, closeIndex);
      const label = parseInt(labelStr, 10);

      if (isNaN(label) || label < 0 || label > 3) {
        throw new Error(`Invalid chalk label "${labelStr}" at position ${i}. Must be 0-3.`);
      }

      steps.push({ type: 'chalk', label });
      i = closeIndex + 1;
    } else if (char >= '0' && char <= '5') {
      const door = parseInt(char, 10);
      steps.push({ type: 'move', door });
      i++;
    } else if (char === ' ' || char === '\t' || char === '\n') {
      // Skip whitespace
      i++;
    } else {
      throw new Error(`Invalid character "${char}" at position ${i}. Expected 0-5 or [n].`);
    }
  }

  return steps;
}

export function findConnection(map: MapStruct, fromRoom: number, door: number): Connection | null {
  return map.connections.find(
    conn => 
      (conn.from.room === fromRoom && conn.from.door === door) ||
      (conn.to.room === fromRoom && conn.to.door === door)
  ) || null;
}

export function getDestinationRoom(connection: Connection, fromRoom: number): number {
  if (connection.from.room === fromRoom) {
    return connection.to.room;
  } else {
    return connection.from.room;
  }
}

export function simulateExploreSteps(map: MapStruct, steps: ExploreStep[]): ExploreState[] {
  const states: ExploreState[] = [];
  let currentRoom = map.startingRoom;
  const pathHistory: PathSegment[] = [];
  const chalkMarks = new Map<number, number>();
  const observedLabels: number[] = [];

  // Add initial state
  states.push({
    currentRoom,
    currentPosition: { x: 0, y: 0 }, // Will be calculated by layout
    pathHistory: [],
    chalkMarks: new Map(chalkMarks),
    observedLabels: [...observedLabels],
    stepIndex: 0,
    totalSteps: steps.length,
  });

  // Add the starting room's label to observed labels
  const startingLabel = chalkMarks.get(currentRoom) ?? map.rooms[currentRoom];
  observedLabels.push(startingLabel);

  for (let i = 0; i < steps.length; i++) {
    const step = steps[i];

    if (step.type === 'chalk') {
      // Update chalk marks
      chalkMarks.set(currentRoom, step.label);
      observedLabels.push(step.label);
    } else if (step.type === 'move') {
      // Find connection for this door
      const connection = findConnection(map, currentRoom, step.door);
      if (!connection) {
        throw new Error(`No connection found from room ${currentRoom} through door ${step.door}`);
      }

      const destinationRoom = getDestinationRoom(connection, currentRoom);

      // Add path segment
      pathHistory.push({
        from: currentRoom,
        to: destinationRoom,
        door: step.door,
      });

      currentRoom = destinationRoom;

      // Observe the label in the new room (chalk mark overrides original)
      const observedLabel = chalkMarks.get(currentRoom) ?? map.rooms[currentRoom];
      observedLabels.push(observedLabel);
    }

    // Add state after this step
    states.push({
      currentRoom,
      currentPosition: { x: 0, y: 0 }, // Will be calculated by layout
      pathHistory: [...pathHistory],
      chalkMarks: new Map(chalkMarks),
      observedLabels: [...observedLabels],
      stepIndex: i + 1,
      totalSteps: steps.length,
    });
  }

  return states;
}

export function predictObservedLabels(map: MapStruct, steps: ExploreStep[]): number[] {
  const states = simulateExploreSteps(map, steps);
  // Return the final state's observed labels
  return states[states.length - 1].observedLabels;
}

export function validateExploreString(input: string): { valid: boolean; error?: string } {
  try {
    parseExploreString(input);
    return { valid: true };
  } catch (error) {
    return { valid: false, error: (error as Error).message };
  }
}

export function formatExploreString(steps: ExploreStep[]): string {
  return steps
    .map(step => {
      if (step.type === 'move') {
        return step.door.toString();
      } else {
        return `[${step.label}]`;
      }
    })
    .join('');
}

export function getExploreStepDescription(step: ExploreStep): string {
  if (step.type === 'move') {
    return `Move through door ${step.door}`;
  } else {
    return `Mark current room with label ${step.label}`;
  }
}

export function countDoorSteps(steps: ExploreStep[]): number {
  return steps.filter(step => step.type === 'move').length;
}

export function validateStepLimit(steps: ExploreStep[], roomCount: number): { valid: boolean; error?: string } {
  const doorSteps = countDoorSteps(steps);
  const limit = 6 * roomCount;
  
  if (doorSteps > limit) {
    return {
      valid: false,
      error: `Too many door steps: ${doorSteps}. Maximum allowed: ${limit} (6 × ${roomCount} rooms)`
    };
  }
  
  return { valid: true };
}