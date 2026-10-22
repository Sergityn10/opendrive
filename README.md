# OpenDrive

Gestor de archivos autoalojado con la experiencia de Google Drive. Servidor
único, sin dependencias externas obligatorias, despliega con un `docker compose
up -d`.

- **Frontend**: React 19 + React Router 8 + Vite 8 (SPA)
- **Backend**: Rust 1.99 + axum 0.8
- **Base de datos**: PostgreSQL 18
- **Almacenamiento**: sistema de archivos local (bind mount)
- **Subidas**: protocolo [tus 1.0](https://tus.io) resumible
- **Proxy**: Caddy con TLS automático

---

## Puesta en marcha

Requisitos: solo Docker con `docker compose` (v2).

```bash
git clone <este-repositorio> opendrive
cd opendrive

cp .env.example .env
$EDITOR .env          # Domain, POSTGRES_PASSWORD y ADMIN_PASSWORD

docker compose up -d --build
```

A partir de ese momento OpenDrive está disponible en `https://$DOMAIN`.

El primer arranque:

1. aplica las migraciones de la base de datos,
2. crea el usuario administrador indicado en `ADMIN_EMAIL` / `ADMIN_PASSWORD`,
3. crea su carpeta raíz «Mi Drive»,
4. emite el certificado TLS de Let's Encrypt para `$DOMAIN`.

Para consultar el estado:

```bash
docker compose ps
docker compose logs -f backend
```

### WebDAV

Cada usuario tiene su propio espacio WebDAV en `/dav`, que apunta a su carpeta
«Mi Drive»:

```bash
# Con rclone
rclone config create opendrive webdav \
  url https://drive.ejemplo.com/dav \
  vendor other \
  user admin@ejemplo.com \
  pass 'mi-contrasena'

rclone lsd opendrive:
rclone copy ./mis-archivos opendrive:Documents
```

También funciona con el explorador de archivos de macOS, Windows (con el
"Cliente WebDAV" de Windows) o cualquier cliente que hable RFC 4918.

La autenticación es por HTTP básico con el correo y la contraseña del usuario,
o con la cookie de sesión si la petición viene de un navegador.

> **Nota de implementación**: la ruta `/dav` no pasa por la capa de compresión
> gzip. `dav-server` genera el cuerpo XML con un generador asíncrono cuyo
> flujo no está fusionado, y comprimirlo provoca que el proceso entre en
> pánico. Los clientes WebDAV tampoco se benefician de la compresión.

---
## Desarrollo en local

Para trabajar sobre `http://localhost` sin certificados, edita el `.env`:

```dotenv
DOMAIN=localhost
```

Y accede por `https://localhost` (el certificado lo emite la CA interna de
Caddy) o directamente a los puertos de los servicios:

```bash
# API
curl -k https://localhost/api/health
```

---

## Despliegue en Servidor Casero / Acceso desde Internet (Dominio Propio)

Si vas a alojar OpenDrive en un ordenador o portátil de tu casa y acceder desde el exterior con un dominio propio:

1. **Diagnóstico y preparación automática (Windows):**
   ```powershell
   powershell -ExecutionPolicy Bypass -File .\deploy\setup-windows-host.ps1
   ```
   Este script detecta tu IP local (LAN), tu IP pública (WAN), comprueba los contenedores y te indica los datos exactos a introducir en tu Router y DNS.
2. **Puertos en el Router:** Reenvía los puertos **80 TCP** y **443 TCP/UDP** a la IP local de tu portátil (`192.168.0.20`).
3. **Registro DNS:** Añade un registro `A` en tu proveedor de dominio apuntando a tu IP pública.
4. **Si tu IP pública es dinámica y el router no tiene DDNS:** Inicia el actualizador de DNS automático con:
   ```bash
   docker compose --profile ddns up -d
   ```
5. **Alternativa sin abrir puertos:** Puedes usar Cloudflare Tunnel con:
   ```bash
   docker compose --profile tunnel up -d
   ```
6. **Despliegue sin clonar el repositorio (Docker Hub):**
   Puedes compilar las imágenes en tu PC y subirlas a Docker Hub con `./deploy/push-dockerhub.sh <tu_usuario>`. En tu servidor o portátil solo necesitas `compose.yaml` y ejecutar `docker compose pull && docker compose up -d`. Consulta la [Guía de Despliegue con Docker Hub](docs/DESPLIEGUE_DOCKER_HUB.md).
7. **Modo Solo Backend (sin Frontend en VPS o Portátil):**
   Si quieres que el servidor funcione exclusivamente como motor de almacenamiento de alta velocidad (WebDAV y API REST):

   - **Paso 1: Subir únicamente el Backend a Docker Hub (desde tu PC):**
     ```bash
     # En Windows:
     .\deploy\push-dockerhub.ps1 -Username "tu_usuario" -BackendOnly

     # En Linux / Mac:
     ./deploy/push-dockerhub.sh tu_usuario latest --backend-only
     ```

   - **Paso 2: Descargar y arrancar directamente en tu VPS o portátil (sin clonar el repo):**
     En la terminal del VPS o portátil, ejecuta:
     ```bash
     mkdir -p ~/opendrive && cd ~/opendrive

     # Descargar el compose exclusivo de backend y su configuración de entorno
     curl -fsSL https://raw.githubusercontent.com/<tu-usuario>/<tu-repo>/main/compose.backend.yaml -o compose.yaml
     curl -fsSL https://raw.githubusercontent.com/<tu-usuario>/<tu-repo>/main/.env.backend.example -o .env

     # Editar usuario de Docker Hub y contraseñas
     nano .env

     # Descargar imagen precompilada y arrancar en 5 segundos
     docker compose pull
     docker compose up -d
     ```
     El backend quedará escuchando en `http://<IP_SERVIDOR>:8080` (API y WebDAV en `/dav`).

Consulta la [Guía Paso a Paso para Servidor (Docker Hub y GHCR)](docs/GUIA_PASO_A_PASO_SERVIDOR.md), la [Guía de Acceso Externo](docs/ACCESO_EXTERNO.md) y la [Guía para Publicar en Repositorio Remoto (GitHub / GitLab)](docs/PUBLICAR_REPOSITORIO.md) para todos los detalles y solución de incidencias (NAT loopback, ahorro de energía, etc.).

---

## Variables de entorno

| Variable | Obligatoria | Por defecto | Descripción |
|---|:---:|---|---|
| `DOMAIN` | sí | — | Dominio público (ej. `drive.tudominio.com`). Caddy emite el TLS para él. |
| `ACME_EMAIL` | no | — | Correo electrónico para el registro en Let's Encrypt (necesario en dominio público). |
| `POSTGRES_USER` | no | `opendrive` | |
| `POSTGRES_PASSWORD` | sí | — | |
| `POSTGRES_DB` | no | `opendrive` | |
| `ADMIN_EMAIL` | no | `admin@opendrive.local` | Administrador del primer arranque. |
| `ADMIN_USERNAME` | no | `admin` | |
| `ADMIN_PASSWORD` | sí | — | |
| `SESSION_TTL_HOURS` | no | `24` | Duración de la sesión. |
| `MAX_UPLOAD_SIZE` | no | `0` | Tamaño máximo por archivo en bytes. 0 = sin límite. |
| `TUS_EXPIRY_HOURS` | no | `24` | Tiempo hasta que se da por abandonado un upload. |
| `DEFAULT_QUOTA_BYTES` | no | `0` | Cuota por usuario. 0 = sin cuota. |
| `ALLOW_REGISTRATION` | no | `false` | Permite el registro público. |
| `RUST_LOG` | no | `info` | |

---

## Arquitectura

```
                    Internet
                       │ 443 (TLS automático)
              ┌────────▼─────────┐
              │  caddy  :443     │
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
              │  postgres   │   │  ./data        │  bind mount
              │  18.6       │   │  postgres/     │
              └─────────────┘   │  blobs/        │
                                │  thumbs/       │
                                └────────────────┘
```

### Modelo de datos

El árbol de archivos se materializa con la extensión `ltree` de PostgreSQL:
cada nodo guarda en `path` la secuencia de identificadores de sus ancestros.
Eso permite resolver subárboles, breadcrumbs y movimientos en cascada con
índices GiST, sin recursar en la aplicación.

```sql
nodes(id, parent_id, owner_id, path ltree, name, name_norm, kind,
      mime_type, size_bytes, blob_key, checksum_sha256,
      trashed_at, content_tsv, created_at, updated_at)
```

La etiqueta `ltree` de cada nodo es su UUID, **no** su nombre: renombrar un
archivo no obliga a reescribir las rutas de sus descendientes.

### Almacenamiento

```
data/
├── postgres/              # base de datos
└── blobs/
    ├── ab/cd/<uuid>       # contenido, por clave opaca
    └── .staging/          # subidas en curso
```

El nombre del archivo en disco es una clave opaca generada por el servidor:
el usuario nunca influye en la ruta del sistema de archivos. Todo el acceso
pasa por `cap-std`, que confina las operaciones al directorio raíz e impide
escapar de él con `..` o rutas absolutas.

El área de staging vive **dentro** de la raíz de blobs, así que el paso de un
upload terminado a su posición definitiva es un `rename`: atómico y sin
copiar los bytes.

---

## Subidas resumibles

El endpoint `POST /api/uploads` implementa el protocolo tus 1.0:

```bash
# 1. Crear la subida
curl -i -X POST https://drive.ejemplo.com/api/uploads \
  -H 'Tus-Resumable: 1.0.0' \
  -H 'Upload-Length: 12345' \
  -H "Upload-Metadata: filename $(printf 'a.txt' | base64 -w0),parent_id $(printf '<uuid>' | base64 -w0)"

# → HTTP/1.1 201 Created
# → Location: /api/uploads/<usuario>_<id>

# 2. Enviar fragmentos, reanudables
curl -X PATCH https://drive.ejemplo.com/api/uploads/<id> \
  -H 'Tus-Resumable: 1.0.0' \
  -H 'Content-Type: application/offset+octet-stream' \
  -H 'Upload-Offset: 0' \
  --data-binary @parte1.bin
```

Características:

- La URL de la subida lleva el identificador del usuario: no es adivinable y
  un usuario no puede continuar la subida de otro.
- Si la conexión se interrumpe, la subida se reanuda desde el último offset
  confirmado por el servidor.
- La extensión `checksum` permite verificar la integridad de cada fragmento.
- Los uploads abandonados se limpian solos tras `TUS_EXPIRY_HOURS`.

---

## API

| Método | Ruta | Descripción |
|---|---|---|
| `POST` | `/api/auth/login` | Inicia sesión. |
| `POST` | `/api/auth/register` | Registra un usuario (si está habilitado). |
| `POST` | `/api/auth/logout` | Cierra la sesión. |
| `GET` | `/api/auth/me` | Perfil del usuario autenticado. |
| `GET` | `/api/nodes` | Lista el contenido de una carpeta. |
| `POST` | `/api/nodes` | Crea una carpeta. |
| `GET` | `/api/nodes/{id}` | Un nodo concreto. |
| `PATCH` | `/api/nodes/{id}` | Renombra o mueve un nodo. |
| `DELETE` | `/api/nodes/{id}` | Mueve a la papelera. |
| `POST` | `/api/nodes/{id}/restore` | Restaura desde la papelera. |
| `GET` | `/api/nodes/{id}/download` | Descarga, con soporte de rangos. |
| `GET` | `/api/nodes/{id}/breadcrumb` | Ruta de ancestros. |
| `GET` | `/api/trash` | Contenido de la papelera. |
| `DELETE` | `/api/trash` | Vacía la papelera. |
| `GET` | `/api/search` | Busca por nombre y contenido. |
| `POST` | `/api/uploads` | Crea una subida tus. |
| `PATCH` | `/api/uploads/{id}` | Envía un fragmento. |
| `HEAD` | `/api/uploads/{id}` | Consulta el offset actual. |
| `GET` | `/api/health` | Comprobación de vida. |
| `GET` | `/api/health/ready` | Comprobación de disponibilidad. |

---

## WebDAV

Cada usuario tiene su propio espacio WebDAV en `/dav`, que apunta a su carpeta
«Mi Drive»:

```bash
# Con rclone
rclone config create opendrive webdav \
  url https://drive.ejemplo.com/dav \
  vendor other \
  user admin@ejemplo.com \
  pass 'mi-contrasena'

rclone lsd opendrive:
rclone copy ./mis-archivos opendrive:Documents
```

También funciona con el explorador de archivos de macOS, Windows (con el
"Cliente WebDAV" de Windows) o cualquier cliente que hable RFC 4918.

La autenticación es por HTTP básico con el correo y la contraseña del usuario,
o con la cookie de sesión si la petición viene de un navegador.

> **Nota de implementación**: la ruta `/dav` no pasa por la capa de compresión
> gzip. `dav-server` genera el cuerpo XML con un generador asíncrono cuyo
> flujo no está fusionado, y comprimirlo provoca que el proceso entre en
> pánico. Los clientes WebDAV tampoco se benefician de la compresión.

---
## Desarrollo

### Backend

Requiere Rust 1.99. Un `rust-toolchain.toml` fija la versión.

```bash
cd backend
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Para compilar y comprobar dentro de un contenedor (igual que en producción):

```bash
./deploy/check.sh check     # cargo check
./deploy/check.sh clippy    # cargo clippy
./deploy/check.sh test      # cargo test
```

Las consultas de sqlx usan la forma con tiempo de ejecución, así que el build
no necesita una base de datos. Si en algún momento se adoptan los macros
verificados (`query!`), hay que commitear la caché `.sqlx/`.

### Frontend

```bash
cd frontend
npm install
npm run dev        # servidor de desarrollo en :5173, con proxy a la API
npm run typecheck
npm run lint
npm run build
```

### Comprobación de extremo a extremo

Con la pila levantada:

```bash
docker run --rm -v "$PWD/deploy:/deploy" --network host alpine:3.20 \
  sh -c "apk add --no-cache bash curl coreutils && bash /deploy/e2e.sh"
```

Recorre el inicio de sesión, la creación de carpetas, la subida por tus, la
descarga con rango parcial, la papelera y la búsqueda.

---

## Estado del proyecto

| Fase | Contenido | Estado |
|---|---|---|
| 0 | Monorepo, compose, Caddy, CI | ✅ |
| 1 | Datos, argon2, sesiones, admin | ✅ |
| 2 | API de nodos, papelera, descarga con rangos | ✅ |
| 3 | Subida resumible (tus) | ✅ |
| 4 | Frontend: explorer, cuadrícula/lista, Uppy | ✅ |
| 5 | Miniaturas, previews, búsqueda por contenido | 🔜 |
| 6 | WebDAV y cliente de escritorio | 🔜 |
| 7 | Compartir, permisos, WOPI/Collabora | 🔜 |

El plan detallado está en [`docs/PLANNING.md`](docs/PLANNING.md).

---

## Licencia

AGPL-3.0-or-later.
