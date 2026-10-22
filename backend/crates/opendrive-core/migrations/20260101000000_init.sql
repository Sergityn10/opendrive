-- Migración inicial de OpenDrive.
--
-- El árbol de archivos se materializa con `ltree`: cada nodo guarda en `path`
-- la secuencia de identificadores de sus ancestros. Esto permite resolver
-- subárboles, breadcrumbs y movimientos en cascada con índices GiST.

CREATE EXTENSION IF NOT EXISTS ltree;
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE EXTENSION IF NOT EXISTS citext;

-- ---------------------------------------------------------------------------
-- usuarios
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS users (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email         CITEXT NOT NULL,
    username      CITEXT NOT NULL,
    password_hash TEXT NOT NULL,
    role          TEXT NOT NULL DEFAULT 'user',
    quota_bytes   BIGINT NOT NULL DEFAULT 0,
    used_bytes    BIGINT NOT NULL DEFAULT 0,
    is_active     BOOLEAN NOT NULL DEFAULT TRUE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT users_role_check CHECK (role IN ('admin', 'user')),
    CONSTRAINT users_quota_check CHECK (quota_bytes >= 0),
    CONSTRAINT users_used_check CHECK (used_bytes >= 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS users_email_key ON users (lower(email));
CREATE UNIQUE INDEX IF NOT EXISTS users_username_key ON users (lower(username));

-- ---------------------------------------------------------------------------
-- sesiones
-- ---------------------------------------------------------------------------
-- El identificador de sesión es un i128 (tower-sessions) que se guarda como
-- texto para no depender del rango de BIGINT de Postgres.
CREATE TABLE IF NOT EXISTS sessions (
    id         TEXT PRIMARY KEY,
    data       JSONB NOT NULL DEFAULT '{}'::jsonb,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS sessions_expires_at_idx ON sessions (expires_at);

-- ---------------------------------------------------------------------------
-- nodos del árbol de archivos
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS nodes (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    parent_id       UUID REFERENCES nodes (id) ON DELETE CASCADE,
    owner_id        UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    path            LTREE NOT NULL,
    name            TEXT NOT NULL,
    name_norm       TEXT NOT NULL,
    kind            TEXT NOT NULL,
    mime_type       TEXT,
    size_bytes      BIGINT NOT NULL DEFAULT 0,
    blob_key        TEXT,
    checksum_sha256 TEXT,
    trashed_at      TIMESTAMPTZ,
    content_tsv     TSVECTOR,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT nodes_kind_check CHECK (kind IN ('file', 'folder')),
    CONSTRAINT nodes_size_check CHECK (size_bytes >= 0),
    -- Un archivo siempre tiene blob; una carpeta nunca.
    CONSTRAINT nodes_blob_check CHECK (
        (kind = 'file' AND blob_key IS NOT NULL) OR (kind = 'folder' AND blob_key IS NULL)
    )
);

-- Listado por carpeta: es la consulta más frecuente de la aplicación.
CREATE INDEX IF NOT EXISTS nodes_parent_idx
    ON nodes (owner_id, parent_id) WHERE trashed_at IS NULL;

-- Consultas de subárbol (mover, papelera, borrado en cascada).
CREATE INDEX IF NOT EXISTS nodes_path_idx ON nodes USING GIST (path);

-- Búsqueda por nombre: subcadena y similitud (pg_trgm).
CREATE INDEX IF NOT EXISTS nodes_name_trgm_idx ON nodes USING GIN (name_norm gin_trgm_ops);

-- Búsqueda por contenido.
CREATE INDEX IF NOT EXISTS nodes_content_tsv_idx ON nodes USING GIN (content_tsv);

-- Unicidad de nombre dentro de una carpeta. `parent_id` nunca es NULL para
-- los nodos normales (la raíz de cada usuario es la única excepción), así que
-- el índice no necesita COALESCE.
CREATE UNIQUE INDEX IF NOT EXISTS nodes_sibling_name_key
    ON nodes (owner_id, parent_id, name_norm) WHERE trashed_at IS NULL;

-- ---------------------------------------------------------------------------
-- uploads tus en curso
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS uploads (
    id           TEXT PRIMARY KEY,
    user_id      UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    node_id      UUID REFERENCES nodes (id) ON DELETE SET NULL,
    filename     TEXT NOT NULL,
    size_bytes   BIGINT NOT NULL DEFAULT 0,
    offset_bytes BIGINT NOT NULL DEFAULT 0,
    state        TEXT NOT NULL DEFAULT 'pending',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT uploads_state_check CHECK (state IN ('pending', 'completed', 'aborted'))
);

CREATE INDEX IF NOT EXISTS uploads_user_idx ON uploads (user_id, state);
CREATE INDEX IF NOT EXISTS uploads_updated_idx ON uploads (updated_at);
