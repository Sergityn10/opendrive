#!/usr/bin/env bash
# Prueba manual del endpoint tus (subida resumible) contra el servidor de
# pruebas. Uso: bash deploy/test-tus.sh
set -euo pipefail

BASE="${BASE:-http://localhost:58080}"
JAR="$(mktemp)"
trap 'rm -f "$JAR"' EXIT

# --- login ---------------------------------------------------------------
curl -sS -c "$JAR" -X POST "$BASE/api/auth/login" \
  -H 'Content-Type: application/json' \
  -d '{"identifier":"admin@opendrive.local","password":"admin12345"}' > /dev/null
echo "1. login ok"

# --- carpeta raiz --------------------------------------------------------
ROOT=$(curl -sS -b "$JAR" "$BASE/api/nodes" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
echo "2. carpeta raiz: $ROOT"

# --- creacion del upload (POST) -----------------------------------------
CONTENT="Hola OpenDrive. Esto es una prueba de subida resumible con contenido de ejemplo."

META="filename $(printf '%s' 'prueba.txt' | base64 -w0),parent_id $(printf '%s' "$ROOT" | base64 -w0),filetype $(printf '%s' 'text/plain' | base64 -w0)"

HEADERS=$(curl -sS -D - -o /dev/null -b "$JAR" -X POST "$BASE/api/uploads" \
  -H 'Tus-Resumable: 1.0.0' \
  -H "Upload-Length: ${#CONTENT}" \
  -H "Upload-Metadata: $META")

LOCATION=$(printf '%s' "$HEADERS" | grep -i '^location:' | tr -d '\r' | awk '{print $2}')
echo "3. upload creado, Location: $LOCATION"

if [ -z "$LOCATION" ]; then
  echo "FALLO: no se ha devuelto la cabecera Location"
  printf '%s\n' "$HEADERS"
  exit 1
fi

# --- PATCH con la primera mitad ------------------------------------------
HALF=$(( ${#CONTENT} / 2 ))
CHUNK1="${CONTENT:0:$HALF}"
CHUNK2="${CONTENT:$HALF}"

OFFSET=$(curl -sS -D - -o /dev/null -b "$JAR" -X PATCH "$BASE$LOCATION" \
  -H 'Tus-Resumable: 1.0.0' \
  -H 'Content-Type: application/offset+octet-stream' \
  -H "Upload-Offset: 0" \
  --data-binary "$CHUNK1" | grep -i '^upload-offset:' | tr -d '\r' | awk '{print $2}')
echo "4. primer chunk enviado, offset: $OFFSET"

# --- HEAD: consulta del offset actual ------------------------------------
OFFSET=$(curl -sS -D - -o /dev/null -b "$JAR" -I "$BASE$LOCATION" \
  -H 'Tus-Resumable: 1.0.0' | grep -i '^upload-offset:' | tr -d '\r' | awk '{print $2}')
echo "5. HEAD devuelve offset: $OFFSET"

# --- PATCH con el resto --------------------------------------------------
STATUS=$(curl -sS -o /dev/null -w '%{http_code}' -b "$JAR" -X PATCH "$BASE$LOCATION" \
  -H 'Tus-Resumable: 1.0.0' \
  -H 'Content-Type: application/offset+octet-stream' \
  -H "Upload-Offset: $OFFSET" \
  --data-binary "$CHUNK2")
echo "6. segundo chunk enviado, estado: $STATUS"

# --- el archivo debe aparecer en el arbol --------------------------------
sleep 1
LISTING=$(curl -sS -b "$JAR" "$BASE/api/nodes?parent_id=$ROOT")
echo "7. contenido de la raiz:"
echo "$LISTING" | python3 -c "import json,sys; d=json.load(sys.stdin); [print('   -', i['name'], i['size_bytes'], 'bytes') for i in d['items']]" 2>/dev/null || echo "   $LISTING"

# --- descarga y comprobacion del contenido ------------------------------
NODE=$(echo "$LISTING" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
DOWNLOADED=$(curl -sS -b "$JAR" "$BASE/api/nodes/$NODE/download")
if [ "$DOWNLOADED" = "$CONTENT" ]; then
  echo "8. descarga verificada: el contenido coincide"
else
  echo "8. FALLO en la descarga"
  echo "   esperado: $CONTENT"
  echo "   obtenido: $DOWNLOADED"
  exit 1
fi

echo
echo "Todas las comprobaciones han pasado."
