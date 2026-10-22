/** Tipos compartidos con la API de OpenDrive. */

/** Rol de un usuario en la instancia. */
export type UserRole = "admin" | "user";

/** Perfil de un usuario. */
export interface UserProfile {
  id: string;
  username: string;
  email: string;
  role: UserRole;
  quota_bytes: number;
  used_bytes: number;
}

/** Tipo de un nodo del árbol de archivos. */
export type NodeKind = "file" | "folder";

/** Resumen de un nodo, tal y como lo devuelve el listado. */
export interface NodeSummary {
  id: string;
  parent_id: string | null;
  name: string;
  kind: NodeKind;
  mime_type: string | null;
  size_bytes: number;
  checksum_sha256: string | null;
  trashed: boolean;
  created_at: string;
  updated_at: string;
  has_children?: boolean;
}

/** Respuesta del listado de una carpeta. */
export interface NodeListResponse {
  items: NodeSummary[];
  next_cursor: string | null;
}

/** Campo de ordenación de un listado. */
export type SortField = "name" | "modified" | "size" | "kind";

/** Dirección de ordenación. */
export type SortOrder = "asc" | "desc";

/** Parámetros de listado. */
export interface ListParams {
  parentId?: string | null;
  limit?: number;
  cursor?: string | null;
  sort?: SortField;
  order?: SortOrder;
  q?: string;
}

/** Error devuelto por la API. */
export interface ApiErrorBody {
  error: {
    code: string;
    message: string;
  };
}
