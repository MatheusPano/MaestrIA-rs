#!/usr/bin/env bash
set -euo pipefail

REPO="${REPO:-/src}"
DIST="${DIST:-$REPO/packaging/linux/dist}"
cd "$REPO"

MAESTRIA_VERSION="${MAESTRIA_VERSION:-$(sed -n '/^\[workspace.package\]/,/^\[/s/^version *= *"\([^"]*\)".*/\1/p' Cargo.toml | head -1)}"
export MAESTRIA_VERSION
echo "==> MAESTRIA_VERSION=$MAESTRIA_VERSION"

mkdir -p "$DIST"
cargo deb -p mx-app --locked --strip -o "$DIST/" "$@"

OUT=$(ls -1t "$DIST"/maestria*.deb | head -1)
echo
echo "==> $OUT"
dpkg-deb --info "$OUT" | sed -n '2,14p'
