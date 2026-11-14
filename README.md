# fungOS

![fungOS mark](brand/fungos.svg)

Debian-based nodes with explicit boot profiles, secure enrollment and trusted
software delivery. Mycelium manages identity, topology, access and updates;
optional edge profiles add Unibus and the native Canvas Wayland compositor.

[Website](https://fungos.dev) · [Website source](https://github.com/ajmwagar/fungos-web) · [Mycelium](https://github.com/ajmwagar/mycelium)

![fungOS edge: native Canvas and two Wayland terminals running in QEMU](docs/screenshots/fungos-workspace.png)

Actual QEMU framebuffer capture, 6 October 2026—not a mockup. See
[screenshot provenance](docs/screenshots/README.md). These are development
builds, not a stable distribution release.

## First public slice

This repository contains the generic [base filesystem builder](base/README.md),
capability-derived Debian package sets and `base`, `edge`, `headless-edge`, and
`cloud` declarations for `amd64` and `arm64`.

The base builder produces a **rootfs tarball**. Separate
[amd64 installer tooling](installer/README.md) assembles a Debian installer ISO
with the same profile and optional owner-built packages. The narrow
[Mycelium-only qualification download](release/PUBLIC-QUALIFICATION.md) is public;
the combined Mycelium/Unibus headless-edge candidate is still being qualified.
Neither is a supported v0.1 release. Pi board firmware, kernels, SD-card assembly
and physical hardware qualification remain separate integration steps.

The [signed APT publisher](packages/README.md) produces immutable, multi-architecture
repository generations. Its [qualification](packages/QUALIFICATION.md) exercises
real Debian client verification; the FPL Cloud origin is not deployed yet.

The [amd64 installer tooling](installer/README.md) and
[torrent release preparer](release/README.md) live here as well. The installer
shares the base profile, pinned snapshot and overlay rather than maintaining
another package list. See [image qualification](installer/QUALIFICATION.md)
for exact tested bytes, public-install findings and remaining release gates.
Torrent preparation is local packaging, not publication or release signing.

## Packaging

[`packaging/`](packaging/README.md) turns a pinned release set into `.deb`,
`.rpm`, Arch, AUR and macOS `.pkg`/`.dmg` artifacts. It consumes published
component tarballs and checksums; the signed APT repository publisher remains in
[`packages/`](packages/README.md).

```sh
scripts/package.sh --dry-run
scripts/package.sh --formats deb --target x86_64-unknown-linux-musl
```

## Check and build

Validate the declarations without building an image:

```sh
./base/scripts/check.sh
```

On a Debian Linux build host with `mmdebstrap`, the Debian archive keyring and
the required privileges:

```sh
sudo ./base/scripts/build.sh amd64 base
sudo ./base/scripts/inspect.sh base/out/fungos-base-amd64.tar
```

See the [runbook](base/RUNBOOK.md) for prerequisites and cross-architecture
builds. Snapshot inputs live in [`base/config/release.env`](base/config/release.env).
No signing authority, private key, enrollment claim or fleet credential is
included in the source or generic base image.

## Edge desktop

![fungOS Undergrowth desktop](docs/screenshots/fungos-desktop.png)

Canvas and Unibus remain independent projects. The image profile supplies
their runtime dependencies; it does not fork their implementations or grant
discovered devices access automatically.

See the [persistent QEMU dogfood runbook](docs/edge-dogfood.md) and
[isolated visual rollback procedure](docs/visual-rollback.md) for boot,
workspace retention and recovery checks. These procedures are qualifications,
not a production installer or permission to run fault tests on your devices.

## Local intranet

The optional [intranet integration](docs/intranet.md) keeps mesh/site DNS in
Mycelium and delegates workload discovery to Shroud. Palisade supplies stable
HTTP entrances; Unibus can consume service observations without being required
for browser access. DNS planning is implemented locally; persistent activation
and image integration are not yet qualified.

## License

First-party code and documentation: **MIT OR Apache-2.0**, at your option.
See [LICENSE](LICENSE), [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE).

Generated distributions contain separately licensed components. Linux, Debian
packages, board firmware and optional applications retain their own terms;
the complete image is not uniformly MIT/Apache. See [THIRD-PARTY.md](THIRD-PARTY.md).

The initial base source was extracted from Mycelium commit
`fbb768b`; [ORIGIN.md](ORIGIN.md) records the import boundary.
