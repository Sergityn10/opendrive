#!/usr/bin/env bash
# ==============================================================================
# OpenDrive — Instalador rápido de SOLO BACKEND en VPS o Portátil Linux
#
# Descarga el archivo compose del backend, genera el archivo .env si no existe,
# descarga las imágenes desde Docker Hub con 'docker compose pull' y arranca
# el servidor en segundo plano.
# ==============================================================================
set -euo pipefail

echo "=========================================================="
echo " OpenDrive — Instalador de Servidor Backend (Headless)"
echo "=========================================================="

# 1. Comprobar Docker
if ! command -v docker &>/dev/null; then
  echo "Error: Docker no está instalado en este sistema."
  echo "Instálalo ejecutando: curl -fsSL https://get.docker.com | sh"
  exit 1
fi

if ! docker compose version &>/dev/null; then
  echo "Error: Docker Compose (v2) no está disponible."
  exit 1
fi

# 2. Descargar compose.backend.yaml si no existe localmente
if [ ! -f compose.yaml ] && [ ! -f compose.backend.yaml ]; then
  echo "==> Descargando configuración de Docker Compose para Backend..."
  REPO_URL="${REPO_RAW_URL:-https://raw.githubusercontent.com/${GITHUB_REPO:-tu-usuario/Google-Drive-OpenSource}/main}"
  curl -fsSL "$REPO_URL/compose.backend.yaml" -o compose.yaml 2>/dev/null || {
    echo "Aviso: No se pudo descargar automáticamente por URL. Asegúrate de tener compose.yaml en este directorio o define GITHUB_REPO=tu-usuario/tu-repo."
  }
fi

# 3. Preparar .env
if [ ! -f .env ]; then
  if [ -f .env.backend.example ]; then
    cp .env.backend.example .env
  elif [ -f .env.example ]; then
    cp .env.example .env
  else
    echo "==> Generando archivo .env con valores por defecto..."
    cat << 'EOF' > .env
DOCKERHUB_USERNAME=opendrive
IMAGE_TAG=latest
BACKEND_PORT=8080
OPENDRIVE_PUBLIC_URL=http://localhost:8080
POSTGRES_USER=opendrive
POSTGRES_PASSWORD=opendrive-password-segura
POSTGRES_DB=opendrive
ADMIN_EMAIL=admin@opendrive.local
ADMIN_USERNAME=admin
ADMIN_PASSWORD=admin12345
SESSION_TTL_HOURS=24
MAX_UPLOAD_SIZE=0
TUS_EXPIRY_HOURS=24
DEFAULT_QUOTA_BYTES=0
ALLOW_REGISTRATION=false
RUST_LOG=info
EOF
  fi
  echo "==> Archivo .env listo. Edítalo para personalizar contraseñas si lo deseas."
fi

COMPOSE_FILE="compose.yaml"
if [ ! -f compose.yaml ] && [ -f compose.backend.yaml ]; then
  COMPOSE_FILE="compose.backend.yaml"
fi

# 4. Descargar imagen desde Docker Hub
echo "==> Descargando imágenes desde Docker Hub (docker compose pull)..."
docker compose -f "$COMPOSE_FILE" pull

# 5. Arrancar el servidor
echo "==> Iniciando backend y base de datos (docker compose up -d)..."
docker compose -f "$COMPOSE_FILE" up -d

# 6. Mostrar estado
LOCAL_IP="$(hostname -I 2>/dev/null | awk '{print $1}' || echo "127.0.0.1")"
PORT="$(grep '^BACKEND_PORT=' .env 2>/dev/null | cut -d'=' -f2 || echo "8080")"
[ -z "$PORT" ] && PORT="8080"

echo ""
echo "=========================================================="
echo " [OK] Servidor Backend de OpenDrive iniciado con éxito!"
echo "=========================================================="
echo " - Healthcheck: http://${LOCAL_IP}:${PORT}/api/health"
echo " - WebDAV:      http://${LOCAL_IP}:${PORT}/dav"
echo " - Tus Uploads: http://${LOCAL_IP}:${PORT}/api/uploads"
echo ""
echo " Para ver logs:     docker compose -f $COMPOSE_FILE logs -f backend"
echo " Para detener:      docker compose -f $COMPOSE_FILE down"
echo "=========================================================="
