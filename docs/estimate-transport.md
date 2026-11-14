# Estimate-v1 client contract — no running service

Frozen namespace `fungos_device_hal::estimate_transport` in
`src/estimate_transport.rs`. CLIENT/types/strict codec only: no estimator service,
input producer/injection, GPL math, numeric profile data, static build or deploy.
Generic battery_profile scaffold frozen. battery-v1 and BatterySample.percent
remain unchanged: the latter is kernel capacity only, NOT this model estimate.

## Rust API

```rust
EstimateClient::new(socket, trusted: Vec<TrustedEstimateReceipts>)
    -> Result<EstimateClient, EstimateFailure>
client.read() -> Result<RemoteEstimate, EstimateFailure>
client.decode_response(frame: &[u8], started: Instant)
    -> Result<RemoteEstimate, EstimateFailure>
remote.provider_age() -> Duration
remote.age() -> Duration
remote.snapshot(max_age: Duration) -> Result<ModelEstimate, EstimateFailure>
```

TrustedEstimateReceipts derives Clone/Debug/Eq/PartialEq and has six public String
fields: profile_id, profile_sha256, algorithm_id, algorithm_sha256,
ocv_qualification_id, temperature_qualification_id. Construct via struct literal,
then pass host-owned Vec to EstimateClient::new. IDs:1..128 ASCII alphanumeric,
`-_.`; digests exactly64 lowercase hex. Constructor rejects relative sockets,
empty or >32 allowlist, invalid metadata with Protocol. Every response must
match ALL SIX fields of ONE allowlisted tuple, not mix fields across tuples.
Metadata/digests are identifiers, not proof of calibration or trusted input.
Host must approve the receipt semantics before placing them in configuration.
No guest chooses this config/socket, and valid strings alone confer no trust.

ModelEstimate derives Clone/Debug/Eq/PartialEq, with public fields:
- percent:u8
- receipts:TrustedEstimateReceipts
- ocv_capture_age_ms:u64
- temperature_capture_age_ms:u64
- estimate_observed_ms:u64
- provider_now_ms:u64

RemoteEstimate has no unchecked numeric sample accessor: only snapshot returns
ModelEstimate after freshness check. Public model fields support host/UI mapping;
they do not make guest numeric claims authoritative. Soil owns trusted token
construction and never accepts guest ModelEstimate/voltage/temperature/receipt
strings as estimator results.

Failures Copy/Debug/Eq/PartialEq:
`EstimateFailure::{Unknown(UnknownReason),Denied,Backend,Disconnected,Protocol,
Stale,InvalidTime,UntrustedReceipt,Overflow}`.
UnknownReason: Unavailable,UnqualifiedOcv,UnqualifiedTemperature,
AlgorithmUnverified,ProfileUnqualified. Unknown is Err with no ModelEstimate or
numeric payload. No last-good cache. PermissionDenied IO maps Denied; other IO
maps Disconnected. Each read opens a new connection.

## Wire

Proposed parent-only endpoint `/run/fungos-estimate-provider.sock` (NONE running).
Request exactly `estimate-v1\n`, write half-closed. Max4096 response bytes,
read capped4097 to detect overflow, read/write timeouts2s.

Tabs, exactly one terminal LF:

```text
FUNGOS-ESTIMATE/1 OK percent profile_id profile_sha256 algorithm_id algorithm_sha256 ocv_qualification_id temperature_qualification_id ocv_capture_age_ms temperature_capture_age_ms estimate_observed_ms provider_now_ms
FUNGOS-ESTIMATE/1 UNKNOWN reason
FUNGOS-ESTIMATE/1 ERR reason
```

OK has13 fields. Percent0..100; all time fields decimal unsignedu64.
UNKNOWN exact tokens: unavailable,unqualified-ocv,unqualified-temperature,
algorithm-unverified,profile-unqualified. ERR: denied,backend,disconnected,
protocol,stale,invalid-time,untrusted-receipt,overflow. Unknown reason tokens
fail Protocol. UNKNOWN/ERR MUST NOT carry numbers, extra fields or provenance.
Strict missing/extra fields, nonUTF8, line injection, ranges/version rejected.
No encode_response API: no service authorized. Test fixtures use synthetic
literal frames; production must never serve synthetic fixture estimates.

## Time and trust

Input capture ages are evaluated AT estimate_observed_ms. Provider age:
`max(ocv_capture_age_ms, temperature_capture_age_ms)
 + (provider_now_ms - estimate_observed_ms)`.
Subtraction and addition checked; future observation => InvalidTime, arithmetic
overflow => Overflow. No inter-process monotonic clock subtraction. Client adds
entire RTT and local retention, with checked Duration arithmetic in snapshot.
`age()` is saturating display-only; consumers MUST use snapshot(max_age) for
eligibility and recheck when consuming/presenting opaque host tokens.
No fixed client stale threshold: host permission defines max_age.
Freshness means current model output, not physical accuracy qualification.

Soil owns distinct permission BEFORE contact, bounded read ABI, opaque single-use
host token/update and expiry/current eligibility. No host-poll bypass or guest
input injection. Separate ModelEstimateView, never BatteryView fallback; fresh
numeric presentation must literally label Estimated and expose safe model
provenance. Nonfresh/unknown/denied hide numeric values. No GPL data/math in UI.
No approved receipt/input producer or running service exists yet; live
unqualified => NO numeric estimate.

## Validation receipt

50tests PASS (35lib+12binary+3integration), cargo check, strict all-target
Clippy -Dwarnings without allowances, fmt and diff check PASS. New tests cover
strict parsing/ranges/unknown-no-number, exact tuple trust/mixed-entry rejection,
future observation/start, u64overflow, conservative ages/RTT/stale snapshots,
Denial mapping and readonly request/disconnect/reconnect. All existing battery,
lid and combined regressions pass. Soil sole writer informed before code and
provided frozen exact public field/method contract; no Soil edits by HAL.
No static rebuild or deployment. Live battery artifact hash remains
938bb6c4b1c7ebc55b7fa54167617cd2fb23e8d1da95a247e1fe77bab3c2e508.
