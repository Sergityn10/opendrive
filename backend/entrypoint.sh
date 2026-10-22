#!/bin/sh
set -e

# Si el contenedor arranca como root, nos aseguramos de que únicamente los
# directorios de almacenamiento de archivos pertenezcan al usuario opendrive.
# NUNCA tocar /data/postgres ya que pertenece al usuario postgres (UID 999).
if [ "$(id -u)" = "0" ]; then
    mkdir -p /data/blobs /data/staging /data/thumbs
    chown -R opendrive:opendrive /data/blobs /data/staging /data/thumbs 2>/dev/null || true
    exec gosu opendrive /usr/local/bin/opendrive "$@"
fi

exec /usr/local/bin/opendrive "$@"
