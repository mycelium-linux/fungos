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
`BatterySample` with optional kernel-reported percent, voltage (µV), signed
current (µA), temperature (tenths °C), explicit `ChargingState` (including
`Unknown`), source-directory provenance, and `observed_monotonic_ms`.
`BatterySample::check_freshness(now, max_age)` accepts the exact age boundary,
returns `Stale` beyond it, and rejects future timestamps as `InvalidTime`.
All timestamps must share the provider's monotonic clock domain. Missing fields
are `None`, not zero/false; full and not-charging remain distinct. Malformed or
out-of-range attributes return `Backend`, permission failures return `Denied`.
Missing hardware returns typed `Unavailable`, never placeholders. Volume is 0–100 and delegates to the existing
platform policy owner, not a competing mixer. BlueZ retains bond ownership.
These seams let Canvas/Soil and ohmOS/Slint provide separate adapters without
binding either frontend into this crate.

Build/run: `cargo build --release`; run `fungos-keyboard-lightd` as a user with
read access to the input node and write access to the LED brightness file.
Required `FUNGOS_INPUT_NAME` is the exact `/proc/bus/input/devices` name. The
service matches the identity through `/sys/class/input/eventN/device/name`,
then opens `/dev/input/eventN`; it never assumes event0, reads without
EVIOCGRAB, and selects a keyboard LED by `FUNGOS_LED_NAME` (optional exact sysfs
name) or keyboard-backlight naming. Roots can be overridden with
`FUNGOS_INPUT_ROOT` (device nodes), `FUNGOS_SYS_INPUT_ROOT` (sysfs identities),
`FUNGOS_SYS_LED_ROOT`; API socket defaults to
`/run/fungos-keyboard-light.sock` and can be overridden by `FUNGOS_SOCKET`.

Local Unix socket API (one newline-terminated request): `status`, `lease LEVEL
MILLISECONDS`, `clear`, `stop`. Stop forces off before orderly exit. Leases are
bounded, and configured identity values are validated. S22 mapping, if selected
by deployment, is `cat:white:kbd_backlight` with binary levels 0/1, which lights
keypad and D-pad; this is not a portable assumption.

## Soil typed lid transport (reversal-8vo7)

Owner library import: `fungos_device_hal::lid_transport::{LidClient,
RemoteLidSample, encode_response, decode_response, REQUEST, MAX_FRAME}`. Path
dependency package `fungos-device-hal` in this HAL worktree; share `lid::{LidSample,
LidPosition,LidFailure}` rather than duplicating enums/source/freshness or parsing
Debug strings. Soil worker owns WIT/core-Wasm lowering, explicit guest permission,
Wasmtime integration and guest initial/live logging. Deny unauthorized guest
access **before** calling the socket client. No display switching.

```rust,ignore
let client = fungos_device_hal::lid_transport::LidClient::new(
    "/run/fungos-device-provider"); // explicit trusted deployment path
let remote = client.read()?;
let provider_age = remote.provider_age(); // wire age, Duration
let sample = remote.snapshot(std::time::Duration::from_millis(1000))?;
```

`sample()` borrows the owner `LidSample`; `provider_age()` returns the age reported
in the provider's monotonic domain, while `age()` conservatively adds the entire
request roundtrip and local retention elapsed time. `snapshot(max_age)` checks
that combined age before cloning the sample. Provider timestamp is provenance,
**not** in the runtime/guest clock domain. Do not manually subtract clocks or
serve a retained sample after disconnect/resync. Each `read()` opens a new
connection, so another call reconnects after errors/restart; the client has no
last-good fallback cache. EOF/connect/transport timeout => Disconnected, DAC
permission => Denied, malformed/unknown-version response => Protocol. Provider
sensor/backend/resynchronization errors remain the same typed LidFailure, not
false/Open. A valid raw reading with Unqualified physical position is distinct
from a failed/unknown reading. Existing owner-only0600 socket remains unchanged.

Version1 request is exactly `lid-v1\n`, half-close write; bounded response:

```text
FUNGOS-LID/1<TAB>OK<TAB>0|1<TAB>unqualified|open|closed<TAB>observed_ms<TAB>age_ms<TAB>hex_unix_event_path<LF>
FUNGOS-LID/1<TAB>ERR<TAB>unavailable|denied|backend|ambiguous|resynchronizing|stale|invalid-time|disconnected|protocol<LF>
```

Version1 provenance kind is Linux evdev SW_LID, node hex preserves Unix bytes.
Bound4096 bytes including newline, exact arity/version, raw/position consistency,
strict unsigned u64 numbers; no trailing frames, missing newline, unknown fields
or unbounded string parsing. `encode_response(Result<LidSample,LidFailure>,
provider_now_ms)->String` and `decode_response(&[u8],request_started:Instant)` are
public for Soil UnixListener integration tests. Client has2s read/write timeouts;
server requests are capped256 bytes with2s timeouts. Failed client requests,
closed peers and oversized records no longer terminate the provider. A same-uid
slow client can still delay this single loop within that timeout; this is not a
multi-tenant untrusted socket. UNIX connect is local; no network or device-node
access is delegated to the guest.

Deployment must inject the actual socket: source default remains
`/run/fungos-keyboard-light.sock`, **not** `/run/fungos-keyboard-lightd.sock`.
Parent confirms combined socket `/run/fungos-device-provider`; retain that via
FUNGOS_SOCKET, and retain `FUNGOS_LID_POLARITY=standard-closed-asserted` only on
qualified .074 (parent physical Open/rawfalse, Closed/rawtrue). No code-name-based
polarity mapping exists. `--request lid-v1` emits the structured diagnostic reply;
old `--request lid`, battery/status and keyboard commands remain compatible.

```sh
FUNGOS_SOCKET=/run/fungos-device-provider /path/to/fungos-keyboard-lightd --request lid-v1
```

Parent must replace/restart the existing provider binary; the earlier caa338
artifact does not implement lid-v1. New static AArch64-musl artifact same path:
`target/aarch64-unknown-linux-musl/release/fungos-keyboard-lightd`, SHA256
`24923c64814290eb4a74d8262a583096c4a2bc78ff400b7d9837b723fddf6809`.
30 tests pass (23library/6binary/1 executable integration), including version/size/
field rejection, type/provenance/age roundtrip, stale/unknown/denied/resync,
disconnect→resync→reconnect success, missing socket, explicit DAC error mapping,
and real provider typed-client requests/oversized peer survival/old commands0600.
Clippy passes with the two preexisting main allowances. This is host validation,
not a worker phone test. No phone/builder access, shared kernel edits, deployment,
commits or pushes. Direct API handoff sent to `66:pi-soil-ui-resume`.

## Lid observation (reversal-8vo7)

### .073 physical evidence and standard-polarity gate

Parent reports combined socket `/run/fungos-device-provider`, event1:
USER OPEN => asserted=true; USER CLOSED => asserted=false. Parent also reports
no main-screen flashing while open with simpledrm shared supply votes. These are
parent-owned observations, not worker tests, and do not authorize panel policy.
Pending reopen confirmation, the current GPIO44 active-high DT produces inverted
standard SW_LID meaning. Recommend the narrow parent-owned DT change: GPIO44
flags0 -> flags1 (`GPIO_ACTIVE_LOW`), preserving EV_SW/code0 and debounce15.
This complements the logical switch level; it does not change pull configuration,
GPIO ownership, event type, or display policy. Confirm reopen first, then next
approved RAMboot must prove OPEN=>raw0, CLOSED=>raw1. Do not deploy this flag change
blindly or assume the name `gpio-keys` qualifies physical polarity.

HAL now supports explicit `LidPolarity::StandardClosedAsserted`, selected on the
**existing provider** by trusted deployment environment:
`FUNGOS_LID_POLARITY=standard-closed-asserted`. Enable ONLY on the corrected and
physically qualified board configuration; leave it unset on .073. Default and
explicit `unqualified` keep physical_position Unqualified for either raw value.
No inverse/open-asserted mapping exists. Unknown settings fail configuration.
With the qualification gate enabled, rawtrue maps Closed and rawfalse maps Open;
`switch_asserted` remains exactly the kernel SW_LID bit. Initial and resync query
results use the same mapping; errors still have no physical reading. This setting
is a deployment qualification assertion, not automatic DT/image attestation;
remove it if booting an older/inverted image.

```sh
# Current .073 and next boot polarity capture (socket owner uid):
FUNGOS_SOCKET=/run/fungos-device-provider /path/to/fungos-keyboard-lightd --request lid
```

After corrected-DT qualification, configure the provider environment/restart via
its existing lifecycle, then run that same client. No per-request polarity option
or global gpio-keys=>Open special casing is introduced. Test initial state with
provider started in each physical position, reopen/reclose, and verify raw0/Open,
raw1/Closed. Keep panel/backlight/suspend behavior unchanged.

Updated validation: **26 tests pass** (19 library,6 binary,1 integration), including
standard initial/resync mapping, raw-bit preservation, default-unqualified and
invalid-setting rejection; Clippy passes with the two existing allowances.
Updated static ARM64-musl binary (same path below) SHA256:
`caa338d74420b28659aea5e765035121cc9fc0228a5569a75627b1d40f53f983`.
This supersedes the earlier artifact hashes below. No shared DT/kernel edits or
phone access were performed by the worker.

Battery expansion is paused. `src/lid.rs` adds typed frontend-neutral
`LidSample`, `LidPosition`, `LidFailure` and read-only Linux `LidInput` to the
**same** provider loop/socket/uid; no new service, thread, grab, input injection,
GPIO access, policy owner, or display/suspend/backlight behavior. Existing light
activity only consumes keyboard input, never lid transitions.

Discovery matches exact `FUNGOS_LID_NAME` (default `gpio-keys`), then requires
EV_SW and SW_LID capability ioctls. Multiple matching capable devices return
`Ambiguous`, not whichever event number is first. Identity metadata comes from
`FUNGOS_SYS_INPUT_ROOT` (default `/sys/class/input`), read-only event nodes from
`FUNGOS_INPUT_ROOT` (default `/dev/input`). These trusted deployment overrides
also fix the keyboard's previous metadata-under-/dev expectation: synthetic tests
must now keep separate sysfs identity and device-node trees.

A successful sample contains `switch_asserted: bool`,
`physical_position: Unqualified` by default (Open/Closed only with the explicit
qualified standard-polarity setting above), event-node provenance, and a provider-monotonic
observation timestamp. Both bit values remain physically **unqualified**. Linux
SW_LID's nominal convention and DT active-high are not a physical polarity test.
No open/closed mapping is silently enabled. No display-policy commands are added.

Initial state uses EVIOCGSW without requiring a transition. Each existing-loop
poll drains up to256 native-layout input_event records, then refreshes EVIOCGSW;
event payload values and kernel event timestamps do not fabricate state. On
SYN_DROPPED, samples become `Resynchronizing` until SYN_REPORT and a successful
query. A flood that exhausts the drain budget is also unknown/resynchronizing.
Short records => Backend, EOF => Unavailable, read/ioctl failure => typed error;
previous good samples are discarded. Discovery retries after1s in the existing
loop. Missing gpio-keys is nonfatal and does not prevent battery/light operation.
`check_freshness` accepts exact max age, returns Stale beyond it and InvalidTime
for future observations. The socket's on-demand query uses a1000ms maximum age;
clients retaining a sample must apply their own max age in the same clock domain.
Observation time is a successful kernel query time, not physical switch-change
or sensor-acquisition time. Non-Linux hosts cannot provide successful evdev
ioctls; portable tests inject bounded read/query behavior instead.

### Parent deployment and qualification (no worker phone access)

Parent reports `.072` has INPUT_EVDEV=y but KEYBOARD_GPIO unset; DT alone
(GPIO44 active-high, EV_SW/code0, debounce15) therefore does not create gpio-keys.
Expected current result: `ERR lid=Unavailable` with nonzero client exit.
**Only parent** enables KEYBOARD_GPIO and RAMboots after the main-screen flashing
regression review. This HAL change does not require panel-policy changes.

Replace/restart the existing provider through the established procedure, retaining
keyboard/LED configuration and socket ownership; do not launch another provider.
Provider uid needs ordinary read permission on the new gpio-keys event node;
use the existing restricted deployment permission model, not world-readable input
or a setuid helper. Default lid identity is gpio-keys; if the kernel names it
differently, set `FUNGOS_LID_NAME` to its exact sysfs name on the existing provider.

```sh
# Same socket-owning uid; one-shot client, not a service or direct device reader.
/path/to/fungos-keyboard-lightd --request lid
/path/to/fungos-keyboard-lightd --request status
/path/to/fungos-keyboard-lightd --request battery
# Nondefault socket, if used by existing deployment:
FUNGOS_SOCKET=/path/to/existing.sock /path/to/fungos-keyboard-lightd --request lid
```

After approved RAMboot, parent records bit values at physically open and closed,
repeats transitions, and confirms unrelated keys do not change the bit. Restart
provider once open and once closed and query **before moving the lid** to test
initial EVIOCGSW. Expect `OK lid=LidSample { switch_asserted: ..., physical_position:
Unqualified, event_node: ..., observed_monotonic_ms: ... }`; timestamp advances on
observation, not only transitions. Capture both physical positions and startup
cases before approving a separate polarity-mapping patch. Keep panel behavior
unchanged. Error/disconnect/dropped-event behavior is host-tested; do not inject
input or force an overflow on the phone merely for this handoff.

### Validation / artifact

`cargo test`: **24 passing** (18 library,5 binary,1 executable/socket integration).
Coverage includes initial query at either bit, ignoring event value as source of
truth, dropped interval/report/failed resync, EOF/short/denied reads, bounded
floods, invalidated cached state, missing discovery, exact/stale/future freshness,
unqualified polarity, executable request routing, existing commands and0600.
These do not claim successful Linux ioctl or physical GPIO qualification on macOS.
`cargo check --target aarch64-unknown-linux-musl` and Clippy pass (only the two
preexisting main-module lint allowances). Local static cross-build:

```sh
cargo zigbuild --release --target aarch64-unknown-linux-musl
# target/aarch64-unknown-linux-musl/release/fungos-keyboard-lightd
# SHA256 773e9b05543a22b241f32cb599ed65837fa94799f27818c40468b2f1bbe37036
```

ELF64 ARM aarch64, static, stripped; only linker warning is deprecated optimization
setting1. This supersedes the battery-only artifact hash. No phone/shared builder
access, kernel edits, deployment, commits, or pushes were performed by this worker.

## Battery integration (reversal-s4ts.1)

`src/power_supply.rs` runs inside the same Rust provider/library and existing
socket loop; there is no new binary, service, worker thread, poller, or lifecycle.
The existing `fungos-keyboard-lightd` executable name is retained for compatibility.
The read-only `battery` request reads `/sys/class/power_supply` on demand.
`FUNGOS_POWER_SUPPLY_ROOT` is a trusted deployment/test override, not a
client-selectable path. Responses are diagnostic `OK battery=[...]` or typed
`ERR battery=Unavailable|Denied|Backend`; existing commands are unchanged.
The Unix socket is now owner-only (0600): frontend adapters must use the
existing permissioned authority, not expose this socket or its lease commands
to arbitrary clients. Library callers must enforce their own caller permissions;
filesystem access uses the provider's ordinary uid, without privilege escalation.
This does not introduce Soil or ohmOS dependencies or a second policy owner.

Only explicit `type=Battery` entries are used, excluding `present=0` and
`scope=Device`. Multiple system batteries remain individual sorted samples;
there is no guessed aggregate. Unknown/missing attributes remain unknown;
unrecognized status/malformed numbers fail rather than guess. Native kernel
units are retained without conversion; percent is strictly 0–100. No voltage to
SOC conversion, charging write, IIO/vendor fallback, or synthetic telemetry
exists. Missing roots and empty inventories return `Unavailable`. Reads are
non-atomic sysfs observations, timestamped before read, not a hardware freshness
guarantee; the kernel exposes no per-field measurement timestamp here. There is
no cache; consumers retaining samples must call `check_freshness` before display.
An inventory adapter can use these samples without pretending battery discovery
is an accessory/BlueZ adapter; existing accessory and audio seams are preserved.

Parent-owned read-only phone `.068` evidence: `/sys/class/power_supply` is empty
and `/sys/bus/iio/devices` is absent. This backend therefore returns unavailable
on that observed layout. Parent now reports **`.071` / kernel #43** exposes
`cat-s22-battery` with `voltage_now` approximately 3,830,000 µV and no capacity,
status, temperature or current. Its qualification shim consumes QCOM processed
µV directly with stock absolute/default scaling and 1/3 prescale. The backend
reads that standard supply unchanged: voltage present, percent/current/temp
`None`, charging `Unknown`. `.070` was a stale image after compilation failure,
not evidence for this supply. These are parent-owned observations, not a worker
phone validation; an exact-layout synthetic regression test covers the HAL path.
No phone or shared-builder access was performed for this change.

### Parent deployment / inspection

Local cross-build (no shared builder):

```sh
cargo zigbuild --release --target aarch64-unknown-linux-musl
# binary: target/aarch64-unknown-linux-musl/release/fungos-keyboard-lightd
```

Parent transfers that static ARM64 executable using its existing `.071`
known-hosts/transport and replaces the existing combined-provider executable
through its normal stop/update/start procedure. Do not start a second provider
or a battery daemon. Keep the existing input/LED identity configuration. The
default power_supply root already finds `cat-s22-battery`; no board-name setting
or alternate battery backend is necessary.

On the phone, as the same uid that owns the provider socket:

```sh
# One-shot client to the already running provider, not a second service.
/path/to/fungos-keyboard-lightd --request battery
/path/to/fungos-keyboard-lightd --request status
# If deployment uses a nondefault socket:
FUNGOS_SOCKET=/path/to/existing.sock /path/to/fungos-keyboard-lightd --request battery
```

The client needs no `FUNGOS_INPUT_NAME` and performs no direct sysfs read. It
half-closes the request to match the existing server protocol and exits after
the response; transport/provider failures exit nonzero. Expect `OK battery=[...]`
with `voltage_microvolts: Some(...)`, `percent: None`, `charging: Unknown`, source
path ending `cat-s22-battery`, and provider-monotonic observation time. Do not
expect the voltage to remain exactly 3,830,000. `ERR battery=Unavailable` is still
the correct result on `.068`. Socket authorization remains owner-only.


Synthetic-root tests exercise absent supplies/fields, native units, strict ranges,
charging states, provenance, stale/exact/future time, read-only behavior, DAC error
mapping and owner-only socket permissions. The filesystem denial assertion runs
under non-root uid; root bypasses DAC, with deterministic mapping tested separately.

Run portable tests with `cargo test`. Cross-build ARM64 on a Linux host with
`cargo build --release --target aarch64-unknown-linux-gnu` and a configured GNU
AArch64 linker/sysroot; the macOS development host used here lacks that linker.
