#!/usr/bin/env bash
#   ./packaging/linux/build-deb.sh               # ubuntu:22.04 no Docker (piso de glibc 2.35)
#   NATIVE=1 ./packaging/linux/build-deb.sh      # nesta máquina, com a glibc dela
#   ./packaging/linux/build-deb.sh --no-build    # empacota o binário já compilado
set -euo pipefail

REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
HERE="$REPO/packaging/linux"
RUST_VERSION="${RUST_VERSION:-1.93.0}"

if [ "${NATIVE:-0}" = 1 ]; then
  command -v cargo-deb >/dev/null || { echo "cargo-deb não instalado -- cargo install cargo-deb" >&2; exit 1; }
  REPO="$REPO" DIST="$HERE/dist" exec "$HERE/make-deb.sh" "$@"
fi

if ! docker info >/dev/null 2>&1; then
  echo "o daemon do Docker não está de pé -- suba o Docker, ou rode com NATIVE=1." >&2
  exit 1
fi

IMAGE="maestria-rs-deb:$RUST_VERSION"
CACHE="$REPO/target-deb"
mkdir -p "$HERE/dist" "$CACHE/cargo-home"

echo "==> imagem (ubuntu:22.04, rust $RUST_VERSION)"
docker build --build-arg "RUST_VERSION=$RUST_VERSION" -t "$IMAGE" "$HERE"

JOBS=()
[ -n "${CARGO_BUILD_JOBS:-}" ] && JOBS=(-e "CARGO_BUILD_JOBS=$CARGO_BUILD_JOBS")

echo "==> build"
docker run --rm ${JOBS[@]+"${JOBS[@]}"} \
  --user "$(id -u):$(id -g)" \
  -e HOME=/tmp \
  -e CARGO_HOME=/src/target-deb/cargo-home \
  -e CARGO_TARGET_DIR=/src/target-deb/target \
  -v "$REPO":/src \
  "$IMAGE" "$@"

echo
echo "pronto: $(ls -1t "$HERE"/dist/*.deb | head -1)"
