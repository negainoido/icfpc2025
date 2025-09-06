import {
  ApiResponse,
  SelectRequest,
  SelectResponse,
  ExploreRequest,
  ExploreResponse,
  GuessRequest,
  GuessResponse,
} from '../types';


class ApiError extends Error {
  constructor(
    message: string,
    public status?: number,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

async function handleResponse<T>(response: Response): Promise<ApiResponse<T>> {
  if (!response.ok) {
    const errorText = await response.text();
    throw new ApiError(`HTTP ${response.status}: ${errorText}`, response.status);
  }

  const result: ApiResponse<T> = await response.json();
  if (!result.success) {
    throw new ApiError(result.message || 'API request failed');
  }

  return result;
}

export const api = {
  async select(request: SelectRequest): Promise<SelectResponse> {
    const response = await fetch(`/api/select`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(request),
    });

    const result = await handleResponse<SelectResponse>(response);
    return result.data!;
  },

  async explore(request: ExploreRequest): Promise<ExploreResponse> {
    const response = await fetch(`/api/explore`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(request),
    });

    const result = await handleResponse<ExploreResponse>(response);
    return result.data!;
  },

  async guess(request: GuessRequest): Promise<GuessResponse> {
    const response = await fetch(`/api/guess`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(request),
    });

    const result = await handleResponse<GuessResponse>(response);
    return result.data!;
  },
};

export { ApiError };