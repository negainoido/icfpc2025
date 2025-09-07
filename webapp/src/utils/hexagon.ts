export interface Point {
  x: number;
  y: number;
}

export interface HexagonGeometry {
  center: Point;
  radius: number;
  vertices: Point[];
  doorPositions: Point[];
}

/**
 * Calculate vertices of a regular hexagon centered at the given point
 * Door 0 is at the top, doors increase clockwise
 */
export function calculateHexagon(center: Point, radius: number): HexagonGeometry {
  const vertices: Point[] = [];
  const doorPositions: Point[] = [];
  
  // Calculate 6 vertices of the hexagon
  // Start at top (90 degrees) and go clockwise
  for (let i = 0; i < 6; i++) {
    const angle = (Math.PI / 2) - (i * Math.PI / 3); // Start at top, go clockwise
    const x = center.x + radius * Math.cos(angle);
    const y = center.y - radius * Math.sin(angle); // Negative for screen coordinates
    vertices.push({ x, y });
  }
  
  // Calculate door positions (midpoints of edges)
  for (let i = 0; i < 6; i++) {
    const currentVertex = vertices[i];
    const nextVertex = vertices[(i + 1) % 6];
    const doorX = (currentVertex.x + nextVertex.x) / 2;
    const doorY = (currentVertex.y + nextVertex.y) / 2;
    doorPositions.push({ x: doorX, y: doorY });
  }
  
  return {
    center,
    radius,
    vertices,
    doorPositions
  };
}

/**
 * Generate SVG path string for a hexagon
 */
export function hexagonToSVGPath(hexagon: HexagonGeometry): string {
  const vertices = hexagon.vertices;
  let path = `M ${vertices[0].x} ${vertices[0].y}`;
  
  for (let i = 1; i < vertices.length; i++) {
    path += ` L ${vertices[i].x} ${vertices[i].y}`;
  }
  
  path += ' Z'; // Close the path
  return path;
}

/**
 * Calculate a smooth curve between two points
 * Returns an SVG path for a quadratic bezier curve
 */
export function createCurvePath(from: Point, to: Point): string {
  // Calculate control point for the curve
  const midX = (from.x + to.x) / 2;
  const midY = (from.y + to.y) / 2;
  
  // Offset the control point perpendicular to the line to create a curve
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const length = Math.sqrt(dx * dx + dy * dy);
  
  // Create a curve that bulges outward (perpendicular to the connection line)
  const curvature = Math.min(length * 0.3, 50); // Curve intensity
  const perpX = -dy / length * curvature;
  const perpY = dx / length * curvature;
  
  const controlX = midX + perpX;
  const controlY = midY + perpY;
  
  return `M ${from.x} ${from.y} Q ${controlX} ${controlY} ${to.x} ${to.y}`;
}

/**
 * Create a loop path for connections from a door to itself
 * Returns an SVG path for a circular loop extending outward from the door
 */
export function createLoopPath(doorPos: Point, doorIndex: number, hexRadius: number): string {
  // Calculate the direction outward from the hexagon center for this door
  const angle = (Math.PI / 2) - (doorIndex * Math.PI / 3);
  const outwardX = Math.cos(angle);
  const outwardY = -Math.sin(angle);
  
  // Loop parameters
  const loopRadius = hexRadius * 0.4;
  const loopDistance = hexRadius * 0.3; // How far from the door to place the loop center
  
  // Calculate loop center
  const centerX = doorPos.x + outwardX * loopDistance;
  const centerY = doorPos.y + outwardY * loopDistance;
  
  // Create a circular arc that loops back to the same door
  // We'll create two arcs to make a complete circle
  const startX = doorPos.x;
  const startY = doorPos.y;
  
  // Control points for the loop
  const cp1X = centerX + outwardY * loopRadius;
  const cp1Y = centerY - outwardX * loopRadius;
  const cp2X = centerX - outwardY * loopRadius;
  const cp2Y = centerY + outwardX * loopRadius;
  
  return `M ${startX} ${startY} Q ${cp1X} ${cp1Y} ${centerX + outwardX * loopRadius} ${centerY + outwardY * loopRadius} Q ${cp2X} ${cp2Y} ${startX} ${startY}`;
}

/**
 * Calculate distance between two points
 */
export function distance(p1: Point, p2: Point): number {
  const dx = p1.x - p2.x;
  const dy = p1.y - p2.y;
  return Math.sqrt(dx * dx + dy * dy);
}

/**
 * Get a color for a room based on its label value (0-3)
 */
export function getRoomColor(label: number): string {
  const colors = [
    '#e3f2fd', // Light blue for 0
    '#f3e5f5', // Light purple for 1  
    '#e8f5e8', // Light green for 2
    '#fff3e0', // Light orange for 3
  ];
  
  return colors[label % colors.length];
}

/**
 * Get border color for a room based on its label value
 */
export function getRoomBorderColor(label: number): string {
  const colors = [
    '#1976d2', // Blue for 0
    '#7b1fa2', // Purple for 1
    '#388e3c', // Green for 2  
    '#f57c00', // Orange for 3
  ];
  
  return colors[label % colors.length];
}