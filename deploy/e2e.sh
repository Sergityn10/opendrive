#!/usr/bin/env bash
# Prueba de extremo a extremo contra la pila completa (Caddy -> backend).
#
#   bash deploy/e2e.sh [https://localhost]
set -euo pipefail

BASE="${1:-https://localhost}"
INSECURE="${INSECURE:-1}"
CURL=(curl -sS)
if [ "$INSECURE" = "1" ]; then CURL+=(-k); fi

COOKIES=$(mktemp)
BODY=$(mktemp)
trap 'rm -f "$COOKIES" "$BODY"' EXIT

say() { printf '\n\033[1m%s\033[0m\n' "$*"; }

# --- 1. la SPA se sirve --------------------------------------------------
say "1. La SPA se sirve en $BASE"
"${CURL[@]}" -o "$BODY" -w '   status=%{http_code} tipo=%{content_type}\n' "$BASE/"
grep -q '<div id="root">' "$BODY" && echo "   OK: index.html contiene el punto de montaje"

# --- 2. la API responde --------------------------------------------------
say "2. La API responde"
"${CURL[@]}" "$BASE/api/health"; echo

# --- 3. inicio de sesion -------------------------------------------------
say "3. Inicio de sesion"
"${CURL[@]}" -c "$COOKIES" -X POST "$BASE/api/auth/login" \
  -H 'Content-Type: application/json' \
  -d '{"identifier":"admin@opendrive.local","password":"admin12345"}' > "$BODY"
grep -q '"authenticated":true' "$BODY" && echo "   OK: sesion iniciada" || { echo "   FALLO"; cat "$BODY"; exit 1; }

# --- 4. perfil ------------------------------------------------------------
say "4. Perfil del usuario"
"${CURL[@]}" -b "$COOKIES" "$BASE/api/auth/me" | head -c 200; echo

# --- 5. carpeta raiz ------------------------------------------------------
say "5. Carpeta raiz (Mi Drive)"
ROOT=$("${CURL[@]}" -b "$COOKIES" "$BASE/api/nodes" | sed -n 's/.*"next_cursor":"\([^"]*\)".*/\1/p')
echo "   elementos en la raiz: ${ROOT:-ninguno}"

# --- 6. crear una carpeta ------------------------------------------------
say "6. Crear una carpeta"
# El nombre lleva marca de tiempo para que el script se pueda repetir.
FOLDER_NAME="Pruebas E2E $(date +%s)"
FOLDER=$("${CURL[@]}" -b "$COOKIES" -X POST "$BASE/api/nodes" \
  -H 'Content-Type: application/json' \
  -d "{\"name\":\"$FOLDER_NAME\"}" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
[ -n "$FOLDER" ] && echo "   OK: carpeta creada con id $FOLDER" || { echo "   FALLO"; exit 1; }

# --- 7. subir un archivo con tus -----------------------------------------
say "7. Subir un archivo (protocolo tus)"
CONTENT="Contenido de prueba generado por el script de extremo a extremo."

b64() { printf '%s' "$1" | base64 | tr -d '\n'; }
META="filename $(b64 'e2e.txt'),parent_id $(b64 "$FOLDER")"

LOCATION=$("${CURL[@]}" -D - -o /dev/null -b "$COOKIES" -X POST "$BASE/api/uploads" \
  -H 'Tus-Resumable: 1.0.0' \
  -H "Upload-Length: ${#CONTENT}" \
  -H "Upload-Metadata: $META" | tr -d '\r' | sed -n 's/^[Ll]ocation: *//p')
echo "   Location: $LOCATION"
[ -n "$LOCATION" ] || { echo "   FALLO: no hay cabecera Location"; exit 1; }

# La cabecera Location puede ser absoluta o relativa: se normaliza.
case "$LOCATION" in
  http*) UPLOAD_URL="$LOCATION" ;;
  *)     UPLOAD_URL="$BASE$LOCATION" ;;
esac

HALF=$(( ${#CONTENT} / 2 ))
CHUNK1="${CONTENT:0:$HALF}"
CHUNK2="${CONTENT:$HALF}"

OFFSET=$("${CURL[@]}" -D - -o /dev/null -b "$COOKIES" -X PATCH "$UPLOAD_URL" \
  -H 'Tus-Resumable: 1.0.0' \
  -H 'Content-Type: application/offset+octet-stream' \
  -H 'Upload-Offset: 0' \
  --data-binary "$CHUNK1" | tr -d '\r' | sed -n 's/^[Uu]pload-[Oo]ffset: *//p')
echo "   primer fragmento -> offset $OFFSET"

OFFSET=$("${CURL[@]}" -D - -o /dev/null -b "$COOKIES" -I "$UPLOAD_URL" \
  -H 'Tus-Resumable: 1.0.0' | tr -d '\r' | sed -n 's/^[Uu]pload-[Oo]ffset: *//p')
echo "   HEAD -> offset $OFFSET"

STATUS=$("${CURL[@]}" -o /dev/null -w '%{http_code}' -b "$COOKIES" -X PATCH "$UPLOAD_URL" \
  -H 'Tus-Resumable: 1.0.0' \
  -H 'Content-Type: application/offset+octet-stream' \
  -H "Upload-Offset: $OFFSET" \
  --data-binary "$CHUNK2")
echo "   segundo fragmento -> estado $STATUS"

sleep 1

# --- 8. el archivo aparece en el arbol ------------------------------------
say "8. El archivo aparece en el arbol"
NODE=$("${CURL[@]}" -b "$COOKIES" "$BASE/api/nodes?parent_id=$FOLDER" \
  | sed -n 's/.*"id":"\([^"]*\)","parent_id[^}]*"name":"e2e.txt".*/\1/p')
[ -n "$NODE" ] && echo "   OK: nodo $NODE" || { echo "   FALLO: el archivo no aparece"; exit 1; }

# --- 9. descarga completa -------------------------------------------------
say "9. Descargar y comparar"
DOWNLOADED=$("${CURL[@]}" -b "$COOKIES" "$BASE/api/nodes/$NODE/download")
if [ "$DOWNLOADED" = "$CONTENT" ]; then
  echo "   OK: el contenido coincide"
else
  echo "   FALLO"
  echo "   esperado: $CONTENT"
  echo "   obtenido: $DOWNLOADED"
  exit 1
fi

# --- 10. rango parcial ----------------------------------------------------
say "10. Descarga parcial (Range: bytes=0-9)"
RANGE=$("${CURL[@]}" -D - -o /dev/null -b "$COOKIES" -H 'Range: bytes=0-9' "$BASE/api/nodes/$NODE/download" | tr -d '\r')
echo "$RANGE" | sed -n 's/^[Cc]ontent-[Rr]ange: */   Content-Range: /p'
echo "$RANGE" | grep -q '206' && echo "   OK: respuesta parcial 206" || { echo "   FALLO"; exit 1; }

# --- 11. la papelera ------------------------------------------------------
say "11. Mover a la papelera y restaurar"
"${CURL[@]}" -b "$COOKIES" -X DELETE "$BASE/api/nodes/$FOLDER" > /dev/null
TRASHED=$("${CURL[@]}" -b "$COOKIES" "$BASE/api/trash" | grep -c "$FOLDER_NAME" || true)
echo "   elementos en la papelera: $TRASHED"
"${CURL[@]}" -b "$COOKIES" -X POST "$BASE/api/nodes/$FOLDER/restore" > /dev/null
# La carpeta se crea en la raíz del usuario, así que se busca sin `parent_id`.
RESTORED=$("${CURL[@]}" -b "$COOKIES" "$BASE/api/nodes" | grep -c "$FOLDER_NAME" || true)
echo "   carpeta restaurada: $([ "$RESTORED" -gt 0 ] && echo si || echo no)"
[ "$RESTORED" -gt 0 ] || exit 1

# --- 12. buscar -----------------------------------------------------------
say "12. Buscar"
"${CURL[@]}" -b "$COOKIES" "$BASE/api/search?q=e2e" | head -c 200; echo

printf '\n\033[1;32mTodas las comprobaciones han pasado.\033[0m\n'
