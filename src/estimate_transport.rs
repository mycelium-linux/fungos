//! Narrow read-only estimate client. No service, inputs, profile data or math.
//! This is model-estimate provenance, never BatterySample.percent authority.
use std::{
    io::{self, Read, Write},
    path::PathBuf,
    time::{Duration, Instant},
};
pub const REQUEST: &str = "estimate-v1";
pub const MAX_FRAME: usize = 4096;
const MAGIC: &str = "FUNGOS-ESTIMATE/1";
const TIMEOUT: Duration = Duration::from_secs(2);

/// Host-owned allowlist entry. All six fields must match the same entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustedEstimateReceipts {
    pub profile_id: String,
    pub profile_sha256: String,
    pub algorithm_id: String,
    pub algorithm_sha256: String,
    pub ocv_qualification_id: String,
    pub temperature_qualification_id: String,
}
impl TrustedEstimateReceipts {
    fn valid(&self) -> bool {
        fn id(s: &str) -> bool {
            !s.is_empty()
                && s.len() <= 128
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        }
        fn digest(s: &str) -> bool {
            s.len() == 64
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        }
        id(&self.profile_id)
            && id(&self.algorithm_id)
            && id(&self.ocv_qualification_id)
            && id(&self.temperature_qualification_id)
            && digest(&self.profile_sha256)
            && digest(&self.algorithm_sha256)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnknownReason {
    Unavailable,
    UnqualifiedOcv,
    UnqualifiedTemperature,
    AlgorithmUnverified,
    ProfileUnqualified,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EstimateFailure {
    Unknown(UnknownReason),
    Denied,
    Backend,
    Disconnected,
    Protocol,
    Stale,
    InvalidTime,
    UntrustedReceipt,
    Overflow,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelEstimate {
    pub percent: u8,
    pub receipts: TrustedEstimateReceipts,
    /// Input ages measured at estimate_observed_ms, not transport receive time.
    pub ocv_capture_age_ms: u64,
    pub temperature_capture_age_ms: u64,
    pub estimate_observed_ms: u64,
    pub provider_now_ms: u64,
}
#[derive(Clone, Debug)]
pub struct RemoteEstimate {
    sample: ModelEstimate,
    provider_age: Duration,
    transport_elapsed: Duration,
    received: Instant,
}
impl RemoteEstimate {
    pub fn provider_age(&self) -> Duration {
        self.provider_age
    }
    /// Saturating display-only age; snapshot uses checked arithmetic and fails
    /// on overflow. Access to the numeric estimate requires snapshot.
    pub fn age(&self) -> Duration {
        self.provider_age
            .saturating_add(self.transport_elapsed)
            .saturating_add(self.received.elapsed())
    }
    pub fn snapshot(&self, max_age: Duration) -> Result<ModelEstimate, EstimateFailure> {
        let age = self
            .provider_age
            .checked_add(self.transport_elapsed)
            .and_then(|v| v.checked_add(self.received.elapsed()))
            .ok_or(EstimateFailure::Overflow)?;
        if age > max_age {
            return Err(EstimateFailure::Stale);
        }
        Ok(self.sample.clone())
    }
}
#[derive(Clone, Debug)]
pub struct EstimateClient {
    socket: PathBuf,
    trusted: Vec<TrustedEstimateReceipts>,
}
impl EstimateClient {
    /// Config is supplied only by trusted host deployment, never the guest.
    pub fn new(
        socket: impl Into<PathBuf>,
        trusted: Vec<TrustedEstimateReceipts>,
    ) -> Result<Self, EstimateFailure> {
        let socket = socket.into();
        if !socket.is_absolute()
            || trusted.is_empty()
            || trusted.len() > 32
            || trusted.iter().any(|r| !r.valid())
        {
            return Err(EstimateFailure::Protocol);
        }
        Ok(Self { socket, trusted })
    }
    /// Caller authorizes the separate estimate capability BEFORE this method.
    /// A new connection per call; no last-good cache and no input injection.
    #[cfg(unix)]
    pub fn read(&self) -> Result<RemoteEstimate, EstimateFailure> {
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
            return Err(EstimateFailure::Disconnected);
        }
        self.decode_response(&frame, started)
    }
    /// Exact bounded codec tied to this client's host-owned allowlist.
    pub fn decode_response(
        &self,
        frame: &[u8],
        started: Instant,
    ) -> Result<RemoteEstimate, EstimateFailure> {
        if frame.len() > MAX_FRAME
            || !frame.ends_with(b"\n")
            || frame[..frame.len() - 1].contains(&b'\n')
        {
            return Err(EstimateFailure::Protocol);
        }
        let line = std::str::from_utf8(&frame[..frame.len() - 1])
            .map_err(|_| EstimateFailure::Protocol)?;
        let fields: Vec<_> = line.split('\t').collect();
        match fields.as_slice() {
            [MAGIC, "UNKNOWN", reason] => Err(EstimateFailure::Unknown(match *reason {
                "unavailable" => UnknownReason::Unavailable,
                "unqualified-ocv" => UnknownReason::UnqualifiedOcv,
                "unqualified-temperature" => UnknownReason::UnqualifiedTemperature,
                "algorithm-unverified" => UnknownReason::AlgorithmUnverified,
                "profile-unqualified" => UnknownReason::ProfileUnqualified,
                _ => return Err(EstimateFailure::Protocol),
            })),
            [MAGIC, "ERR", reason] => Err(match *reason {
                "denied" => EstimateFailure::Denied,
                "backend" => EstimateFailure::Backend,
                "disconnected" => EstimateFailure::Disconnected,
                "protocol" => EstimateFailure::Protocol,
                "stale" => EstimateFailure::Stale,
                "invalid-time" => EstimateFailure::InvalidTime,
                "untrusted-receipt" => EstimateFailure::UntrustedReceipt,
                "overflow" => EstimateFailure::Overflow,
                _ => EstimateFailure::Protocol,
            }),
            [MAGIC, "OK", percent, pid, psha, aid, asha, oq, tq, oa, ta, observed, now] => {
                let receipts = TrustedEstimateReceipts {
                    profile_id: (*pid).into(),
                    profile_sha256: (*psha).into(),
                    algorithm_id: (*aid).into(),
                    algorithm_sha256: (*asha).into(),
                    ocv_qualification_id: (*oq).into(),
                    temperature_qualification_id: (*tq).into(),
                };
                if !receipts.valid() {
                    return Err(EstimateFailure::Protocol);
                }
                if !self.trusted.contains(&receipts) {
                    return Err(EstimateFailure::UntrustedReceipt);
                }
                let percent =
                    u8::try_from(number(percent)?).map_err(|_| EstimateFailure::Protocol)?;
                if percent > 100 {
                    return Err(EstimateFailure::Protocol);
                }
                let ocv_capture_age_ms = number(oa)?;
                let temperature_capture_age_ms = number(ta)?;
                let estimate_observed_ms = number(observed)?;
                let provider_now_ms = number(now)?;
                let elapsed = provider_now_ms
                    .checked_sub(estimate_observed_ms)
                    .ok_or(EstimateFailure::InvalidTime)?;
                let age_ms = ocv_capture_age_ms
                    .max(temperature_capture_age_ms)
                    .checked_add(elapsed)
                    .ok_or(EstimateFailure::Overflow)?;
                let transport_elapsed = Instant::now()
                    .checked_duration_since(started)
                    .ok_or(EstimateFailure::InvalidTime)?;
                Ok(RemoteEstimate {
                    sample: ModelEstimate {
                        percent,
                        receipts,
                        ocv_capture_age_ms,
                        temperature_capture_age_ms,
                        estimate_observed_ms,
                        provider_now_ms,
                    },
                    provider_age: Duration::from_millis(age_ms),
                    transport_elapsed,
                    received: Instant::now(),
                })
            }
            _ => Err(EstimateFailure::Protocol),
        }
    }
}
fn number(s: &str) -> Result<u64, EstimateFailure> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(EstimateFailure::Protocol);
    }
    s.parse().map_err(|_| EstimateFailure::Protocol)
}
fn io_failure(e: io::Error) -> EstimateFailure {
    if e.kind() == io::ErrorKind::PermissionDenied {
        EstimateFailure::Denied
    } else {
        EstimateFailure::Disconnected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn receipt() -> TrustedEstimateReceipts {
        TrustedEstimateReceipts {
            profile_id: "synthetic-profile".into(),
            profile_sha256: "a".repeat(64),
            algorithm_id: "synthetic-algorithm".into(),
            algorithm_sha256: "b".repeat(64),
            ocv_qualification_id: "ocv-qualified-test".into(),
            temperature_qualification_id: "temp-qualified-test".into(),
        }
    }
    fn client() -> EstimateClient {
        EstimateClient::new("/not-a-live-estimator", vec![receipt()]).unwrap()
    }
    fn wire() -> String {
        let r = receipt();
        format!(
            "FUNGOS-ESTIMATE/1\tOK\t42\t{}\t{}\t{}\t{}\t{}\t{}\t10\t20\t100\t105\n",
            r.profile_id,
            r.profile_sha256,
            r.algorithm_id,
            r.algorithm_sha256,
            r.ocv_qualification_id,
            r.temperature_qualification_id
        )
    }
    #[test]
    fn trust_freshness_and_checked_time() {
        let c = client();
        let r = c
            .decode_response(wire().as_bytes(), Instant::now() - Duration::from_millis(5))
            .unwrap();
        assert_eq!(r.provider_age(), Duration::from_millis(25));
        assert!(r.age() >= Duration::from_millis(30));
        assert_eq!(r.snapshot(Duration::from_secs(1)).unwrap().percent, 42);
        assert_eq!(r.snapshot(Duration::ZERO), Err(EstimateFailure::Stale));
        let mut fields: Vec<String> = wire().trim_end().split('\t').map(String::from).collect();
        fields[12] = "99".into();
        assert_eq!(
            c.decode_response(
                format!("{}\n", fields.join("\t")).as_bytes(),
                Instant::now()
            )
            .unwrap_err(),
            EstimateFailure::InvalidTime
        );
        fields[12] = "105".into();
        fields[9] = u64::MAX.to_string();
        assert_eq!(
            c.decode_response(
                format!("{}\n", fields.join("\t")).as_bytes(),
                Instant::now()
            )
            .unwrap_err(),
            EstimateFailure::Overflow
        );
        assert_eq!(
            c.decode_response(wire().as_bytes(), Instant::now() + Duration::from_secs(1))
                .unwrap_err(),
            EstimateFailure::InvalidTime
        );
        assert_eq!(
            c.decode_response(
                wire()
                    .replace("synthetic-profile", "other-profile")
                    .as_bytes(),
                Instant::now()
            )
            .unwrap_err(),
            EstimateFailure::UntrustedReceipt
        );
        // Every field in the tuple must match one complete entry, not a mix of entries.
        let mut other = receipt();
        other.profile_id = "other-profile".into();
        other.algorithm_id = "other-algorithm".into();
        let mixed = EstimateClient::new("/unused", vec![receipt(), other]).unwrap();
        assert_eq!(
            mixed
                .decode_response(
                    wire()
                        .replace("synthetic-algorithm", "other-algorithm")
                        .as_bytes(),
                    Instant::now()
                )
                .unwrap_err(),
            EstimateFailure::UntrustedReceipt
        );
        assert!(EstimateClient::new("relative", vec![receipt()]).is_err());
        assert!(EstimateClient::new("/unused", vec![]).is_err());
    }
    #[test]
    fn strict_malformed_unknown_no_numbers() {
        let c = client();
        for frame in [
            b"".as_slice(),
            b"\xff\n",
            b"FUNGOS-ESTIMATE/2\tUNKNOWN\tunavailable\n",
            b"FUNGOS-ESTIMATE/1\tUNKNOWN\tunavailable\t42\n",
            b"FUNGOS-ESTIMATE/1\tUNKNOWN\tguess\n",
            b"FUNGOS-ESTIMATE/1\tERR\tdenied\textra\n",
            b"FUNGOS-ESTIMATE/1\tERR\tdenied\nextra\n",
            b"FUNGOS-ESTIMATE/1\tERR\tdenied",
            b"FUNGOS-ESTIMATE/1\tOK\t42\n",
        ] {
            assert_eq!(
                c.decode_response(frame, Instant::now()).unwrap_err(),
                EstimateFailure::Protocol
            )
        }
        for value in ["101", "256", "-1", "+1", "1.5", ""] {
            assert_eq!(
                c.decode_response(
                    wire()
                        .replacen("\t42\t", &format!("\t{value}\t"), 1)
                        .as_bytes(),
                    Instant::now()
                )
                .unwrap_err(),
                EstimateFailure::Protocol
            )
        }
        assert_eq!(
            c.decode_response(&vec![b'X'; MAX_FRAME + 1], Instant::now())
                .unwrap_err(),
            EstimateFailure::Protocol
        );
        for (s, e) in [
            ("unavailable", UnknownReason::Unavailable),
            ("unqualified-ocv", UnknownReason::UnqualifiedOcv),
            (
                "unqualified-temperature",
                UnknownReason::UnqualifiedTemperature,
            ),
            ("algorithm-unverified", UnknownReason::AlgorithmUnverified),
            ("profile-unqualified", UnknownReason::ProfileUnqualified),
        ] {
            assert_eq!(
                c.decode_response(
                    format!("FUNGOS-ESTIMATE/1\tUNKNOWN\t{s}\n").as_bytes(),
                    Instant::now()
                )
                .unwrap_err(),
                EstimateFailure::Unknown(e)
            )
        }
        assert_eq!(
            c.decode_response(b"FUNGOS-ESTIMATE/1\tERR\tdenied\n", Instant::now())
                .unwrap_err(),
            EstimateFailure::Denied
        );
        assert_eq!(
            io_failure(io::ErrorKind::PermissionDenied.into()),
            EstimateFailure::Denied
        );
    }
    #[cfg(unix)]
    #[test]
    fn socket_readonly_reconnect() {
        use std::os::unix::net::UnixListener;
        let path = std::env::temp_dir().join(format!("estimate-test-{}", std::process::id()));
        let listener = UnixListener::bind(&path).unwrap();
        let c = EstimateClient::new(&path, vec![receipt()]).unwrap();
        let t = std::thread::spawn(move || {
            for reply in [
                None,
                Some("FUNGOS-ESTIMATE/1\tUNKNOWN\tunqualified-ocv\n".into()),
                Some(wire()),
            ] {
                let (mut s, _) = listener.accept().unwrap();
                let mut q = String::new();
                s.read_to_string(&mut q).unwrap();
                assert_eq!(q, "estimate-v1\n");
                if let Some(reply) = reply {
                    s.write_all(reply.as_bytes()).unwrap();
                }
            }
        });
        assert_eq!(c.read().unwrap_err(), EstimateFailure::Disconnected);
        assert_eq!(
            c.read().unwrap_err(),
            EstimateFailure::Unknown(UnknownReason::UnqualifiedOcv)
        );
        assert_eq!(
            c.read()
                .unwrap()
                .snapshot(Duration::from_secs(1))
                .unwrap()
                .percent,
            42
        );
        t.join().unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(c.read().unwrap_err(), EstimateFailure::Disconnected);
    }
}
