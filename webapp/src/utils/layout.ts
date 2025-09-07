import { Point } from './hexagon';
import { Map } from '../types';

export interface RoomLayout {
  roomIndex: number;
  position: Point;
  label: number;
}

/**
 * Calculate positions for all rooms in the map using a simple force-directed approach
 */
export function calculateRoomLayout(
  map: Map,
  containerWidth: number,
  containerHeight: number
): RoomLayout[] {
  const roomCount = map.rooms.length;

  if (roomCount === 0) return [];

  // If only one room, center it
  if (roomCount === 1) {
    return [
      {
        roomIndex: 0,
        position: { x: containerWidth / 2, y: containerHeight / 2 },
        label: map.rooms[0],
      },
    ];
  }

  // For small numbers of rooms, use simple circular layout
  if (roomCount <= 6) {
    return circularLayout(map, containerWidth, containerHeight);
  }

  // For larger numbers, use force-directed layout
  return forceDirectedLayout(map, containerWidth, containerHeight);
}

/**
 * Arrange rooms in a circle
 */
function circularLayout(
  map: Map,
  containerWidth: number,
  containerHeight: number
): RoomLayout[] {
  const roomCount = map.rooms.length;
  const centerX = containerWidth / 2;
  const centerY = containerHeight / 2;
  const radius = Math.min(containerWidth, containerHeight) * 0.3;

  const layout: RoomLayout[] = [];

  for (let i = 0; i < roomCount; i++) {
    const angle = (i * 2 * Math.PI) / roomCount - Math.PI / 2; // Start at top
    const x = centerX + radius * Math.cos(angle);
    const y = centerY + radius * Math.sin(angle);

    layout.push({
      roomIndex: i,
      position: { x, y },
      label: map.rooms[i],
    });
  }

  return layout;
}

/**
 * Use a simplified force-directed layout
 */
function forceDirectedLayout(
  map: Map,
  containerWidth: number,
  containerHeight: number
): RoomLayout[] {
  const roomCount = map.rooms.length;
  const layout: RoomLayout[] = [];

  // Initialize with random positions
  for (let i = 0; i < roomCount; i++) {
    layout.push({
      roomIndex: i,
      position: {
        x: Math.random() * containerWidth * 0.8 + containerWidth * 0.1,
        y: Math.random() * containerHeight * 0.8 + containerHeight * 0.1,
      },
      label: map.rooms[i],
    });
  }

  // Build adjacency list from connections
  const adjacent = new Set<string>();
  for (const conn of map.connections) {
    adjacent.add(`${conn.from.room}-${conn.to.room}`);
    adjacent.add(`${conn.to.room}-${conn.from.room}`);
  }

  // Run simulation
  const iterations = 100;
  const dt = 0.1;
  const repulsionStrength = 10000;
  const attractionStrength = 50;
  const idealDistance = 150;

  for (let iter = 0; iter < iterations; iter++) {
    const forces: Point[] = layout.map(() => ({ x: 0, y: 0 }));

    // Repulsion between all pairs
    for (let i = 0; i < roomCount; i++) {
      for (let j = i + 1; j < roomCount; j++) {
        const dx = layout[j].position.x - layout[i].position.x;
        const dy = layout[j].position.y - layout[i].position.y;
        const distance = Math.sqrt(dx * dx + dy * dy) + 0.01; // Avoid division by zero

        const force = repulsionStrength / (distance * distance);
        const fx = (dx / distance) * force;
        const fy = (dy / distance) * force;

        forces[i].x -= fx;
        forces[i].y -= fy;
        forces[j].x += fx;
        forces[j].y += fy;
      }
    }

    // Attraction between connected rooms
    for (const conn of map.connections) {
      const i = conn.from.room;
      const j = conn.to.room;

      const dx = layout[j].position.x - layout[i].position.x;
      const dy = layout[j].position.y - layout[i].position.y;
      const distance = Math.sqrt(dx * dx + dy * dy) + 0.01;

      const force =
        (attractionStrength * (distance - idealDistance)) / distance;
      const fx = (dx / distance) * force;
      const fy = (dy / distance) * force;

      forces[i].x += fx;
      forces[i].y += fy;
      forces[j].x -= fx;
      forces[j].y -= fy;
    }

    // Apply forces and constrain to bounds
    for (let i = 0; i < roomCount; i++) {
      layout[i].position.x += forces[i].x * dt;
      layout[i].position.y += forces[i].y * dt;

      // Keep within bounds
      const margin = 100;
      layout[i].position.x = Math.max(
        margin,
        Math.min(containerWidth - margin, layout[i].position.x)
      );
      layout[i].position.y = Math.max(
        margin,
        Math.min(containerHeight - margin, layout[i].position.y)
      );
    }
  }

  return layout;
}

/**
 * Get the bounding box of all room positions
 */
export function getLayoutBounds(layout: RoomLayout[]): {
  minX: number;
  maxX: number;
  minY: number;
  maxY: number;
} {
  if (layout.length === 0) {
    return { minX: 0, maxX: 100, minY: 0, maxY: 100 };
  }

  let minX = layout[0].position.x;
  let maxX = layout[0].position.x;
  let minY = layout[0].position.y;
  let maxY = layout[0].position.y;

  for (const room of layout) {
    minX = Math.min(minX, room.position.x);
    maxX = Math.max(maxX, room.position.x);
    minY = Math.min(minY, room.position.y);
    maxY = Math.max(maxY, room.position.y);
  }

  return { minX, maxX, minY, maxY };
}
