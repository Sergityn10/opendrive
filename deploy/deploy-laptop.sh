#!/usr/bin/env bash
# ==============================================================================
# OpenDrive — Script de despliegue y actualización en Portátil Linux
#
# Este script descarga las imágenes desde Docker Hub y arranca el servidor
# sin necesidad de compilar código fuente en el portátil.
#
# Uso:
#   chmod +x deploy-laptop.sh
#   ./deploy-laptop.sh                  # Levanta Backend + Frontend
#   ./deploy-laptop.sh --backend-only   # Levanta solo Backend + Postgres (sin Frontend)
# ==============================================================================
set -euo pipefail

MODE="${1:-all}"
COMPOSE_TARGET=""
if [ "$MODE" = "--backend-only" ] || [ "$MODE" = "-b" ]; then
  COMPOSE_TARGET="backend"
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.." 2>/dev/null || cd "$SCRIPT_DIR"

echo "========================================================"
if [ -n "$COMPOSE_TARGET" ]; then
  echo " OpenDrive — Despliegue SOLO BACKEND (API + Postgres)"
else
  echo " OpenDrive — Despliegue en Portátil Linux (Docker Hub)"
fi
echo "========================================================"

# 1. Comprobar Docker
if ! command -v docker &>/dev/null; then
  echo "Error: Docker no está instalado en este portátil."
  echo "Instálalo ejecutando: curl -fsSL https://get.docker.com | sh"
  exit 1
fi

if ! docker compose version &>/dev/null; then
  echo "Error: Docker Compose (v2) no está disponible."
  exit 1
fi

# 2. Comprobar .env
if [ ! -f .env ]; then
  if [ -f .env.example ]; then
    echo "==> Creando .env a partir de .env.example..."
    cp .env.example .env
    echo "AVISO: Se ha creado el archivo .env. Edítalo con tu usuario de Docker Hub y tus contraseñas."
  else
    echo "Error: No se encuentra el archivo .env ni .env.example."
    exit 1
  fi
fi

# 3. Descargar las últimas imágenes desde Docker Hub
echo "==> Descargando imágenes desde Docker Hub..."
if [ -n "$COMPOSE_TARGET" ]; then
  docker compose pull "$COMPOSE_TARGET"
else
  docker compose pull
fi

# 4. Levantar los contenedores en segundo plano
echo "==> Levantando servicios..."
if [ -n "$COMPOSE_TARGET" ]; then
  docker compose up -d "$COMPOSE_TARGET"
else
  docker compose up -d
fi

# 5. Detectar la IP local del portátil
LOCAL_IP=""
if command -v hostname &>/dev/null; then
  LOCAL_IP="$(hostname -I 2>/dev/null | awk '{print $1}' || true)"
fi
if [ -z "$LOCAL_IP" ] && command -v ip &>/dev/null; then
  LOCAL_IP="$(ip -4 route get 1.1.1.1 2>/dev/null | awk '{print $7}' || true)"
fi
if [ -z "$LOCAL_IP" ]; then
  LOCAL_IP="192.168.0.XX"
fi

WEB_PORT="$(grep '^WEB_PORT=' .env 2>/dev/null | cut -d'=' -f2 || echo "3000")"
[ -z "$WEB_PORT" ] && WEB_PORT="3000"
BACKEND_PORT="$(grep '^BACKEND_PORT=' .env 2>/dev/null | cut -d'=' -f2 || echo "8080")"
[ -z "$BACKEND_PORT" ] && BACKEND_PORT="8080"

echo ""
echo "========================================================"
echo " [OK] OpenDrive está funcionando correctamente."
echo "========================================================"
if [ -n "$COMPOSE_TARGET" ]; then
  echo " Modo: SOLO BACKEND ACTIVO (Frontend desactivado)"
  echo " - API REST:    http://${LOCAL_IP}:${BACKEND_PORT}/api"
  echo " - WebDAV:      http://${LOCAL_IP}:${BACKEND_PORT}/dav"
  echo " - Tus uploads: http://${LOCAL_IP}:${BACKEND_PORT}/api/uploads"
else
  echo " Modo: COMPLETO (Frontend + Backend + Postgres)"
  echo " - Web UI:      http://${LOCAL_IP}:${WEB_PORT}"
  echo " - WebDAV:      http://${LOCAL_IP}:${BACKEND_PORT}/dav"
fi
echo ""
echo " Para ver los logs en directo:"
echo "   docker compose logs -f"
echo " Para detener el servidor:"
echo "   docker compose down"
echo "========================================================"
