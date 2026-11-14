#!/usr/bin/env bash
# Smoke test for scripts/package.sh with a synthetic release set.
set -Eeuo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
WORK=$(mktemp -d "${TMPDIR:-/tmp}/fungos-package-test.XXXXXX")
trap 'rm -rf "$WORK"' EXIT

fail() { printf 'FAIL: %s\n' "$1" >&2; exit 1; }
ok() { printf 'ok - %s\n' "$1"; }
sha256_file() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

mkdir -p "$WORK/artifacts" "$WORK/src"
printf '#!/bin/sh\nexit 0\n' >"$WORK/src/mycelium"
chmod 0755 "$WORK/src/mycelium"
printf '#!/bin/sh\nexit 0\n' >"$WORK/src/myceliumd"
chmod 0755 "$WORK/src/myceliumd"
printf 'test license\n' >"$WORK/src/LICENSE"
for target in x86_64-unknown-linux-musl aarch64-apple-darwin; do
    tar -C "$WORK/src" -czf "$WORK/artifacts/fake-0.1.0-$target.tar.gz" .
done

cat >"$WORK/release-set.json" <<EOF
{
  "release": "0.1.0",
  "channel": "test",
  "package": {
    "name": "mycelium",
    "vendor": "Test",
    "homepage": "https://example.test",
    "maintainer": "Test <test@example.test>",
    "license": "Apache-2.0 OR MIT",
    "description": "Package test payload"
  },
  "components": [
    {
      "name": "mycelium",
      "version": "0.1.0",
      "binaries": ["mycelium", "myceliumd"],
      "licenses": ["LICENSE"],
      "targets": {
        "x86_64-unknown-linux-musl": {
          "archive": "fake-0.1.0-x86_64-unknown-linux-musl.tar.gz",
          "url": "file:///nonexistent/x86_64.tar.gz",
          "sha256": "$(sha256_file "$WORK/artifacts/fake-0.1.0-x86_64-unknown-linux-musl.tar.gz")"
        },
        "aarch64-apple-darwin": {
          "archive": "fake-0.1.0-aarch64-apple-darwin.tar.gz",
          "url": "file:///nonexistent/aarch64.tar.gz",
          "sha256": "$(sha256_file "$WORK/artifacts/fake-0.1.0-aarch64-apple-darwin.tar.gz")"
        }
      }
    }
  ]
}
EOF

"$ROOT/scripts/package.sh" --release-set "$WORK/release-set.json" --dry-run \
    --formats deb,pkg --target x86_64-unknown-linux-musl >/dev/null
ok "dry run renders a plan"

if command -v "${NFPM:-nfpm}" >/dev/null 2>&1 || [ -x "$HOME/go/bin/nfpm" ]; then
    export NFPM=${NFPM:-$HOME/go/bin/nfpm}
    "$ROOT/scripts/package.sh" --release-set "$WORK/release-set.json" \
        --formats deb --target x86_64-unknown-linux-musl \
        --artifacts-dir "$WORK/artifacts" --out "$WORK/deb" >/dev/null
    [ -f "$WORK/deb/mycelium_0.1.0_amd64.deb" ] || fail "deb was not written"
    ar t "$WORK/deb/mycelium_0.1.0_amd64.deb" | grep -qx data.tar.gz \
        || fail "deb has no data archive"
    ok "builds a deb with nfpm"
else
    printf 'skip - nfpm is not installed\n'
fi

if [ "$(uname -s)" = Darwin ]; then
    mkdir -p "$WORK/fungOS.app/Contents/MacOS" "$WORK/fungOS.app/Contents/Resources"
    printf '#!/bin/sh\nexit 0\n' >"$WORK/fungOS.app/Contents/MacOS/canvas-winit"
    chmod 0755 "$WORK/fungOS.app/Contents/MacOS/canvas-winit"
    : >"$WORK/fungOS.app/Contents/Resources/launcher.wasm"
    "$ROOT/scripts/package.sh" --release-set "$WORK/release-set.json" \
        --formats pkg,dmg --target aarch64-apple-darwin --app "$WORK/fungOS.app" \
        --artifacts-dir "$WORK/artifacts" --out "$WORK/mac" >/dev/null
    pkg="$WORK/mac/mycelium-0.1.0-arm64.pkg"
    [ -f "$pkg" ] || fail "pkg was not written"
    pkgutil --payload-files "$pkg" | grep -q './usr/local/bin/mycelium' \
        || fail "pkg payload is missing mycelium"
    dmg="$WORK/mac/mycelium-0.1.0-arm64.dmg"
    [ -f "$dmg" ] || fail "dmg was not written"

    # The dmg is the launcher: app with the daemon inside, Applications link,
    # and the package for shell access.
    mountpoint="$WORK/dmg-mount"
    mkdir -p "$mountpoint"
    hdiutil attach -quiet -nobrowse -readonly -mountpoint "$mountpoint" "$dmg"
    trap 'hdiutil detach -quiet "$mountpoint" 2>/dev/null || true' EXIT
    [ -x "$mountpoint/fungOS.app/Contents/Resources/bin/myceliumd" ] \
        || fail "dmg app is missing myceliumd"
    [ -x "$mountpoint/fungOS.app/Contents/MacOS/canvas-winit" ] \
        || fail "dmg app is missing canvas-winit"
    [ -L "$mountpoint/Applications" ] || fail "dmg has no Applications link"
    [ -f "$mountpoint/$(basename "$pkg")" ] || fail "dmg is missing the pkg"
    hdiutil detach -quiet "$mountpoint"
    trap - EXIT
    ok "builds a macOS pkg and a launcher dmg"
else
    printf 'skip - pkg/dmg needs macOS\n'
fi

printf 'all packaging tests passed\n'
