# Permissioned volume / navigation audit (reversal-vcug)

## Freeze: production audio volume unavailable

`src/services.rs` has only the `AudioRouteControl` seam and synthetic test
implementations; no production route/volume adapter or volume transport was
found in HAL or Soil. Added `services::UnavailableAudioOwner`, a zero-I/O
adapter returning `AudioFailure::Unavailable` for both outputs and set_volume.
This does NOT open PCM, call tinymix, adjust a route or touch hardware.

Existing seam types: AudioOutput(identity, available, volume_percent, muted);
AudioFailure(Unavailable, Denied, InvalidVolume, Backend). Legacy output has no
observation timestamp, range provenance or validity. set_volume returning Ok
alone does NOT prove verified readback. Do not promote this seam into a claim
of observed speaker volume, or adapt it by inventing time/range/mute/percent.

### HAL -> Soil -> actual Wasm -> Canvas contract for this build

- Separate read and adjust grants, checked BEFORE any owner contact (including
  read-before-write). Keypad permission never grants audio permission.
- Host owns adapter/result/command. Guest receives only bounded ABI and opaque,
  invocation-bound single-use command token; no route, mixer name, socket or
  backend selection from guest. Replay/expiry/budget/tampering rejected.
- Authorized read/adjust currently returns typed Unavailable, denied returns
  Denied; widget has no observed numeric, range or mute value. Never render0%
  or muted=false as unknown defaults. No production socket to configure.
- Requested target may be UI intent only, never observed truth. Pending/failed
  command cannot update observed value. No optimistic success.
- Future binding requires owner-backed observed level, scale/range/step, mute,
  identity/generation and monotonic age. Fresh owner readback after set must
  independently confirm actual value; clamped/mismatch/error is not requested
  target success. RTT/retention count toward age; stale hides numeric.
- Synthetic mock read/set/readback tests may prove permission/token/UI paths;
  fixtures must never become the production default adapter. Current owner
  cannot verify a set and always reports Unavailable.

Exact missing evidence: current sole audio owner/process and endpoint identity,
control type and hardware path, safe supported range/units/step/percent mapping,
read semantics, mute observation, clock/freshness policy, set+independent
readback semantics, clamp/error/route-transition/concurrency behavior, safety
ceiling and explicit authorization. No guessed ALSA control names/scales.

## Sources that are NOT a volume backend

`reversal/cat-s22/tools/boot-shim/cat_audio_play.rs` only gates
QUIN_MI2S_RX Audio Mixer MultiMedia1 around bounded playback and a fixed kernel
-24dB ceiling; it is not a speaker volume/mute observation API.
`reversal/cat-s22/docs/internal-audio-stock-trace.md` at CAT set_volume c0c6738c
establishes register0f attenuation encoding, with actual TLV 0.5dB/unit. That is
stock binary evidence, NOT a live mainline user-volume mapping or authorization
to touch amplifier gain. Fade/start/stop may overwrite attenuation. Offline
cat_amp_profile.rs deliberately performs no device access. Do not repurpose
route enable, qualification state or attenuation ceiling as volume percent.

## Source-backed key mapping, not physical button proof

Stock `reversal/cat-s22/out/resume-20261029/lk2nd-stock-live.dts`:
- PMIC pon_2 line9331 code0x72=114 (VOLUMEDOWN).
- gpio_keys/vol_up lines11576–82 GPIO91 active-low code0x73=115 (VOLUMEUP).
- matrix_keypad line11631; same full map in mainline
  `cat-s22/mainline/kernel/dts/cat-s22-keypad.dtsi`.

Matrix row/col (zero based): left(1,0)=105, right(3,1)=106,
up(3,3)=103, down(3,4)=108, center(3,0)=28; Back(4,0)=158,
Homepage(4,2)=172, Appselect(4,1)=580. Logical code presence does not prove
physical legends or live applied DT. PTT physical mapping is not established:
custom matrix codes0x20a..0x20d must not be assigned by guessing. Parent owns
bounded press/release scan and live identity receipts. PTT mapping only, no
recording/capture or implicit microphone permission. Side volume keys belong
to separate stock gpio/PMIC nodes, not matrix keypad; a single matrix reader
cannot claim to deliver them. Do not widen input ownership without parent.

## Keypad-only artifact / handoff

Early exact `--keypad-only` bypasses combined Config/backend. Requires explicit
absolute `FUNGOS_KEYPAD_SOCKET` and exact `FUNGOS_INPUT_NAME`; discovers a unique
sysfs identity, never assumes event0. Parent confirms live identity `keypad`.
Root overrides FUNGOS_SYS_INPUT_ROOT and FUNGOS_INPUT_ROOT default to
/sys/class/input and /dev/input. Read-only nonblocking fd, no EVIOCGRAB;
EVIOCSCLOCKID selects per-fd CLOCK_MONOTONIC (unsupported => fail closed).
Uses original kernel event timestamps to avoid making queued input fresh.

Parent-deployed command (parent alone owns execution):

```sh
FUNGOS_KEYPAD_SOCKET=/run/fungos-keypad-provider.sock \
FUNGOS_INPUT_NAME=keypad /run/fungos-keypad-provider --keypad-only
```

Artifact relative to HAL worktree:
`target/keypad-only/aarch64-unknown-linux-musl/release/fungos-keyboard-lightd`
SHA256 `0fb726e276e3ce8db4207cd41258a39dc96abfa95dc949c3f44949ec241ddeed`.
Static AArch64 musl stripped. Parent reports exact hash deployed, socket0600,
alive RO1, no other owner changes; `/tmp/cat-keypad-provider-deploy.log`.
This is a parent receipt, not HAL hardware execution.

Existing mutable KeypadClient::poll(cursor) uses key-v1, 4096byte response,
2s read/write timeouts. Ring256, batch32, age<=1000ms includes full RTT.
Initial poll returns empty NewClient/head, no historical input. Overflow,
SYN_DROPPED, restart/random instance, stale batches clear events/resync.
Failed exchange forces next success to head/Reconnect; connection-per-poll is
normal, not reconnect. Transport-stale response reacquires current head.
No retained freshness accessor: Soil must expire retained events/actions and
clear held state on resync; do not replay an old batch after consumption.
Input EOF/partial/error/drain-budget failure exits, no synthetic releases;
supervisor/client reconnection is required after device loss. Slow socket
peer can delay acquisition up to bounded I/O; kernel timestamps prevent false
freshness. Owner socket no replacement, TERM/INT dev+inode cleanup shared with
lid-only; trusted directory required, identity-check/unlink is not atomic,
SIGKILL leaves stale path. Collision never unlinks existing path.

Validation before artifact:52 tests, cargo check, strict all-target Clippy,
fmt/diff check passed; crosscheck and zigbuild passed (linker optimization
warning only). No real evdev ioctl/hardware tested locally. Preserved battery
938bb6c4... and lid4988a65e... artifacts verified unchanged. Later zero-I/O
UnavailableAudioOwner addition does not change the already deployed artifact.
