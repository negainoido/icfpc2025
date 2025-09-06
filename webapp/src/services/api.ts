import {
  ApiResponse,
  SelectRequest,
  SelectResponse,
  ExploreRequest,
  ExploreResponse,
  GuessRequest,
  GuessResponse,
  Session,
  SessionDetail,
  SessionsListResponse,
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

// APIサーバーは自分と同じドメインで常に動いている
// localで動かしているときはvite.config.tsの設定によりプロキシされる
const API_BASE_URL = '';

export const api = {
  async select(request: SelectRequest): Promise<SelectResponse> {
    const response = await fetch(`${API_BASE_URL}/api/select`, {
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
    const response = await fetch(`${API_BASE_URL}/api/explore`, {
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
    const response = await fetch(`${API_BASE_URL}/api/guess`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(request),
    });

    const result = await handleResponse<GuessResponse>(response);
    return result.data!;
  },

  async getSessions(): Promise<SessionsListResponse> {
    const response = await fetch(`${API_BASE_URL}/api/sessions`, {
      method: 'GET',
    });

    const result = await handleResponse<SessionsListResponse>(response);
    return result.data!;
  },

  async getCurrentSession(): Promise<Session | null> {
    const response = await fetch(`${API_BASE_URL}/api/sessions/current`, {
      method: 'GET',
    });

    const result = await handleResponse<Session | null>(response);
    return result.data || null;
  },

  async getSessionDetail(sessionId: string): Promise<SessionDetail> {
    const response = await fetch(`${API_BASE_URL}/api/sessions/${sessionId}`, {
      method: 'GET',
    });

    const result = await handleResponse<SessionDetail>(response);
    return result.data!;
  },

  async abortSession(sessionId: string): Promise<void> {
    const response = await fetch(`${API_BASE_URL}/api/sessions/${sessionId}/abort`, {
      method: 'PUT',
    });

    await handleResponse<void>(response);
  },
};

export { ApiError };