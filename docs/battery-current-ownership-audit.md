# Stock current: execution ownership and bounded alternate-writer audit

Offline follow-up to `battery-vm-bms-current-producer.md`, reversal-oead.2.
No new acquisition, phone, builder, execution of vendor binaries, runtime/HAL
changes or percentage implementation. Mathematical scaling to µA is **not**
physical accuracy. This file supersedes only the earlier *init owner unknown*
statement; it does not change the producer equation or qualify current accuracy.

## Retained init ownership: two names, one executable

Private root `V=reversal/.worktrees/s22-outer-display-impl/.private/vendor-stock`.
Search **with `--hidden --no-ignore`**: ordinary rg respects ignore rules and
previously missed these private init files. This is a search-coverage correction,
not evidence that new files appeared.

- `V/etc/init/hw/init.qcom.rc:31` imports
  `/vendor/etc/init/hw/init.target.rc`.
- `init.qcom.rc:825–829`: **service vm_bms /vendor/bin/vm_bms**,
  `class main`, `user root`, `group system wakelock`; neither `disabled` nor
  `oneshot` in this stanza. Thus it is configured for main-class startup and
  normal init restart semantics. The retained vendor subset does not include
  the root init action that starts the main class; don't invent its exact time.
- `init.target.rc:303–307`: **service vendor.vm_bms /vendor/bin/vm_bms**,
  same class/uid/groups, also neither disabled nor oneshot. It additionally has
  an explicit `start vendor.vm_bms` in the **on charger** action at:245–252.
  These are distinct init service names targeting the same file, not proof of
  concurrent processes. Potential dual ownership is a hazard, not an observed
  race or a new daemon to reproduce.
- `V/bin/init.qcom.sh:72–81`: `start_vm_bms()` checks `/dev/vm_bms`, chowns
  current_now and voltage_ocv to root.system, chmods0664 and calls `start vm_bms`.
  It changes permissions; it does **not** write a current value. Calls at:256
  and:316 are under msm8916 and msm8909 cases. Retained device platform is
  **msm8937**, so those calls do not establish this device's startup trigger.
  `init.qcom.rc:702` defines qcom-sh as late_start root/oneshot.
- `V/ueventd.rc:315`: `/dev/vm_bms 0660 system system` is device-node access
  configuration, not a producer start or proof of measurement.
- The executable's `ctl.stop` target string is `vendor.vm_bms`; that name is
  **not** a reliable way to infer which of the two service identities launched
  a process. Some failure paths could stop a different service identity.

### Historical execution corroboration (not a new live observation)

Retained `reversal/cat-s22/out/20261025T214920/props/all.txt`:

```text
181 [init.svc.vendor.vm_bms]: [stopped]
184 [init.svc.vm_bms]: [running]
290 [init.svc_debug_pid.vendor.vm_bms]: []
293 [init.svc_debug_pid.vm_bms]: [748]
436 [ro.board.platform]: [msm8937]
```

This supports **historical init-reported execution under `vm_bms`**, PID748 at
that snapshot, not under `vendor.vm_bms`. It is not a /proc executable hash,
continuous liveness proof, evidence of successful writes, present-phone state,
or current freshness. It does not prove which init event launched the service.

SHA256 identities:

| Retained file | SHA256 |
|---|---|
|init.qcom.rc|8762fb331352fc6f54c938cf4663a54104023e4b325c625422b19e23070578fc|
|init.target.rc|0ca9d0af53b97f07f3c2a4ae42c0882274592752ce3f3b45ecaead443bfabbbd|
|init.qcom.sh|8b9661bc5814a3cf8010dfc0f8ede27f67513cb8942457d42103a0ea7561df66|
|20261025T214920/props/all.txt|790c2683f3525c7831b8f5fd8f1e81ad22f51bf2bb388f5a9d06853e4c58e84d|

## Alternate CURRENT_NOW writers: findings and limits

Full retained vendor tree literal scan:
`rg -l -a --hidden --no-ignore current_now "$V"` yields six files:

1. `bin/vm_bms`: exact model-current writer already traced.
2. `bin/init.qcom.sh`: permission changes only (above).
3. `bin/hvdcp_opti`: `current_now` string and both read/write imports;
   **unresolved candidate**, not a proven writer. It also contains unrelated
   charger-control/current-sense strings. Do not infer a bms write from imports,
   log text or a matching attribute basename.
4. `bin/hw/android.hardware.health@2.0-service`: `%s/%s/current_now` string;
   **unresolved candidate**, likely telemetry role but reader-only is not
   established by this scan.
5. `lib/egl/libGLESv2_adreno.so`.
6. `lib/hw/vulkan.adreno.so`: graphics occurrences are not proven battery
   writers (nor fully audited merely because they are graphics libraries).

Candidate binary SHA256:
`hvdcp_opti` 30ca60b2b433ec9ef1ce02ae83624a6b6086fa1d1bf5d984f2b81d5c6b6b800c;
health service 5ea5c9cd9985bcb7c89c07dce0ad3f69658283cac2a571afe9db460f850926c7.
No retained `.rc` or `.sh` numeric `current_now` write was found. This excludes
neither constructed paths, indirect calls, absent system/ramdisk files nor shell
commands outside the retained corpus.

Retained `V/lib/modules/*.ko`: no literal current_now/vm_bms/vbms matches.
Undefined-symbol scan finds **two** modules importing power_supply_set_property.
ARM relocation/call-site inspection (private disassembly) shows:

| Module | Relocated call text offset | r1 property integer |
|---|---|---|
|audio_mbhc.ko|0x3c00 and0x3c7c|114 (`mov r1,#114`)|
|audio_q6.ko|0x31158|81 (`mov r1,#81`)|
|audio_q6.ko|0x34d38|80 (`mov r1,#80`)|

These four direct imported calls are **not property17**, so they do not qualify
as alternate CURRENT_NOW writers. No reference enum names are assigned to those
other integers. This is not a complete kernel-wide writer or indirect-call audit.
The fitted kernel setter17 accepts external writes; DAC/SELinux configuration
and imported setters establish access possibilities, not an executed write.

## Freshness / invalid-input offline cases

Extended existing standalone `current_math_guard.rs` without linking it to HAL:

- Byte-check publication gate0x3272–76 and write-result compare/branch0x3294–98,
  in addition to the already checked arithmetic/no-sample-zero/write instructions.
- Abstract cache cases: no samples/no OCV change skips write; failed write leaves
  prior value; no samples with OCV change can publish zero. These test cache
  consequences, **not an init/sysfs emulator**. They cannot infer observation age
  from an unchanged integer or qualify zero as measured zero.
- Synthetic zero denominator with zero numerator -> NaN, nonzero numerator ->
  infinity; zero FCC can invalidate recursive OCV. No exceptional float is cast
  to an integer: host cast saturation is not ARM VFP exception behavior.
- Existing sign/truncation/last-entry vectors remain synthetic, ordinary-input
  binary32 checks, not measurement accuracy or independent physical sign tests.

Remaining bounded evidence gaps: root init main-class trigger; process executable
identity at historical snapshot; hvdcp/health candidate dataflow and any constructed
paths; kernel-wide direct/indirect writers; short read/count bounds, stale sequence
handling, restart cache lifetime, and exceptional ARM conversion behavior. None
permits a HAL measured-current field or voltage-derived percentage fallback.
