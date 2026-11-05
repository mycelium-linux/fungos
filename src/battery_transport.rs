//! Shared bounded v1 battery wire codec and permissioned one-shot socket client.
//! Consumers import owner types; Debug formatting is NOT a wire protocol.
use crate::services::{AccessoryFailure, BatteryProvenance, BatterySample, ChargingState};
use std::{
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub const REQUEST: &str = "battery-v1";
pub const MAX_FRAME: usize = 4096;
const MAGIC: &str = "FUNGOS-BATTERY/1";
const TIMEOUT: Duration = Duration::from_secs(2);

fn error_name(error: BatteryFailure) -> &'static str {
    match error {
        BatteryFailure::Unavailable => "unavailable",
        BatteryFailure::Denied => "denied",
        BatteryFailure::Backend => "backend",
        BatteryFailure::Ambiguous => "ambiguous",
        BatteryFailure::Stale => "stale",
        BatteryFailure::InvalidTime => "invalid-time",
        BatteryFailure::Disconnected => "disconnected",
        BatteryFailure::Protocol => "protocol",
    }
}
fn parse_error(value: &str) -> Result<BatteryFailure, BatteryFailure> {
    [
        BatteryFailure::Unavailable,
        BatteryFailure::Denied,
        BatteryFailure::Backend,
        BatteryFailure::Ambiguous,
        BatteryFailure::Stale,
        BatteryFailure::InvalidTime,
        BatteryFailure::Disconnected,
        BatteryFailure::Protocol,
    ]
    .into_iter()
    .find(|e| error_name(*e) == value)
    .ok_or(BatteryFailure::Protocol)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BatteryFailure {
    Unavailable,
    Denied,
    Backend,
    Ambiguous,
    Stale,
    InvalidTime,
    Disconnected,
    Protocol,
}
impl From<AccessoryFailure> for BatteryFailure {
    fn from(e: AccessoryFailure) -> Self {
        match e {
            AccessoryFailure::Unavailable => Self::Unavailable,
            AccessoryFailure::Denied => Self::Denied,
            AccessoryFailure::Backend => Self::Backend,
            AccessoryFailure::Stale => Self::Stale,
            AccessoryFailure::InvalidTime => Self::InvalidTime,
        }
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
fn decode_path(hex: &str) -> Result<PathBuf, BatteryFailure> {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    if hex.is_empty() || !hex.len().is_multiple_of(2) || !hex.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(BatteryFailure::Protocol);
    }
    let bytes = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| BatteryFailure::Protocol))
        .collect::<Result<Vec<_>, _>>()?;
    if !bytes.starts_with(b"/") || bytes.iter().any(|b| matches!(b, 0 | 10 | 13)) {
        return Err(BatteryFailure::Protocol);
    }
    Ok(OsString::from_vec(bytes).into())
}
/// Valid only while retained locally. Provider timestamp is provenance, not a
/// timestamp in the guest's clock domain. No stale/disconnected fallback cache.
#[derive(Clone, Debug)]
pub struct RemoteBatterySample {
    sample: BatterySample,
    provider_age_ms: u64,
    transport_elapsed: Duration,
    received: Instant,
}
impl RemoteBatterySample {
    pub fn sample(&self) -> &BatterySample {
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
    pub fn snapshot(&self, max_age: Duration) -> Result<BatterySample, BatteryFailure> {
        if self.age() > max_age {
            Err(BatteryFailure::Stale)
        } else {
            Ok(self.sample.clone())
        }
    }
}

fn io_failure(e: io::Error) -> BatteryFailure {
    match e.kind() {
        io::ErrorKind::PermissionDenied => BatteryFailure::Denied,
        _ => BatteryFailure::Disconnected,
    }
}
#[derive(Clone, Debug)]
pub struct BatteryClient {
    socket: PathBuf,
}
impl BatteryClient {
    /// Explicit trusted deployment socket path; no device discovery in the guest.
    pub fn new(socket: impl Into<PathBuf>) -> Self {
        Self {
            socket: socket.into(),
        }
    }
    /// New connection on every call (reconnect by calling again). Caller must
    /// authorize its guest before invoking this method. Unix DAC remains active.
    #[cfg(unix)]
    pub fn read(&self) -> Result<RemoteBatterySample, BatteryFailure> {
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
            return Err(BatteryFailure::Disconnected);
        }
        decode_response(&frame, started)
    }
}

fn charging_name(c: ChargingState) -> &'static str {
    match c {
        ChargingState::Unknown => "unknown",
        ChargingState::Charging => "charging",
        ChargingState::Discharging => "discharging",
        ChargingState::NotCharging => "not-charging",
        ChargingState::Full => "full",
    }
}
fn optional<T: std::fmt::Display>(v: Option<T>) -> String {
    v.map(|v| v.to_string()).unwrap_or("unknown".into())
}
/// Read timestamp must precede sysfs reads; now is sampled after them.
#[cfg(unix)]
pub fn encode_response(sample: Result<BatterySample, BatteryFailure>, now: u64) -> String {
    let result = sample.and_then(|s| {
        s.check_freshness(now, 1000).map_err(BatteryFailure::from)?;
        if s.percent.is_some_and(|p| p > 100) {
            return Err(BatteryFailure::Protocol);
        }
        let BatteryProvenance::LinuxPowerSupply(path) = s.provenance;
        let hex = path_hex(&path);
        decode_path(&hex)?;
        let frame = format!(
            "{MAGIC}\tOK\t{}\t{}\t{}\t{}\t{}\t{}\t{}\tlinux-power-supply\t{}\n",
            optional(s.percent),
            charging_name(s.charging),
            optional(s.voltage_microvolts),
            optional(s.current_microamps),
            optional(s.temperature_deci_celsius),
            s.observed_monotonic_ms,
            now - s.observed_monotonic_ms,
            hex
        );
        if frame.len() > MAX_FRAME {
            Err(BatteryFailure::Protocol)
        } else {
            Ok(frame)
        }
    });
    result.unwrap_or_else(|e| format!("{MAGIC}\tERR\t{}\n", error_name(e)))
}
fn number<T: std::str::FromStr>(s: &str) -> Result<T, BatteryFailure> {
    let digits = s.strip_prefix('-').unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(BatteryFailure::Protocol);
    }
    s.parse().map_err(|_| BatteryFailure::Protocol)
}
fn parse_optional<T: std::str::FromStr>(s: &str) -> Result<Option<T>, BatteryFailure> {
    if s == "unknown" {
        Ok(None)
    } else {
        number(s).map(Some)
    }
}
#[cfg(unix)]
pub fn decode_response(
    frame: &[u8],
    request_started: Instant,
) -> Result<RemoteBatterySample, BatteryFailure> {
    if frame.len() > MAX_FRAME
        || !frame.ends_with(b"\n")
        || frame[..frame.len() - 1].contains(&b'\n')
    {
        return Err(BatteryFailure::Protocol);
    }
    let line =
        std::str::from_utf8(&frame[..frame.len() - 1]).map_err(|_| BatteryFailure::Protocol)?;
    let fields: Vec<_> = line.split('\t').collect();
    match fields.as_slice() {
        [MAGIC, "ERR", reason] => Err(parse_error(reason)?),
        [MAGIC, "OK", percent, charging, voltage, current, temp, observed, age, "linux-power-supply", path] =>
        {
            let percent: Option<u8> = parse_optional(percent)?;
            if percent.is_some_and(|p| p > 100) {
                return Err(BatteryFailure::Protocol);
            }
            let charging = match *charging {
                "unknown" => ChargingState::Unknown,
                "charging" => ChargingState::Charging,
                "discharging" => ChargingState::Discharging,
                "not-charging" => ChargingState::NotCharging,
                "full" => ChargingState::Full,
                _ => return Err(BatteryFailure::Protocol),
            };
            let sample = BatterySample {
                percent,
                charging,
                voltage_microvolts: parse_optional(voltage)?,
                current_microamps: parse_optional(current)?,
                temperature_deci_celsius: parse_optional(temp)?,
                observed_monotonic_ms: number(observed)?,
                provenance: BatteryProvenance::LinuxPowerSupply(decode_path(path)?),
            };
            let provider_age_ms = number(age)?;
            if provider_age_ms > 1000 {
                return Err(BatteryFailure::Stale);
            }
            Ok(RemoteBatterySample {
                sample,
                provider_age_ms,
                transport_elapsed: request_started.elapsed(),
                received: Instant::now(),
            })
        }
        _ => Err(BatteryFailure::Protocol),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> BatterySample {
        BatterySample {
            percent: None,
            charging: ChargingState::Unknown,
            voltage_microvolts: Some(3878682),
            current_microamps: None,
            temperature_deci_celsius: None,
            provenance: BatteryProvenance::LinuxPowerSupply(
                "/sys/class/power_supply/cat-s22-battery".into(),
            ),
            observed_monotonic_ms: 100,
        }
    }
    #[test]
    fn unknown_zero_bounds_and_freshness() {
        let mut s = sample();
        for p in [None, Some(0), Some(100)] {
            s.percent = p;
            let wire = encode_response(Ok(s.clone()), 105);
            let r = decode_response(wire.as_bytes(), Instant::now() - Duration::from_millis(5))
                .unwrap();
            assert_eq!(r.sample(), &s);
            assert_eq!(r.provider_age(), Duration::from_millis(5));
            assert!(r.age() >= Duration::from_millis(10));
            assert_eq!(r.snapshot(Duration::ZERO), Err(BatteryFailure::Stale));
            assert_eq!(r.snapshot(Duration::from_secs(1)).unwrap(), s);
        }
        assert!(encode_response(Ok(sample()), 99).contains("invalid-time"));
        assert!(encode_response(Ok(sample()), 1101).contains("stale"));
        s.percent = Some(101);
        assert!(encode_response(Ok(s), 100).contains("protocol"));
        let mut s = sample();
        s.current_microamps = Some(i32::MIN);
        s.temperature_deci_celsius = Some(i32::MAX);
        s.voltage_microvolts = Some(u32::MAX);
        assert_eq!(
            decode_response(
                encode_response(Ok(s.clone()), 100).as_bytes(),
                Instant::now()
            )
            .unwrap()
            .sample(),
            &s
        );
    }
    #[test]
    fn exact_bounded_parser_and_errors() {
        let good = encode_response(Ok(sample()), 100);
        for (from, to) in [
            ("unknown", "101"),
            ("3878682", "4294967296"),
            ("3878682", "-1"),
            ("100\t0", "-1\t0"),
            ("100\t0", "100\t1001"),
            ("linux-power-supply", "invented"),
            (
                "unknown\tunknown\t3878682\tunknown",
                "unknown\tunknown\t3878682\t2147483648",
            ),
            ("3878682\tunknown\tunknown", "3878682\tunknown\t-2147483649"),
            ("2f737973", "zz737973"),
        ] {
            assert!(
                decode_response(good.replacen(from, to, 1).as_bytes(), Instant::now()).is_err()
            );
        }
        for path in [
            "00",
            "2f00",
            "2f0a",
            "2f0d",
            "1",
            "gg",
            "72656c6174697665",
            "",
        ] {
            let mut fields: Vec<_> = good.trim_end().split('\t').collect();
            fields[10] = path;
            assert_eq!(
                decode_response(
                    format!("{}\n", fields.join("\t")).as_bytes(),
                    Instant::now()
                )
                .unwrap_err(),
                BatteryFailure::Protocol
            );
        }
        for wire in [
            b"".as_slice(),
            b"\xff\n",
            b"FUNGOS-BATTERY/1\tERR\tdenied\textra\n",
            b"FUNGOS-BATTERY/1\tERR\n",
            b"FUNGOS-BATTERY/1\tERR\tguess\n",
            b"FUNGOS-BATTERY/1\tERR\tdenied\nextra\n",
        ] {
            assert_eq!(
                decode_response(wire, Instant::now()).unwrap_err(),
                BatteryFailure::Protocol
            )
        }
        assert_eq!(
            decode_response(&vec![b'X'; MAX_FRAME + 1], Instant::now()).unwrap_err(),
            BatteryFailure::Protocol
        );
        for e in [
            BatteryFailure::Unavailable,
            BatteryFailure::Denied,
            BatteryFailure::Backend,
            BatteryFailure::Ambiguous,
            BatteryFailure::Stale,
            BatteryFailure::InvalidTime,
            BatteryFailure::Disconnected,
            BatteryFailure::Protocol,
        ] {
            assert_eq!(
                decode_response(encode_response(Err(e), 0).as_bytes(), Instant::now()).unwrap_err(),
                e
            )
        }
    }
    #[test]
    fn disconnect_denial_reconnect() {
        use std::os::unix::net::UnixListener;
        let path = std::env::temp_dir().join(format!("battery-codec-{}", std::process::id()));
        let listener = UnixListener::bind(&path).unwrap();
        let client = BatteryClient::new(&path);
        let t = std::thread::spawn(move || {
            for response in [
                None,
                Some(encode_response(Err(BatteryFailure::Denied), 0)),
                Some(encode_response(Ok(sample()), 100)),
            ] {
                let (mut s, _) = listener.accept().unwrap();
                let mut q = String::new();
                s.read_to_string(&mut q).unwrap();
                assert_eq!(q, "battery-v1\n");
                if let Some(r) = response {
                    s.write_all(r.as_bytes()).unwrap();
                }
            }
        });
        assert_eq!(client.read().unwrap_err(), BatteryFailure::Disconnected);
        assert_eq!(client.read().unwrap_err(), BatteryFailure::Denied);
        assert_eq!(client.read().unwrap().sample().percent, None);
        t.join().unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(client.read().unwrap_err(), BatteryFailure::Disconnected);
        assert_eq!(
            io_failure(io::ErrorKind::PermissionDenied.into()),
            BatteryFailure::Denied
        );
    }
}
