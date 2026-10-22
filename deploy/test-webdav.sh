#!/usr/bin/env bash
# Prueba de WebDAV con rclone: listar, subir, descargar y borrar.
#
#   bash deploy/test-webdav.sh
set -euo pipefail

BASE="${BASE:-https://localhost}"
USER="${DAV_USER:-admin@opendrive.local}"
PASS="${DAV_PASS:-admin12345}"

say() { printf '\n\033[1m%s\033[0m\n' "$*"; }

# En desarrollo, Caddy usa su CA interna: hay que desactivar la verificación
# del certificado. En producción, esta línea no hace falta.
export RCLONE_TLS_SKIP_VERIFY="${RCLONE_TLS_SKIP_VERIFY:-1}"

# Configuración de rclone para el WebDAV de OpenDrive.
# El campo `pass` espera la contraseña en el formato "obscured" de rclone,
# que no es base64 sino su propio esquema de ofuscación.
export RCLONE_CONFIG=/tmp/rclone.conf
cat > "$RCLONE_CONFIG" <<EOF
[opendrive]
type = webdav
url = $BASE/dav
vendor = other
user = $USER
pass = $(rclone obscure "$PASS")
# En desarrollo, Caddy usa su CA interna: hay que desactivar la verificación
# del certificado. En producción, esta línea no hace falta.
no_check_certificate = ${SKIP_CERT_VERIFY:-true}
EOF

WORK=$(mktemp -d)
trap 'rm -rf "$WORK" "$RCLONE_CONFIG"' EXIT

say "1. Listar la raiz"
rclone lsd opendrive: 2>&1 | sed 's/^/   /'

say "2. Crear una carpeta y subir un archivo"
mkdir -p "$WORK/subdir"
printf 'Archivo de prueba para WebDAV.\nContenido con acentos: ñ, á, é.\n' > "$WORK/subdir/documento.txt"
printf 'Otro archivo.\n' > "$WORK/otro.txt"

# `rclone copy <dir> <remoto>` copia el contenido de <dir> dentro del remoto.
rclone copy "$WORK/subdir" opendrive:Pruebas 2>&1 | sed 's/^/   /'
rclone copy "$WORK/otro.txt" opendrive:Pruebas/ 2>&1 | sed 's/^/   /'

say "3. Listar lo que hay en la carpeta remota"
rclone lsd opendrive:Pruebas 2>&1 | sed 's/^/   /'
rclone ls opendrive:Pruebas 2>&1 | sed 's/^/   /'

say "4. Descargar y comparar"
mkdir -p "$WORK/descarga"
rclone copy opendrive:Pruebas "$WORK/descarga" 2>&1 | sed 's/^/   /'

if diff "$WORK/subdir/documento.txt" "$WORK/descarga/documento.txt" > /dev/null 2>&1 \
    && diff "$WORK/otro.txt" "$WORK/descarga/otro.txt" > /dev/null 2>&1; then
  echo "   OK: el contenido descargado es identico"
else
  echo "   FALLO: el contenido difiere"
  ls -la "$WORK/descarga" || true
  exit 1
fi

say "5. Renombrar con rclone (MOVE)"
rclone moveto opendrive:Pruebas/otro.txt opendrive:Pruebas/renombrado.txt 2>&1 | sed 's/^/   /'
rclone ls opendrive:Pruebas 2>&1 | sed 's/^/   /'

say "6. Borrar"
rclone purge opendrive:Pruebas 2>&1 | sed 's/^/   /'
rclone lsd opendrive: 2>&1 | sed 's/^/   /'

printf '\n\033[1;32mTodas las comprobaciones han pasado.\033[0m\n'
