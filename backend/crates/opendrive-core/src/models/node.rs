//! Modelo de nodo del árbol de archivos.
//!
//! Un nodo es un archivo o una carpeta. El árbol se materializa con `ltree`:
//! cada nodo guarda su ruta completa desde la raíz, lo que permite consultas
//! de subárbol, breadcrumbs y movimientos en cascada directamente en SQL.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Tipo de un nodo del árbol.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, utoipa::ToSchema,
)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum NodeKind {
    /// Un archivo con contenido.
    File,
    /// Una carpeta que puede contener otros nodos.
    Folder,
}

impl NodeKind {
    /// `true` si es una carpeta.
    pub fn is_folder(self) -> bool {
        matches!(self, NodeKind::Folder)
    }
}

/// Un nodo completo del árbol de archivos.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Node {
    /// Identificador único.
    pub id: Uuid,
    /// Carpeta que contiene este nodo. `None` para la raíz.
    pub parent_id: Option<Uuid>,
    /// Propietario del nodo.
    pub owner_id: Uuid,
    /// Ruta materializada desde la raíz (formato ltree).
    pub path: String,
    /// Nombre visible.
    pub name: String,
    /// Nombre normalizado para búsquedas y unicidad.
    pub name_norm: String,
    /// Tipo de nodo.
    pub kind: NodeKind,
    /// Tipo MIME detectado.
    pub mime_type: Option<String>,
    /// Tamaño en bytes. 0 para carpetas.
    pub size_bytes: i64,
    /// Clave opaca del blob en disco. `None` para carpetas.
    pub blob_key: Option<String>,
    /// Hash SHA-256 del contenido, en hexadecimal.
    pub checksum_sha256: Option<String>,
    /// Fecha en que se movió a la papelera.
    pub trashed_at: Option<DateTime<Utc>>,
    /// Fecha de creación.
    pub created_at: DateTime<Utc>,
    /// Última modificación.
    pub updated_at: DateTime<Utc>,
}

/// Resumen compacto de un nodo, usado en listados.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NodeSummary {
    /// Identificador único.
    pub id: Uuid,
    /// Carpeta que contiene este nodo.
    pub parent_id: Option<Uuid>,
    /// Nombre visible.
    pub name: String,
    /// Tipo de nodo.
    pub kind: NodeKind,
    /// Tipo MIME detectado.
    pub mime_type: Option<String>,
    /// Tamaño en bytes.
    pub size_bytes: i64,
    /// Hash SHA-256 del contenido.
    pub checksum_sha256: Option<String>,
    /// `true` si está en la papelera.
    pub trashed: bool,
    /// Fecha de creación.
    pub created_at: DateTime<Utc>,
    /// Última modificación.
    pub updated_at: DateTime<Utc>,
    /// `true` si tiene un hijo en la previsualización.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_children: Option<bool>,
}

impl From<Node> for NodeSummary {
    fn from(n: Node) -> Self {
        Self {
            id: n.id,
            parent_id: n.parent_id,
            name: n.name,
            kind: n.kind,
            mime_type: n.mime_type,
            size_bytes: n.size_bytes,
            checksum_sha256: n.checksum_sha256,
            trashed: n.trashed_at.is_some(),
            created_at: n.created_at,
            updated_at: n.updated_at,
            has_children: None,
        }
    }
}

/// Parámetros de listado de una carpeta.
#[derive(Debug, Clone, Deserialize, utoipa::IntoParams)]
pub struct NodeListQuery {
    /// Carpeta a listar. Si se omite, se lista la raíz del usuario.
    pub parent_id: Option<Uuid>,
    /// Número máximo de elementos a devolver.
    #[serde(default = "default_limit")]
    pub limit: i64,
    /// Cursor de paginación: el `id` del último elemento de la página anterior.
    pub cursor: Option<Uuid>,
    /// Campo por el que ordenar.
    pub sort: Option<SortField>,
    /// Dirección del orden.
    pub order: Option<SortOrder>,
    /// Texto de búsqueda por nombre dentro de la carpeta.
    pub q: Option<String>,
    /// Incluir los elementos de la papelera.
    pub include_trashed: Option<bool>,
}

impl Default for NodeListQuery {
    /// Valores por defecto de un listado.
    ///
    /// Se implementa a mano en lugar de derivar `Default` porque el límite
    /// debe tener sentido: con `limit: 0` el recorte a un mínimo de un
    /// elemento devolvería solo el primer hijo de la carpeta.
    fn default() -> Self {
        Self {
            parent_id: None,
            limit: default_limit(),
            cursor: None,
            sort: None,
            order: None,
            q: None,
            include_trashed: None,
        }
    }
}

fn default_limit() -> i64 {
    100
}

/// Campo de ordenación de un listado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SortField {
    /// Por nombre.
    Name,
    /// Por fecha de modificación.
    Modified,
    /// Por tamaño.
    Size,
    /// Por tipo.
    Kind,
}

/// Dirección de ordenación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    /// Ascendente.
    Asc,
    /// Descendente.
    Desc,
}
