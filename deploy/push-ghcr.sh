#!/usr/bin/env bash
# ==============================================================================
# OpenDrive — Compilar y subir imágenes a GitHub Container Registry (ghcr.io)
#
# Uso:
#   ./deploy/push-ghcr.sh [usuario_github] [tag] [--backend-only | --frontend-only]
#
# Ejemplos:
#   ./deploy/push-ghcr.sh sergio latest                 # Sube Backend + Frontend
#   ./deploy/push-ghcr.sh sergio latest --backend-only  # Sube ÚNICAMENTE el Backend
# ==============================================================================
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if [ -f .env ]; then
  export $(grep -v '^#' .env | xargs -d '\n' 2>/dev/null || true)
fi

TARGET="all"
USERNAME=""
TAG=""

for arg in "$@"; do
  case "$arg" in
    --backend-only|-b) TARGET="backend" ;;
    --frontend-only|-f) TARGET="frontend" ;;
    *)
      if [ -z "$USERNAME" ]; then
        USERNAME="$arg"
      elif [ -z "$TAG" ]; then
        TAG="$arg"
      fi
      ;;
  esac
done

USERNAME="${USERNAME:-${GITHUB_USER:-${DOCKERHUB_USERNAME:-}}}"
TAG="${TAG:-${IMAGE_TAG:-latest}}"

if [ -z "$USERNAME" ] || [ "$USERNAME" = "tu_usuario_dockerhub" ] || [ "$USERNAME" = "opendrive" ]; then
  echo "Error: Debes especificar tu usuario de GitHub."
  echo "Uso: $0 <usuario_github> [tag] [--backend-only]"
  exit 1
fi

# Convertir usuario a minúsculas (requisito de Docker / GHCR)
USERNAME="$(echo "$USERNAME" | tr '[:upper:]' '[:lower:]')"
REGISTRY="ghcr.io/$USERNAME"

echo "========================================================"
echo " Subiendo OpenDrive a GitHub Container Registry (ghcr.io)"
echo " Repositorio: $REGISTRY"
echo " Tag:         $TAG"
echo " Modo:        $TARGET"
echo "========================================================"

# Verificar login en ghcr.io
echo "==> Verificando sesión en ghcr.io..."
if ! docker login ghcr.io -u "$USERNAME" 2>/dev/null; then
  echo "Para iniciar sesión en ghcr.io necesitas un Personal Access Token (PAT) de GitHub con permiso 'write:packages'."
  echo "Ejecuta: echo <TU_TOKEN> | docker login ghcr.io -u $USERNAME --password-stdin"
  exit 1
fi

# 1. Backend
if [ "$TARGET" = "all" ] || [ "$TARGET" = "backend" ]; then
  echo "==> Compilando Backend..."
  docker build -t "$REGISTRY/opendrive-backend:$TAG" ./backend
  echo "==> Subiendo Backend a ghcr.io..."
  docker push "$REGISTRY/opendrive-backend:$TAG"
  echo " [OK] Backend subido: $REGISTRY/opendrive-backend:$TAG"
fi

# 2. Frontend
if [ "$TARGET" = "all" ] || [ "$TARGET" = "frontend" ]; then
  echo "==> Compilando Frontend..."
  docker build -t "$REGISTRY/opendrive-frontend:$TAG" ./frontend
  echo "==> Subiendo Frontend a ghcr.io..."
  docker push "$REGISTRY/opendrive-frontend:$TAG"
  echo " [OK] Frontend subido: $REGISTRY/opendrive-frontend:$TAG"
fi

echo ""
echo "========================================================"
echo " [OK] Proceso completado exitosamente."
echo " En tu servidor o portátil Linux configura tu .env con:"
echo "   IMAGE_PREFIX=$REGISTRY"
echo " Y descarga con:"
if [ "$TARGET" = "backend" ]; then
  echo "   docker compose pull backend && docker compose up -d backend"
else
  echo "   docker compose pull && docker compose up -d"
fi
echo "========================================================"
