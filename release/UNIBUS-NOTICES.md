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

This is preparatory package evidence, not a combined-image or v0.1 acceptance.
Final owner archives, source retention, installed notice hashes and the exact
combined ISO still require their own recorded verification. No public release,
fleet change or compliance certification follows from collecting notice files.
