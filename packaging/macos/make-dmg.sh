#!/usr/bin/env bash
#   ./packaging/macos/make-dmg.sh              # compila (arm64 + x86_64) e empacota
#   ./packaging/macos/make-dmg.sh --no-build   # só empacota o que já está em target/
set -euo pipefail

REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
HERE="$REPO/packaging/macos"
DIST="$HERE/dist"
TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)

cd "$REPO"

VERSION=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version *= *"\([^"]*\)".*/\1/p' Cargo.toml | head -1)
REVISION=$(sed -n '/^\[package.metadata.deb\]/,/^\[/s/^revision *= *"\([^"]*\)".*/\1/p' crates/mx-app/Cargo.toml | head -1)
REVISION="${REVISION:-1}"
FULL="${VERSION}-${REVISION}"
export MAESTRIA_VERSION="${MAESTRIA_VERSION:-$VERSION}"
TARGET_DIR=$(cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')

if [ "${1:-}" != "--no-build" ]; then
  command -v cargo-bundle >/dev/null || { echo "cargo-bundle não instalado -- cargo install cargo-bundle" >&2; exit 1; }
  for t in "${TARGETS[@]}"; do
    rustup target add "$t" >/dev/null
    echo "==> cargo build --release --target $t"
    cargo build -p mx-app --release --locked --target "$t"
  done
  echo "==> cargo bundle"
  (cd crates/mx-app && cargo bundle --release --target "${TARGETS[0]}" --format osx)
fi

BUNDLE_DIR="$TARGET_DIR/${TARGETS[0]}/release/bundle/osx"
APP_SRC=$(ls -1d "$BUNDLE_DIR"/*.app 2>/dev/null | head -1 || true)
[ -n "$APP_SRC" ] && [ -d "$APP_SRC" ] || { echo "não achei o .app em $BUNDLE_DIR -- rode sem --no-build." >&2; exit 1; }

STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT
APP="$STAGE/maestria-rs.app"
ditto "$APP_SRC" "$APP"

EXE="$APP/Contents/MacOS/maestria"
lipo -create \
  "$TARGET_DIR/${TARGETS[0]}/release/maestria" \
  "$TARGET_DIR/${TARGETS[1]}/release/maestria" \
  -output "$EXE"
chmod 755 "$EXE"

ARCHS=$(lipo -info "$EXE" | sed 's/.*: //')
for want in x86_64 arm64; do
  case " $ARCHS " in
    *" $want "*) ;;
    *) echo "o binário não tem $want (só: $ARCHS)" >&2; exit 1 ;;
  esac
done

PLIST="$APP/Contents/Info.plist"
plist_set() {
  /usr/libexec/PlistBuddy -c "Set :$1 $3" "$PLIST" 2>/dev/null \
    || /usr/libexec/PlistBuddy -c "Add :$1 $2 $3" "$PLIST"
}
plist_set CFBundleName string MaestrIA-rs
plist_set CFBundleDisplayName string "MaestrIA (Rust)"
plist_set CFBundleIdentifier string br.com.marrow.maestria-rs
plist_set CFBundleExecutable string maestria
plist_set CFBundleShortVersionString string "$VERSION"
plist_set CFBundleVersion string "$REVISION"
plist_set LSMinimumSystemVersion string 12.0
plist_set NSPrincipalClass string NSApplication
plist_set NSHighResolutionCapable bool true
plist_set NSHumanReadableCopyright string "Copyright © 2026 br.com.marrow. All rights reserved."

echo "==> maestria-rs ${FULL} (${ARCHS})"

codesign --force --deep --sign - --entitlements "$HERE/entitlements.plist" "$APP"
codesign --verify --deep "$APP" \
  || { echo "a assinatura do .app não confere" >&2; exit 1; }

ln -s /Applications "$STAGE/Applications"

mkdir -p "$DIST"
OUT="$DIST/maestria-rs_${FULL}_macos.dmg"
rm -f "$OUT"

echo "==> hdiutil"
hdiutil create -quiet -volname "maestria-rs ${VERSION}" -srcfolder "$STAGE" \
  -ov -format UDZO "$OUT"

echo
echo "==> $OUT ($(du -h "$OUT" | cut -f1))"
