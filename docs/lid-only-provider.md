# Lid-only provider handoff

Parent-authorized implementation; no phone/kernel/Soil/battery edits or deployment.
Same executable, explicit **`--lid-only`** early dispatch before combined Config/run.
No arguments still select combined mode: **do not omit the flag**.

Artifact (static AArch64 musl, stripped):
`/Users/ajmwagar/Documents/src/fungOS-linux/fungos/.worktrees/device-hal/target/aarch64-unknown-linux-musl/release/fungos-keyboard-lightd`

SHA256 `4988a65e655748b7df2705a87211f5574be106699186bfdf08c64a919d192afa`.
This replaces the earlier ab698 artifact at this build output path; ab698 has no
lid-only mode. Parent must verify hash, not infer mode from filename.

## Parent-owned start configuration (not executed here)

Use a distinct executable path `/run/fungos-lid-provider` and socket path
`/run/fungos-device-provider`; the following assumes parent stages the verified
artifact at that executable path:

```sh
FUNGOS_SOCKET=/run/fungos-device-provider \
FUNGOS_LID_NAME=gpio-keys \
FUNGOS_LID_POLARITY=standard-closed-asserted \
/run/fungos-lid-provider --lid-only
```

No keyboard identity/LED environment needed or consumed by lid-only mode.
Roots default `/sys/class/input`, `/dev/input`; optional existing
`FUNGOS_SYS_INPUT_ROOT`, `FUNGOS_INPUT_ROOT` are trusted deployment overrides.
Current parent read-only identity receipt `ssh089-outer-dma-lid-inventory.log`
reports gpio-keys, EV23/SW1, handler event1. Code discovers by exact name and
SW_LID capability; **does not hardcode event1**. Standard polarity relies on
prior board qualification, not the device name. Unset polarity is Unqualified.

## Ownership, protocol and lifecycle

- One `LidInput`, one listener, exactly existing typed `lid-v1\n` codec.
- No light/policy/reconcile, keypad reader/ring, battery or audio/PCM backend.
- Explicit absolute FUNGOS_SOCKET required. Parent directory must already exist.
- Bind never unlinks an existing path, even stale socket/file/symlink. Collision
  fails startup without replacing another owner. Parent must establish exclusive
  ownership before deciding how to remove a stale path; do not blindly rm/start.
- Socket creation uses restrictive umask and mode0600. Same-UID clients only.
- Every other request, including legacy lid, status, key-v1, battery, lease,
  clear and stop, returns `ERR lid-only supports only lid-v1` with no command
  side effects. Requests bounded257 bytes/read timeout2s/write timeout2s.
- Parent retains PID; **SIGTERM or SIGINT** ends loop, drops evdev fd/listener and
  removes only path matching recorded device+inode. A replacement at the same
  path is retained. Handling may wait for a bounded in-flight socket read/write.
  SIGKILL/crash cannot clean up; stale socket deliberately blocks restart.
- The unlink identity check is not a kernel-atomic compare-and-unlink; parent
  must keep socket directory trusted and avoid concurrent path replacement.
- Typed client reconnects per request; no cached last-good after failure.
  No display/presenter rotation, suspend or keyboard illumination actions.

## Validation receipt

`cargo check`, full `cargo test`: **38 passed** (27 library,9 binary,2 integration).
`cargo clippy --all-targets -- -D warnings` passes **without allowances**.
`cargo fmt --all -- --check` passes. Earlier pre-existing strict Clippy warnings
resolved mechanically: moved test modules after production items in main/lid,
and replaced constant chunks_exact with as_chunks; no combined policy change.

New actual-executable test starts without keyboard identity or LED capability,
with invalid unused LED name and sentinel non-directory LED/power roots. It
checks typed initial Unavailable, all command rejections,0600, duplicate bind
failure with original alive, TERM success/cleanup, disconnected client, restart
and typed reconnect, unchanged sentinels. Architecture guard rejects combined/
audio construction dependencies in lid-only production source. This is not a
syscall tracer or successful host SW_LID ioctl/hardware proof. Existing backend/
codec tests cover initial query, freshness/stale/future, polarity, dropped-event
resync and typed failures. Ownership tests preserve replacement paths.

After tests: `cargo check --target aarch64-unknown-linux-musl` and
`cargo zigbuild --release --target aarch64-unknown-linux-musl` passed. Linker emits
only existing deprecated optimization-setting warning; output is statically linked.
No target execution, open/read of phone evdev, or no-PCM phone receipt claimed.
Presenter180 geometry remains pending user confirmation; untouched.
