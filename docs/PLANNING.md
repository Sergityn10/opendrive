# OpenDrive — Plan de desarrollo

> Clon de Google Drive autoalojado, open source, en un único `docker compose`.

**Fecha del plan:** 2026-10-21
**Estado:** aprobado, en ejecución

---

## 0. Objetivo

Construir **OpenDrive**: un gestor de archivos autoalojado con la experiencia de
Google Drive (navegación por carpetas, subida resumible, previsualización, búsqueda,
compartido, WebDAV y sincronización de escritorio), que se despliegue con un solo
`docker compose up -d` en cualquier ordenador que tenga Docker, sin instalar nada
más en el host.

---

## 1. Decisiones de stack

Cada elección tiene una razón verificada y una alternativa descartada.

| Área | Elegido | Por qué | Descartado y por qué |
|---|---|---|---|
| Framework backend | **axum 0.8.9** + tokio 1.54 | hiper/tokio nativo, `http` 1.x, extractor-first, WebSocket gratis | Actix (el ecosistema WebDAV/tus solo soporta axum), Rocket (menos ecosistema) |
| Capa SQL | **sqlx 0.9.0** + `PgPool` | type-checking en compile time, migraciones embebidas con `sqlx::migrate!()` | **sea-orm 2.0.4**: salió 2026-07-19 con 17 breaking changes; su CLI requiere DB viva. `deadpool-postgres` usa `tokio-postgres`, no es drop-in con sqlx. `bb8` no publica `bb8-pg` |
| Frontend | **React 19.3.1 + React Router 8.5.0 (SPA mode) + Vite 8.3.5** | SPA pura: el 95% de la interacción es cliente. `ssr:false` → salida estática `build/client` | **Next.js 16.4.2**: `middleware.ts` pasó a llamarse `proxy.ts` y no se soporta en static export. RSC/SSR no aportan nada en una app autenticada con datos privados por usuario |
| Build frontend | **Vite 8.3.5** (Rolldown) | 10–30× builds más rápidos, API de plugins de Rollup compatible | ⚠️ publicado el día de redactar este plan. Fallback documentado: `vite@7.3.7` + `@vitejs/plugin-react@5.2.0` |
| Upload resumible | **fileloft 0.3.1** (Rust, embebido en axum) | tus 1.0 nativo + hooks en proceso para auth/cuota, sin frontera de lenguaje | **tusd v2.10.1 (Go, sidecar)**: sin soporte JWT (cero ocurrencias de `jwt` en su árbol de fuentes), los hooks HTTP añaden latencia y no vinculan el upload al usuario en PATCH. **Multipart plano**: sin resume ni progreso por chunk |
| WebDAV | **dav-server 0.11.0** | RFC 4918 completo: PROPFIND/PROPPATCH/**LOCK-UNLOCK**, pasa litmus, sin avisos RustSec, ejemplo oficial de axum 0.8 | `webdav-server` (Node): single-maintainer, 2 CVEs de path traversal en 2.6.3. `nephele`: alpha |
| Sesiones | **tower-sessions 0.15.0** + `SessionStore` Postgres propio (~100 LOC) | estándar en axum 0.8 | ⚠️ `tower-sessions-sqlx-store 0.15.0` fija `sqlx ^0.8` → **duplica sqlx en el árbol** y los `PgPool` no son compatibles |
| Hash de contraseñas | **argon2 0.6.0** (Argon2id) | RustCrypto, estándar de facto | — |
| Frontend state | **TanStack Query 5.104.5** + **Zustand 5.0.15** + **TanStack Virtual 3.14.21** | Query para estado de servidor, Zustand para UI local, nunca duplicar | Redux Toolkit 2.13: `latest` y `next` apuntan a líneas mayores distintas |
| Subidas (UI) | **Uppy 6** (`@uppy/tus` 6.0.0) | tus 1.0, `onBeforeRequest` para auth, `allowedMetaFields` | `tus-js-client` 4.3.1 sin publicar desde 2025-01 (v5 en prerelease) |
| Búsqueda | **Postgres `pg_trgm` + `tsvector`** | Cero contenedores extra; cubre nombre (subcadena + fuzzy) y cuerpo | Meilisearch / Typesense: doble escritura y bug clásico de consistencia. OpenSearch: JVM + 1–3 GB de heap |
| Proxy / TLS | **Caddy 2.11.7** | TLS automático Let's Encrypt, config de ~12 líneas | Traefik: ~3× más verboso. Nginx: sin TLS automático |
| Miniaturas | **`image` 0.25.10** + `fast_image_resize` 6.1.0** | libvips vía fast_image_resize, sin salto de red | ⚠️ `image` **no decodifica HEIC** (fotos iPhone) → sidecar aparte |
| Sync escritorio (Fase 6) | **rclone v1.75.2** primero, luego **Tauri 2.12.4** | rclone funciona hoy contra WebDAV sin escribir motor de sync | Clientes ownCloud/Nextcloud desktop **no** son clientes WebDAV genéricos: fijan `/remote.php/dav/files` y el API de capabilities de su servidor |

### 1.1 Pines de versiones

```
Backend   axum 0.8.9 · tokio 1.54.0 · tower-http 0.7.1 · axum-extra 0.12.6
          sqlx 0.9.0 · argon2 0.6.0 · tower-sessions 0.15.0 · jsonwebtoken 11.1.0
          dav-server 0.11.0 · image 0.25.10 · fast_image_resize 6.1.0
          cap-std 4.0.3 · tempfile 3.27.0 · utoipa 6.0.0 · clap 4.6.7
          fileloft-core 0.3.1  ← GIT rev 980a906a (¡NO crates.io!)
Frontend  react 19.3.1 · react-router 8.5.0 · vite 8.3.5 · typescript 5.9.3
          @tanstack/react-query 5.104.5 · @tanstack/react-virtual 3.14.21
          zustand 5.0.15 · @uppy/core 6.3.0 · @uppy/tus 6.0.0 · tailwindcss 4.3.3
Imágenes  caddy:2.11.7-alpine · postgres:18.6-bookworm · rust:1.99.1-bookworm
          node:24.22.1-bookworm-slim
```

### 1.2 MSRV

**Rust 1.99** (impuesto por `fileloft` 1.95 y `sqlx` 0.9 con 1.94).

---

## 2. Arquitectura

```
                    Internet
                       │ 443 (TLS automático Caddy)
              ┌────────▼─────────┐
              │  caddy  :443     │  reverse_proxy, timeouts 12h para uploads
              └────────┬─────────┘
        ┌──────────────┼───────────────┐
        │              │               │
   /            /api/*                /dav/*
        │              │               │
┌───────▼──────┐ ┌─────▼──────────────┐ │
│  frontend    │ │  backend (Rust)    │ │
│  (nginx      │ │  axum             │ │
│   estático)  │ │  ├ /api/*       JSON│ │
│  :80         │ │  ├ /api/uploads tus│ │
└──────────────┘ │  └ /dav/*     WebDAV│ │
                 └─────┬──────────────┘ │
                       │                │
              ┌────────▼────┐   ┌───────▼────────┐
              │  postgres   │   │  ./data/files  │  bind mount
              │  18.6       │   │  blobs/ staging/ thumbs/ │
              └─────────────┘   └────────────────┘
```

Un solo binario Rust sirve API + tus + WebDAV. El frontend se compila a estáticos y lo
sirve nginx (cache headers correctos, build independiente).

### 2.1 Layout de disco

```
data/
├── postgres/          # PGDATA
├── blobs/ab/cd/<uuid> # contenido por blob_key (nunca ruta proporcionada por el usuario)
├── staging/           # uploads tus en curso
└── thumbs/            # cache de miniaturas
```

---

## 3. Modelo de datos (Postgres)

```sql
CREATE EXTENSION IF NOT EXISTS ltree;    -- jerarquía de carpetas
CREATE EXTENSION IF NOT EXISTS pg_trgm;  -- búsqueda fuzzy por nombre

users(id uuid pk, email citext unique, username citext unique,
      password_hash text, role text, quota_bytes bigint,
      is_active bool, created_at, updated_at)

sessions(id text pk, user_id uuid fk, data bytea,
         created_at, last_seen_at, expires_at)  -- SessionStore propio

nodes(id uuid pk, parent_id uuid fk, owner_id uuid fk,
      path ltree NOT NULL,                 -- materialized path
      name text, name_norm text,
      kind text CHECK(kind IN ('file','folder')),
      mime_type text, size_bytes bigint, blob_key text,
      checksum_sha256 text,
      trashed_at timestamptz,              -- papelera
      content_tsv tsvector,                -- texto extraído
      created_at, updated_at)

CREATE INDEX ON nodes (owner_id, parent_id) WHERE trashed_at IS NULL;
CREATE INDEX ON nodes USING gist (path);
CREATE INDEX ON nodes USING gin (name_norm gin_trgm_ops);
CREATE INDEX ON nodes USING gin (content_tsv);

uploads(id text pk, user_id uuid fk, node_id uuid,
        filename text, size_bytes bigint, offset_bytes bigint,
        state text, created_at, updated_at)
```

`ltree` da breadcrumbs (`npath(parent_id)`), "mover carpeta" recursivo (`path <@ old_path`),
borrado en cascada y deduplicación de blobs, todo en SQL puro sin contenedor extra.

---

## 4. Fases de desarrollo

### Fase 0 — Cimientos
- Monorepo: `backend/` (Cargo workspace), `frontend/`, `deploy/`, `docs/`
- `rust-toolchain.toml` fijando Rust 1.99
- `compose.yaml` con 4 servicios + red + volumen
- CI: `cargo fmt --clippy --test`, `cargo sqlx prepare --check`, `npm run lint --typecheck --test`

**Decisión clave de build**: sqlx exige `DATABASE_URL` en compile-time para sus macros
verificados. Solución: **commitear `.sqlx/`** al repo y `ENV SQLX_OFFLINE=true` en el
Dockerfile → el build de Docker **no necesita base de datos**. CI falla si `.sqlx` está
desactualizado.

### Fase 1 — Datos + Autenticación
- Migraciones sqlx (`sqlx::migrate!()` embebido, corre solo al arrancar el contenedor)
- `users` + bootstrap del admin: `OPENDRIVE_ADMIN_EMAIL` / `OPENDRIVE_ADMIN_PASSWORD`
  → si no existe el usuario, se crea en el arranque
- argon2id (m=64MiB, t=3, p=1) + `SessionStore` Postgres propio
- Login/logout/registro, middleware de sesión, rotación de ID de sesión en login
- Caddy: `drive.tudominio.com` → TLS automático, `api` y `dav` en el mismo origen
- **Aceptación**: `docker compose up -d` → login funciona con el admin del `.env`

### Fase 2 — API de archivos
- `GET /api/nodes?parent=<id>` — listado paginado con cursor
- `POST /api/nodes` (carpeta), `PATCH /api/nodes/:id` (renombrar/mover), `DELETE`
  (a papelera), `POST /api/nodes/:id/restore`
- `GET /api/nodes/:id/download` — **soporte Range** (`http-range-header` + `axum-extra`
  `FileStream` → 206 + `Content-Range`), `ETag` = checksum, `Accept-Ranges: bytes`
- Capa de blobs: `cap-std` como **única** forma de tocar disco (el path lo decide el
  `blob_key` opaco, nunca el usuario) + `sanitize-filename` para el nombre visible
- Escrituras atómicas: `tempfile` en el mismo directorio → `fsync` → `persist()`
- OpenAPI con `utoipa` + `utoipa-swagger-ui` (feature `vendored`, sin descarga en build)

### Fase 3 — Upload resumible (tus)
**Detalle crítico de diseño.** `fileloft-axum`'s `tus_router` **no soporta auth por
request**, y los hooks (`pre_create`, `pre_finish`) reciben solo `UploadInfo` — **no
reciben los headers HTTP**. Solución:

1. Adaptador axum propio (~80 líneas, basado en `fileloft-axum/src/router.rs`) que
   valida la sesión **antes** de llamar a `TusHandler::handle`.
2. El adaptador reescribe la URI a `/api/uploads/{user_id}/{upload_id}` → el `Location`
   devuelto es por-usuario y no adivinable.
3. `SendDataStore` propio (wrapper de `FileStore`) valida en `get_upload()` que el
   prefijo del id pertenezca al usuario actual, con `tokio::task_local!` para el
   contexto de request (el hook se awaits inline, no en un `spawn`, así que hereda el
   task-local).
4. `pre_create` → valida cuota contra Postgres, fija `owner` en metadata.
   `pre_finish` → mueve el blob de `staging/` a `blobs/`, inserta la fila en `nodes`.
5. `Upload-Checksum` (extensión checksum) activo para integridad.

**Advertencia**: crates.io tiene `fileloft-core 0.3.0` (2026-04-27), que **no incluye 5
correcciones** del 2026-08-02, dos de ellas marcadas `fix(security)`. Hay que fijar el
**rev de git `980a906a` (v0.3.1)**.

- Uppy 6 en el frontend: `@uppy/tus` con `onBeforeRequest` para refrescar token,
  `allowedMetaFields` para no filtrar el user id al servidor tus
- Extensión `expiration` con TTL 24h para uploads abandonados + job de limpieza
- Límite de tamaño por cuota, por archivo y global

### Fase 4 — Frontend explorer
- React Router 8 en SPA mode; rutas: `/login`, `/drive`, `/drive/:folderId`, `/shared`,
  `/search`, `/settings`
- Grid/lista conmutables, virtualizados con TanStack Virtual, orden por nombre/fecha/tamaño
- Breadcrumb, selección múltiple (Shift/Ctrl), menú contextual, renombrar inline,
  arrastrar y soltar para mover
- **Bandeja de subidas** con progreso por archivo, reanudar, cancelar
- TanStack Query para servidor, Zustand para UI (modo vista, selección, bandeja)
- Filtros de búsqueda, atajos de teclado, skeletons, estados vacíos, toasts

### Fase 5 — Previews y búsqueda
- Miniaturas: `image` + `fast_image_resize`, cacheadas en `data/thumbs/` por
  `etag+tamaño`, generadas bajo demanda
- Preview de PDF **en el cliente** con `pdfjs-dist` (sin round-trip al servidor)
- Preview de imagen, audio y video nativos
- Texto plano y Markdown: visor simple
- Sidecar opcional `apache/tika:4.1.0` para extraer texto → alimenta `content_tsv`
- Búsqueda unificada: nombre (`pg_trgm`, difusa) + contenido (`tsvector`)
- ⚠️ PG18: al migrar desde PG ≤17 hay que **reindexar** los índices FTS y trigram
  (cambio documentado de proveedor de collation)

### Fase 6 — WebDAV + sincronización
- `dav-server 0.11.0` en `/dav`, con `GuardedFileSystem<SessionUser>` propio:
  metadatos en Postgres, bytes en disco
- `handle_guarded(req, principal, credentials)` inyecta el usuario → cada operación
  verifica ownership
- Lock system: `MemLs` en memoria (suficiente en nodo único)
- Compatibilidad verificada con `rclone v1.75.2` (`lsd`, `copy`, `bisync`)
- Cliente escritorio: **Tauri 2.12.4** envolviendo rclone + `webdav@5.11.0` (TS) para
  operaciones ligeras; credenciales en el keychain del OS

### Fase 7 — Compartir y colaborar
- Enlaces públicos: token opaco, `permission` (read/write), password opcional, expiración
- Permisos por usuario en carpetas
- **WOPI + Collabora Online** (`collabora/code:26.04.5.1.1`). Las imágenes 26.x **no
  traen shell** → generar la `proof_key` con un contenedor one-shot `alpine/openssl`;
  `cap_add: [SYS_ADMIN]`; healthcheck `coolwsd --probe --use-env-vars`
- Versionado de archivos e historial
- Registro de auditoría (`audit_log`), cuotas por usuario, panel de administración

---

## 5. Docker Compose

```yaml
services:
  caddy:
    image: caddy:2.11.7-alpine
    ports: ["80:80", "443:443"]
    volumes:
      - ./deploy/Caddyfile:/etc/caddy/Caddyfile:ro
      - caddy_data:/data          # certificados persistidos
      - caddy_config:/config
    depends_on: [backend]
    restart: unless-stopped

  backend:
    build: ./backend              # multi-stage: rust:1.99.1-bookworm → debian:bookworm-slim
    environment:
      DATABASE_URL: postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@postgres:5432/${POSTGRES_DB}
      OPENDRIVE_DATA_DIR: /data
      OPENDRIVE_PUBLIC_URL: https://${DOMAIN}
      OPENDRIVE_ADMIN_EMAIL: ${ADMIN_EMAIL}
      OPENDRIVE_ADMIN_PASSWORD: ${ADMIN_PASSWORD}
      RUST_LOG: ${RUST_LOG:-info}
      SQLX_OFFLINE: "true"        # build-time: sin DB
    volumes:
      - ./data:/data              # blobs + staging + thumbs
    depends_on:
      postgres: { condition: service_healthy }
    restart: unless-stopped

  postgres:
    image: postgres:18.6-bookworm
    environment:
      POSTGRES_USER: ${POSTGRES_USER}
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
      POSTGRES_DB: ${POSTGRES_DB}
    volumes: ["./data/postgres:/var/lib/postgresql/data"]
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U ${POSTGRES_USER}"]
      interval: 5s
      retries: 10
    restart: unless-stopped

  frontend:
    build: ./frontend             # node:24.22.1 → nginx:alpine
    ports: ["127.0.0.1:8080:80"]  # solo Caddy accede
    depends_on: [backend]
    restart: unless-stopped
```

Caddyfile:
```
{$DOMAIN} {
    encode zstd gzip
    request_body { max_size 0 }
    handle /api/uploads/* {
        reverse_proxy backend:8080 { transport http { read_timeout 12h } }
    }
    handle /dav/* {
        reverse_proxy backend:8080 { transport http { read_timeout 12h } }
    }
    handle /api/* { reverse_proxy backend:8080 }
    handle { reverse_proxy frontend:80 }
}
```

**"Sin descargar nada"**: `git clone` → editar `.env` → `docker compose up -d --build`.
Docker descarga las imágenes base (inherente), pero **no hay que instalar Rust, Node,
Postgres ni nada en el host**. Migraciones, usuario admin y limpieza de uploads
expirados arrancan solos. Opcionalmente, imágenes preconstruidas en GHCR:
`docker compose pull && docker compose up -d`.

---

## 6. Riesgos

| # | Riesgo | Impacto | Mitigación |
|---|---|---|---|
| 1 | **fileloft es joven** (9★, 1 mantenedor, v0.3.1 sin publicar) | Alto | Fijar rev `980a906a`; `vendor/` en el repo. Plan B: implementar el core de tus 1.0 en casa (~500–1000 LOC) o sidecar `tusd` con hooks HTTP |
| 2 | Vite 8/Rolldown muy reciente | Medio | Fallback a `vite@7.3.7` + `plugin-react@5.2.0`. React Router 8 acepta ambos (`vite: ^7 \|\| ^8`) |
| 3 | `dav-server` locks solo en memoria | Bajo (nodo único) | Redis o impl Postgres cuando haga falta HA |
| 4 | `image` no decodifica HEIC | Medio (fotos iPhone) | Sidecar `libheif`/ffmpeg, o `heic 0.1.6` |
| 5 | `tower-sessions-sqlx-store` duplica sqlx | Alto si se usa | `SessionStore` propio (~100 LOC) |
| 6 | PG18 cambió el proveedor de collation de FTS | Medio | Reindexar FTS/trigram al migrar de ≤17 |
| 7 | `utoipa-swagger-ui` descarga en build-time | Medio | Feature `vendored` |
| 8 | Caddy + WebDAV: clientes sin TUS necesitan timeouts largos | Bajo | `read_timeout 12h` ya configurado |

---

## 7. Verificaciones por fase

- `cargo fmt --check` · `cargo clippy --deny warnings` · `cargo test`
- `cargo sqlx prepare --check --workspace` (falla si `.sqlx` está stale)
- `npm run lint` · `npm run typecheck` · `vitest` · `playwright` (E2E: login → subir →
  listar → previsualizar → descargar)
- `docker compose config` + `docker compose up -d --build` + smoke test de healthchecks
- Compatibilidad real con `rclone lsd` / `rclone copy` contra `/dav`
- Litmus tests de WebDAV si están disponibles

---

## 8. Variables de entorno

| Variable | Requerida | Default | Descripción |
|---|---|---|---|
| `DOMAIN` | sí | — | Dominio público, p.ej. `drive.tudominio.com` |
| `POSTGRES_USER` | no | `opendrive` | |
| `POSTGRES_PASSWORD` | **sí** | — | |
| `POSTGRES_DB` | no | `opendrive` | |
| `ADMIN_EMAIL` | no | `admin@opendrive.local` | Admin creado en el primer arranque |
| `ADMIN_PASSWORD` | no | `changeme` | **Cambiar en producción** |
| `OPENDRIVE_PUBLIC_URL` | no | `http://localhost` | |
| `RUST_LOG` | no | `info` | |
| `OPENDRIVE_SESSION_TTL_HOURS` | no | `24` | |
| `OPENDRIVE_MAX_UPLOAD_SIZE` | no | `0` (sin límite) | |
| `OPENDRIVE_TUS_EXPIRY_HOURS` | no | `24` | |

---

## 9. Registro de progreso

| Fase | Estado | Notas |
|---|---|---|
| 0 - Cimientos | Hecho | Monorepo, compose, Caddy, CI, toolchain |
| 1 - Datos + Auth | Hecho | Migraciones, argon2, SessionStore propio, admin bootstrap |
| 2 - API de archivos | Hecho | CRUD de nodos, papelera, descarga con rangos, `ltree` |
| 3 - Subida tus | Hecho | fileloft git `980a906a`, adaptador axum propio, cuota |
| 4 - Frontend | Hecho | Explorer, cuadrícula/lista virtualizada, Uppy, previews |
| 5 - Previews + busqueda | Hecho | Miniaturas JPEG/PNG, pdf.js, `pg_trgm` + `tsvector` |
| 6 - WebDAV | Hecho | `dav-server` con `GuardedFileSystem` propio; verificado con rclone |
| 7 - Compartir | Pendiente | Enlaces públicos, permisos, WOPI/Collabora, auditoría |

### 9.1 Decisiones que cambiaron durante la implementación

**`fileloft` se fija por commit, no por versión.** La versión publicada en
crates.io (`0.3.0`, abril 2026) no incluye cinco correcciones del 2 de agosto,
dos de ellas marcadas `fix(security)`. Se usa el commit `980a906a` (v0.3.1).

**El adaptador de tus es propio, no el de `fileloft-axum`.** El router del
crate no permite autenticar la petición, y sus hooks reciben solo el
`UploadInfo`, sin las cabeceras HTTP. OpenDrive usa su propio adaptador
(unas 80 líneas) que valida la sesión antes de delegar y expone el usuario a
los hooks mediante `tokio::task_local!`.

**WebDAV no pasa por la capa de compresión.** `dav-server` produce el cuerpo
XML con un generador asíncrono cuyo `Stream` no está *fusionado*: al terminar
el productor, un segundo `poll` lo reanuda y el proceso entra en pánico con
«async fn resumed after completion». La capa de compresión gzip hace ese
segundo `poll`. La ruta `/dav` queda fuera de `CompressionLayer` y del límite
de cuerpo de 16 MiB (un `PUT` de WebDAV es el archivo entero).

> El defecto está en `dav-server` 0.11.0 y también en `main` (0.12.0-dev, sin
> publicar): `src/async_stream.rs` no limpia `self.fut` cuando el productor
> termina. Workaround aplicado: no comprimir la ruta WebDAV, más
> `CatchPanicLayer` para que un pánico no tumbe el servidor.

**`panic = "abort"` está desactivado.** Con él, un pánico en una petición
mataba el proceso entero. Ahora el pánico se convierte en un 500 y el servidor
sigue en pie.

**`NodeListQuery` implementa `Default` a mano.** El `derive` dejaba
`limit: 0`, que al recortarse a un mínimo de 1 devolvía solo el primer hijo de
la carpeta: en WebDAV se listaba un único archivo por directorio.

### 9.2 Verificación

Con la pila levantada (`docker compose up -d --build`):

```bash
# API, tus, papelera, búsqueda y descarga con rangos
docker run --rm -v "$PWD/deploy:/deploy" --network host alpine:3.20 \
  sh -c "apk add --no-cache bash curl coreutils ca-certificates && \
         cp /deploy/certs/caddy-root.crt /usr/local/share/ca-certificates/ && \
         update-ca-certificates && bash /deploy/e2e.sh"

# Miniaturas y búsqueda por contenido
docker run --rm -v "$PWD/deploy:/deploy" --network host alpine:3.20 \
  sh -c "apk add --no-cache bash curl coreutils python3 ca-certificates && \
         cp /deploy/certs/caddy-root.crt /usr/local/share/ca-certificates/ && \
         update-ca-certificates && bash /deploy/test-thumbs.sh"

# WebDAV con rclone
docker run --rm -v "$PWD/deploy:/deploy" --network host alpine:3.20 \
  sh -c "apk add --no-cache bash curl coreutils rclone ca-certificates && \
         cp /deploy/certs/caddy-root.crt /usr/local/share/ca-certificates/ && \
         update-ca-certificates && bash /deploy/test-webdav.sh"
```

El certificado `deploy/certs/caddy-root.crt` es la CA interna de Caddy, útil
solo en desarrollo: en producción Let's Encrypt emite un certificado público.

### 9.3 Lo que queda para la Fase 7

- Tabla `shares` con enlace público, permiso, contraseña y caducidad
- Permisos por usuario en carpetas
- Rutas públicas `/s/{token}` con descarga y previsualización
- Registro de auditoría (`audit_log`)
- Integración WOPI con Collabora Online
- Cliente de escritorio (Tauri 2.12.4) o integración con `rclone bisync`