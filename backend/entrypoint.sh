#!/bin/sh
set -e

# Si el contenedor arranca como root, nos aseguramos de que el directorio
# /data (que puede haber sido montado por Docker como root) pertenezca al usuario opendrive.
if [ "$(id -u)" = "0" ]; then
    mkdir -p /data/blobs /data/staging /data/thumbs
    chown -R opendrive:opendrive /data 2>/dev/null || true
    exec gosu opendrive /usr/local/bin/opendrive "$@"
fi

exec /usr/local/bin/opendrive "$@"
