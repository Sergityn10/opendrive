//! Repositorio de nodos del árbol de archivos.
//!
//! El árbol se materializa con `ltree`: cada nodo guarda en `path` la
//! secuencia de identificadores de sus ancestros (desde la raíz hasta él
//! mismo). Esto permite resolver subárboles, breadcrumbs y movimientos en
//! cascada con índices GiST, sin recursar en la aplicación.
//!
//! La etiqueta `ltree` de cada nodo es su UUID en hexadecimal, **no** su
//! nombre. Así, renombrar un archivo no obliga a reescribir las rutas de sus
//! descendientes; solo moverlo.

use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::models::node::{Node, NodeKind, NodeListQuery, NodeSummary, SortField, SortOrder};

/// Operaciones sobre la tabla `nodes`.
#[derive(Debug, Clone)]
pub struct NodeRepository {
    pool: PgPool,
}

impl NodeRepository {
    /// Crea el repositorio sobre el pool indicado.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Pool de conexiones, por si el llamante necesita abrir una transacción.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Crea la carpeta raíz de un usuario.
    ///
    /// La raíz es la carpeta "Mi Drive". Todo lo demás cuelga de ella, lo que
    /// permite que `parent_id` nunca sea `NULL` y que el índice de unicidad de
    /// nombres funcione sin `COALESCE`.
    pub async fn create_root(&self, owner_id: Uuid, name: &str) -> Result<Node> {
        let id = Uuid::new_v4();
        let label = ltree_label(id);
        let node = sqlx::query_as::<_, Node>(
            "INSERT INTO nodes (id, parent_id, owner_id, path, name, name_norm, kind, size_bytes)
             VALUES ($1, NULL, $2, $3::ltree, $4, $5, 'folder', 0)
             RETURNING id, parent_id, owner_id, path::text AS path, name, name_norm, kind,
                       mime_type, size_bytes, blob_key, checksum_sha256,
                       trashed_at, created_at, updated_at",
        )
        .bind(id)
        .bind(owner_id)
        .bind(&label)
        .bind(name)
        .bind(normalize_name(name))
        .fetch_one(&self.pool)
        .await?;
        Ok(node)
    }

    /// Devuelve la carpeta raíz de un usuario.
    pub async fn get_root(&self, owner_id: Uuid) -> Result<Option<Node>> {
        let node = sqlx::query_as::<_, Node>(
            "SELECT id, parent_id, owner_id, path::text AS path, name, name_norm, kind,
                    mime_type, size_bytes, blob_key, checksum_sha256,
                    trashed_at, created_at, updated_at
             FROM nodes
             WHERE owner_id = $1 AND parent_id IS NULL AND trashed_at IS NULL
             LIMIT 1",
        )
        .bind(owner_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(node)
    }

    /// Obtiene un nodo por identificador, verificando que pertenece al usuario.
    pub async fn get(&self, owner_id: Uuid, id: Uuid) -> Result<Option<Node>> {
        let node = sqlx::query_as::<_, Node>(
            "SELECT id, parent_id, owner_id, path::text AS path, name, name_norm, kind,
                    mime_type, size_bytes, blob_key, checksum_sha256,
                    trashed_at, created_at, updated_at
             FROM nodes WHERE id = $1 AND owner_id = $2",
        )
        .bind(id)
        .bind(owner_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(node)
    }

    /// Lista el contenido de una carpeta.
    ///
    /// La paginación es por cursor: se devuelven los elementos cuyo id sea
    /// mayor que `cursor` en el orden de sort, de forma estable.
    pub async fn list_children(
        &self,
        owner_id: Uuid,
        parent_id: Uuid,
        query: &NodeListQuery,
    ) -> Result<Vec<NodeSummary>> {
        let limit = query.limit.clamp(1, 500);
        let include_trashed = query.include_trashed.unwrap_or(false);
        let search = query.q.as_deref().map(normalize_name);

        let (sort_expr, order_expr) = sort_expressions(query.sort, query.order);

        // El SQL es dinámico solo en la cláusula de ordenación, y sus dos
        // valores salen de `sort_expressions`, que devuelve exclusively
        // literales estáticos. Todo lo demás va por parámetro vinculado.
        let sql = format!(
            "SELECT n.id, n.parent_id, n.name, n.kind, n.mime_type, n.size_bytes,
                    n.checksum_sha256, n.trashed_at, n.created_at, n.updated_at,
                    EXISTS (SELECT 1 FROM nodes c WHERE c.parent_id = n.id AND c.trashed_at IS NULL)
                        AS has_children
             FROM nodes n
             WHERE n.owner_id = $1
               AND n.parent_id = $2
               AND ($3 OR n.trashed_at IS NULL)
               AND ($4::text IS NULL OR n.name_norm LIKE '%' || $4::text || '%')
               AND ($5::uuid IS NULL OR n.id > $5::uuid)
             ORDER BY {sort_expr} {order_expr}, n.id {order_expr}
             LIMIT $6"
        );

        let rows: Vec<NodeRow> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(owner_id)
            .bind(parent_id)
            .bind(include_trashed)
            .bind(search)
            .bind(query.cursor)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;

        Ok(rows.into_iter().map(NodeSummary::from).collect())
    }

    /// Lista el contenido de la papelera de un usuario.
    pub async fn list_trashed(&self, owner_id: Uuid, limit: i64) -> Result<Vec<NodeSummary>> {
        let rows: Vec<NodeRow> = sqlx::query_as(
            "SELECT n.id, n.parent_id, n.name, n.kind, n.mime_type, n.size_bytes,
                    n.checksum_sha256, n.trashed_at, n.created_at, n.updated_at,
                    FALSE AS has_children
             FROM nodes n
             WHERE n.owner_id = $1 AND n.trashed_at IS NOT NULL
             ORDER BY n.trashed_at DESC
             LIMIT $2",
        )
        .bind(owner_id)
        .bind(limit.clamp(1, 500))
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(NodeSummary::from).collect())
    }

    /// Crea una carpeta dentro de un padre.
    pub async fn create_folder(&self, owner_id: Uuid, parent_id: Uuid, name: &str) -> Result<Node> {
        let name = sanitize_name(name)?;
        let parent = self
            .get(owner_id, parent_id)
            .await?
            .ok_or_else(|| Error::NotFound("carpeta padre".into()))?;

        if !parent.kind.is_folder() {
            return Err(Error::Validation("el padre no es una carpeta".into()));
        }
        if parent.trashed_at.is_some() {
            return Err(Error::Validation(
                "no se puede crear dentro de una carpeta de la papelera".into(),
            ));
        }

        let id = Uuid::new_v4();
        let label = ltree_label(id);
        let path = format!("{}.{}", parent.path, label);

        let node = sqlx::query_as::<_, Node>(
            "INSERT INTO nodes (id, parent_id, owner_id, path, name, name_norm, kind, size_bytes)
             VALUES ($1, $2, $3, $4::ltree, $5, $6, 'folder', 0)
             RETURNING id, parent_id, owner_id, path::text AS path, name, name_norm, kind,
                       mime_type, size_bytes, blob_key, checksum_sha256,
                       trashed_at, created_at, updated_at",
        )
        .bind(id)
        .bind(parent_id)
        .bind(owner_id)
        .bind(&path)
        .bind(&name)
        .bind(normalize_name(&name))
        .fetch_one(&self.pool)
        .await
        .map_err(map_unique_violation)?;

        Ok(node)
    }

    /// Inserta un nodo de archivo ya subido.
    pub async fn create_file(&self, owner_id: Uuid, new_file: NewFile) -> Result<Node> {
        let NewFile {
            parent_id,
            name,
            mime_type,
            size_bytes,
            blob_key,
            checksum_sha256,
        } = new_file;

        let name = sanitize_name(&name)?;
        let parent = self
            .get(owner_id, parent_id)
            .await?
            .ok_or_else(|| Error::NotFound("carpeta padre".into()))?;

        if !parent.kind.is_folder() {
            return Err(Error::Validation("el padre no es una carpeta".into()));
        }

        let id = Uuid::new_v4();
        let label = ltree_label(id);
        let path = format!("{}.{}", parent.path, label);

        let node = sqlx::query_as::<_, Node>(
            "INSERT INTO nodes
                (id, parent_id, owner_id, path, name, name_norm, kind,
                 mime_type, size_bytes, blob_key, checksum_sha256)
             VALUES ($1, $2, $3, $4::ltree, $5, $6, 'file', $7, $8, $9, $10)
             RETURNING id, parent_id, owner_id, path::text AS path, name, name_norm, kind,
                       mime_type, size_bytes, blob_key, checksum_sha256,
                       trashed_at, created_at, updated_at",
        )
        .bind(id)
        .bind(parent_id)
        .bind(owner_id)
        .bind(&path)
        .bind(&name)
        .bind(normalize_name(&name))
        .bind(mime_type)
        .bind(size_bytes)
        .bind(&blob_key)
        .bind(&checksum_sha256)
        .fetch_one(&self.pool)
        .await
        .map_err(map_unique_violation)?;

        Ok(node)
    }

    /// Renombra un nodo.
    pub async fn rename(&self, owner_id: Uuid, id: Uuid, name: &str) -> Result<Node> {
        let name = sanitize_name(name)?;
        let node = sqlx::query_as::<_, Node>(
            "UPDATE nodes SET name = $3, name_norm = $4, updated_at = now()
             WHERE id = $1 AND owner_id = $2 AND trashed_at IS NULL
             RETURNING id, parent_id, owner_id, path::text AS path, name, name_norm, kind,
                       mime_type, size_bytes, blob_key, checksum_sha256,
                       trashed_at, created_at, updated_at",
        )
        .bind(id)
        .bind(owner_id)
        .bind(&name)
        .bind(normalize_name(&name))
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| Error::NotFound("nodo".into()))?;

        Ok(node)
    }

    /// Mueve un nodo (y su subárbol) a otra carpeta del mismo propietario.
    pub async fn move_to(&self, owner_id: Uuid, id: Uuid, new_parent_id: Uuid) -> Result<Node> {
        let node = self
            .get(owner_id, id)
            .await?
            .ok_or_else(|| Error::NotFound("nodo".into()))?;
        if node.trashed_at.is_some() {
            return Err(Error::Validation("el nodo está en la papelera".into()));
        }
        if node.parent_id == Some(new_parent_id) {
            return Ok(node);
        }

        let new_parent = self
            .get(owner_id, new_parent_id)
            .await?
            .ok_or_else(|| Error::NotFound("carpeta destino".into()))?;
        if !new_parent.kind.is_folder() {
            return Err(Error::Validation("el destino no es una carpeta".into()));
        }
        if new_parent.trashed_at.is_some() {
            return Err(Error::Validation(
                "no se puede mover dentro de la papelera".into(),
            ));
        }

        // No se puede mover una carpeta dentro de sí misma.
        if new_parent.path.starts_with(&format!("{}.", node.path)) || new_parent.path == node.path {
            return Err(Error::Validation(
                "no se puede mover una carpeta dentro de sí misma".into(),
            ));
        }

        // La etiqueta ltree del nodo no cambia: el movimiento solo altera el
        // prefijo de la ruta de todo el subárbol.
        let old_prefix_len = node.path.split('.').count();
        let new_prefix = format!("{}.{}", new_parent.path, ltree_label(node.id));

        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "UPDATE nodes
             SET path = $3::ltree || subpath(path, $4), updated_at = now()
             WHERE owner_id = $1 AND path <@ $2::ltree",
        )
        .bind(owner_id)
        .bind(&node.path)
        .bind(&new_prefix)
        .bind(old_prefix_len as i32)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "UPDATE nodes SET parent_id = $3, updated_at = now() WHERE id = $1 AND owner_id = $2",
        )
        .bind(id)
        .bind(owner_id)
        .bind(new_parent_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        let updated = self
            .get(owner_id, id)
            .await?
            .ok_or_else(|| Error::NotFound("nodo".into()))?;
        Ok(updated)
    }

    /// Mueve un nodo a la papelera.
    ///
    /// Si el nodo es una carpeta, se papelera todo su subárbol.
    pub async fn trash(&self, owner_id: Uuid, id: Uuid) -> Result<Vec<Node>> {
        let node = self
            .get(owner_id, id)
            .await?
            .ok_or_else(|| Error::NotFound("nodo".into()))?;
        if node.trashed_at.is_some() {
            return Ok(vec![node]);
        }

        let mut tx = self.pool.begin().await?;

        let trashed: Vec<Node> = sqlx::query_as(
            "UPDATE nodes SET trashed_at = now(), updated_at = now()
             WHERE owner_id = $1 AND path <@ $2::ltree AND trashed_at IS NULL
             RETURNING id, parent_id, owner_id, path::text AS path, name, name_norm, kind,
                       mime_type, size_bytes, blob_key, checksum_sha256,
                       trashed_at, created_at, updated_at",
        )
        .bind(owner_id)
        .bind(&node.path)
        .fetch_all(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(trashed)
    }

    /// Restaura un nodo desde la papelera.
    pub async fn restore(&self, owner_id: Uuid, id: Uuid) -> Result<Vec<Node>> {
        let node = self
            .get(owner_id, id)
            .await?
            .ok_or_else(|| Error::NotFound("nodo".into()))?;
        if node.trashed_at.is_none() {
            return Ok(vec![node]);
        }

        // El padre también debe estar activo; si no, se restaura en la raíz.
        let restore_parent = match node.parent_id {
            Some(parent_id) => match self.get(owner_id, parent_id).await? {
                Some(p) if p.trashed_at.is_none() => Some(parent_id),
                _ => None,
            },
            None => None,
        };

        let mut tx = self.pool.begin().await?;

        let restored: Vec<Node> = sqlx::query_as(
            "UPDATE nodes SET trashed_at = NULL, updated_at = now()
             WHERE owner_id = $1 AND path <@ $2::ltree AND trashed_at IS NOT NULL
             RETURNING id, parent_id, owner_id, path::text AS path, name, name_norm, kind,
                       mime_type, size_bytes, blob_key, checksum_sha256,
                       trashed_at, created_at, updated_at",
        )
        .bind(owner_id)
        .bind(&node.path)
        .fetch_all(&mut *tx)
        .await?;

        if let Some(parent_id) = restore_parent {
            sqlx::query("UPDATE nodes SET parent_id = $2 WHERE id = $1")
                .bind(id)
                .bind(parent_id)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;
        Ok(restored)
    }

    /// Elimina permanentemente un nodo y su subárbol.
    ///
    /// Devuelve los nodos eliminados para que la capa de aplicación pueda
    /// borrar sus blobs.
    pub async fn purge(&self, owner_id: Uuid, id: Uuid) -> Result<Vec<Node>> {
        let node = self
            .get(owner_id, id)
            .await?
            .ok_or_else(|| Error::NotFound("nodo".into()))?;

        let deleted: Vec<Node> = sqlx::query_as(
            "DELETE FROM nodes
             WHERE owner_id = $1 AND path <@ $2::ltree
             RETURNING id, parent_id, owner_id, path::text AS path, name, name_norm, kind,
                       mime_type, size_bytes, blob_key, checksum_sha256,
                       trashed_at, created_at, updated_at",
        )
        .bind(owner_id)
        .bind(&node.path)
        .fetch_all(&self.pool)
        .await?;

        Ok(deleted)
    }

    /// Vacía la papelera de un usuario.
    pub async fn empty_trash(&self, owner_id: Uuid) -> Result<Vec<Node>> {
        let deleted: Vec<Node> = sqlx::query_as(
            "DELETE FROM nodes
             WHERE owner_id = $1 AND path <@ (
                 SELECT path FROM nodes
                 WHERE owner_id = $1 AND trashed_at IS NOT NULL
                 ORDER BY trashed_at DESC LIMIT 1
             )::ltree
             RETURNING id, parent_id, owner_id, path::text AS path, name, name_norm, kind,
                       mime_type, size_bytes, blob_key, checksum_sha256,
                       trashed_at, created_at, updated_at",
        )
        .bind(owner_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(deleted)
    }

    /// Busca un hijo directo de una carpeta por su nombre.
    pub async fn find_child(
        &self,
        owner_id: Uuid,
        parent_id: Uuid,
        name: &str,
    ) -> Result<Option<Node>> {
        let node = sqlx::query_as::<_, Node>(
            "SELECT id, parent_id, owner_id, path::text AS path, name, name_norm, kind,
                    mime_type, size_bytes, blob_key, checksum_sha256,
                    trashed_at, created_at, updated_at
             FROM nodes
             WHERE owner_id = $1 AND parent_id = $2 AND name_norm = $3 AND trashed_at IS NULL
             LIMIT 1",
        )
        .bind(owner_id)
        .bind(parent_id)
        .bind(normalize_name(name))
        .fetch_optional(&self.pool)
        .await?;
        Ok(node)
    }

    /// Reemplaza el contenido de un archivo que ya existe.
    ///
    /// Se usa en las escrituras WebDAV, donde `PUT` sobre un archivo
    /// existente debe sustituir su contenido.
    pub async fn replace_file_content(
        &self,
        id: Uuid,
        blob_key: &str,
        size_bytes: i64,
        checksum_sha256: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE nodes
             SET blob_key = $2, size_bytes = $3, checksum_sha256 = $4, updated_at = now()
             WHERE id = $1",
        )
        .bind(id)
        .bind(blob_key)
        .bind(size_bytes)
        .bind(checksum_sha256)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Actualiza el tipo MIME de un nodo.
    pub async fn set_mime_type(&self, id: Uuid, mime_type: Option<&str>) -> Result<()> {
        sqlx::query("UPDATE nodes SET mime_type = $2, updated_at = now() WHERE id = $1")
            .bind(id)
            .bind(mime_type)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Devuelve la ruta de ancestros de un nodo, desde la raíz hasta él.
    pub async fn ancestors(&self, owner_id: Uuid, id: Uuid) -> Result<Vec<Node>> {
        let node = self
            .get(owner_id, id)
            .await?
            .ok_or_else(|| Error::NotFound("nodo".into()))?;

        let labels: Vec<String> = node.path.split('.').map(|s| s.to_string()).collect();
        let mut uuids = Vec::with_capacity(labels.len());
        for label in &labels {
            // La etiqueta ltree es el UUID en hexadecimal sin guiones.
            if let Some(parsed) = u128::from_str_radix(label, 16).ok().map(Uuid::from_u128) {
                uuids.push(parsed);
            }
        }

        let nodes: Vec<Node> = sqlx::query_as(
            "SELECT id, parent_id, owner_id, path::text AS path, name, name_norm, kind,
                    mime_type, size_bytes, blob_key, checksum_sha256,
                    trashed_at, created_at, updated_at
             FROM nodes WHERE owner_id = $1 AND id = ANY($2) ORDER BY array_position($2, id)",
        )
        .bind(owner_id)
        .bind(&uuids)
        .fetch_all(&self.pool)
        .await?;

        Ok(nodes)
    }

    /// Busca nodos por nombre y, opcionalmente, por contenido.
    pub async fn search(
        &self,
        owner_id: Uuid,
        text: &str,
        include_content: bool,
        limit: i64,
    ) -> Result<Vec<NodeSummary>> {
        let needle = normalize_name(text);
        let pattern = format!("%{needle}%");

        // El SQL solo cambia en la cláusula del ranking: ambas variantes son
        // literales estáticos de este archivo. Los valores del usuario viajan
        // como parámetros vinculados.
        let sql = if include_content {
            "SELECT n.id, n.parent_id, n.name, n.kind, n.mime_type, n.size_bytes,
                    n.checksum_sha256, n.trashed_at, n.created_at, n.updated_at,
                    FALSE AS has_children
             FROM nodes n
             WHERE n.owner_id = $1
               AND n.trashed_at IS NULL
               AND (n.name_norm LIKE $2 OR n.content_tsv @@ plainto_tsquery('simple', $3))
             ORDER BY
               GREATEST(similarity(n.name_norm, $3), ts_rank(n.content_tsv, plainto_tsquery('simple', $3))) DESC,
               n.updated_at DESC
             LIMIT $4"
        } else {
            "SELECT n.id, n.parent_id, n.name, n.kind, n.mime_type, n.size_bytes,
                    n.checksum_sha256, n.trashed_at, n.created_at, n.updated_at,
                    FALSE AS has_children
             FROM nodes n
             WHERE n.owner_id = $1
               AND n.trashed_at IS NULL
               AND n.name_norm LIKE $2
             ORDER BY similarity(n.name_norm, $3) DESC, n.updated_at DESC
             LIMIT $4"
        };

        let rows: Vec<NodeRow> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(owner_id)
            .bind(&pattern)
            .bind(&needle)
            .bind(limit.clamp(1, 200))
            .fetch_all(&self.pool)
            .await?;

        Ok(rows.into_iter().map(NodeSummary::from).collect())
    }

    /// Indexa el texto extraído de un documento para la búsqueda por contenido.
    ///
    /// El texto se guarda en la columna `content_tsv`, que es un `tsvector` de
    /// Postgres con la configuración de idioma `simple`, para que la búsqueda
    /// no dependa de los diccionarios instalados en el sistema.
    pub async fn set_content(&self, id: Uuid, text: &str) -> Result<()> {
        sqlx::query(
            "UPDATE nodes SET content_tsv = to_tsvector('simple', $2), updated_at = now()
             WHERE id = $1",
        )
        .bind(id)
        .bind(text)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Suma el espacio usado por el usuario, sumando solo los nodos activos.
    pub async fn compute_used_bytes(&self, owner_id: Uuid) -> Result<i64> {
        // `SUM(bigint)` devuelve NUMERIC en Postgres, de ahí el cast.
        let used: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(size_bytes), 0)::bigint FROM nodes
             WHERE owner_id = $1 AND kind = 'file' AND trashed_at IS NULL",
        )
        .bind(owner_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(used)
    }
}

/// Fila del listado, equivalente a `NodeSummary` pero con `content_tsv`
/// excluido para no traer la columna de texto completo.
#[derive(Debug, sqlx::FromRow)]
struct NodeRow {
    id: Uuid,
    parent_id: Option<Uuid>,
    name: String,
    kind: NodeKind,
    mime_type: Option<String>,
    size_bytes: i64,
    checksum_sha256: Option<String>,
    trashed_at: Option<chrono::DateTime<chrono::Utc>>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    has_children: Option<bool>,
}

impl From<NodeRow> for NodeSummary {
    fn from(r: NodeRow) -> Self {
        Self {
            id: r.id,
            parent_id: r.parent_id,
            name: r.name,
            kind: r.kind,
            mime_type: r.mime_type,
            size_bytes: r.size_bytes,
            checksum_sha256: r.checksum_sha256,
            trashed: r.trashed_at.is_some(),
            created_at: r.created_at,
            updated_at: r.updated_at,
            has_children: r.has_children,
        }
    }
}

/// Expresiones SQL de ordenación, según el campo y la dirección solicitados.
fn sort_expressions(
    field: Option<SortField>,
    order: Option<SortOrder>,
) -> (&'static str, &'static str) {
    let dir = match order {
        Some(SortOrder::Desc) => "DESC",
        _ => "ASC",
    };
    let expr = match field {
        Some(SortField::Size) => "n.size_bytes",
        Some(SortField::Modified) => "n.updated_at",
        Some(SortField::Kind) => "n.kind",
        _ => "n.name_norm",
    };
    (expr, dir)
}

/// Etiqueta `ltree` de un nodo: su UUID sin guiones.
fn ltree_label(id: Uuid) -> String {
    id.simple().to_string()
}

/// Normaliza un nombre para búsquedas y comparaciones.
pub fn normalize_name(name: &str) -> String {
    name.trim().to_lowercase()
}

/// Limpia un nombre de archivo de caracteres no permitidos.
///
/// No recorta la longitud: el nombre visible lo limita el frontend. Lo que sí
/// hace es eliminar separadores de ruta y caracteres de control, de modo que
/// el nombre nunca pueda inyectar una ruta en el sistema de archivos.
pub fn sanitize_name(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(Error::Validation("el nombre no puede estar vacío".into()));
    }
    if trimmed.len() > 255 {
        return Err(Error::Validation("el nombre es demasiado largo".into()));
    }
    if trimmed == "." || trimmed == ".." {
        return Err(Error::Validation("el nombre no es válido".into()));
    }
    if trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains('\0') {
        return Err(Error::Validation(
            "el nombre contiene caracteres no válidos".into(),
        ));
    }
    // Los caracteres de control (excepto tabulador) no son válidos en nombres.
    if trimmed.chars().any(|c| c.is_control() && c != '\t') {
        return Err(Error::Validation(
            "el nombre contiene caracteres no válidos".into(),
        ));
    }
    Ok(trimmed.to_string())
}

/// Datos para insertar un nodo de archivo.
#[derive(Debug, Clone)]
pub struct NewFile {
    /// Carpeta que lo contiene.
    pub parent_id: Uuid,
    /// Nombre visible.
    pub name: String,
    /// Tipo MIME detectado.
    pub mime_type: Option<String>,
    /// Tamaño en bytes.
    pub size_bytes: i64,
    /// Clave opaca del blob en disco.
    pub blob_key: String,
    /// Hash SHA-256 del contenido.
    pub checksum_sha256: String,
}

/// Traduce una violación de unicidad a un error de conflicto.
fn map_unique_violation(err: sqlx::Error) -> Error {
    match &err {
        sqlx::Error::Database(db) if db.code().as_deref() == Some("23505") => {
            Error::Conflict("ya existe un elemento con ese nombre en la carpeta".into())
        }
        _ => Error::from(err),
    }
}
