# Battery-v1 isolated provider / Soil interface

## Validated local artifact

`/Users/ajmwagar/Documents/src/fungOS-linux/fungos/.worktrees/device-hal/target/battery-only/aarch64-unknown-linux-musl/release/fungos-keyboard-lightd`

SHA256 `938bb6c4b1c7ebc55b7fa54167617cd2fb23e8d1da95a247e1fe77bab3c2e508`

ELF64 AArch64, statically linked musl, stripped. Separate local target directory;
previous lid4988 output retained and hash verified. No deployed binary/socket or
phone access. No build server, commit, push, charger controls or Soil edits.

## Parent-owned deployment, not executed by HAL

Stage verified artifact as executable `/run/fungos-battery-provider`, then:

```sh
FUNGOS_BATTERY_SOCKET=/run/fungos-battery-provider.sock \
/run/fungos-battery-provider --battery-only

FUNGOS_BATTERY_SOCKET=/run/fungos-battery-provider.sock \
/run/fungos-battery-provider --request battery-v1
```

Flag mandatory. Noargs still runs combined provider. Early battery dispatch
precedes combined Config/run; does not construct keyboard/light/policy/keypad,
lid, audio, charger control or combined services. Existing PowerSupply is the
sole telemetry reader. FUNGOS_POWER_SUPPLY_ROOT defaults /sys/class/power_supply;
root override is trusted deployment configuration, not client-selected.
FUNGOS_BATTERY_SOCKET must be explicit/absolute; FUNGOS_SOCKET is not used.
Do not set it to the running lid socket. Directory must exist and be trusted.

One0600 listener; bind never unlinks any existing active/stale/file/symlink path.
SIGTERM/SIGINT normal exit cleans only recorded device+inode pathname, preserving
replacement. Check/unlink is not atomic; no concurrent directory manipulation.
SIGKILL/crash can leave stale socket; parent assesses ownership before removal.
In-flight request read/write can delay TERM by bounded IO timeouts. No stop command.

## Shared typed API

`fungos_device_hal::battery_transport::{BatteryClient, RemoteBatterySample, BatteryFailure}`

- `BatteryClient::new(socket).read() -> Result<RemoteBatterySample, BatteryFailure>`
- `remote.sample() -> &services::BatterySample`
- `remote.provider_age()`, `remote.age() -> Duration`
- `remote.snapshot(max_age: Duration) -> Result<BatterySample, BatteryFailure>`

Soil must authorize battery capability BEFORE read; Unix DAC also applies.
Remote age includes provider age + entire request round trip + local retention.
Provider timestamps are provenance in its monotonic domain, never guest clocks.
Client reconnects for each read; no last-good cache after failure. Permissions
map to Denied, connection failure to Disconnected; backend absence to Unavailable.
UI v1 scope is percent and voltage only; no voltage-to-SOC inference.

Existing BatterySample reused: Option<u8> percent, Option<u32> voltage_microvolts,
Option<i32> current_microamps/temperature_deci_celsius, ChargingState,
BatteryProvenance::LinuxPowerSupply(PathBuf), observed_monotonic_ms.
Current091 supplied inventory: type Battery, voltage_now3878682, NO capacity,
current,status,temp. Hence percentNone, voltageSome3878682, current/tempNone,
chargingUnknown. Sysfs provenance is not a measured-current accuracy claim.

## Wire v1 (tabs, exactly one LF)

Request exactly `battery-v1\n`, write-half closed. Other commands return typed
`FUNGOS-BATTERY/1\tERR\tprotocol\n` with no telemetry/control side effects.

```text
FUNGOS-BATTERY/1<TAB>OK<TAB>percent<TAB>charging<TAB>voltage_uV<TAB>current_uA<TAB>temp_deciC<TAB>observed_ms<TAB>age_ms<TAB>linux-power-supply<TAB>path_hex<LF>
FUNGOS-BATTERY/1<TAB>ERR<TAB>reason<LF>
```

Missing numeric values are literal `unknown`, not0. Percent0..100; voltageu32,
current/tempi32; charging unknown/charging/discharging/not-charging/full.
Observed/age unsignedu64. Provider captures timestamp BEFORE PowerSupply.read,
then samples now AFTER read and rejects age>1000ms or invalid/future timestamp.
Multiple eligible host batteries return ambiguous rather than first-selection.
Remote decode also rejects claimed provider age>1000ms.

Errors exact: unavailable,denied,backend,ambiguous,stale,invalid-time,
disconnected,protocol. Unknown error names, missing/extra fields, version,
nonUTF8, malformed/oversized frames, numeric ranges and invalid hex path rejected.
Hex path must be absolute, no NUL/LF/CR. Max4096 response bytes, read capped4097
for overflow detection; read/write timeouts2s. Provider request cap257 bytes.
No Debug parsing. CLI reconstructs validated typed response; use typed client's
snapshot for transport+retention freshness enforcement rather than CLI text.

## Validation

Ordered local check -> full test -> strict all-target Clippy -Dwarnings -> fmt
check -> diff check PASS, no lint allowances. **45 tests passed** (30lib,12binary,
3integration), including all existing lid and combined regressions.

New tests cover unknown vs0/100, voltage-only/noSOC, stale/future/RTT/retention,
integer bounds, error names/extra/missing/nonUTF8/hex paths/newlines/oversize,
PermissionDenied mapping, disconnect/reconnect; actual executable has no keyboard
identity and invalid unused LED config, sentinel roots unchanged, battery absence,
voltage-only/unknown percent, malformed capacity failure/no cached good,
multiple-battery ambiguity, command rejection,0600, collision preserving owner,
TERM cleanup/restart/reconnect. Architecture guard rejects other backend
references; ownership unit test preserves replacement pathname. These are host
source/executable tests, not syscall tracing or new phone hardware proof.

After successful validation: target musl check then local zigbuild release PASS;
only existing linker deprecated optimization setting warning. Parent owns target
start/noPCM/read-only/telemetry receipts and physical widget qualification.
Sole Soil writer coordinated: `66:pi-soil-battery-impl`; HAL edits no Soil files.
