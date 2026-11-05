//! Shared bounded v1 lid wire codec and permissioned one-shot socket client.
//! Consumers import owner types; Debug formatting is NOT a wire protocol.
use crate::lid::{LidFailure, LidPosition, LidSample};
use std::{
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub const REQUEST: &str = "lid-v1";
pub const MAX_FRAME: usize = 4096;
const MAGIC: &str = "FUNGOS-LID/1";
const TIMEOUT: Duration = Duration::from_secs(2);

fn error_name(error: LidFailure) -> &'static str {
    match error {
        LidFailure::Unavailable => "unavailable",
        LidFailure::Denied => "denied",
        LidFailure::Backend => "backend",
        LidFailure::Ambiguous => "ambiguous",
        LidFailure::Resynchronizing => "resynchronizing",
        LidFailure::Stale => "stale",
        LidFailure::InvalidTime => "invalid-time",
        LidFailure::Disconnected => "disconnected",
        LidFailure::Protocol => "protocol",
    }
}
fn parse_error(value: &str) -> Result<LidFailure, LidFailure> {
    [
        LidFailure::Unavailable,
        LidFailure::Denied,
        LidFailure::Backend,
        LidFailure::Ambiguous,
        LidFailure::Resynchronizing,
        LidFailure::Stale,
        LidFailure::InvalidTime,
        LidFailure::Disconnected,
        LidFailure::Protocol,
    ]
    .into_iter()
    .find(|e| error_name(*e) == value)
    .ok_or(LidFailure::Protocol)
}
fn position_name(position: LidPosition) -> &'static str {
    match position {
        LidPosition::Unqualified => "unqualified",
        LidPosition::Open => "open",
        LidPosition::Closed => "closed",
    }
}
/// Unix paths are hex encoded, preserving non-UTF8 bytes without line injection.
#[cfg(unix)]
fn path_hex(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str()
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
#[cfg(unix)]
fn decode_path(hex: &str) -> Result<PathBuf, LidFailure> {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    if hex.is_empty() || !hex.len().is_multiple_of(2) || !hex.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(LidFailure::Protocol);
    }
    let bytes = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| LidFailure::Protocol))
        .collect::<Result<Vec<_>, _>>()?;
    if bytes.contains(&0) {
        return Err(LidFailure::Protocol);
    }
    Ok(OsString::from_vec(bytes).into())
}
/// Provider monotonic observation + age in that domain. Version fixes provenance
/// kind to Linux evdev SW_LID; node is evidence, never a client-selected device.
#[cfg(unix)]
pub fn encode_response(sample: Result<LidSample, LidFailure>, now: u64) -> String {
    let result = sample.and_then(|s| {
        s.check_freshness(now, 1000)?;
        let frame = format!(
            "{MAGIC}\tOK\t{}\t{}\t{}\t{}\t{}\n",
            u8::from(s.switch_asserted),
            position_name(s.physical_position),
            s.observed_monotonic_ms,
            now - s.observed_monotonic_ms,
            path_hex(&s.event_node)
        );
        if frame.len() > MAX_FRAME {
            Err(LidFailure::Protocol)
        } else {
            Ok(frame)
        }
    });
    result.unwrap_or_else(|e| format!("{MAGIC}\tERR\t{}\n", error_name(e)))
}

/// Valid only while retained locally. Provider timestamp is provenance, not a
/// timestamp in the guest's clock domain. No stale/disconnected fallback cache.
#[derive(Clone, Debug)]
pub struct RemoteLidSample {
    sample: LidSample,
    provider_age_ms: u64,
    transport_elapsed: Duration,
    received: Instant,
}
impl RemoteLidSample {
    pub fn sample(&self) -> &LidSample {
        &self.sample
    }
    /// Age sent by provider in its monotonic domain (excludes transport time).
    pub fn provider_age(&self) -> Duration {
        Duration::from_millis(self.provider_age_ms)
    }
    pub fn age(&self) -> Duration {
        Duration::from_millis(self.provider_age_ms)
            .saturating_add(self.transport_elapsed)
            .saturating_add(self.received.elapsed())
    }
    pub fn snapshot(&self, max_age: Duration) -> Result<LidSample, LidFailure> {
        if self.age() > max_age {
            Err(LidFailure::Stale)
        } else {
            Ok(self.sample.clone())
        }
    }
}
#[cfg(unix)]
pub fn decode_response(
    frame: &[u8],
    request_started: Instant,
) -> Result<RemoteLidSample, LidFailure> {
    if frame.len() > MAX_FRAME
        || !frame.ends_with(b"\n")
        || frame[..frame.len() - 1].contains(&b'\n')
    {
        return Err(LidFailure::Protocol);
    }
    let line = std::str::from_utf8(&frame[..frame.len() - 1]).map_err(|_| LidFailure::Protocol)?;
    let fields: Vec<_> = line.split('\t').collect();
    match fields.as_slice() {
        [MAGIC, "ERR", reason] => Err(parse_error(reason)?),
        [MAGIC, "OK", raw, position, observed, age, path] => {
            let asserted = match *raw {
                "0" => false,
                "1" => true,
                _ => return Err(LidFailure::Protocol),
            };
            let position = match *position {
                "unqualified" => LidPosition::Unqualified,
                "open" if !asserted => LidPosition::Open,
                "closed" if asserted => LidPosition::Closed,
                _ => return Err(LidFailure::Protocol),
            };
            fn number(s: &str) -> Result<u64, LidFailure> {
                if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(LidFailure::Protocol);
                }
                s.parse().map_err(|_| LidFailure::Protocol)
            }
            let sample = LidSample {
                switch_asserted: asserted,
                physical_position: position,
                event_node: decode_path(path)?,
                observed_monotonic_ms: number(observed)?,
            };
            // Conservatively include the entire round trip: never subtract
            // timestamps from independent monotonic clocks or hide transport lag.
            let provider_age_ms = number(age)?;
            Ok(RemoteLidSample {
                sample,
                provider_age_ms,
                transport_elapsed: request_started.elapsed(),
                received: Instant::now(),
            })
        }
        _ => Err(LidFailure::Protocol),
    }
}
fn io_failure(e: io::Error) -> LidFailure {
    match e.kind() {
        io::ErrorKind::PermissionDenied => LidFailure::Denied,
        _ => LidFailure::Disconnected,
    }
}
#[derive(Clone, Debug)]
pub struct LidClient {
    socket: PathBuf,
}
impl LidClient {
    /// Explicit trusted deployment socket path; no device discovery in the guest.
    pub fn new(socket: impl Into<PathBuf>) -> Self {
        Self {
            socket: socket.into(),
        }
    }
    /// New connection on every call (reconnect by calling again). Caller must
    /// authorize its guest before invoking this method. Unix DAC remains active.
    #[cfg(unix)]
    pub fn read(&self) -> Result<RemoteLidSample, LidFailure> {
        use std::{net::Shutdown, os::unix::net::UnixStream};
        let started = Instant::now();
        let mut stream = UnixStream::connect(&self.socket).map_err(io_failure)?;
        stream.set_read_timeout(Some(TIMEOUT)).map_err(io_failure)?;
        stream
            .set_write_timeout(Some(TIMEOUT))
            .map_err(io_failure)?;
        writeln!(stream, "{REQUEST}").map_err(io_failure)?;
        stream.shutdown(Shutdown::Write).map_err(io_failure)?;
        let mut frame = Vec::new();
        stream
            .take((MAX_FRAME + 1) as u64)
            .read_to_end(&mut frame)
            .map_err(io_failure)?;
        if frame.is_empty() {
            return Err(LidFailure::Disconnected);
        }
        decode_response(&frame, started)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample(raw: bool, position: LidPosition) -> LidSample {
        LidSample {
            switch_asserted: raw,
            physical_position: position,
            event_node: "/dev/input/event1".into(),
            observed_monotonic_ms: 100,
        }
    }
    #[test]
    fn roundtrip_types_source_and_freshness() {
        for (raw, position) in [
            (false, LidPosition::Open),
            (true, LidPosition::Closed),
            (true, LidPosition::Unqualified),
        ] {
            let s = sample(raw, position);
            let wire = encode_response(Ok(s.clone()), 105);
            let remote = decode_response(wire.as_bytes(), Instant::now()).unwrap();
            assert_eq!(remote.provider_age(), Duration::from_millis(5));
            assert_eq!(remote.snapshot(Duration::from_secs(1)).unwrap(), s);
            assert_eq!(remote.snapshot(Duration::ZERO), Err(LidFailure::Stale));
        }
    }
    #[test]
    fn unknown_denied_disconnect_and_resync_not_cached() {
        for error in [
            LidFailure::Unavailable,
            LidFailure::Denied,
            LidFailure::Backend,
            LidFailure::Resynchronizing,
            LidFailure::Disconnected,
            LidFailure::Stale,
        ] {
            let wire = encode_response(Err(error), 100);
            assert_eq!(
                decode_response(wire.as_bytes(), Instant::now()).unwrap_err(),
                error
            );
        }
        assert!(encode_response(Ok(sample(false, LidPosition::Open)), 99).contains("invalid-time"));
    }
    #[test]
    fn strict_bounded_versioned_parser() {
        for wire in [
            "FUNGOS-LID/2\tERR\tdenied\n",
            "FUNGOS-LID/1\tERR\tguess\n",
            "FUNGOS-LID/1\tOK\t1\topen\t100\t0\t2f\n",
            "FUNGOS-LID/1\tERR\tdenied\nextra\n",
            "FUNGOS-LID/1\tERR\tdenied",
        ] {
            assert_eq!(
                decode_response(wire.as_bytes(), Instant::now()).unwrap_err(),
                LidFailure::Protocol
            );
        }
        assert_eq!(
            decode_response(&vec![b'X'; MAX_FRAME + 1], Instant::now()).unwrap_err(),
            LidFailure::Protocol
        );
    }
    #[cfg(unix)]
    #[test]
    fn socket_disconnect_and_reconnect() {
        use std::os::unix::net::UnixListener;
        let path = std::env::temp_dir().join(format!("lid-transport-{}", std::process::id()));
        let listener = UnixListener::bind(&path).unwrap();
        let client = LidClient::new(&path);
        let server = std::thread::spawn(move || {
            for response in [
                None,
                Some(encode_response(Err(LidFailure::Resynchronizing), 100)),
                Some(encode_response(Ok(sample(false, LidPosition::Open)), 100)),
            ] {
                let (mut s, _) = listener.accept().unwrap();
                let mut q = String::new();
                s.read_to_string(&mut q).unwrap();
                assert_eq!(q, "lid-v1\n");
                if let Some(response) = response {
                    s.write_all(response.as_bytes()).unwrap();
                }
            }
        });
        assert_eq!(client.read().unwrap_err(), LidFailure::Disconnected);
        assert_eq!(client.read().unwrap_err(), LidFailure::Resynchronizing);
        assert_eq!(
            client
                .read()
                .unwrap()
                .snapshot(Duration::from_secs(1))
                .unwrap()
                .physical_position,
            LidPosition::Open
        );
        server.join().unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(client.read().unwrap_err(), LidFailure::Disconnected);
        assert_eq!(
            io_failure(io::ErrorKind::PermissionDenied.into()),
            LidFailure::Denied
        );
    }
}
