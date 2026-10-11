# Runtime component input gate

An OS-only installer is not the operational v0.1.0 headless-edge release.
`component-bundle` validates real owner-built Debian packages and prepares the
payload the installer consumes. It does not generate substitute executables,
sign provenance, build another project's payload, or admit a package merely
because somebody supplied its checksum.

Run on Linux with `dpkg-deb` and `sha256sum`:

For isolated Mycelium qualification while Unibus remains separate, explicitly
use `prepare-mycelium-qualification` and `verify-mycelium-qualification` instead
of `prepare` and `verify`. This requires exactly the genuine Mycelium package;
the normal full-bundle commands still reject a missing Unibus package.
Pass `mycelium-qualification` as the install builder's fourth argument:
`sh installer/build.sh debian.iso qa.iso /new/mycelium-bundle mycelium-qualification`.
The ISO records `runtime_components=mycelium-only-qualification`; it is not a
complete operational headless-edge release. Checksums and package provenance
validation are identical; no enrollment claim or private authority is embedded.

```sh
cargo check --locked --manifest-path packages/Cargo.toml --bin component-bundle
cargo test --locked --manifest-path packages/Cargo.toml --bin component-bundle
cargo build --locked --release --manifest-path packages/Cargo.toml --bin component-bundle
packages/target/release/component-bundle prepare /build/components.tsv /new/component-bundle
packages/target/release/component-bundle verify /new/component-bundle
sh installer/build.sh debian.iso fungos.iso /new/component-bundle
```

The input TSV lives beside its packages, with exactly nine nonempty columns:
`Package`, `Version`, `Architecture`, `SHA256`, `Filename`, `SourceURL`,
`SourceCommit`, `License`, `UpdateOwner`. Use real tab characters. Package names
are `mycelium` and `unibus-core`, architectures `amd64` or `arm64`, hashes are
full lowercase SHA-256, commits are full lowercase 40-character Git IDs,
filenames are plain `.deb` basenames, source URLs are HTTPS and update ownership
is exclusively `apt`. Both required components must have the same architecture.
The amd64 installer additionally rejects an arm64 bundle.

The verifier compares hashes and Debian metadata, requires runtime executables,
systemd units and owner copyright notices, rejects operational credential paths,
symlinked package inputs and maintainer scripts that would silently run code.
It creates a new directory (never overwrites), copies only the packages and
records `components.tsv` and a strictly bounded `SHA256SUMS`. The installer
verifies the complete bundle before embedding it, rechecks hashes before APT
installation, resolves dependencies through the shared signed Debian snapshot,
explicitly enables the owner services and retains provenance on the installed
system under `/usr/share/fungos/components/`. A valid checksum is integrity,
not evidence of an authorized build or of redistribution permission.

Two-argument `installer/build.sh` remains useful for baseline BIOS/UEFI testing;
its manifest explicitly says `runtime_components=absent`. It is **not** an
operational release candidate. Three-argument builds record `owner-debs` and
the component manifest hash. Operational qualification still must exercise
boot, enrollment, personal SSH, real authorized Unibus delivery, update,
rollback and reboot against the exact final bytes.

## Owner contract status (2026-10-11)

- Mycelium's genuine owner Debian package, packaged first-contact adapter and
  installed certificate SSH have been qualified. See
  [package evidence](MYCELIUM-PACKAGE-QUALIFICATION.md),
  [signed APT evidence](MYCELIUM-SIGNED-APT-QUALIFICATION.md) and
  [published qualification](../release/PUBLIC-QUALIFICATION.md).
  These are Mycelium-only results, not proof for a new combined image.
- Unibus owns the real packager at
  `crates/unibus-router/examples/linux_core_deb.rs` and documents it in
  `docs/linux-core-package.md`. Its router/CLI package has a loopback-only
  default, empty grants, a dynamic unprivileged systemd identity and no implicit
  activation. The isolated owner packaging branch now requires an explicit
  copyright/notice file and installs it verbatim; input validation does not
  establish completeness or permission. The committed owner workspace still
  declares `LicenseRef-Proprietary`; pending MIT/Apache changes are uncommitted
  and must be confirmed by the owner before redistribution. Reviewed notices
  and corresponding sources for the actual locked router/CLI build remain gates.
- Generic claim delivery installs exactly one executable adapter at
  `/usr/lib/fungos/first-contact.d/`, consume private claim material outside the
  ISO and preserves the common first-contact service's existing sandbox.
- Optional Canvas requires owner-built compositor/adapters/theme/assets together;
  this bounded headless-edge gate does not pretend to supply Canvas packages.

No fleet keys, private certificates, claim tokens or live installations were
changed to implement this assembly boundary. Keep this gate blocking the
release bundle until owner packages and final-image qualification are real.
