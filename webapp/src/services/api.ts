import {
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
    public status?: number
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

async function handleResponse<T>(response: Response): Promise<T> {
  if (!response.ok) {
    const errorText = await response.text();
    throw new ApiError(
      `HTTP ${response.status}: ${errorText}`,
      response.status
    );
  }

  const contentLength = response.headers.get('content-length');
  if (contentLength === '0') {
    return undefined as T;
  }

  const text = await response.text();
  if (text.trim() === '') {
    return undefined as T;
  }

  return JSON.parse(text);
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

    return await handleResponse<SelectResponse>(response);
  },

  async explore(request: ExploreRequest): Promise<ExploreResponse> {
    const response = await fetch(`${API_BASE_URL}/api/explore`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(request),
    });

    return await handleResponse<ExploreResponse>(response);
  },

  async guess(request: GuessRequest): Promise<GuessResponse> {
    const response = await fetch(`${API_BASE_URL}/api/guess`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(request),
    });

    return await handleResponse<GuessResponse>(response);
  },

  async getSessions(): Promise<SessionsListResponse> {
    const response = await fetch(`${API_BASE_URL}/api/sessions`, {
      method: 'GET',
    });

    return await handleResponse<SessionsListResponse>(response);
  },

  async getCurrentSession(): Promise<Session | null> {
    const response = await fetch(`${API_BASE_URL}/api/sessions/current`, {
      method: 'GET',
    });

    return await handleResponse<Session | null>(response);
  },

  async getSessionDetail(sessionId: string): Promise<SessionDetail> {
    const response = await fetch(`${API_BASE_URL}/api/sessions/${sessionId}`, {
      method: 'GET',
    });

    return await handleResponse<SessionDetail>(response);
  },

  async abortSession(sessionId: string): Promise<void> {
    const response = await fetch(
      `${API_BASE_URL}/api/sessions/${sessionId}/abort`,
      {
        method: 'PUT',
      }
    );

    await handleResponse<void>(response);
  },
};

export { ApiError };
