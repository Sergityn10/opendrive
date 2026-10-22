#!/usr/bin/env bash
# Compila y comprueba el backend de OpenDrive dentro de un contenedor Linux,
# igual que en el Dockerfile de producción.
#
#   ./deploy/check.sh              -> cargo check
#   ./deploy/check.sh clippy       -> cargo clippy
#   ./deploy/check.sh test         -> cargo test
#   ./deploy/check.sh fmt          -> cargo fmt --check
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODE="${1:-check}"
IMAGE="${RUST_IMAGE:-rust:1.99.1-bookworm}"

# Caché de artefactos y registry entre ejecuciones: la primera compilación
# tarda varios minutos, las siguientes son incrementales.
docker run --rm \
  -v "${ROOT}/backend:/app" \
  -v opendrive-cargo-registry:/usr/local/cargo/registry \
  -v opendrive-cargo-git:/usr/local/cargo/git \
  -v opendrive-target:/app/target \
  -w /app \
  -e SQLX_OFFLINE=true \
  -e CARGO_TERM_COLOR=never \
  "${IMAGE}" \
  bash -c "set -euo pipefail
    case '${MODE}' in
      check)  cargo check --workspace --all-targets ;;
      clippy) cargo clippy --workspace --all-targets --all-features -- -D warnings ;;
      test)   cargo test --workspace ;;
      fmt)    cargo fmt --all --check ;;
      build)  cargo build --workspace --release ;;
      *) echo \"modo desconocido: ${MODE}\"; exit 2 ;;
    esac"
