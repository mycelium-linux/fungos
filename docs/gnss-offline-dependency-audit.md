# CAT S22 bounded offline GNSS dependency audit — reversal-oead.7

2026-10-31. Offline retained artifacts only; **no GPS fix qualification**, satellite
visibility, service discovery, running modem, receiver health or RF isolation claim.
No phone/builder/network access, daemon, protocol implementation, firmware boot,
location transmission, shared kernel changes, commit or push. Battery followthrough
remains priority. Existing issue stays OPEN; audit does not assign runtime ownership.

## Sources and evidence

Paths below relative to `/Users/ajmwagar/Documents/src/reversal`:

- `cat-s22/out/resume-20261029/lk2nd-stock-live.dts`:
  lines10264+: MSS at0x4080000, `qcom,pil-q6v55-mss`, firmware-name `modem`,
  self-auth/sequential load, reserved memory, supplies/clocks and SMP2P handshake.
  lines10100+: vendor IPC router, modem SMD transport channel `IPCRTR`, link1,
  version1, fragmented data, `qcom,disable-pil-loading`. Transport declaration
  therefore does NOT establish that the modem is already booted or auto-loaded.
  SMD packet/TTY nodes also exist; none inspected establishes a standalone UART
  NMEA GPS device. Do not infer such a device from generic SMD tty presence.
- Stock vendor subset at
  `.worktrees/s22-outer-display-impl/.private/vendor-stock` (call this V).
  `V/etc/init/android.hardware.gnss@2.1-service-qti.rc`: stock GNSS HAL process
  `/vendor/bin/hw/android.hardware.gnss@2.1-service-qti`, class hal, user gps,
  groups system/gps/radio/vendor_qti_diag.
  `V/etc/init/hw/init.qcom.rc` lines318+: creates location data and socket
  directories, including mq/xtra/dgnss; lines697+ declares loc_launcher.
  This is a dependency inventory, NOT permission to launch these daemons.
- `file V/bin/hw/android.hardware.gnss@2.1-service-qti`: ARM32 EABI5 dynamic ELF,
  Android `/system/bin/linker` interpreter, not a native ARM64-musl utility.
  `llvm-readelf -d`: HIDL GNSS1.0/1.1/2.0/2.1, libhidlbase, Android utils/log/base,
  libc++ and bionic dependencies. Cannot just copy it into current rootfs.
- `V/lib/libloc_api_v02.so`: ARM32 ELF; dynamic dependencies include
  libqmi_cci, libqmi_common_so, libloc_core, libgps.utils and Android libraries.
  `strings` exposes qmiLocEventPositionReportIndMsgT_v02, reportNmea,
  qmiLocEventGnssSvMeasInfoIndMsgT_v02 and network-initiated request/assistance
  handlers. Evidence strongly supports Qualcomm QMI LOC v02 software path,
  not an independent generic Linux GPS driver. Presence does not prove live
  advertised service ID/version or any functioning fix.
- `strings V/lib/libqmi_cci.so`: qcci_ipc_router_ops AND qcci_qrtr_ops,
  QRTR supported/not-supported detection, QMUX transport registration symbols.
  These show compiled transport options, not which transport stock runtime used.
  Do not equate vendor IPC router/SMD with a proven mainline QRTR LOC endpoint.
- `V/etc/gps.conf`: MODEM_TYPE=1, NMEA_PROVIDER=1; SUPL_MODE=3,
  AGPS_CONFIG_INJECT=1, configured SUPL host/port and NTP server.
  Config includes /dev/pps0 but does not prove fitted PPS hardware or node.
  `izat.conf`, `lowi.conf`, `sap.conf` also retained; no Wi-Fi/cell network
  location, SUPL/XTRA/time fetch or assistance injection authorized. Never start
  stock defaults as a supposedly passive/offline GNSS test.
- The inspected V firmware subset has no modem/MBA image match and its
  firmware_mnt directory is empty. This is incomplete extraction evidence, NOT
  proof the device lacks firmware. `cat-s22/docs/wifi-scan-audit.md` records
  p1 PARTNAME=modem VFAT mounted read-only `/mnt/android-firmware` previously;
  no mount/read performed here and no GPS firmware selected/authenticated.
- `cat-s22/docs/mainline-cat-plan.md` line139 records missing upstream MSM8937
  MSS remoteproc node and modem firmware dependency. That older plan is not a
  verified inventory of current .080/kernel46; no claim .080 has/hasn't booted
  modem can be made from it. Existing ADSP/audio bringup is not MSS proof.

## Minimal next step / gates

1. OFFLINE first: identify exact modem/MBA firmware manifest/hashes from already
   retained modem-partition evidence (if available); compare current saved .080
   DT/config/boot logs for MSS remoteproc, reserved memory, firmware auth,
   regulators/clocks, SMP2P, SMD and QRTR support. Do not enable MSS as a side
   effect of microphone work. Missing evidence is a request for parent inventory,
   not permission for the worker to read phone partitions.
2. Parent alone, separately approved: passive inspection of existing remoteproc
   state/boot logs and already-running IPC service advertisements; no firmware
   start, session command, cellular registration, network lookup or RF activation
   in this step. Need verified LOCAL LOC service reachability before designing
   any HAL capability boundary or choosing an existing qualified client library.
3. Only with explicit later RF/privacy approval: qualify standalone no-assistance
   receiver session and satellite reports before a fix test. Modem boot may have
   cellular/RF side effects; airplane-mode assertions are not proof of isolation.
   Fix validity, age, uncertainty and no-fix semantics need their own evidence.
   No current location API exists in this HAL; no new daemon/protocol proposed.

Conclusion: retained stock chain is Android GNSS HAL/location stack → QMI LOC v02
→ selectable vendor IPC/QRTR/QMUX transport → modem/MSS firmware dependencies.
Exact runtime transport, live service, required firmware and all fixes remain
**unqualified**. This bounded audit closes no hardware bringup gate.
