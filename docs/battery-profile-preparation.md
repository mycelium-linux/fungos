# Profile port preparation — live capacity HOLD

## Local scaffold receipt

`src/battery_profile.rs` contains original pure Rust structural validation for
owned row-major imports (OCV, FCC, rbatt, ACC) and explicit QualifiedOcv /
QualifiedTemperature inputs carrying qualifier receipts and monotonic timestamps.
No IO, DTS parser, numeric Q2805 import, interpolation implementation, generic
bilinear approximation, 25C default, current guess, provider integration, or
voltage_now-to-capacity conversion. `estimate_capacity` fails closed with
UnverifiedAlgorithm even with fresh explicitly supplied inputs. Qualification
receipt structs are preliminary trusted-boundary declarations, NOT hardware
qualification proof. Final producer contract remains capacity audit owner's.

Structural checks cover dimensions/checked products, strict temperature order,
descending percent rows and OCV100..0 endpoints, nonzero OCV/FCC/rbatt values,
ascending ACC current axis. They do NOT assert fitted parser parity, exact
selection, physical calibration or interpolation semantics. No unverified
fetched ACC maximum is encoded.

Validation: cargo check, full47tests (32lib+12binary+3integration), strict
all-target Clippy -Dwarnings without allowances, fmt and diff check PASS.
No static rebuild needed: live artifact path
`target/battery-only/aarch64-unknown-linux-musl/release/fungos-keyboard-lightd`
still SHA256938bb6c4b1c7ebc55b7fa54167617cd2fb23e8d1da95a247e1fe77bab3c2e508.
No phone, build server, kernel changes, commits or pushes.

## Evidence, explicitly separated

DT inspected read-only:
`/Users/ajmwagar/Documents/src/reversal/cat-s22/out/resume-20261029/lk2nd-stock-live.dts`
Q2805 child at11869 under `/qcom,batterydata` at11864, phandle0x27a.
Parent/profile audit reports enabled BMS reference line9708 and sole child.
Table shapes: FCC1x5, OCV31x5, rbatt30x5, ACC6x3. Temperature Celsius axes
[-20,0,25,40,60], ACC first3. OCV percent row legend100..0 with nonuniform
low-SOC spacing. Selection/identity of physical installed battery still needs
its own evidence, not inferred from table availability.

Reference-only fetched helpers in `/tmp/cat-soc-stock49/`:
source-identity.log records msm-4.9 c348797f41abb995dbdac36d2c6e8324202244d7.
batterydata-lib.c SHA256
d959bb8e6f118e2b3dbead209e1206795bee4e05369ff06f7abc2293a90b8dfa.
Header and helpers GPLv2-only, not approval to copy source into this crate.
No source algorithm copied. DEGC_SCALE10 means deciC input vs Celsius axis;
exact inverse OCV branching must not be replaced by generic bilinear lookup.

### Parent-supplied fitted binary receipts (not independently reproduced here)

Latest user binary evidence supersedes earlier interpretation of names/layout:
- Actual `kallsyms-fixed.txt` contains `*_bms` symbols. Unsuffixed retained
  qpnp-vm-bms.c caller names show reference-source mismatch, not binary mismatch.
- `c0b6084c mov r1,#3` (ACC columns), `c0b60850 mov r2,#6` (maxrows),
  `c0b60858 bl of_batterydata_read_lut` at `c0b60b24`: fitted parser accepts
  ACC6x3. Actual ACC data offset44 = header20 + 6*4; fetched max4 implies
  offset36, WRONG fitted layout. Do not import fetched struct limits/layout.
- Actual OCV helper `c0a83064` inspected control flow matches fetched inverse
  lookup branches including zero sentinel/100 fallback. This is a selected
  function observation, NOT wholesale source/binary equivalence or approval
  of exceptional arithmetic behavior, parser bounds, selection or licensing.

Profile audit owner `66:pi-s22-soc-profile`; capacity/validity owner
`66:pi-s22-reset-semantics`; HAL sole writer in device-hal. Await their exact
contract/receipts before porting additional verified pieces.

## Private raw contract validation received

Read full owner report
`/Users/ajmwagar/Documents/src/reversal/.worktrees/s22-battery49/docs/cat-s22-q2805-profile-audit.md`.
Raw contract at that worktree `.private/stock49-profile/q2805-raw-profile.json`,
SHA25643cb84dab4fb4f30cb3f47fc27e21c4aaab6e4ede440986dc47b07f0e4dea0ff,
verified locally. It explicitly sets runtime_qualified=false. Private generated
harness `/tmp/device-hal-stock-audit/private/q2805-import-check.rs` imports this
crate's actual battery_profile.rs and passes exact row-major values through
ProfileImport/ValidatedProfile. PASS FCC1x5, OCV31x5(155), rbatt30x5(150),
ACC6x3(18). No raw table numbers committed/copied into public source; no new
runtime data loader or provider integration. This proves structural acceptance,
NOT physical selection/calibration, interpolation or final reported SOC.

Owner report includes actual allocation116/data44/maxrows6, helper branch and
edge asymmetry/zero-sentinel/100fallback evidence. Static inspected branches are
not executed stock differential testing. Fetched GPLv2-only helpers remain
reference-only; exact raw DT redistribution license NOT established. Keep raw
private, no source copy/translation/permissive-license assumption. No algorithm
implementation is authorized by receiving this data alone.

## Private executed reference oracle received

Read full private owner receipt at
`/Users/ajmwagar/Documents/src/reversal/.worktrees/s22-battery49/.private/stock49-profile/inverse-oracle/RECEIPT.md`.
Owner reports97,543 executed comparisons:94,222 inverse-PC and3,321 linear,
all matching compiled unmodified retained reference C. All seven inverse path
classes covered, including synthetic legacy-fallback100; Q2805 tested vectors
had no such fallback, which does NOT prove globally impossible. C runs UBSan
without signed overflow; Rust separately rejects overflow/malformed shapes.
Private Rust translation GPL2-only, not imported here. No numeric vectors/source
copied into HAL. No emulator available: NOT fitted-binary execution/oracle;
c0a83064/c0a828b8 remain statically inspected correspondence only. Full stock
SOC/current/cutoff/ACC/FCC/rbatt state and input qualification remain outside
this inverse-only receipt. Live capacity and qualification remain HOLD.

## Integration gates

1. Import raw exact Q2805 tables only after audit's final data/selection contract;
   retain source identity, dimensions/units/row order and exact-value tests.
2. Obtain fitted inverse lookup arithmetic/branch/rounding/bounds proof and
   license-compatible implementation direction. Use independently specified
   behavior/tests, no unapproved GPL source copy or parity overclaim.
3. Finalize qualified OCV AND measured temperature producer contracts, freshness,
   reset/cache/validity and missing/error rules. No raw terminal-voltage fallback,
   fabricated temperature/current, or forged receipt implies qualification.
4. Test exact vectors/boundaries/failures and provenance. Derived model estimate
   must not be presented as measured SOC or accidentally conflated with kernel
   capacity. Any wire/provenance extension needs explicit reviewed contract.
5. Only then propose provider integration and distinct-target static build;
   parent explicitly approves deployment/end-to-end proof. Live capacity remains
   HOLD until qualified inputs and end-to-end proof exist.
