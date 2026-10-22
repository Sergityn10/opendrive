# syntax=docker/dockerfile:1

# ---------------------------------------------------------------------------
# Etapa 1: compilación de la SPA
# ---------------------------------------------------------------------------
FROM node:24.22.1-bookworm-slim AS builder

WORKDIR /app

# Las dependencias se instalan en una capa aparte para reutilizar la caché.
COPY package.json package-lock.json* ./
RUN npm ci --no-audit --no-fund || npm install --no-audit --no-fund

COPY . .

# `tsc -b` comprueba los tipos antes de generar el bundle.
RUN npm run build

# ---------------------------------------------------------------------------
# Etapa 2: nginx sirve los estáticos y enruta peticiones hacia el backend
# ---------------------------------------------------------------------------
FROM nginx:1.27-alpine AS runtime

# Variables de entorno para configurar el host y puerto del backend.
# Nginx sustituye automáticamente ${BACKEND_HOST} y ${BACKEND_PORT} en la plantilla.
ENV BACKEND_HOST=backend \
    BACKEND_PORT=8080 \
    NGINX_ENVSUBST_FILTER="BACKEND_HOST|BACKEND_PORT"

RUN mkdir -p /etc/nginx/templates
COPY default.conf.template /etc/nginx/templates/default.conf.template
COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/dist /usr/share/nginx/html

EXPOSE 80

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD wget -qO- http://127.0.0.1/ >/dev/null || exit 1

CMD ["nginx", "-g", "daemon off;"]
