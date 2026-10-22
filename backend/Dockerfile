# syntax=docker/dockerfile:1

# ---------------------------------------------------------------------------
# Etapa 1: compilación
# ---------------------------------------------------------------------------
FROM rust:1.99.1-bookworm AS builder

WORKDIR /app

# Las dependencias se compilan en una capa aparte para que se reutilicen
# mientras no cambien los Cargo.toml.
COPY Cargo.toml Cargo.lock ./
COPY crates/opendrive-core/Cargo.toml crates/opendrive-core/
COPY crates/opendrive-api/Cargo.toml crates/opendrive-api/

RUN mkdir -p crates/opendrive-core/src crates/opendrive-api/src \
    && echo "" > crates/opendrive-core/src/lib.rs \
    && echo "fn main() {}" > crates/opendrive-api/src/main.rs \
    && cargo build --release --locked 2>/dev/null || true \
    && rm -rf crates/opendrive-core/src crates/opendrive-api/src

# El código fuente real y las migraciones.
COPY crates ./crates

# `COPY` conserva las fechas de modificación originales del contexto de
# compilación, que pueden ser anteriores a los artefactos que dejó la
# compilación de prueba. Sin este `touch`, Cargo daría el binario de prueba
# por válido y no recompilaría el código real.
RUN find crates -type f -exec touch {} +

# `SQLX_OFFLINE=true` hace que sqlx use la caché `.sqlx/` versionada en lugar
# de intentar conectar con una base de datos: el build no necesita Postgres.
ENV SQLX_OFFLINE=true
ENV CARGO_TERM_COLOR=never

RUN cargo build --release --locked --bin opendrive

# Borra los artefactos de compilación que no hacen falta en runtime.
RUN strip target/release/opendrive || true

# ---------------------------------------------------------------------------
# Etapa 2: runtime
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# ca-certificates: necesario para las conexiones TLS salientes.
# tzdata: para que las fechas se muestren en la zona horaria del usuario.
# gosu: permite ajustar permisos de volumenes montados antes de pasar al usuario no-root.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tzdata gosu \
    && rm -rf /var/lib/apt/lists/*

# El servidor no corre como root.
RUN groupadd --system --gid 1000 opendrive \
    && useradd --system --uid 1000 --gid opendrive --home-dir /app opendrive

WORKDIR /app

COPY --from=builder /app/target/release/opendrive /usr/local/bin/opendrive
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

# Directorio de datos por defecto.
RUN mkdir -p /data && chown -R opendrive:opendrive /data
VOLUME ["/data"]

ENV OPENDRIVE_BIND=0.0.0.0:8080 \
    OPENDRIVE_DATA_DIR=/data \
    RUST_LOG=info

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
    CMD ["/usr/local/bin/opendrive", "--health-check"]

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
