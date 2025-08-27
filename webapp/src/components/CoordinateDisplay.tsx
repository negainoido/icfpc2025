import React from 'react';
import { Point2D } from '../types';

interface CoordinateDisplayProps {
  hoveredPoint: Point2D | null;
  screenPosition?: { x: number; y: number } | null;
}

const CoordinateDisplay: React.FC<CoordinateDisplayProps> = ({
  hoveredPoint,
  screenPosition,
}) => {
  if (!hoveredPoint) {
    return (
      <div
        style={{
          position: 'absolute',
          bottom: 10,
          right: 10,
          background: 'rgba(0, 0, 0, 0.7)',
          color: 'white',
          padding: '8px 12px',
          borderRadius: '4px',
          fontSize: '12px',
          fontFamily: 'monospace',
          pointerEvents: 'none',
          minWidth: '180px',
        }}
      >
        <div>Point: -</div>
        <div>Screen: -</div>
      </div>
    );
  }

  return (
    <div
      style={{
        position: 'absolute',
        bottom: 10,
        right: 10,
        background: 'rgba(0, 0, 0, 0.7)',
        color: 'white',
        padding: '8px 12px',
        borderRadius: '4px',
        fontSize: '12px',
        fontFamily: 'monospace',
        pointerEvents: 'none',
        minWidth: '180px',
      }}
    >
      <div>Point: ({hoveredPoint.x}, {hoveredPoint.y})</div>
      {screenPosition && (
        <div>Screen: ({screenPosition.x}, {screenPosition.y})</div>
      )}
    </div>
  );
};

export default CoordinateDisplay;