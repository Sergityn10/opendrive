#!/usr/bin/env bash
# ==============================================================================
# OpenDrive — Compilar y subir imágenes a Docker Hub
#
# Uso:
#   ./deploy/push-dockerhub.sh [usuario] [tag] [--backend-only | --frontend-only]
#
# Ejemplos:
#   ./deploy/push-dockerhub.sh sergio latest                 # Sube Backend + Frontend
#   ./deploy/push-dockerhub.sh sergio latest --backend-only  # Sube ÚNICAMENTE el Backend
#   ./deploy/push-dockerhub.sh --backend-only               # Usa usuario y tag de .env
# ==============================================================================
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# Cargar .env si existe
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

USERNAME="${USERNAME:-${DOCKERHUB_USERNAME:-}}"
TAG="${TAG:-${IMAGE_TAG:-latest}}"

if [ -z "$USERNAME" ] || [ "$USERNAME" = "tu_usuario_dockerhub" ] || [ "$USERNAME" = "opendrive" ]; then
  echo "Error: Debes especificar tu usuario de Docker Hub."
  echo "Uso: $0 <tu_usuario_dockerhub> [tag] [--backend-only]"
  echo "O configúralo en .env con DOCKERHUB_USERNAME=tu_usuario"
  exit 1
fi

echo "========================================================"
echo " Subiendo OpenDrive a Docker Hub"
echo " Usuario: $USERNAME"
echo " Tag:     $TAG"
echo " Modo:    $TARGET"
echo "========================================================"

# Comprobar login en docker
echo "==> Verificando sesión en Docker Hub..."
if ! docker info 2>/dev/null | grep -q "Username"; then
  echo "Iniciando sesión en Docker Hub (ejecuta 'docker login' si falla):"
  docker login || exit 1
fi

# 1. Backend
if [ "$TARGET" = "all" ] || [ "$TARGET" = "backend" ]; then
  echo "==> Compilando Backend..."
  docker build -t "$USERNAME/opendrive-backend:$TAG" ./backend
  echo "==> Subiendo Backend a Docker Hub..."
  docker push "$USERNAME/opendrive-backend:$TAG"
  echo " [OK] Backend subido: $USERNAME/opendrive-backend:$TAG"
fi

# 2. Frontend
if [ "$TARGET" = "all" ] || [ "$TARGET" = "frontend" ]; then
  echo "==> Compilando Frontend..."
  docker build -t "$USERNAME/opendrive-frontend:$TAG" ./frontend
  echo "==> Subiendo Frontend a Docker Hub..."
  docker push "$USERNAME/opendrive-frontend:$TAG"
  echo " [OK] Frontend subido: $USERNAME/opendrive-frontend:$TAG"
fi

echo ""
echo "========================================================"
echo " [OK] Proceso completado exitosamente."
echo " En tu servidor o portátil Linux puedes descargar con:"
if [ "$TARGET" = "backend" ]; then
  echo "   docker compose pull backend && docker compose up -d backend"
else
  echo "   docker compose pull && docker compose up -d"
fi
echo "========================================================"
