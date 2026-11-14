//! Read-only keypad mode: no combined backend construction or control endpoints.
use fungos_device_hal::{keypad::KeypadRing, keypad_transport};
use std::{
    env, fs,
    io::{self, Read, Write},
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
    path::PathBuf,
    thread,
    time::Duration,
};
fn monotonic_ms() -> Result<u64, String> {
    let mut t: libc::timespec = unsafe { std::mem::zeroed() };
    // SAFETY: valid output pointer, clock has no device side effects.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut t) } != 0 {
        return Err(io::Error::last_os_error().to_string());
    }
    Ok(t.tv_sec as u64 * 1000 + t.tv_nsec as u64 / 1_000_000)
}
fn drain(input: &mut impl Read, ring: &mut KeypadRing) -> Result<(), String> {
    let mut buf = [0u8; 24];
    for _ in 0..256 {
        match input.read(&mut buf) {
            Ok(24) => {
                let sec = i64::from_ne_bytes(buf[..8].try_into().unwrap());
                let usec = i64::from_ne_bytes(buf[8..16].try_into().unwrap());
                if sec < 0 || !(0..1_000_000).contains(&usec) {
                    return Err("invalid input timestamp".into());
                }
                let observed = (sec as u64)
                    .checked_mul(1000)
                    .and_then(|n| n.checked_add(usec as u64 / 1000))
                    .ok_or("input timestamp overflow")?;
                ring.input(
                    u16::from_ne_bytes(buf[16..18].try_into().unwrap()),
                    u16::from_ne_bytes(buf[18..20].try_into().unwrap()),
                    i32::from_ne_bytes(buf[20..24].try_into().unwrap()),
                    observed,
                );
            }
            Ok(_) => return Err("input disconnected or partial input_event".into()),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    // Never serve while an unread backlog may contain SYN_DROPPED or old input.
    Err("input drain budget exhausted".into())
}
pub fn run() -> Result<(), String> {
    let socket = env::var_os("FUNGOS_KEYPAD_SOCKET")
        .map(PathBuf::from)
        .ok_or("--keypad-only requires explicit FUNGOS_KEYPAD_SOCKET")?;
    if !socket.is_absolute() {
        return Err("FUNGOS_KEYPAD_SOCKET must be absolute".into());
    }
    let name = env::var("FUNGOS_INPUT_NAME").map_err(|_| "FUNGOS_INPUT_NAME required")?;
    if name.trim().is_empty() || name.contains('\n') || name.len() > 256 {
        return Err("invalid input identity".into());
    }
    let sys = env::var_os("FUNGOS_SYS_INPUT_ROOT")
        .map(PathBuf::from)
        .unwrap_or("/sys/class/input".into());
    let dev = env::var_os("FUNGOS_INPUT_ROOT")
        .map(PathBuf::from)
        .unwrap_or("/dev/input".into());
    let mut matches = Vec::new();
    for entry in fs::read_dir(sys).map_err(|e| e.to_string())? {
        let p = entry.map_err(|e| e.to_string())?.path();
        if p.file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.starts_with("event"))
            && fs::read_to_string(p.join("device/name"))
                .or_else(|_| fs::read_to_string(p.join("name")))
                .is_ok_and(|s| s.trim() == name)
        {
            matches.push(p);
        }
    }
    if matches.len() != 1 {
        return Err("keypad identity missing or ambiguous".into());
    }
    let path = dev.join(matches[0].file_name().ok_or("missing event name")?);
    let mut input = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(&path)
        .map_err(|e| e.to_string())?;
    // Linux EVIOCSCLOCKID: per-file timestamp clock only, no EVIOCGRAB or hardware write.
    let clock = libc::CLOCK_MONOTONIC as libc::c_int;
    if unsafe { libc::ioctl(input.as_raw_fd(), 0x400445a0, &clock) } < 0 {
        return Err(format!("EVIOCSCLOCKID: {}", io::Error::last_os_error()));
    }
    let mut bytes = [0; 8];
    fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|e| e.to_string())?;
    let mut ring = KeypadRing::new(u64::from_ne_bytes(bytes));
    let _signals = super::lid_only::Signals::install()?;
    let (listener, _owned) = super::lid_only::bind(socket)?;
    eprintln!(
        "keypad-only: read-only {} identity={name}; only key-v1",
        path.display()
    );
    while !super::lid_only::terminated() {
        drain(&mut input, &mut ring)?;
        match listener.accept() {
            Ok((mut stream, _)) => {
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .map_err(|e| e.to_string())?;
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .map_err(|e| e.to_string())?;
                let mut request = String::new();
                let read = (&mut stream).take(129).read_to_string(&mut request);
                let response = if read.is_ok() {
                    match keypad_transport::decode_request(&request) {
                        Ok(cursor) => {
                            drain(&mut input, &mut ring)?;
                            let now = monotonic_ms()?;
                            keypad_transport::encode_response(&ring.poll(cursor, now), now)
                        }
                        Err(_) => "ERR keypad-only supports only key-v1\n".into(),
                    }
                } else {
                    "ERR invalid bounded request\n".into()
                };
                let _ = stream.write_all(response.as_bytes());
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10))
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Mock {
        bytes: std::io::Cursor<Vec<u8>>,
    }
    impl Read for Mock {
        fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
            if self.bytes.position() == self.bytes.get_ref().len() as u64 {
                Err(io::ErrorKind::WouldBlock.into())
            } else {
                self.bytes.read(b)
            }
        }
    }
    #[test]
    fn acquisition_preserves_kernel_age_and_fails_on_eof() {
        let mut ev = vec![0; 24];
        ev[..8].copy_from_slice(&1i64.to_ne_bytes());
        ev[16..18].copy_from_slice(&1u16.to_ne_bytes());
        ev[18..20].copy_from_slice(&28u16.to_ne_bytes());
        ev[20..24].copy_from_slice(&1i32.to_ne_bytes());
        let mut ring = KeypadRing::new(1);
        let c = ring.head();
        drain(
            &mut Mock {
                bytes: io::Cursor::new(ev),
            },
            &mut ring,
        )
        .unwrap();
        assert_eq!(
            ring.poll(Some(c), 1000).events[0].observed_monotonic_ms,
            1000
        );
        assert_eq!(
            ring.poll(Some(c), 2001).resync,
            Some(fungos_device_hal::keypad::KeypadResync::Stale)
        );
        assert!(drain(&mut io::Cursor::new(Vec::<u8>::new()), &mut ring).is_err());
    }
    #[test]
    fn no_combined_backend() {
        let s = include_str!("keypad_only.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        for forbidden in [
            "KeyboardLight",
            "PowerSupply",
            "reconcile",
            "Sysfs",
            "super::run(",
            "Config::",
            "/dev/snd",
        ] {
            assert!(!s.contains(forbidden));
        }
    }
}
