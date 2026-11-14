# Remaining CAT S22 telemetry: stock-backed feasibility and next patches

Audit for reversal-oead.2. No phone/builder access, shared source edits, PMIC
transactions, deployment, or charging-policy changes. This is an implementation
plan backed by retained stock DT/kernel bytes, not live qualification. Existing
.071 voltage-only supply and combined Rust provider remain intact.

## Current qualification correction (supersedes stale index8 blocker)

Latest owned acquisition audit:
`reversal/.worktrees/s22-outer-display-impl/cat-s22/microphone/STOCK49-OCV-THERM-ACQUISITION-AUDIT.md`.
Authoritative retained stock VADC batt_therm node9524–9533 selects26 (0x1a),
not8. Fitted dispatch pointer c0f89d6c+26*4=c0f89dd4 resolves c0a9497c QRD215;
index8 SMB is a different entry, NOT selected for this battery channel. Thus
QRD215 selection is statically proved, not an unresolved selection blocker.
It is NOT sensor/physical qualification. Current applied091 DT lacks channel0x30;
legacy ADC RAW clips endpoints and omits qualification provenance. New issuer
requires the upstream provider boundary/safety review described in that audit.
Fetched QRD215 table differs from fitted data: do not copy fetched values.
Prior 'next patch T1' below is historical candidate scope, NOT authorization or
implementation-ready instruction; exact provider validity/calibration/coherence,
input skew, physical sensor and fault qualification still precede use.

## Primary evidence / reproduction

Parent retained files under `cat-s22/out/`:

| Artifact | SHA256 |
|---|---|
| `vmlinux.bin` | `86a2a3094fc018db47a7dfa8723913791cf0cf339546e616006cf5b52a776680` |
| `kallsyms-fixed.txt` | `4b44b881da77cb7260f84b88a9ea51cd72b361eec4f635b1428e1c045bf626f8` |
| `resume-20261029/lk2nd-stock-live.dts` | `829f17ba9c87cb678c2990c8ec57c4682772424196dd25cfefb92e210989e279` |

Raw ARM kernel load base **0xc0008000**, corroborated by retained
`ghidra-import.log`. Address to file offset = address minus base. Disassembly
extracts and fitted thermistor CSV are retained beside this document in
`battery-stock-evidence/`. Example reproduction (read-only input):

```sh
arm-none-eabi-objdump -D -b binary -m arm --adjust-vma=0xc0008000 \
  --start-address=0xc0a9497c --stop-address=0xc0a94acc /path/to/vmlinux.bin
od -An -tx4 -j $((0xf81d6c)) -N 108 /path/to/vmlinux.bin
```

Reference source **not claimed exact fitted source**: LineageOS
`android_kernel_qcom_msm8916`, `lineage-16.0` branch, downloaded via
`https://raw.githubusercontent.com/LineageOS/android_kernel_qcom_msm8916/lineage-16.0/`:

- `drivers/power/qpnp-vm-bms.c`: SHA256 `8b7029aeb4d73097a51c09455a39d2d9f8d389b52e1e684357c774a075bfef4c`
- `drivers/power/qpnp-linear-charger.c`: `b77d448a2d9af91d802cb213f7eb23c9d5753c3d8596cabb0c75e1077e4d9d24`
- `drivers/hwmon/qpnp-adc-common.c`: `fb820a88df8be58402334bd3d9f649eb39446485359338e40508fdc7a727d421`
- `include/linux/qpnp/qpnp-adc.h`: `80930bf4f07e461905980899758e7ef9aa0d23c6435889bc99cc6724401657e3`

Mainline v6.12 `drivers/power/supply/pm8916_lbc.c` reference SHA256
`9b2d4b9b61435e7c0acf356740b8987e33c2227852285db9d9c922ed9a5c4672`.
Parent's actual Linux 7.3 source must be compared before patching.

## 1. Battery temperature: fitted table FOUND in stock binary

Correct the earlier parent audit's blanket "scale-function8":

- DT VADC `chan@30`: ratiometric, prescale0 (1/1), **scale-function0x1a**,
  settle0x0b, fast-average0.
- DT ADC_TM `chan@30`: **scale-function0x08**, settle0x0b,
  fast-average2, BTM channel0x48. This is a different driver's threshold scaling
  namespace; do not index the VADC table with ADC_TM's 8.
- Stock `vadc_scale_fn` at **0xc0f89d6c**: index26 pointer is
  **0xc0a9497c**, symbol `qpnp_adc_batt_therm_qrd_215`.
- Its ARM code loads table **0xc0f8ac24**, symbol
  `adcmap_batt_therm_qrd_215`, loops over **51** pairs, returns a signed 64-bit
  physical temperature at result offset16 and calibrated voltage at offset8.
- CSV contains the exact little-endian signed pairs extracted from
  `[0xc0f8ac24,0xc0f8adbc)`: input calibrated **mV**, output **tenths °C**.
  Anchors: 1534mV=-20°C,1263mV=0°C,939mV=20°C,787mV=30°C,
  451mV=60°C,327mV=80°C. End clamps in stock code are -200 and800.
  These anchors/table are not the generic 100k/104EF curve.

The scaler computes `(raw - gnd) * dx / dy` using the selected calibration graph,
adds dx only for absolute calibration, and clamps negative calibrated input to
zero. This channel selects ratiometric. Parent should recover/compare the exact
ratiometric graph setup and integer truncation against mainline before porting;
reference nominal1800mV alone is not a raw-code conversion constant. Parent DT's
battery-data rpull-up100k and vref1800000µV support the circuit context, but do not
replace the fitted conversion table.

**Next patch T1:** add a narrowly named QRD215 battery-therm scale path/table to
VADC/common scaling; leave existing generic thermistor scale unchanged. Instantiate
chan0x30 ratiometric with stock settle/divider. Preserve RAW, expose processed
IIO temperature in **milli°C** (stock deci°C multiplied100). Then extend the
existing qualification power_supply shim with TEMP in **deci°C** (IIO milli°C
/100), with one explicit conversion. Never put die temperature in battery TEMP.
This adds no userspace service. Existing Rust backend already parses `temp`.

Tests: all51 knots, midpoints/truncation, cold/hot endpoints, signed negative,
calibration denominator error, ADC failure, disconnected/saturated sensor. Do not
silently advertise a clamped endpoint as a trustworthy physical measurement for
an open/short; retain raw/diagnostic evidence or return error/unknown when fault
qualification establishes limits. Compare parent-owned independent temperature
observation at multiple temperatures before claiming sensor accuracy. No ADC_TM
threshold programming is required for on-demand VADC telemetry.

## 2. Charger presence, battery present, status, health: passive reads are feasible

Don't enable the full LBC driver just to observe it: both downstream and mainline
probe/configure paths program limits/enables and safety controls. Current mainline
`CONSTANT_CHARGE_CURRENT` is cached **configured limit**, NOT measured battery
current (`pm8916_lbc_charger_get_property`, field `charge_current_max`).

Useful source-backed register candidates from downstream (must corroborate exact
stock accessors/type IDs before a parent build):

| Address on stock SPMI slave0 | Meaning / qualification |
|---|---|
| USB0x1300+0x10, bit1 | USBIN_VALID; mainline reads this too. Input presence/online, not charging current/status. |
| BAT_IF0x1200+0x08, bit7 | pack presence per `get_prop_batt_present`; propagate read error, don't copy downstream's error=>absent fallback. |
| BAT_IF0x1200+0x09, bits7:6 | comparator thermal status; downstream hot maskbit6, cold test maskbits7:6. Corroborate encoding before health mapping; do not assume flags prove calibrated temperature. |
| CHGR0x1000+0x10, bit5 | FAST_CHG_ON realtime state; evidence of charger active, not instantaneous net battery current. |
| CHGR0x1000+0x08 | retained DT explicitly names this charger-presence status resource for VM-BMS; decode exact fitted masks before use. |

**Next patch S1:** separate opt-in passive reader within the qualification shim,
read-only regmap accesses to confirmed status registers. Add PRESENT to the same
battery supply; publish input ONLINE on a correctly typed USB supply (not as a
battery). No interrupt configuration/ack writes, detection-mode setup, LED,
thermal thresholds, current limit, timer disable, or charge enable writes.
Do not bind two overlapping active policy owners. Existing parent register layout
and disabled full charger/BMS preserve this boundary.

**Next patch S2:** map hardware charger state conservatively after corroborating
stock code/register semantics. FAST_CHG_ON can support Charging when valid;
absence alone must not imply Discharging. Full requires qualified termination
state, not merely a remembered chg-done IRQ; unknown conflicting/error states
remain Unknown/error. Port only verified comparator health cases, not hardcoded
Good when data is absent. State is non-atomic and can change between reads.

Parent test transitions: cable absent/present, attached-but-not-charging, active
charge, stop/termination, sensor absence and read error. Independent current
measurement is needed to distinguish charger operation from net battery flow.
The HAL needs a separate optional input-supply view for USB ONLINE, and optional
present/health fields if these are published; do not turn USB into SOC/charging.

## 3. VM-BMS / current: distinguish software feed from hardware

Mainline `pm8916_bms_vm` exports voltage, last OCV, health, status, **not measured
current or capacity**. Status infers charging from `power_supply_am_i_supplied()`.
OCV is reliable only in off/suspend conditions; reference driver expires boot OCV
after180s. Probe writes sampling/FIFO/mode controls; suspend forces S3 via debug
registers. It is not a drop-in passive observer or stock SOC algorithm.

**Exact retained stock kernel evidence:**

- **Correction (supersedes the earlier swapped labels):** the fitted
  `power_supply_attrs` table at **0xc15e3874 has 16-byte entries**. Read each
  entry's first little-endian pointer and its NUL-terminated string; do not
  import enum values from comparison source. Entry14 at **0xc15e3954** points
  to **0xc1295312 `voltage_ocv`**; entry17 at **0xc15e3984** points to
  **0xc12a62d0 `current_now`**; entry44 at **0xc15e3b34** points to
  **0xc1225ae3 `capacity`**. Neighbor entries12/13/15/16/18 are voltage_now,
  voltage_avg, voltage_boot, current_max, current_avg, corroborating stride.
- `qpnp_vm_bms_power_get_property` **0xc0a7ef54**, dispatch table
  **0xc0a7ef98**: property14 target **0xc0a7f134** loads `[chip+0x68]`,
  therefore **VOLTAGE_OCV**. Setter compares14 at **0xc0a7f398**, branches
  at **0xc0a7f39c** to **0xc0a7f3e0**, stores at **0xc0a7f3fc**, schedules work.
- **CURRENT_NOW is property17**: dispatch entry **0xc0a7efdc** targets
  **0xc0a7f13c**, instruction `e5950030` loads `[chip+0x30]`. Setter compares17
  at **0xc0a7f3a0**, loads caller integer at **0xc0a7f3a8**, and stores via
  `e5850030` at **0xc0a7f3ac**, then returns success. This establishes a writable
  cached current field; it does **not** establish a hardware current sensor.
  The earlier statement that c0a7f134/+0x68 was a cached *current* feed was wrong;
  it was the cached **OCV** feed. No runtime code depended on that audit label.
- CAPACITY property44 dispatch **0xc0a7f194** uses reported SOC or calls
  `report_vm_bms_soc` **0xc0a7fc28**; exact stock code has fallback50 and special
  battery-ID fallback67 paths. Neither is acceptable new telemetry.
- `estimate_ocv` **0xc0a81670** reads VADC channel6 five times with20ms delays and
  selects the minimum. This is a terminal-voltage boot fallback, not evidence of
  a current sensor or a true relaxed OCV measurement.
- `vm_bms_read` **0xc0a817d4**, open **0xc0a81964**, release **0xc0a819bc** and
  `bms_fops` **0xc0f893f4** prove a character-device sample path. Reference driver
  provides FIFO/accumulator voltage records on `/dev/vm_bms` and receives updated
  software current/OCV via power_supply properties. The accumulator is voltage
  accumulation, not demonstrated coulomb counting.

Stock DT contains no configured IADC node/current-sense channel in this retained
board tree. VADC USB voltage/VSYS/VBAT channels measure voltage, and a 1/3 divider
is not a current shunt. The stock image contains symbols for several unrelated
SMB/QG drivers; compiled-in symbols do NOT identify fitted hardware. Therefore
there is **no qualified direct battery-current measurement path yet**, not a
proof that the physical platform can never measure current.

**Exact userspace producer found (offline C1 followthrough):** retained
`.worktrees/s22-outer-display-impl/.private/vendor-stock/bin/vm_bms` in the reversal
repository is ARM32 Android, BuildID `9c2b53129881116bc10fd5dcf51caed1`, SHA256
`ef79e2df75b18a6054995e11264ce145f23ce949576df9e0ded3351729288b86`.
The earlier smaller subset's missing-executable blocker is superseded. Its
embedded XZ `.gnu_debugdata` recovers `write_int_to_file`0x23d4,
`lookup_slope`0x2494, `lookup_rbatt_sf`0x2548, `lookup_soc`0x25fc,
`lookup_fcc`0x26b8 and main0x276c. Decode **Thumb**, not ARM; code file
position=VA-0x1000. Private binary/disassembly remains outside Git.

At **0x3288–0x3290**, main resolves `/sys/class/power_supply/bms/` and
`current_now`, passes integer r5 in r2, and calls `write_int_to_file`0x23d4.
The processing path **0x3258–0x3268** forms r5 by floating-point multiplication
`s29*s28`, converts with `vcvt.s32.f32` and transfers to r5. The no-sample path
0x326e sets r5=0. The debug log at0x3238 labels s29 `ibatt`; this is corroborating
label evidence, not a completed derivation of units/sign/measurement. The
upstream FIFO/model arithmetic and scaling constant's provenance still require
tracing before publishing current. It is **not yet a qualified measurement**.
The executable also opens `/dev/vm_bms` and `/dev/battery_data`; exact init/property
startup ownership remains unresolved. Kernel `monitor_soc_work` passes cached
OCV(+0x68) to `lookup_soc_ocv`0xc0a7f92c, which also reads current(+0x30) for its
load correction. This is not a simple VBAT-to-percent mapping.

**Regression guard:** `battery-stock-evidence/property_guard.rs` is a standalone
Rust offline evidence checker, not linked into the HAL. It reads the exact
16-byte table names, getter jump targets/loads, and setter compares/branch/stores
from an explicitly supplied retained image. Synthetic negative tests reject
swapped names, wrong dispatch/load/store and truncated input. Reproduce:

```sh
rustc --edition=2021 --test docs/battery-stock-evidence/property_guard.rs -o /tmp/property-guard-tests
/tmp/property-guard-tests
rustc --edition=2021 docs/battery-stock-evidence/property_guard.rs -o /tmp/property-guard
shasum -a 256 /path/to/vmlinux.bin # match primary evidence hash above
/tmp/property-guard /path/to/vmlinux.bin
```

Continue C1 with exact producer math, FIFO layout, battery_data ioctls, sign,
units, cadence and invalid-input branches. Comparison Lineage code remains
non-fitted reference. Check board schematic/PMIC subtype and device inventory for
actual IADC/shunt or external gauge separately. No new monitor, service or
runtime change is authorized by these findings.

## 4. Capacity: Q2805 model exists, not a percentage license

Stock DT phandle0x27a points to batterydata with fitted `Q2805`,100k ID,
nominal2000mAh, default resistance222mΩ, capacitive50mΩ, termination90000µA,
max4350000µV, cutoff3400000µV. Tables retained at DT11868 onward:

- `fcc-temp-lut`: five temperatures -20,0,25,40,60°C;
- `pc-temp-ocv-lut`:31 percentage rows ×5 temperatures;
- `rbatt-sf-lut`:30 rows ×5 temperatures;
- `ibat-acc-lut`:6 current rows ×3 temperatures.

This is substantially more than a linear voltage lookup, but a table alone does
not qualify its use against live loaded VBAT. DT VM-BMS also requests reported
SOC, aging compensation, charging FSM behavior, and boot/shutdown SOC policy.
Reference driver consumes temperature/FIFO/OCV, uses resistance/current/load
corrections and reported SOC smoothing. Stock battery model max differs from
VM-BMS's own max4200000 and charger4325000; preserve this discrepancy as evidence,
never copy model settings into charging control automatically.

**Next patch M1 (offline, no device):** lossless extraction of Q2805 tables with
source hash/phandle, signed temperature legends, dimensions and units; test
reference interpolation against exact stock `interpolate_*_bms` functions
(0xc0a828b8 onward). Recover `report_vm_bms_soc` and callers, sample protocol,
vendor current feed and saved-state validity/age. Golden vectors must include
negative temperatures, low SOC, load transitions, charging, missing current,
invalid saved state, battery replacement, and aged pack. Never import the 50/67
fallbacks as valid readings.

**Next patch M2 (explicit approval/qualification):** only after current/OCV/temp
inputs and algorithm provenance are understood, implement a named model-backed
estimator inside the existing provider module/lifecycle, not a daemon. Distinguish
estimated SOC from direct kernel capacity with model ID/hash, input freshness,
confidence/validity and provenance. Current BatterySample's power_supply path
provenance is insufficient for that model: extend it deliberately before any
estimated percent. Under the current no-voltage-to-SOC instruction this document
proposes research/tests only; it does **not authorize publishing model SOC**.

## Priority delivery order

T1 fitted temperature table/scaler → S1 passive presence/input status → S2
qualified charger/comparator mapping → passive OCV read/validity investigation →
C1 stock estimator and direct-current hardware identification → M1 model/protocol
recovery → separately approved M2 estimator. None requires enabling mainline
charger policy or blindly porting stock VM-BMS probe. Until each is qualified,
optional HAL fields remain unknown rather than fabricated. All userland work
continues through the existing combined provider, with no additional service.
