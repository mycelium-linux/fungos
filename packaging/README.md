# Installable packages from a release set

This directory turns pinned release artifacts into the formats users expect:
`.deb`, `.rpm`, Arch packages, an AUR recipe, and macOS `.pkg`/`.dmg` files.
It consumes artifacts; it never builds a component. The component owners keep
publishing tarballs and checksums, and the release set records exactly which
bytes are packaged.

This is deliberately separate from [`packages/`](../packages/README.md): that
tool signs `.deb` files into APT generations, while this one produces the
packages and the non-Debian formats. Mycelium's own `install.sh` remains the
native macOS/Linux installer; package formats are an additional delivery path.

## Release set

[`release-set.json`](release-set.json) is the bill of materials:

```json
{
  "release": "0.1.0",
  "channel": "canary",
  "package": {
    "name": "mycelium",
    "vendor": "Future Present Labs",
    "homepage": "https://fungos.dev",
    "maintainer": "Future Present Labs <support@fungos.dev>",
    "license": "Apache-2.0 OR MIT",
    "description": "..."
  },
  "components": [
    {
      "name": "mycelium",
      "version": "0.1.0",
      "binaries": ["mycelium", "myceliumd"],
      "licenses": ["LICENSE", "LICENSE-APACHE"],
      "targets": {
        "x86_64-unknown-linux-musl": {
          "archive": "mycelium-v0.1.0-x86_64-unknown-linux-musl.tar.gz",
          "url": "https://.../mycelium-v0.1.0-x86_64-unknown-linux-musl.tar.gz",
          "sha256": "..."
        }
      }
    }
  ]
}
```

Add a component by adding its tarball metadata and install list. The packager
selects the target from `--target`, or all targets in the set. A target that is
missing from the set is an error, not a fallback build.

## Build

```sh
# Plan only
scripts/package.sh --dry-run

# Everything the host can build
scripts/package.sh --artifacts-dir /path/to/published/tarballs

# One format and target
scripts/package.sh --formats deb --target x86_64-unknown-linux-musl \
  --artifacts-dir /path/to/tarballs --out dist/packages

# Sign the macOS product package
scripts/package.sh --formats pkg,dmg --target aarch64-apple-darwin \
  --sign-identity "Developer ID Installer: ..."
```

`--artifacts-dir` is for testing a draft release or a local build; without it
the packager downloads from the URLs in the set. Every archive is checked
against its pinned SHA-256 before it is extracted.

### Formats

| Format | Tool | Notes |
|---|---|---|
| `deb` | `nfpm` | Installs binaries under `/usr/bin` and licenses under `/usr/share/doc`. |
| `rpm` | `nfpm` | Same payload; architecture names use the RPM spelling. |
| `arch` | `nfpm` | `.pkg.tar.zst`; the AUR recipe is separate. |
| `aur` | generated `PKGBUILD` | `mycelium-bin` downloads the pinned tarball and verifies its sha256. `.SRCINFO` is generated beside it. |
| `pkg` | `pkgbuild` + `productbuild` | macOS payload under `/usr/local`; signs with `productsign` when `--sign-identity` is set. |
| `dmg` | `hdiutil` | Ships `fungOS.app` as a drag-install launcher, with the release binaries embedded at `Contents/Resources/bin`, an `/Applications` link, and the `.pkg` for shell access. Needs `--app`. |

`nfpm` is the only extra tool for the Linux formats:

```sh
go install github.com/goreleaser/nfpm/v2/cmd/nfpm@latest
```

macOS package builds use the Xcode command line tools. `pkgbuild` may preserve
extended attributes as AppleDouble `._*` entries in the payload; they are
harmless metadata, and signing/notarization is the next gate.

### macOS: the launcher DMG

The `.dmg` is what macOS users get: `fungOS.app` is the launcher, and the
release binaries are embedded at `Contents/Resources/bin`, so dragging the app
to `/Applications` gives a working launcher with its daemon inside the bundle.
The `.pkg` rides along for anyone who wants `mycelium`, `myceliumd` and the
other tools on `PATH`.

The app bundle is not a release-set artifact yet, so name it explicitly:

```sh
scripts/package.sh --formats dmg --target aarch64-apple-darwin \
  --app /path/to/fungOS.app
```

Embedding the binaries invalidates whatever signature the bundle arrived with,
so the `.dmg` step drops `Contents/_CodeSignature`. Sign the assembled bundle
(Developer ID Application) and the product package (Developer ID Installer)
before distribution.

## Signing and promotion

- Linux packages are unsigned here. Build a signed APT generation with
  [`packages/`](../packages/README.md), and keep the repository key out of this
  builder.
- macOS products need a Developer ID Installer identity and notarization before
  public distribution. `--sign-identity` signs the product package; notarize
  the `.pkg`/`.dmg` separately.
- AUR expects the author to commit `PKGBUILD` and `.SRCINFO` to the AUR git
  repository. This packager only renders them.

## Scope and next steps

The current release set contains the Mycelium workspace only. Folding in Soil
(`soil`, `soil-store`, `soil-spore`, `files-serve`, `terminal-host`),
`iroh-tun`, `fungos-vfs` and `fpl-bob` is additive: publish each component's
tarball with a checksum, add it to `components`, and extend the install lists.
`fungOS.app` is the same shape of gap on macOS: once the bundle is published,
the `.dmg` step should read it from the release set instead of requiring
`--app`. The Windows `.exe`/MSI format is intentionally absent until
Mycelium has a Windows service/RPC/DNS port; a wrapper around the Unix daemon
would install a service that cannot run.

Run the synthetic smoke test with:

```sh
NFPM="$HOME/go/bin/nfpm" ./packaging/test-package.sh
```

The Debian package currently installs the binaries but does not own service
activation or enrollment. Mycelium's native enrollment path still generates the
service definition; a production `.deb` needs the packaged-enrollment path
called out in [`packages/COMPONENTS.md`](../packages/COMPONENTS.md) before APT
is the sole owner of an installation.
