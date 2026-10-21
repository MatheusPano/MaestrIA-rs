#!/usr/bin/env bash
set -euo pipefail

REPO="${REPO:-/src}"
DIST="${DIST:-$REPO/packaging/linux/dist}"
cd "$REPO"

mkdir -p "$DIST"
cargo deb -p mx-app --locked --strip -o "$DIST/" "$@"

OUT=$(ls -1t "$DIST"/maestria_*.deb | head -1)
echo
echo "==> $OUT"
dpkg-deb --info "$OUT" | sed -n '2,14p'
