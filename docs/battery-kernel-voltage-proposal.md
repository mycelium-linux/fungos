# Kernel voltage-only publication proposal (parent-owned; not applied)

## Source audit and scope

Read-only audit of upstream **Linux v6.12**, not a claim that the parent's
builder uses these exact revisions. Compare the actual build source before
applying. Sources below are relative to
https://github.com/torvalds/linux/blob/v6.12/ :

- `drivers/power/supply/generic-adc-battery.c`, SHA256
  `723ccabe807bb8388bea4231ed53e0123cee5269d5267d79c8bea4fcc8cea3bc`
- `drivers/iio/adc/qcom-spmi-vadc.c`, SHA256
  `36cb256316f9bde086fe26d6fdea7badc08006f04a4279df6ac034da008471c1`
- `drivers/iio/adc/qcom-vadc-common.c`, SHA256
  `970d09649280f74c577a6f593b846bca6f3bc66e8d7273ee50efc66d02fc7bbd`
- `Documentation/devicetree/bindings/power/supply/adc-battery.yaml`, SHA256
  `05515cecd87c778dbfea43948b2b3e7b672ed3cb4faeea376242a17d6fd878cd`
- `Documentation/ABI/testing/sysfs-bus-iio`, SHA256
  `5321bffe260258e36e65ffbc757bfeb8c7acdf86dfe8f341d0a16050efce18f9`

Local evidence read: parent `cat-s22/docs/battery-telemetry-audit.md` records stock
VBAT_SNS channel 6, prescale 1/3, and earlier raw code 38960. No shared kernel,
phone, or builder access/mutation was performed. Parent is restoring the ADC-only
profile for .069; that is not evidence of battery power_supply publication yet.

## Do not wire the existing QCOM processed voltage straight to generic-adc-battery

`gab_read_channel()` (lines 86–99) calls `iio_read_channel_processed()` then
multiplies the result by **1000**. Conventional IIO voltage units are mV;
power_supply voltage units are µV. However QCOM `vadc_read_raw(PROCESSED)`
(lines 456–472) returns `IIO_VAL_INT` with the result of `qcom_vadc_scale()`.
Its SCALE_DEFAULT voltage path (`qcom_vadc_scale_volt`, lines 408–422) returns
**µV**, with no division by 1000. This is a provider ABI mismatch, not a reason
for userspace to auto-detect units.

For example a QCOM result of 4,000,000 µV becomes 4,000,000,000 in generic ADC
battery, exceeding signed 32-bit `intval`; neither the magnitude nor any wrapped
result is valid telemetry. Do not use a voltage plausibility clamp to conceal it.
Do not change all QCOM channels' units casually: existing IIO consumers/sysfs
behavior require a separate compatibility audit. The HAL expects standard
power_supply µV and intentionally does not divide surprising readings by 1000.

Generic battery also always advertises STATUS (line 66), initializes it to
DISCHARGING (175), then uses `power_supply_am_i_supplied()` and optional charged
GPIO to infer discharging/not-charging/charging (134–138). An absent supplier is
not measured discharge; USB voltage is not measured charging. This is unsuitable
for the currently qualified ADC-only board. DT binding requires
`monitored-battery`; its usual simple-battery description can require model
parameters that are not needed to measure terminal voltage. Do not invent those
parameters to satisfy the generic binding. The C driver itself does not estimate
capacity or explicitly call `power_supply_get_battery_info()` in this version.

## Minimal source-backed VBAT processed extension

Upstream's table contains:

```c
VADC_CHAN_NO_SCALE(VBAT_SNS, 1)
VADC_CHAN_VOLT(VSYS, 1, SCALE_DEFAULT)
```

The narrow proposed table change, **only after parent confirms stock channel 6
really is VBAT_SNS with the documented divider/calibration**, is:

```diff
- VADC_CHAN_NO_SCALE(VBAT_SNS, 1)
+ VADC_CHAN_VOLT(VBAT_SNS, 1, SCALE_DEFAULT)
```

This preserves RAW and adds PROCESSED using the same voltage scaler as VSYS.
Prescale index 1 is 1/3; DT must retain explicit `qcom,pre-scaling = <1 3>` and
absolute calibration (not ratiometric) for this path. No fixed raw-to-voltage
constant or SOC curve is needed. `vadc_measure_ref_points()` populates absolute
`dx=625000 µV`, `gnd=raw_625mV`, `dy=raw_1250mV - raw_625mV`.
The scaler performs signed 64-bit arithmetic and integer division:

```
v_adc_uv = trunc((raw - gnd) * 625000 / dy) + 625000
v_adc_uv = max(v_adc_uv, 0)
v_terminal_uv = trunc(v_adc_uv * 3 / 1)
```

Use live calibration codes from the driver, not previously reported *processed*
reference voltages. Raw38960 alone cannot determine voltage. Validate dy positive
and nonzero and audit saturation/calibration failure in the actual target source.
The current upstream reference setup rejects equality, not every bad calibration.
Do not double-apply the divider in either consumer or DT.

## Recommended voltage-only power_supply glue

Prefer a narrowly scoped, explicitly bound kernel IIO consumer over the current
generic helper for this experiment. It is not a charger/fuel-gauge replacement
and does not introduce a userspace daemon. A provider-specific compatible/binding
must explicitly document the legacy QCOM processed-µV input; do not pretend it is
a unit-generic `adc-battery` or silently reuse that binding with different units.

Minimal intended behavior (proposal, not compiled kernel code):

- Acquire exactly the board's VBAT IIO channel using `devm_iio_channel_get()`;
  propagate deferred probe/errors and ensure it is a voltage channel. Do not
  accept arbitrary VSYS/USB channels or fallback to raw codes.
- Register one `POWER_SUPPLY_TYPE_BATTERY` via `devm_power_supply_register()`.
  Set `cfg.drv_data` and DT node; optionally expose SCOPE=SYSTEM.
- Properties: **VOLTAGE_NOW only** (plus optional SCOPE). No STATUS, CAPACITY,
  PRESENT, HEALTH, temperature, technology/model, design capacity/voltage, or
  current unless separately measured/qualified. Type Battery describes the
  measurement target, not detection of a physically present pack.
- VOLTAGE_NOW calls `iio_read_channel_processed()`, propagates negative errors,
  rejects negative voltage, and passes the qualified QCOM µV integer through
  **unchanged**. No multiply by 1000, guessed scale, or constant fallback.
- No set_property/property_is_writeable callbacks, GPIOs, supply-status inference,
  charger/BMS register access, timer, workqueue, or IRQ is necessary for on-demand
  reads. ADC conversions/calibration still write ADC registers; this is
  charger-control-read-only, not literally PMIC-write-free.
- Use devm cleanup; no separate lifecycle or new userspace authority. A future
  upstream unit-correct IIO provider can replace the explicit legacy-unit boundary,
  but both sides must change together and be tested, not runtime guessed.

The HAL in this tree already accepts that exact standard output: `type=Battery`
and `voltage_now` produce `percent=None`, `charging=Unknown`, optional voltage,
provenance and monotonic observation time. Absent status/capacity is intentional.
No HAL IIO fallback should be added, and no charger/BMS enabling is needed.

## Parent qualification gates

1. Compare pinned source paths/units with actual .069 source; preserve the restored
   ADC-only config/DT and disabled charger/BMS until explicitly reviewed.
2. Before publication, test the scaler with known raw calibration vectors: raw at
   625mV/1250mV references maps to 1,875,000/3,750,000 µV at divider 1/3;
   midpoint rounds according to integer division. These are arithmetic tests, not
   claims about observed pack voltage. Test conversion errors and bad calibration.
3. Confirm VBAT sysfs RAW still works and added PROCESSED is µV by source-backed
   comparison, not merely plausible magnitude. Compare a safe independent terminal
   measurement if parent can obtain one. VSYS is not a substitute for VBAT.
4. Confirm the supply exports voltage only, no invented status/capacity; conversion
   failure must produce read error, never cached/zero/default success. Exercise
   unbind/rebind and probe deferral. Confirm charger/BMS still disabled.
5. Run the combined Rust provider against the resulting sysfs, expecting optional
   voltage only. Hardware support remains unqualified until those observations;
   .068's empty power_supply still correctly returns Unavailable.
