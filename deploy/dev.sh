#!/usr/bin/env bash
# Ejecuta un comando cargo dentro del contenedor de desarrollo.
#
#   ./deploy/dev.sh check
#   ./deploy/dev.sh clippy
#   ./deploy/dev.sh test
#   ./deploy/dev.sh fmt
#   ./deploy/dev.sh build
#   ./deploy/dev.sh shell        (consola interactiva)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE="${RUST_IMAGE:-rust:1.99.1-bookworm}"
MODE="${1:-check}"

case "$MODE" in
  check)  CMD="cargo check --workspace --all-targets" ;;
  clippy) CMD="cargo clippy --workspace --all-targets --all-features -- -D warnings" ;;
  test)   CMD="cargo test --workspace" ;;
  fmt)    CMD="cargo fmt --all" ;;
  fmtcheck) CMD="cargo fmt --all --check" ;;
  build)  CMD="cargo build --workspace --release" ;;
  shell)  CMD="bash" ;;
  *)      CMD="$*" ;;
esac

docker run --rm -it \
  -v "${ROOT}/backend:/app" \
  -v opendrive-cargo-registry:/usr/local/cargo/registry \
  -v opendrive-cargo-git:/usr/local/cargo/git \
  -v opendrive-target:/app/target \
  -w /app \
  -e SQLX_OFFLINE=true \
  -e CARGO_TERM_COLOR=never \
  "$IMAGE" \
  bash -c "set -euo pipefail; ${CMD}"
