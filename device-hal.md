# Portable device HAL and Linux service

`fungos-device-hal` exposes capability-honest keyboard illumination and an
activity-only policy. Default off; key presses and repeats extend a monotonic
five-second window; override leases take priority and are capped at one hour.
Unsupported capabilities are not fabricated. The Mycelium firmware HAL is not
a dependency: its no_std register boundary does not match this host contract.

`src/services.rs` defines frontend-neutral `AccessoryInventory` and
`AudioRouteControl` seams with typed failures; no Linux/Android runtime adapter
is claimed. Implement `AccessoryInventory::snapshot(now_monotonic_ms,
max_age_ms)` and `AudioRouteControl::{outputs, set_volume}`. Inventory entries
are `AccessoryItem { identity, kind, connected }`; battery is optional
`BatterySample { percent, charging, observed_monotonic_ms }`. Stale battery
snapshots return `AccessoryFailure::Stale`; missing hardware returns typed
`Unavailable`, never placeholders. Volume is 0–100 and delegates to the existing
platform policy owner, not a competing mixer. BlueZ retains bond ownership.
These seams let Canvas/Soil and ohmOS/Slint provide separate adapters without
binding either frontend into this crate.

Build/run: `cargo build --release`; run `fungos-keyboard-lightd` as a user with
read access to the input node and write access to the LED brightness file.
Required `FUNGOS_INPUT_NAME` is the exact `/proc/bus/input/devices` name. The
service matches the identity through sysfs, never assumes event0, reads without
EVIOCGRAB, and selects a keyboard LED by `FUNGOS_LED_NAME` (optional exact sysfs
name) or keyboard-backlight naming. Roots can be overridden with
`FUNGOS_INPUT_ROOT`, `FUNGOS_SYS_LED_ROOT`; API socket defaults to
`/run/fungos-keyboard-light.sock` and can be overridden by `FUNGOS_SOCKET`.

Local Unix socket API (one newline-terminated request): `status`, `lease LEVEL
MILLISECONDS`, `clear`, `stop`. Stop forces off before orderly exit. Leases are
bounded, and configured identity values are validated. S22 mapping, if selected
by deployment, is `cat:white:kbd_backlight` with binary levels 0/1, which lights
keypad and D-pad; this is not a portable assumption.

Run portable tests with `cargo test`. Cross-build ARM64 on a Linux host with
`cargo build --release --target aarch64-unknown-linux-gnu` and a configured GNU
AArch64 linker/sysroot; the macOS development host used here lacks that linker.
