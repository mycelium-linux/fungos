# Exact stock `vm_bms`: current producer followthrough

Offline continuation for **reversal-oead.2**, scoped to current provenance. This
file does not modify the parent audit or any HAL/runtime code. No phone, builder,
PMIC transaction, deployment, commit or push. No SOC/percentage is implemented.

## Evidence identity and interpretation

Retained private vendor executable:
`reversal/.worktrees/s22-outer-display-impl/.private/vendor-stock/bin/vm_bms`.
SHA256 `ef79e2df75b18a6054995e11264ce145f23ce949576df9e0ded3351729288b86`,
BuildID `9c2b53129881116bc10fd5dcf51caed1`. ARM32 Android executable, **Thumb**
code. `.text` VA0x21ec/file0x11ec; for code below, file offset=VA-0x1000.
Embedded `.gnu_debugdata` identifies helper names. Code and literal pools must
be separated: ordinary ARM disassembly and Thumb decoding of pool data both
produce misleading instructions. Binary and disassembly stay private in
`/tmp/device-hal-stock-audit/private/`.

Exact kernel: `reversal/cat-s22/out/vmlinux.bin`, base0xc0008000, SHA256
`86a2a3094fc018db47a7dfa8723913791cf0cf339546e616006cf5b52a776680`.
Symbol names from retained `kallsyms-fixed.txt`. Public comparison C is not the
fitted source and is not the authority for enum numbers or struct offsets.

**Result:** the traced userspace writer produces a **voltage/model-based current
estimate**, not a direct current-sensor sample and not a charger current limit.
Its signed output has the dimensional interpretation **µA**, positive for
OCV greater than sampled terminal voltage under a positive denominator (the
stock discharge convention). This proves this writer's provenance, **not**
accuracy on the present battery, actual service execution, an independently
measured sign, absence of every other writer, or absence of all possible hardware
current sensors. Do not expose it as measured current or port it into the HAL.

## Correct property chain (do not regress to reference enums)

Fitted `power_supply_attrs`0xc15e3874 has **16-byte** entries, first word a string
pointer. Entry17 at0xc15e3984 ->0xc12a62d0 `current_now`; entry14 at0xc15e3954
->0xc1295312 `voltage_ocv`. The existing standalone `property_guard.rs` verifies
these names, getter dispatch/loads, and setter dispatch/stores against kernel
bytes. Earlier +0x68/current labels are explicitly superseded.

- CURRENT_NOW17 jump-table word0xc0a7efdc -> getter0xc0a7f13c, `ldr r0,[r5,#48]`:
  cached chip+0x30. Setter17 at0xc0a7f3a8 loads the caller integer and
  0xc0a7f3ac stores chip+0x30. No ADC read occurs in this property getter.
- VOLTAGE_OCV14 -> getter0xc0a7f134, chip+0x68. Its setter stores at0xc0a7f3fc
  and schedules SOC-monitor work. This is the feed previously mislabeled current.
- Vendor0x3288–0x3290 passes `/sys/class/power_supply/bms/`, `current_now` and
  integer r5 to `write_int_to_file`0x23d4. At0x23f4 it passes r2 to formatting;
  helper0x2428 resolves `%d` at0x243e and calls vsnprintf PLT0x3610 at0x2440.
  The resulting signed decimal string goes to `write_string_to_file`0x238c,
  then `write_to_fd`0x245c. Negative values are not forced positive by formatting.
- PLT identities are corroborated by `.rel.plt`: ioctl at0x3630, read0x36d0,
  vsnprintf0x3610, __write_chk0x3620. Embedded helper names alone are not enough.

## `s29 * s28`: recovered dataflow and equation

Register names below are VFP single precision unless stated. Constants are exact
IEEE754 binary32 pool values (not inferred decimal labels):

| Register | Load VA | Pool VA | Value |
|---|---|---|---|
| s18 |0x2a7e|0x2bf4|1000.0|
| s21 |0x2a82|0x2bf8|3600.0|
| s26 |0x2a86|0x2bfc|10000.0|
| **s28** |**0x2a8a**|**0x2c00**|**1000000.0**|
| s30 |0x2a8e|0x2c04|100000.0|

For one processed FIFO entry (or the final accumulator entry), define:

- `E = signed(OCV_uv)/1e6`, volts; stack+0xb4, fetched by get_bms_ocv0x3460
  from `bms/voltage_ocv`, then updated recursively inside this loop.
- `V = signed(sample_uv)/1e6`, volts; r7 holds the selected voltage value.
- `R = (rbatt_mohm_adjusted + resistance_capacitive_mohm)/1000`, ohms.
- `F = adjusted_fcc_mah/1000`, Ah; `C = F*3600`, ampere-seconds.
- `L = signed(slope_ioctl_result)/10000`, volts per unit fractional SOC.
- `T = signed(sample_interval_ms)/1000`, seconds.
- `N = signed(sample_count)` for FIFO; accumulator uses its accumulator count.
- `D = ((T*L)*N)/C`, ohms dimensionally.

The ordered instruction chain at **0x3138–0x31ac** computes:

```text
s2  = float(sample_interval_ms) / 1000       # 0x313c,0x3148,0x3150
s31 = float(slope_result) / 10000            # 0x3138,0x3154,0x315c
s0  = rbatt_adjusted + float(R_cap_mohm)     # 0x314c,0x3160,0x3168
s16 = float(sample_uv) / 1000000             # 0x3140,0x3164,0x3170
s19 = s0 / 1000                             # 0x3174 => R
s2  = ((s2*s31)*float(sample_count)) / s27   # 0x316c,0x317c,0x3184,0x3190 => D
s20 = float(OCV_uv) / 1000000                # 0x3158,0x3178,0x3188 => E
s0  = (s20-s16) + (s20-s16)                 # 0x318c,0x3194
s4  = (s19+s19) + s2                        # 0x3180,0x3198
s29 = s0 / s4                              # 0x319c => I in A
s20 = s20 - s2*s29                          # 0x31a0 => updated model OCV
OCV_uv = trunc_i32(s20 * 1000000)            # 0x31a4–0x31b4
...
r5 = trunc_i32(s29 * s28)                   # 0x3258,0x325e,0x3268
write_int_to_file(bms_root, current_now, r5) # 0x3288–0x3290
```

Thus, algebraically (not a claim of identical floating-point reassociation):

```text
I_A = 2*(E_V - V_V) / (2*R_ohm + D_ohm)
E_next_V = E_V - D_ohm*I_A
current_now_uA = trunc_i32(I_A * 1000000)
```

At0x2f22–0x2f2e, `s27=(s24/1000)*3600`, where s24 is adjusted FCC in mAh.
At0x3238 debug formatting labels s29 `ibatt`, s19 `rbatt`, s31 `slope`, s16
`fifo_avg`, s20 `ocv`, and d12 `fcc_ah`. These labels corroborate, rather than
replace, the instructions and dimensional analysis. `s29` is recomputed for each
entry; only the **last processed entry's** current is written, not a batch mean.
The recursive integer OCV state truncates to µV before the next iteration.
The OCV is updated for each entry and used on the next iteration.

For finite ordinary operands and positive `2R+D`, `E>V` produces positive current,
`E<V` negative current, and equality zero. There is no abs() or clamp to positive
in this chain. `vcvt.s32.f32` truncates toward zero for ordinary representable
values. NaN, overflow, FPSCR exception/status behavior and saturation are **not
qualified** by the scalar notebook; don't emulate them via Rust `as i32`.

## Where the inputs actually come from

### Voltage sample transport, not a current FIFO

Vendor0x2ab6–0x2abc calls read PLT0x36d0 on the `/dev/vm_bms` fd, requests
**56 bytes**, and places the record at context+0x38. Exact kernel
`vm_bms_read`0xc0a817d4 copies **56 bytes** from chip+0x3b8 at0xc0a8183c–58.
The matched relative layout is:

| Record offset | Vendor context offset | Value / evidence |
|---|---|---|
|0|0x38|fifo_count; iteration bound0x2f08|
|4..32|0x3c..0x58|FIFO voltage array (8 slots in matched layout)|
|36|0x5c|sample interval; loaded into sl0x2f60|
|40|0x60|FIFO sample count; selected0x2f50–58|
|44|0x64|accumulator voltage; selected0x2f48–4e|
|48|0x68|accumulator count; selected0x2f56–58|
|52|0x6c|sequence; loaded0x2ca0–a2|

Kernel `read_and_populate_fifo_data`0xc0a80ed0 reads PMIC BMS base+0xc0 onward
at0xc0a80fc4, converts each little-endian raw voltage through0xc0a8124c and stores
chip+0x3bc onward at0xc0a81210. It stores interval at chip+0x3dc, deriving it as
register byte*10 at0xc0a810cc–dc; sample count at+0x3e0 is derived from the
sampling field at0xc0a811c4–e4. Exact `show_bms_data` uses strings
`fifo_uv[%d]=%d sample_count=%d interval_ms=%d` and `acc_uv=...`. Accumulator
producer0xc0a8137c averages raw voltage, converts via the **same**0xc0a8124c,
then stores voltage+0x3e4/count+0x3e8 at0xc0a814b0–b4. This is voltage
accumulation, not a demonstrated charge/current integrator.

The voltage calibration helper also invokes VADC compensation. Its exact
calibration and runtime acquisition validity remain separate qualification gates.
Do not equate the FIFO's processing time with measurement freshness.

### Model resistance, FCC and slope

Vendor startup reads `bms/resistance` into context+12 at0x2902–10 and
`resistance_capacitive` into+16 at0x291a–26. For each sample,
`lookup_rbatt_sf`0x2548 uses `/dev/battery_data`;0x3090–9e computes signed
integer `(scale_factor * default_rbatt_mohm)/100` (integer truncation before
float conversion). Failed scaling lookup uses100 at0x3076, not a sensor value.

`lookup_fcc`0x26b8 uses battery temperature and `/dev/battery_data`; exact kernel
ioctl branch0xc0a83dd4–e0 calls `interpolate_fcc_bms`0xc0a8290c on the model FCC
LUT. Failed lookup uses **1500mAh** at0x2dba–0x2dce. Temperature read failure
uses **250 deci°C** at0x2d86–9a. These are stock fallbacks, not qualified inputs.

`lookup_slope`0x2494 sets ioctl **0xc0184203**, supplies SOC and temperature,
and retrieves output at request+16. Kernel corresponding branch0xc0a83d5c–6c
calls `interpolate_slope_bms`0xc0a83428 on the model's OCV table. In that
function0xc0a835b4–d4 the returned slope is **1000*delta_OCV_mV/delta_SOC_percent**
(with interpolation around temperature). Hence dividing by10000 yields V per
fractional SOC: `(1000*mV/percent)/10000 = V/fraction`. Nonpositive table voltage
delta is replaced by1 at0xc0a83598–b0; vendor failed ioctl uses1000 at0x2f74.
Those substitutions do not establish physical validity. rbatt lookup SOC is
clamped to at least2 at0x2f92–98. This is estimator/model plumbing, not current
measurement plumbing.

When aging compensation is supported (context+52), cycles are read from
`bms/charge_cycles`; `A=cycles*(31 if cycles<101 else 23)` at0x2cfc–0x2d04.
FCC uses `fcc*(100000-A)/100000` at0x2dea–0x2e0a, with the subtraction converted
as **unsigned**. Resistance uses `rbatt*(100000+A)/100000` at0x2f1c–3c and
0x30ea–f6. Cached adjusted values can be reused. Do not simplify this into a
safe aging law: wrap/overflow, extreme cycles and missing-cycle behavior need
explicit reconstruction before any port.

## Failure and publication boundaries

- No FIFO and no accumulator:0x326e sets r5=0;0x3272–76 may skip publication
  unless the OCV-change flag is set. A published zero on that path is **not** a
  measured zero. An earlier write can also remain cached in the kernel.
- OCV read failure falls back to previous OCV at0x2d40–5a; slope/FCC/temp/rbatt
  fallbacks above keep computing without an external validity tag.
- Read path only checks return >=1 at0x2ac4–c8, not exact56. Short-record behavior,
  record count bounds, per-entry freshness and error recovery are not qualified.
- No denominator/finite/range validation is visible in0x3138–0x3268. Do not
  assume all model/record inputs are valid or that the arithmetic cannot fault.
- Sysfs writes can fail; no-sample skipped writes and failed writes leave cached
  fields. Reading `CURRENT_NOW` alone cannot prove producer freshness.
- Exact init owner/trigger, live service execution, independent current comparison,
  fitted model selection during execution, OCV relaxation validity, aged/replaced
  battery behavior and every alternate kernel writer remain unqualified.

## Offline regression scope

`battery-stock-evidence/current_math_guard.rs` verifies selected Thumb instruction
bytes and literal constants against the explicitly supplied retained executable.
Its scalar, binary32 arithmetic tests check the recovered formula's dimensional
scales, sign, truncation and final-entry behavior using **synthetic inputs**.
These are not hardware vectors, an ARM/bionic emulator, a full vendor algorithm
port, kernel telemetry publication, SOC conversion or evidence of accuracy.

```sh
rustc --edition=2021 --test docs/battery-stock-evidence/current_math_guard.rs -o /tmp/current-math-tests
/tmp/current-math-tests
rustc --edition=2021 docs/battery-stock-evidence/current_math_guard.rs -o /tmp/current-math-guard
shasum -a 256 /path/to/private/vendor-stock/bin/vm_bms
/tmp/current-math-guard /path/to/private/vendor-stock/bin/vm_bms
```

Safe next work remains offline completeness: byte-check the remaining helper
and ioctl branches, resolve initialization/alternate writers and invalid-input
semantics. No voltage-to-percentage adaptation is proposed. Unknown HAL fields
remain unknown; any future estimated-current API needs explicit provenance,
validity/freshness and separate approval, not a disguised measured-current field.
