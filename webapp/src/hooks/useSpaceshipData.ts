import { useState, useEffect } from 'react';
import { Point2D, SpaceshipApiResponse } from '../types';

interface UseSpaceshipDataResult {
  points: Point2D[];
  loading: boolean;
  error: string | null;
}

export const useSpaceshipData = (filename: string | null): UseSpaceshipDataResult => {
  const [points, setPoints] = useState<Point2D[]>([]);
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!filename) {
      setPoints([]);
      setError(null);
      return;
    }

    const fetchData = async () => {
      setLoading(true);
      setError(null);

      try {
        const response = await fetch(`/api/spaceship/${filename}`);
        
        if (!response.ok) {
          throw new Error(`HTTP error! status: ${response.status}`);
        }

        const data: SpaceshipApiResponse = await response.json();
        
        if (!data.success) {
          throw new Error(data.message || 'API request failed');
        }

        const parsedPoints = parsePointData(data.data.content);
        setPoints(parsedPoints);
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Unknown error occurred');
        setPoints([]);
      } finally {
        setLoading(false);
      }
    };

    fetchData();
  }, [filename]);

  return { points, loading, error };
};

const parsePointData = (content: string): Point2D[] => {
  const lines = content.trim().split('\n');
  const points: Point2D[] = [];

  for (const line of lines) {
    const trimmedLine = line.trim();
    if (trimmedLine === '') continue;

    const parts = trimmedLine.split(/\s+/);
    if (parts.length >= 2) {
      const x = parseFloat(parts[0]);
      const y = parseFloat(parts[1]);
      
      if (!isNaN(x) && !isNaN(y)) {
        points.push({ x, y });
      }
    }
  }

  return points;
};