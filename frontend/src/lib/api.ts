import type {
  ApiErrorBody,
  ListParams,
  NodeListResponse,
  NodeSummary,
  UserProfile,
} from "./types";

/**
 * Error de la API con el código estable del servidor.
 */
export class ApiError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
  }

  /** `true` si la sesión ha caducado o nunca existió. */
  get isUnauthenticated(): boolean {
    return this.status === 401;
  }
}

/**
 * Cliente HTTP de la API.
 *
 * Todas las peticiones son relativas a `/api`, porque la SPA y el servidor
 * comparten origen. La cookie de sesión la gestiona el navegador.
 */
class ApiClient {
  private readonly base = "/api";

  async request<T>(path: string, init?: RequestInit): Promise<T> {
    const response = await fetch(`${this.base}${path}`, {
      credentials: "same-origin",
      headers: {
        "Content-Type": "application/json",
        ...init?.headers,
      },
      ...init,
    });

    if (!response.ok) {
      throw await toApiError(response);
    }

    // 204 No Content y 205 Reset Content no tienen cuerpo.
    if (response.status === 204 || response.status === 205) {
      return undefined as T;
    }

    const contentType = response.headers.get("content-type") ?? "";
    if (!contentType.includes("application/json")) {
      return undefined as T;
    }
    return (await response.json()) as T;
  }

  get<T>(path: string): Promise<T> {
    return this.request<T>(path, { method: "GET" });
  }

  post<T>(path: string, body?: unknown): Promise<T> {
    return this.request<T>(path, {
      method: "POST",
      body: body === undefined ? undefined : JSON.stringify(body),
    });
  }

  patch<T>(path: string, body?: unknown): Promise<T> {
    return this.request<T>(path, {
      method: "PATCH",
      body: body === undefined ? undefined : JSON.stringify(body),
    });
  }

  delete<T>(path: string): Promise<T> {
    return this.request<T>(path, { method: "DELETE" });
  }
}

/** Convierte una respuesta de error en {@link ApiError}. */
async function toApiError(response: Response): Promise<ApiError> {
  let code = "unknown_error";
  let message = `Error ${response.status}`;

  try {
    const contentType = response.headers.get("content-type") ?? "";
    if (contentType.includes("application/json")) {
      const body = (await response.json()) as ApiErrorBody;
      if (body?.error?.code) {
        code = body.error.code;
        message = body.error.message;
      }
    }
  } catch {
    // Si el cuerpo no es JSON válido, se mantiene el mensaje por defecto.
  }

  return new ApiError(response.status, code, message);
}

export const api = new ApiClient();

/* -------------------------------------------------------------------------
 * Endpoints
 * ---------------------------------------------------------------------- */

export const auth = {
  me: () => api.get<UserProfile>("/auth/me"),
  login: (identifier: string, password: string) =>
    api.post<{ user: UserProfile; authenticated: boolean }>("/auth/login", {
      identifier,
      password,
    }),
  register: (email: string, username: string, password: string) =>
    api.post<{ user: UserProfile; authenticated: boolean }>("/auth/register", {
      email,
      username,
      password,
    }),
  logout: () => api.post<{ authenticated: boolean }>("/auth/logout"),
};

export const nodes = {
  list: (params: ListParams = {}) => {
    const query = new URLSearchParams();
    if (params.parentId) query.set("parent_id", params.parentId);
    if (params.limit) query.set("limit", String(params.limit));
    if (params.cursor) query.set("cursor", params.cursor);
    if (params.sort) query.set("sort", params.sort);
    if (params.order) query.set("order", params.order);
    if (params.q) query.set("q", params.q);
    const suffix = query.toString();
    return api.get<NodeListResponse>(`/nodes${suffix ? `?${suffix}` : ""}`);
  },
  get: (id: string) => api.get<NodeSummary>(`/nodes/${id}`),
  createFolder: (parentId: string | null, name: string) =>
    api.post<NodeSummary>("/nodes", { parent_id: parentId, name }),
  rename: (id: string, name: string) =>
    api.patch<NodeSummary>(`/nodes/${id}`, { name }),
  move: (id: string, parentId: string) =>
    api.patch<NodeSummary>(`/nodes/${id}`, { parent_id: parentId }),
  trash: (id: string) => api.delete<{ trashed: number }>(`/nodes/${id}`),
  restore: (id: string) => api.post<{ restored: number }>(`/nodes/${id}/restore`),
  breadcrumb: (id: string) => api.get<NodeSummary[]>(`/nodes/${id}/breadcrumb`),
  downloadUrl: (id: string) => `/api/nodes/${id}/download`,
};

export const trash = {
  list: () => api.get<NodeSummary[]>("/trash"),
  empty: () => api.delete<{ deleted: number }>("/trash"),
};

export const search = {
  query: (q: string, content = false) => {
    const params = new URLSearchParams({ q });
    if (content) params.set("content", "true");
    return api.get<NodeSummary[]>(`/search?${params.toString()}`);
  },
};
