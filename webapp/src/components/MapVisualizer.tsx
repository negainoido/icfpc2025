import React, { useMemo, useState, useEffect, useRef } from 'react';
import { MapStruct, ExploreVisualizationProps, PathSegment } from '../types';
import {
  calculateHexagon,
  hexagonToSVGPath,
  createCurvePath,
  createLoopPath,
  getRoomColor,
  getRoomBorderColor,
} from '../utils/hexagon';
import { calculateRoomLayout, getLayoutBounds } from '../utils/layout';

interface Props extends ExploreVisualizationProps {
  map: MapStruct;
}

export default function MapVisualizer({
  map,
  chalkMarks,
  pathHistory,
  highlightCurrentRoom,
}: Props) {
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const [isDragging, setIsDragging] = useState(false);
  const [lastMousePos, setLastMousePos] = useState({ x: 0, y: 0 });
  const containerRef = useRef<HTMLDivElement | null>(null);

  // Calculate layout and hexagons
  const { hexagons, bounds, hexRadius } = useMemo(() => {
    // Use a virtual canvas scaled by room count so large maps spread out.
    const n = Math.max(1, map.rooms.length);
    const scale = Math.sqrt(n / 12); // baseline at 12 rooms
    const containerWidth = Math.round(1200 * scale);
    const containerHeight = Math.round(900 * scale);
    const roomLayout = calculateRoomLayout(
      map,
      containerWidth,
      containerHeight
    );
    const layoutBounds = getLayoutBounds(roomLayout);

    // Choose hex radius based on nearest neighbor distance to avoid overlaps
    let minNeighborDist = Infinity;
    for (let i = 0; i < roomLayout.length; i++) {
      for (let j = i + 1; j < roomLayout.length; j++) {
        const dx = roomLayout[j].position.x - roomLayout[i].position.x;
        const dy = roomLayout[j].position.y - roomLayout[i].position.y;
        const dist = Math.hypot(dx, dy);
        if (dist < minNeighborDist) minNeighborDist = dist;
      }
    }
    if (!isFinite(minNeighborDist)) minNeighborDist = 120;
    const hexRadius = Math.max(
      28,
      Math.min(60, Math.floor(minNeighborDist * 0.28))
    );

    const roomHexagons = roomLayout.map((room) => ({
      ...room,
      hexagon: calculateHexagon(room.position, hexRadius),
    }));

    return {
      hexagons: roomHexagons,
      bounds: layoutBounds,
      hexRadius,
    };
  }, [map]);

  // Calculate viewBox to fit all rooms
  const viewBox = useMemo(() => {
    const margin = 120;
    const width = bounds.maxX - bounds.minX + 2 * margin;
    const height = bounds.maxY - bounds.minY + 2 * margin;
    return {
      x: bounds.minX - margin,
      y: bounds.minY - margin,
      width,
      height,
    };
  }, [bounds]);

  // Scale-dependent sizes (SVG units) derived from hex radius
  const sizes = useMemo(() => {
    const doorRadius = Math.max(7, Math.min(14, hexRadius * 0.22));
    const roomLabelFont = Math.max(14, Math.min(22, hexRadius * 0.6));
    const indexFont = Math.max(10, Math.min(16, hexRadius * 0.42));
    const chalkBadgeRadius = Math.max(10, Math.min(16, hexRadius * 0.32));
    const pathWidth = Math.max(3, Math.min(6, hexRadius * 0.12));
    const stepCircleRadius = Math.max(8, Math.min(14, hexRadius * 0.28));
    const borderWidthBase = Math.max(2, Math.min(6, hexRadius * 0.12));
    return {
      doorRadius,
      roomLabelFont,
      indexFont,
      chalkBadgeRadius,
      pathWidth,
      stepCircleRadius,
      borderWidthBase,
    };
  }, [hexRadius]);

  // Auto-zoom to ensure rooms remain readable (min on-screen hex radius)
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const pxPerUnitX = rect.width / viewBox.width;
    const currentHexPx = hexRadius * pxPerUnitX * zoom;
    const minHexPx = 34;
    if (currentHexPx < minHexPx) {
      const neededZoom = Math.min(5, Math.max(zoom, minHexPx / (hexRadius * pxPerUnitX)));
      if (Math.abs(neededZoom - zoom) > 0.01) setZoom(neededZoom);
    }
  }, [hexRadius, viewBox.width]);

  const handleMouseDown = (e: React.MouseEvent) => {
    setIsDragging(true);
    setLastMousePos({ x: e.clientX, y: e.clientY });
  };

  const handleMouseMove = (e: React.MouseEvent) => {
    if (!isDragging) return;

    const deltaX = e.clientX - lastMousePos.x;
    const deltaY = e.clientY - lastMousePos.y;

    setPan((prev) => ({
      x: prev.x - deltaX / zoom,
      y: prev.y - deltaY / zoom,
    }));

    setLastMousePos({ x: e.clientX, y: e.clientY });
  };

  const handleMouseUp = () => {
    setIsDragging(false);
  };

  // Block page scroll and apply zoom with a non-passive wheel listener
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      e.stopPropagation();
      const zoomFactor = e.deltaY > 0 ? 0.9 : 1.1;
      setZoom((prev) => Math.max(0.1, Math.min(5, prev * zoomFactor)));
    };
    el.addEventListener('wheel', onWheel, { passive: false });
    return () => el.removeEventListener('wheel', onWheel as EventListener);
  }, []);

  const handleResetView = () => {
    setZoom(1);
    setPan({ x: 0, y: 0 });
  };

  return (
    <div style={{ height: '100%', display: 'flex', flexDirection: 'column' }}>
      {/* Controls */}
      <div
        style={{
          padding: '10px',
          borderBottom: '1px solid #dee2e6',
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
        }}
      >
        <div>
          <strong>Map Visualization</strong>
          <span
            style={{ marginLeft: '10px', color: '#6c757d', fontSize: '14px' }}
          >
            {map.rooms.length} rooms, {map.connections.length} connections
          </span>
        </div>

        <div style={{ display: 'flex', gap: '10px', alignItems: 'center' }}>
          <span style={{ fontSize: '14px' }}>
            Zoom: {(zoom * 100).toFixed(0)}%
          </span>
          <button
            onClick={() => setZoom((prev) => Math.min(5, prev * 1.2))}
            style={{
              padding: '5px 10px',
              backgroundColor: '#007bff',
              color: 'white',
              border: 'none',
              borderRadius: '3px',
              cursor: 'pointer',
            }}
          >
            +
          </button>
          <button
            onClick={() => setZoom((prev) => Math.max(0.1, prev / 1.2))}
            style={{
              padding: '5px 10px',
              backgroundColor: '#007bff',
              color: 'white',
              border: 'none',
              borderRadius: '3px',
              cursor: 'pointer',
            }}
          >
            -
          </button>
          <button
            onClick={handleResetView}
            style={{
              padding: '5px 10px',
              backgroundColor: '#6c757d',
              color: 'white',
              border: 'none',
              borderRadius: '3px',
              cursor: 'pointer',
            }}
          >
            Reset
          </button>
        </div>
      </div>

      {/* SVG Visualization */}
      <div
        ref={containerRef}
        style={{
          flex: 1,
          overflow: 'hidden',
          position: 'relative',
          overscrollBehavior: 'none',
          touchAction: 'none',
        }}
      >
        <svg
          width="100%"
          height="100%"
          viewBox={`${viewBox.x + pan.x} ${viewBox.y + pan.y} ${viewBox.width / zoom} ${viewBox.height / zoom}`}
          style={{
            cursor: isDragging ? 'grabbing' : 'grab',
            userSelect: 'none',
            overscrollBehavior: 'contain',
            touchAction: 'none',
          }}
          onMouseDown={handleMouseDown}
          onMouseMove={handleMouseMove}
          onMouseUp={handleMouseUp}
          onMouseLeave={handleMouseUp}
        >
          {/* Background grid */}
          <defs>
            <pattern
              id="grid"
              width="50"
              height="50"
              patternUnits="userSpaceOnUse"
            >
              <path
                d="M 50 0 L 0 0 0 50"
                fill="none"
                stroke="#f0f0f0"
                strokeWidth="1"
              />
            </pattern>
          </defs>
          <rect
            x={viewBox.x}
            y={viewBox.y}
            width={viewBox.width}
            height={viewBox.height}
            fill="url(#grid)"
          />

          {/* Connections (drawn first, so they appear behind rooms) */}
          <g>
            {map.connections.map((conn, index) => {
              const fromHex = hexagons.find(
                (h) => h.roomIndex === conn.from.room
              );
              const toHex = hexagons.find((h) => h.roomIndex === conn.to.room);

              if (!fromHex || !toHex) return null;

              // Check if it's a loop (same room and same door)
              const isLoop =
                conn.from.room === conn.to.room &&
                conn.from.door === conn.to.door;

              if (isLoop) {
                // Create a loop for connections to the same door
                const doorPos = fromHex.hexagon.doorPositions[conn.from.door];
                const loopPath = createLoopPath(doorPos, conn.from.door, 50); // 50 is hex radius

                return (
                  <g key={index}>
                    <path
                      d={loopPath}
                      stroke="#495057"
                      strokeWidth="3"
                      fill="none"
                      strokeLinecap="round"
                      opacity="0.8"
                    />
                  </g>
                );
              } else {
                // Regular connection between different rooms or doors
                const fromDoor = fromHex.hexagon.doorPositions[conn.from.door];
                const toDoor = toHex.hexagon.doorPositions[conn.to.door];
                const curvePath = createCurvePath(fromDoor, toDoor);

                return (
                  <g key={index}>
                    <path
                      d={curvePath}
                      stroke="#495057"
                      strokeWidth={sizes.pathWidth}
                      fill="none"
                      strokeLinecap="round"
                      opacity="0.7"
                    />
                    {/* Connection label */}
                    <text
                      x={(fromDoor.x + toDoor.x) / 2}
                      y={(fromDoor.y + toDoor.y) / 2}
                      textAnchor="middle"
                      dominantBaseline="middle"
                      fontSize="10"
                      fill="#495057"
                      style={{ pointerEvents: 'none' }}
                    >
                      {conn.from.door}↔{conn.to.door}
                    </text>
                  </g>
                );
              }
            })}
          </g>

          {/* Explore Path Visualization */}
          {pathHistory && pathHistory.length > 0 && (
            <g>
              {pathHistory.map((segment, index) => {
                const fromHex = hexagons.find(
                  (h) => h.roomIndex === segment.from
                );
                const toHex = hexagons.find((h) => h.roomIndex === segment.to);

                if (!fromHex || !toHex) return null;

                const fromDoor = fromHex.hexagon.doorPositions[segment.door];
                const toCenter = toHex.position;
                const curvePath = createCurvePath(fromDoor, toCenter);

                // Color path segments with gradient from blue to red
                const progress = index / Math.max(1, pathHistory.length - 1);
                const red = Math.round(50 + progress * 200);
                const blue = Math.round(255 - progress * 200);
                const pathColor = `rgb(${red}, 100, ${blue})`;

                return (
                  <g key={`path-${index}`}>
                    <path
                      d={curvePath}
                      stroke={pathColor}
                      strokeWidth={sizes.pathWidth + 2}
                      fill="none"
                      strokeLinecap="round"
                      opacity="0.8"
                      strokeDasharray="5,3"
                    />
                    {/* Step number */}
                    <circle
                      cx={(fromDoor.x + toCenter.x) / 2}
                      cy={(fromDoor.y + toCenter.y) / 2}
                      r={sizes.stepCircleRadius}
                      fill={pathColor}
                      opacity="0.9"
                    />
                    <text
                      x={(fromDoor.x + toCenter.x) / 2}
                      y={(fromDoor.y + toCenter.y) / 2}
                      textAnchor="middle"
                      dominantBaseline="middle"
                      fontSize={Math.max(10, sizes.stepCircleRadius * 0.9)}
                      fontWeight="bold"
                      fill="white"
                      style={{ pointerEvents: 'none' }}
                    >
                      {index + 1}
                    </text>
                  </g>
                );
              })}
            </g>
          )}

          {/* Rooms */}
          <g>
            {hexagons.map((room, index) => {
              const hexPath = hexagonToSVGPath(room.hexagon);
              const isStartingRoom = index === map.startingRoom;
              const isCurrentRoom = highlightCurrentRoom === index;
              const hasChalkMark = chalkMarks?.has(index);
              const chalkLabel = chalkMarks?.get(index);

              // Determine the label to display (chalk mark overrides original)
              const displayLabel = hasChalkMark ? chalkLabel : room.label;

              // Determine border color and width
              let borderColor = getRoomBorderColor(room.label);
              let borderWidth = sizes.borderWidthBase;

              if (isCurrentRoom) {
                borderColor = '#ffc107'; // Amber for current room
                borderWidth = sizes.borderWidthBase + 2;
              } else if (isStartingRoom) {
                borderColor = '#dc3545'; // Red for starting room
                borderWidth = sizes.borderWidthBase + 1;
              }

              return (
                <g key={index}>
                  {/* Current room highlight glow */}
                  {isCurrentRoom && (
                    <path
                      d={hexPath}
                      fill="none"
                      stroke="#ffc107"
                      strokeWidth="10"
                      opacity="0.3"
                    />
                  )}

                  {/* Room hexagon */}
                  <path
                    d={hexPath}
                    fill={getRoomColor(displayLabel || room.label)}
                    stroke={borderColor}
                    strokeWidth={borderWidth}
                    opacity="0.9"
                  />

                  {/* Chalk mark background */}
                  {hasChalkMark && (
                    <circle
                      cx={room.position.x + sizes.chalkBadgeRadius + 8}
                      cy={room.position.y - sizes.chalkBadgeRadius - 8}
                      r={sizes.chalkBadgeRadius}
                      fill="#28a745"
                      stroke="white"
                      strokeWidth={Math.max(1, sizes.chalkBadgeRadius * 0.16)}
                      opacity="0.9"
                    />
                  )}

                  {/* Room label */}
                  <text
                    x={room.position.x}
                    y={room.position.y - 5}
                    textAnchor="middle"
                    dominantBaseline="middle"
                    fontSize={sizes.roomLabelFont}
                    fontWeight="bold"
                    fill={hasChalkMark ? '#28a745' : '#343a40'}
                    style={{ pointerEvents: 'none' }}
                  >
                    {displayLabel}
                  </text>

                  {/* Room index */}
                  <text
                    x={room.position.x}
                    y={room.position.y + sizes.indexFont * 0.8}
                    textAnchor="middle"
                    dominantBaseline="middle"
                    fontSize={sizes.indexFont}
                    fill="#6c757d"
                    style={{ pointerEvents: 'none' }}
                  >
                    Room {index}
                  </text>

                  {/* Starting room indicator */}
                  {isStartingRoom && (
                    <text
                      x={room.position.x}
                      y={room.position.y + 25}
                      textAnchor="middle"
                      dominantBaseline="middle"
                      fontSize="10"
                      fontWeight="bold"
                      fill="#dc3545"
                      style={{ pointerEvents: 'none' }}
                    >
                      START
                    </text>
                  )}

                  {/* Current room indicator */}
                  {isCurrentRoom && (
                    <text
                      x={room.position.x}
                      y={room.position.y + (isStartingRoom ? 40 : 25)}
                      textAnchor="middle"
                      dominantBaseline="middle"
                      fontSize="10"
                      fontWeight="bold"
                      fill="#ffc107"
                      style={{ pointerEvents: 'none' }}
                    >
                      CURRENT
                    </text>
                  )}

                  {/* Chalk mark indicator */}
                  {hasChalkMark && (
                    <text
                      x={room.position.x + sizes.chalkBadgeRadius + 8}
                      y={room.position.y - sizes.chalkBadgeRadius - 8}
                      textAnchor="middle"
                      dominantBaseline="middle"
                      fontSize={Math.max(10, sizes.chalkBadgeRadius * 0.8)}
                      fontWeight="bold"
                      fill="white"
                      style={{ pointerEvents: 'none' }}
                    >
                      ✓
                    </text>
                  )}

                  {/* Door markers */}
                  {room.hexagon.doorPositions.map((doorPos, doorIndex) => (
                    <g key={doorIndex}>
                      <circle
                        cx={doorPos.x}
                        cy={doorPos.y}
                        r={sizes.doorRadius}
                        fill="white"
                        stroke={getRoomBorderColor(room.label)}
                        strokeWidth={Math.max(1.5, sizes.doorRadius * 0.25)}
                      />
                      <text
                        x={doorPos.x}
                        y={doorPos.y}
                        textAnchor="middle"
                        dominantBaseline="middle"
                        fontSize={Math.max(9, sizes.doorRadius * 0.9)}
                        fontWeight="bold"
                        fill={getRoomBorderColor(room.label)}
                        style={{ pointerEvents: 'none' }}
                      >
                        {doorIndex}
                      </text>
                    </g>
                  ))}
                </g>
              );
            })}
          </g>
        </svg>

        {/* Instructions overlay */}
        <div
          style={{
            position: 'absolute',
            bottom: '10px',
            left: '10px',
            backgroundColor: 'rgba(255, 255, 255, 0.9)',
            padding: '8px 12px',
            borderRadius: '4px',
            fontSize: '12px',
            color: '#6c757d',
          }}
        >
          Drag to pan • Scroll to zoom • Numbers on hexagons are room labels •
          Numbers in circles are door IDs
        </div>
      </div>

      {/* Legend */}
      <div
        style={{
          padding: '15px',
          borderTop: '1px solid #dee2e6',
          backgroundColor: '#f8f9fa',
        }}
      >
        <div
          style={{
            display: 'flex',
            flexWrap: 'wrap',
            gap: '20px',
            fontSize: '14px',
          }}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <div
              style={{
                width: '20px',
                height: '20px',
                backgroundColor: getRoomColor(0),
                border: `2px solid ${getRoomBorderColor(0)}`,
                borderRadius: '3px',
              }}
            />
            <span>Label 0</span>
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <div
              style={{
                width: '20px',
                height: '20px',
                backgroundColor: getRoomColor(1),
                border: `2px solid ${getRoomBorderColor(1)}`,
                borderRadius: '3px',
              }}
            />
            <span>Label 1</span>
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <div
              style={{
                width: '20px',
                height: '20px',
                backgroundColor: getRoomColor(2),
                border: `2px solid ${getRoomBorderColor(2)}`,
                borderRadius: '3px',
              }}
            />
            <span>Label 2</span>
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <div
              style={{
                width: '20px',
                height: '20px',
                backgroundColor: getRoomColor(3),
                border: `2px solid ${getRoomBorderColor(3)}`,
                borderRadius: '3px',
              }}
            />
            <span>Label 3</span>
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <div
              style={{
                width: '20px',
                height: '20px',
                backgroundColor: getRoomColor(0),
                border: '4px solid #dc3545',
                borderRadius: '3px',
              }}
            />
            <span>Starting Room</span>
          </div>
        </div>
      </div>
    </div>
  );
}
