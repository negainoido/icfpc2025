export interface User {
  id: number;
  name: string;
  email: string;
  createdAt: string;
}

export interface ApiResponse<T> {
  data: T;
  message?: string;
  success: boolean;
}

export interface Point2D {
  x: number;
  y: number;
}

export interface SpaceshipFileData {
  filename: string;
  content: string;
}

export type SpaceshipApiResponse = ApiResponse<SpaceshipFileData>;
