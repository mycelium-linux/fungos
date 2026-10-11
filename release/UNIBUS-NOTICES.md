# Unibus core notice qualification

The owner confirmed `MIT OR Apache-2.0` for first-party Unibus on 2026-10-11.
Original dependencies retain their own licenses. The owner Debian packager ships
both complete first-party license texts and accepts reviewed original dependency
notices without flattening them into the first-party license.

The registry-only audit reuses `owner-license-audit`:

```sh
owner-license-audit registry /clean/owner-source /exact/versioned-vendor /new/audit
```

It verifies locked registry archive and per-file checksums and inventories notice
files and conservative source-header attributions. It does not claim legal
clearance, a minimal linked-code graph, compiler runtime coverage or review of
missing notice evidence. Runtime sources/notices are separate inputs. Its original
Mycelium runtime-superset mode and historical output labels remain unchanged.

On Agora, the Unibus lockfile SHA-256
`b994261987b83096b09c98715275e0226ac4e9cbfe65d8563a82cb698802d677`
passed checksum verification for all 484 registry source archives. The amd64
default router/CLI normal-and-build dependency scope has 80 distinct packages,
including three first-party workspace crates. Each external package in this
scope has retained notice evidence; inactive dependencies with absent notices
are not silently promoted into that claim. Rust 1.98.1 original notices are
retained separately as a conservative compiler/library superset.

## Retained owner package (2026-10-11 UTC)

The isolated owner source is commit
`eff42b728d33a6893de6d4292d8d4dad9a2c0714`. The active owner's dirty checkout
was not replaced. This build includes the opt-in, local authenticated readiness
endpoint; the shipped default still has no application grants or health client.

| Input | SHA-256 |
|---|---|
| `unibus-core_0.1.0_amd64.deb` | `bf2d12a14405c8fc26bce816cac00f581d4c374e64dd9e571f8a17b7d14b2074` |
| Installed router bytes | `70310b2a073e99d68aa4ddc4b5307bcb522f7f11629c6f7c96d30d9e52731d06` |
| Installed CLI bytes | `ec703985fc2ad7b65f2dff869109bee2b237e6790340f4c55a95c61c93995665` |
| `unibus-owner-sources.tar.zst` | `0443b6f66ceb43d092730fcae709404ddecccf8719bddfe1e5900fa90e12e228` |

Readback of the actual package verified all 550 retained notice-file hashes.
The source archive contains the clean owner source, locked versioned vendor
sources and offline Cargo configuration, not private runtime credentials.
Compression integrity and an extracted-source `cargo check --offline --locked
-p unibus-router --bins --example linux_core_delivery` passed on Agora. The
archive expands to 707,328,000 bytes.

Evidence is retained on Agora under
`/home/ajmwagar/.cache/fungos-headless-qa8.fTarSb/`. These are local qualification
artifacts, not published downloads. Source/notices inspection does not establish
installed delivery, update/rollback, a combined-image acceptance or compliance
certification. Those gates are recorded separately.
