//! Shared bounded keyboard protocol. Authorization precedes calling this client.
use crate::keypad::*;
use std::{
    io::{self, Read, Write},
    path::PathBuf,
    time::{Duration, Instant},
};
pub const REQUEST: &str = "key-v1";
pub const MAX_FRAME: usize = 4096;
fn number<T: std::str::FromStr>(s: &str) -> Result<T, KeypadFailure> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(KeypadFailure::Protocol);
    }
    s.parse().map_err(|_| KeypadFailure::Protocol)
}
pub fn encode_request(cursor: Option<KeypadCursor>) -> String {
    match cursor {
        None => format!("{REQUEST}\n"),
        Some(c) => format!("{REQUEST} {} {}\n", c.provider_instance, c.sequence),
    }
}
pub fn decode_request(s: &str) -> Result<Option<KeypadCursor>, KeypadFailure> {
    if s.len() > 128 || !s.ends_with('\n') {
        return Err(KeypadFailure::Protocol);
    }
    let fields: Vec<_> = s.trim_end_matches('\n').split(' ').collect();
    match fields.as_slice() {
        [REQUEST] => Ok(None),
        [REQUEST, i, s] => Ok(Some(KeypadCursor {
            provider_instance: number(i)?,
            sequence: number(s)?,
        })),
        _ => Err(KeypadFailure::Protocol),
    }
}
fn reason(r: KeypadResync) -> &'static str {
    match r {
        KeypadResync::NewClient => "new",
        KeypadResync::Overflow => "overflow",
        KeypadResync::Restart => "restart",
        KeypadResync::Dropped => "dropped",
        KeypadResync::Stale => "stale",
        KeypadResync::Reconnect => "reconnect",
    }
}
pub fn encode_response(batch: &KeypadBatch, now: u64) -> String {
    let mut s = format!(
        "FUNGOS-KEY/1 {} {} {} {}\n",
        batch.cursor.provider_instance,
        batch.cursor.sequence,
        now,
        batch.resync.map_or("ok", reason)
    );
    for e in batch.events.iter().take(BATCH_CAPACITY) {
        s.push_str(&format!(
            "{} {} {} {}\n",
            e.sequence,
            e.code,
            match e.action {
                KeyAction::Press => "p",
                KeyAction::Release => "u",
                KeyAction::Repeat => "r",
            },
            e.observed_monotonic_ms
        ));
    }
    s
}
pub fn decode_response(
    bytes: &[u8],
    request_started: Instant,
) -> Result<KeypadBatch, KeypadFailure> {
    if bytes.len() > MAX_FRAME || !bytes.ends_with(b"\n") {
        return Err(KeypadFailure::Protocol);
    }
    let s = std::str::from_utf8(bytes).map_err(|_| KeypadFailure::Protocol)?;
    let mut lines = s[..s.len() - 1].split('\n');
    let header: Vec<_> = lines
        .next()
        .ok_or(KeypadFailure::Protocol)?
        .split(' ')
        .collect();
    let ["FUNGOS-KEY/1", instance, sequence, now, status] = header.as_slice() else {
        return Err(KeypadFailure::Protocol);
    };
    let cursor = KeypadCursor {
        provider_instance: number(instance)?,
        sequence: number(sequence)?,
    };
    let now = number::<u64>(now)?;
    let resync = match *status {
        "ok" => None,
        "new" => Some(KeypadResync::NewClient),
        "overflow" => Some(KeypadResync::Overflow),
        "restart" => Some(KeypadResync::Restart),
        "dropped" => Some(KeypadResync::Dropped),
        "stale" => Some(KeypadResync::Stale),
        "reconnect" => Some(KeypadResync::Reconnect),
        _ => return Err(KeypadFailure::Protocol),
    };
    let mut events = Vec::new();
    let mut expired = false;
    for line in lines {
        if events.len() >= BATCH_CAPACITY || resync.is_some() {
            return Err(KeypadFailure::Protocol);
        }
        let f: Vec<_> = line.split(' ').collect();
        let [seq, code, action, time] = f.as_slice() else {
            return Err(KeypadFailure::Protocol);
        };
        let e = KeypadEvent {
            sequence: number(seq)?,
            code: number(code)?,
            action: match *action {
                "p" => KeyAction::Press,
                "u" => KeyAction::Release,
                "r" => KeyAction::Repeat,
                _ => return Err(KeypadFailure::Protocol),
            },
            observed_monotonic_ms: number(time)?,
        };
        if e.code == 0
            || e.code > 0x2ff
            || e.sequence > cursor.sequence
            || events
                .last()
                .is_some_and(|last: &KeypadEvent| last.sequence.checked_add(1) != Some(e.sequence))
        {
            return Err(KeypadFailure::Protocol);
        }
        let age = now
            .checked_sub(e.observed_monotonic_ms)
            .ok_or(KeypadFailure::Protocol)?;
        expired |= Duration::from_millis(age).saturating_add(request_started.elapsed())
            > Duration::from_millis(MAX_AGE_MS);
        events.push(e);
    }
    if events.last().is_some_and(|e| e.sequence != cursor.sequence) {
        return Err(KeypadFailure::Protocol);
    }
    Ok(KeypadBatch {
        cursor,
        events: if expired { Vec::new() } else { events },
        resync: if expired {
            Some(KeypadResync::Stale)
        } else {
            resync
        },
    })
}
fn failure(e: io::Error) -> KeypadFailure {
    if e.kind() == io::ErrorKind::PermissionDenied {
        KeypadFailure::Denied
    } else {
        KeypadFailure::Disconnected
    }
}
#[derive(Debug)]
pub struct KeypadClient {
    socket: PathBuf,
    reconnect: bool,
}
impl KeypadClient {
    pub fn new(socket: impl Into<PathBuf>) -> Self {
        Self {
            socket: socket.into(),
            reconnect: false,
        }
    }
    /// A failed exchange forces the next successful response to be a fresh head
    /// with Reconnect. Connection-per-poll is normal, not a reconnect event.
    #[cfg(unix)]
    pub fn poll(&mut self, cursor: Option<KeypadCursor>) -> Result<KeypadBatch, KeypadFailure> {
        use std::{net::Shutdown, os::unix::net::UnixStream};
        let reconnect = self.reconnect;
        let result = (|| {
            let start = Instant::now();
            let mut s = UnixStream::connect(&self.socket).map_err(failure)?;
            s.set_read_timeout(Some(Duration::from_secs(2)))
                .map_err(failure)?;
            s.set_write_timeout(Some(Duration::from_secs(2)))
                .map_err(failure)?;
            s.write_all(encode_request(if reconnect { None } else { cursor }).as_bytes())
                .map_err(failure)?;
            s.shutdown(Shutdown::Write).map_err(failure)?;
            let mut b = Vec::new();
            s.take((MAX_FRAME + 1) as u64)
                .read_to_end(&mut b)
                .map_err(failure)?;
            if b.is_empty() {
                return Err(KeypadFailure::Disconnected);
            }
            let mut batch = decode_response(&b, start)?;
            if reconnect {
                batch.events.clear();
                batch.resync = Some(KeypadResync::Reconnect)
            }
            if !reconnect {
                if let Some(c) = cursor {
                    if batch.resync.is_none()
                        && (batch.cursor.provider_instance != c.provider_instance
                            || batch
                                .events
                                .first()
                                .is_some_and(|e| c.sequence.checked_add(1) != Some(e.sequence))
                            || (batch.events.is_empty() && batch.cursor.sequence != c.sequence))
                    {
                        return Err(KeypadFailure::Protocol);
                    }
                }
            }
            Ok(batch)
        })();
        self.reconnect = result.is_err();
        // If transport lag expired a delivered batch, acquire the provider's
        // current head rather than returning its partial-batch cursor.
        if cursor.is_some()
            && result
                .as_ref()
                .is_ok_and(|b| b.resync == Some(KeypadResync::Stale))
        {
            let mut head = self.poll(None)?;
            head.events.clear();
            head.resync = Some(KeypadResync::Stale);
            return Ok(head);
        }
        result
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn codec_bounds_version_and_stale() {
        let mut ring = KeypadRing::new(7);
        let c = ring.head();
        ring.input(1, 28, 1, 0);
        let batch = ring.poll(Some(c), 0);
        let wire = encode_response(&batch, 0);
        assert_eq!(
            decode_response(wire.as_bytes(), Instant::now()).unwrap(),
            batch
        );
        assert_eq!(decode_request(&encode_request(Some(c))).unwrap(), Some(c));
        assert!(decode_response(b"FUNGOS-KEY/2 1 0 0 ok\n", Instant::now()).is_err());
        assert!(decode_response(&vec![0; MAX_FRAME + 1], Instant::now()).is_err());
        assert_eq!(
            decode_response(encode_response(&batch, 1001).as_bytes(), Instant::now())
                .unwrap()
                .resync,
            Some(KeypadResync::Stale)
        );
        assert_eq!(
            failure(io::ErrorKind::PermissionDenied.into()),
            KeypadFailure::Denied
        );
    }
    #[cfg(unix)]
    #[test]
    fn disconnect_reconnect_starts_at_head() {
        use std::os::unix::net::UnixListener;
        let p = std::env::temp_dir().join(format!("keypad-client-{}", std::process::id()));
        let l = UnixListener::bind(&p).unwrap();
        let mut client = KeypadClient::new(&p);
        let t = std::thread::spawn(move || {
            for i in 0..2 {
                let (mut s, _) = l.accept().unwrap();
                let mut q = String::new();
                s.read_to_string(&mut q).unwrap();
                if i == 1 {
                    assert_eq!(q, "key-v1\n");
                    s.write_all(encode_response(&KeypadRing::new(1).poll(None, 0), 0).as_bytes())
                        .unwrap();
                }
            }
        });
        assert_eq!(client.poll(None), Err(KeypadFailure::Disconnected));
        assert_eq!(
            client
                .poll(Some(KeypadCursor {
                    provider_instance: 1,
                    sequence: 5
                }))
                .unwrap()
                .resync,
            Some(KeypadResync::Reconnect)
        );
        t.join().unwrap();
        std::fs::remove_file(p).unwrap();
    }
}
