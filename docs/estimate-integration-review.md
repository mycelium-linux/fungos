# Estimate integration review and exact remaining boundary

Read-only source review; no Soil edits, test execution/builds in Soil, service,
hardware/deployment or GPL math/data import. HAL client remains completed.

## Soil current use inspected

Worktree `soil/.worktrees/s22-runtime`:
- crates/soil-runtime/src/estimate.rs
- crates/soil-runtime/tests/estimate_capability.rs
- docs/soil-estimate.md

EstimateGrant constructs the frozen HAL client with host-owned socket/tuple
allowlist/max_age. Grant absence returns Denied before client.read. Host retains
Result<RemoteEstimate,...>, never guest-supplied ModelEstimate. Read, token
consumption and presentation use snapshot(max_age); one read/update budget,
single-use token, reset drops retained data. Unknowns hide numeric. No apparent
client API integration mismatch in inspected paths. No production estimate CLI,
issuer, real approved tuple config or running estimator endpoint is configured.

Existing tests inspected (not rerun here) cover actual Wasm fresh synthetic57/0,
all unknown reasons/errors, untrusted tuple, disconnect/malformed/recovery,
permission no contact, import separation, memory tampering/bounds, token replay,
read budget, actual owner pixel equality both dimensions, host retained expiry.
Soil receipt reports53tests/check/strictClippy PASS; that is owner's receipt,
not a new HAL-run claim. Metadata in UI currently literal Model estimate label;
full receipt tuple remains host-owned, not guest-authoritative.

## Missing production interfaces, not a client/UI blocker

1. **Qualified-input provider boundary** under existing ADC owner: exact unclipped
   code/status, requested/realized channel/config, validity/error evidence,
   conversion timestamps, calibration graph+epoch/age and serialized coherence/
   skew. Legacy clipped RAW/success-only is insufficient. Applied091 lacks
   thermistor channel0x30. No second direct-SPMI owner or guessed recovery writes.
2. **Trusted qualification issuer**: must reject unknown capture/rest state,
   reset/boot/battery-generation discontinuity, calibration/revision/compensation
   uncertainty, clipping/fault/errors/staleness and excessive OCV/temp skew.
   Needs actual OCV capture/rest age/generation evidence, not merely fresh latch
   read or terminal voltage_now. Temperature must be battery thermistor, not die
   compensation temperature; no25C/current fallback. Policy thresholds require
   explicit qualification, not arbitrary invented constants.
3. **Separate licensed estimator process**, still absent: consume issuer-approved
   inputs through trusted internal boundary, own private approved profile/math,
   expose only read-only estimate-v1 output. Producer must honestly report input
   ages at estimate observation, provider time, exact approved receipts and
   typed UNKNOWN when not qualified. No guest input injection. Private97,543
   C-reference comparisons do not establish physical accuracy/fitted execution.
4. **Receipt policy and process authority**: deployment binds allowlisted profile/
   algorithm digests and OCV/temp qualification IDs to a known issuer/policy,
   source/license/physical validity records and trusted socket owner/directory.
   Strings/digests alone are not issuer authentication. Unix socket read access
   must not imply write/replace authority; parent must establish ownership/mode
   and avoid malicious listener substitution. No production tuple is approved
   merely because it passes syntax. Revocation/config replacement drops pending
   tokens/snapshots; no last-good fallback or generation reuse.
5. **End-to-end approval**: actual qualified producer->licensed estimator->HAL
   strict client->permissioned Soil guest->opaque host token->owner rendering,
   including unknown/fault/recovery receipts. No live numeric output until then.

No new service or issuer API code is proposed by this read-only review. Above
is the missing producer/issuer contract to be owned upstream, not permission to
construct qualified inputs from the HAL scaffold's free-form receipt strings.

## Fail-closed synthetic end-to-end coverage matrix

Already in inspected Soil tests: permission zero contact, token/guest tampering,
unknown/error number suppression, disconnect/malformed then recovery, native
owner rendering both sizes; host unit retention expiry. HAL already tests exact
mixed-tuple rejection, future/overflow/range/strict codec, conservative ages.

Suggested completion at actual Wasm/socket boundary (sent sole Soil writer):
- Trusted-looking OK with future provider observation, u64 age overflow,
  malformed digest or mixed allowlist tuple -> Unknown and no numeric pixels.
- Nonzero input ages with max(input)+providerelapsed crossing max_age; never
  choose younger input or reset age on read.
- Delayed mock socket response makes full RTT exceed max_age -> Stale/no number.
- Delay guest token consumption past max_age -> Stale; present again later still
  numberless. Existing retained host unit does not alone prove delayed Wasm path.
- Missing endpoint then mock restart/reconnect; no cached last-good number.
- No production trusted tuple/issuer => no executable path silently injecting
  test metadata, synthetic fixture estimates or default profile authority.

These are temporary test listeners only, NOT a production estimator service or
license/math/data integration. Sole Soil writer owns any test changes.

## Stale QRD215/index notes resolved

Read full acquisition audit at
`reversal/.worktrees/s22-outer-display-impl/cat-s22/microphone/STOCK49-OCV-THERM-ACQUISITION-AUDIT.md`.
Authoritative stock DT9524–9533 VADC batt_therm selects26=0x1a. Fitted pointer
c0f89dd4 (c0f89d6c+26*4) resolves c0a9497c QRD215; dispatch permits26.
Index8 SMB is real but NOT selected. Do not retain 'SMB vs unprovenQRD215' as a
selection blocker. Stock ADC_TM index8 is a separate namespace. Physical sensor
qualification remains unproved; fitted QRD215 table differs from fetched C.
Applied091 ADC has no0x30, BMS disabled. Provider validity, PMIC compensation,
OCV capture/rest/continuity and trusted issuer are the actual remaining blockers.
Updated HAL telemetry audit correction header; no foreign docs/source edits.
