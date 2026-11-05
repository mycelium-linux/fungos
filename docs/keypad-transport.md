# Typed keypad stream (reversal-ozgo)

Owner modules `fungos_device_hal::{keypad,keypad_transport}`, package
`fungos-device-hal` in this worktree. Existing input reader and0600 socket only;
no new evdev owner/thread/service, no navigation/display policy in HAL. Soil
must deny ungranted capabilities before contacting the client and host-map only
qualified Linux codes into guest navigation.

```rust,ignore
use fungos_device_hal::keypad_transport::KeypadClient;
let mut client = KeypadClient::new("/run/fungos-device-provider");
let mut cursor = None;
let batch = client.poll(cursor)?;
cursor = Some(batch.cursor);
// Any batch.resync or Err: clear held/repeat state, invent no releases.
// Otherwise process batch.events in order; retain returned cursor.
```

`keypad` owns `KeyAction::{Press,Release,Repeat}`,
`KeypadCursor { provider_instance:u64,sequence:u64 }`,
`KeypadEvent {sequence:u64,code:u16,action:KeyAction,observed_monotonic_ms:u64}`,
`KeypadBatch {cursor,events:Vec<KeypadEvent>,resync:Option<KeypadResync>}`,
`KeypadResync::{NewClient,Overflow,Restart,Dropped,Stale,Reconnect}`,
`KeypadFailure::{Denied,Disconnected,Protocol,Backend}`.

`KeypadClient::new(socket)` and mutable `poll(Option<KeypadCursor>)` return
`Result<KeypadBatch,KeypadFailure>`. New client None starts **at head, no history**.
Each batch has at most32 events from a256-event non-destructive ring; two clients
with identical cursors see identical records. Sequence is global within a random
64-bit provider instance (OS /dev/urandom per startup). Stale cursor/overflow,
restart, SYN_DROPPED/keyboard EOF and disconnect recovery explicitly discard
history and return current head. Dropped interval stays unknown until SYN_REPORT;
no key-state ioctl or synthetic release/repeat is invented. New clients during a
dropped interval first get NewClient then Dropped on subsequent polls.

Every poll uses a fresh socket (normal operation); after a failed exchange the
client forces its next poll to head and returns Reconnect rather than replaying
old history. Keep the client object across polls to preserve this behavior.
Provider enforces1s maximum age of pending records; transport parser adds request
elapsed time conservatively. If the transport itself expires a partial batch,
client performs a head request and returns Stale/no events. Neither side compares
independent clock timestamps. Event monotonic timestamps are provider receipt
observations, NOT kernel event acquisition timestamps. Guest consumers should
process immediately, not retain events to replay later.

Public codec for Soil integration tests:
`REQUEST="key-v1"`, `MAX_FRAME=4096`,
`encode_request(Option<KeypadCursor>)->String`,
`decode_request(&str)->Result<Option<KeypadCursor>,KeypadFailure>`,
`encode_response(&KeypadBatch,provider_now_ms:u64)->String`,
`decode_response(&[u8],request_started:Instant)->Result<KeypadBatch,KeypadFailure>`.

Wire request `key-v1\n` or `key-v1 INSTANCE SEQUENCE\n`.
Response header `FUNGOS-KEY/1 INSTANCE CURSOR PROVIDER_NOW STATUS\n`, followed by
0..32 `SEQUENCE CODE p|u|r OBSERVED_MS\n` records. Status is
`ok|new|overflow|restart|dropped|stale|reconnect`; resync frames contain no events.
Strict version/arity/numeric/type/order/maxsize checks. Client checks continuity
against its cursor. Request bounds/timeouts inherit existing socket handling;
read/write2s deadlines, strict client4096-byte response bound. Same trusted uid
permissions, not a separate guest-accessible input socket.

Existing battery/status/light and lid-v1/diagnostic lid commands preserved. The
existing keyboard reader drains at most256 records/loop rather than only one,
and fans each record into both owner ring and unchanged light-activity policy.
Native target currently ARM64 uses existing24-byte keyboard decoder; lid decoder
uses native timeval. Keyboard short record invalidates ring; fatal read errors
still terminate provider (next instance reconnect resync), EOF is Dropped.

34 tests pass (27lib/6binary/1 executable integration), including independent
reader order, repeat/release, no-history, overflow,32 batch bound, exact1000ms vs
stale1001ms, restart/drop, disconnect/reconnect-at-head, protocol version/size,
permission-error mapping and real provider keypad/lid compatibility0600. Soil
owns actual guest permission/deny-with-zero-contact tests; those are not claimed
as a HAL guest test. Clippy passes with two preexisting main lint allowances.

Static artifact:
`target/aarch64-unknown-linux-musl/release/fungos-keyboard-lightd`
SHA256 `ab6985191dd33eec1f2b1fa4d3540599325da861475e2991bc3368b4d1a28367`.
Parent replaces/restarts existing owner at actual configured
`/run/fungos-device-provider`; do not launch another provider. This supersedes
24923 lid-only binary. Preserve qualified standard-closed-asserted .074 settings.
No worker phone/shared-builder access, kernel edits, deployment, commits or pushes.
