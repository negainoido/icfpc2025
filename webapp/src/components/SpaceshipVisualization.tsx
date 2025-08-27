import React, { useState } from 'react';
import DeckGL from '@deck.gl/react';
import { ScatterplotLayer } from '@deck.gl/layers';
import { COORDINATE_SYSTEM, OrthographicView } from '@deck.gl/core';
import { Point2D } from '../types';
import CoordinateDisplay from './CoordinateDisplay';

interface SpaceshipVisualizationProps {
  points: Point2D[];
  width?: number;
  height?: number;
}

const SpaceshipVisualization: React.FC<SpaceshipVisualizationProps> = ({
  points,
  width = 800,
  height = 600,
}) => {
  const [hoveredPoint, setHoveredPoint] = useState<Point2D | null>(null);
  const [screenPosition, setScreenPosition] = useState<{ x: number; y: number } | null>(null);
  const layers = [
    new ScatterplotLayer({
      id: 'spaceship-points',
      data: points,
      coordinateSystem: COORDINATE_SYSTEM.CARTESIAN,
      getPosition: (d: Point2D) => [d.x, d.y, 0],
      getRadius: 2,
      getFillColor: [255, 140, 0, 200],
      getLineColor: [0, 0, 0, 255],
      getLineWidth: 1,
      radiusMinPixels: 2,
      radiusMaxPixels: 5,
      pickable: true,
      onHover: ({ object, x, y }) => {
        if (object) {
          setHoveredPoint(object);
          setScreenPosition({ x, y });
        } else {
          setHoveredPoint(null);
          setScreenPosition(null);
        }
      },
    }),
  ];

  const getInitialViewState = () => {
    if (points.length === 0) {
      return {
        target: [0, 0, 0] as [number, number, number],
        zoom: 1,
      };
    }

    const xValues = points.map(p => p.x);
    const yValues = points.map(p => p.y);
    
    const minX = Math.min(...xValues);
    const maxX = Math.max(...xValues);
    const minY = Math.min(...yValues);
    const maxY = Math.max(...yValues);
    
    const centerX = (minX + maxX) / 2;
    const centerY = (minY + maxY) / 2;
    
    const rangeX = maxX - minX;
    const rangeY = maxY - minY;
    const maxRange = Math.max(rangeX, rangeY);
    
    let zoom = 1;
    if (maxRange > 0) {
      zoom = Math.log2(Math.min(width, height) / (maxRange * 2)) - 1;
      zoom = Math.max(zoom, -10);
      zoom = Math.min(zoom, 10);
    }

    return {
      target: [centerX, centerY, 0] as [number, number, number],
      zoom,
    };
  };

  return (
    <div style={{ width, height, position: 'relative', border: '1px solid #ccc' }}>
      <DeckGL
        width={width}
        height={height}
        views={[new OrthographicView({id: 'orthographic'})]}
        initialViewState={{orthographic: getInitialViewState()}}
        controller={true}
        layers={layers}
        getCursor={() => 'crosshair'}
      />
      <div
        style={{
          position: 'absolute',
          top: 10,
          left: 10,
          background: 'rgba(0, 0, 0, 0.7)',
          color: 'white',
          padding: '8px 12px',
          borderRadius: '4px',
          fontSize: '12px',
          pointerEvents: 'none',
        }}
      >
        Points: {points.length}
      </div>
      <CoordinateDisplay hoveredPoint={hoveredPoint} screenPosition={screenPosition} />
    </div>
  );
};

export default SpaceshipVisualization;