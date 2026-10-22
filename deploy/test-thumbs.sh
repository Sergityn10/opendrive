#!/usr/bin/env bash
# Prueba de las miniaturas y de la búsqueda por contenido.
#
#   bash deploy/test-thumbs.sh
set -euo pipefail

BASE="${BASE:-https://localhost}"
COOKIES=$(mktemp)
trap 'rm -f "$COOKIES"' EXIT

CURL=(curl -sS -k)
say() { printf '\n\033[1m%s\033[0m\n' "$*"; }
b64() { printf '%s' "$1" | base64 | tr -d '\n'; }

"${CURL[@]}" -c "$COOKIES" -X POST "$BASE/api/auth/login" \
  -H 'Content-Type: application/json' \
  -d '{"identifier":"admin@opendrive.local","password":"admin12345"}' > /dev/null
say "1. Sesion iniciada"

# --- imagen PNG valida ----------------------------------------------------
# Se genera con Python para tener un PNG con los CRC correctos.
python3 - <<'PY'
import struct, zlib, sys

def chunk(tag, data):
    return (struct.pack(">I", len(data)) + tag + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF))

width = height = 64
raw = b""
for y in range(height):
    raw += b"\x00"                      # filtro "None"
    for x in range(width):
        raw += bytes(((x * 4) % 256, (y * 4) % 256, 160))

png = b"\x89PNG\r\n\x1a\n"
png += chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
png += chunk(b"IDAT", zlib.compress(raw, 9))
png += chunk(b"IEND", b"")

with open("/tmp/pixel.png", "wb") as handle:
    handle.write(png)
PY

FOLDER=$("${CURL[@]}" -b "$COOKIES" -X POST "$BASE/api/nodes" \
  -H 'Content-Type: application/json' \
  -d "{\"name\":\"Miniaturas $(date +%s)\"}" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
say "2. Carpeta creada: $FOLDER"

# --- subir la imagen por tus ----------------------------------------------
SIZE=$(wc -c < /tmp/pixel.png)
META="filename $(b64 'pixel.png'),parent_id $(b64 "$FOLDER"),filetype $(b64 'image/png')"
LOCATION=$("${CURL[@]}" -D - -o /dev/null -b "$COOKIES" -X POST "$BASE/api/uploads" \
  -H 'Tus-Resumable: 1.0.0' \
  -H "Upload-Length: $SIZE" \
  -H "Upload-Metadata: $META" | tr -d '\r' | sed -n 's/^[Ll]ocation: *//p')

"${CURL[@]}" -o /dev/null -b "$COOKIES" -X PATCH "$LOCATION" \
  -H 'Tus-Resumable: 1.0.0' \
  -H 'Content-Type: application/offset+octet-stream' \
  -H 'Upload-Offset: 0' \
  --data-binary @/tmp/pixel.png
sleep 1
say "3. Imagen subida"

NODE=$("${CURL[@]}" -b "$COOKIES" "$BASE/api/nodes?parent_id=$FOLDER" \
  | sed -n 's/.*"id":"\([^"]*\)","parent_id[^}]*"name":"pixel.png".*/\1/p')
[ -n "$NODE" ] && say "   nodo: $NODE" || { echo "   FALLO: la imagen no aparece"; exit 1; }

# --- miniatura ------------------------------------------------------------
say "4. Miniatura"
INFO=$("${CURL[@]}" -o /dev/null -w '%{content_type} %{size_download} %{http_code}' \
  -b "$COOKIES" "$BASE/api/nodes/$NODE/thumbnail?size=256")
echo "   tipo/tamano/estado: $INFO"
if echo "$INFO" | grep -qE '^image/(jpeg|png) [1-9][0-9]* 200$'; then
  echo "   OK: la miniatura se ha generado"
else
  echo "   FALLO"
  exit 1
fi

CACHED=$("${CURL[@]}" -o /dev/null -w '%{http_code} %{size_download}' -b "$COOKIES" "$BASE/api/nodes/$NODE/thumbnail?size=256")
echo "   segunda peticion (debe servirse de la cache): $CACHED"

# --- busqueda por nombre --------------------------------------------------
say "5. Busqueda por nombre"
"${CURL[@]}" -b "$COOKIES" "$BASE/api/search?q=pixel" | head -c 200; echo

# --- busqueda por contenido -----------------------------------------------
say "6. Subir un texto y buscar por su contenido"
# Solo ASCII: la longitud en bytes y en caracteres coincide, que es lo que
# espera el protocolo tus.
TEXT="El murcielago se alimenta de insectos nocturnos"
TNAME="nota.txt"
TSIZE=$(printf '%s' "$TEXT" | wc -c)
LOCATION2=$("${CURL[@]}" -D - -o /dev/null -b "$COOKIES" -X POST "$BASE/api/uploads" \
  -H 'Tus-Resumable: 1.0.0' \
  -H "Upload-Length: $TSIZE" \
  -H "Upload-Metadata: filename $(b64 "$TNAME"),parent_id $(b64 "$FOLDER"),filetype $(b64 'text/plain')" \
  | tr -d '\r' | sed -n 's/^[Ll]ocation: *//p')
"${CURL[@]}" -o /dev/null -b "$COOKIES" -X PATCH "$LOCATION2" \
  -H 'Tus-Resumable: 1.0.0' \
  -H 'Content-Type: application/offset+octet-stream' \
  -H 'Upload-Offset: 0' \
  --data-binary "$TEXT"
sleep 1

RESULT=$("${CURL[@]}" -b "$COOKIES" "$BASE/api/search?q=insectos&content=true")
echo "$RESULT" | head -c 300; echo
if echo "$RESULT" | grep -q "$TNAME"; then
  echo "   OK: la busqueda por contenido encuentra el archivo"
else
  echo "   FALLO: la busqueda por contenido no encuentra el archivo"
  exit 1
fi

printf '\n\033[1;32mTodas las comprobaciones han pasado.\033[0m\n'
