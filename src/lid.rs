//! Read-only lid observation. Hardware assertion is separate from qualified
//! physical mapping and never controls display, suspend, or keyboard-light policy.
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LidPosition {
    /// Physical polarity has not been independently qualified on this device.
    Unqualified,
    Open,
    Closed,
}
/// Deployment qualification, not inferred from an input device's name or DT.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LidPolarity {
    #[default]
    Unqualified,
    /// Enable only after board qualification proves standard SW_LID semantics.
    StandardClosedAsserted,
}
impl LidPolarity {
    fn position(self, asserted: bool) -> LidPosition {
        match self {
            Self::Unqualified => LidPosition::Unqualified,
            Self::StandardClosedAsserted if asserted => LidPosition::Closed,
            Self::StandardClosedAsserted => LidPosition::Open,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LidSample {
    pub switch_asserted: bool,
    pub physical_position: LidPosition,
    pub event_node: PathBuf,
    /// Successful EVIOCGSW observation, in the provider monotonic clock domain.
    pub observed_monotonic_ms: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LidFailure {
    Unavailable,
    Denied,
    Backend,
    Ambiguous,
    Resynchronizing,
    Stale,
    InvalidTime,
    Disconnected,
    Protocol,
}
impl LidSample {
    pub fn check_freshness(&self, now: u64, max_age: u64) -> Result<(), LidFailure> {
        let age = now
            .checked_sub(self.observed_monotonic_ms)
            .ok_or(LidFailure::InvalidTime)?;
        if age > max_age {
            Err(LidFailure::Stale)
        } else {
            Ok(())
        }
    }
}

/// SYN_DROPPED invalidates all cached state until the next SYN_REPORT and a
/// successful EVIOCGSW. Querying during the discarded interval is insufficient.
#[derive(Default)]
struct SyncState {
    dropped: bool,
}
impl SyncState {
    fn event(&mut self, ty: u16, code: u16) -> bool {
        if ty == 0 && code == 3 {
            self.dropped = true;
        }
        if ty == 0 && code == 0 {
            self.dropped = false;
            return true;
        }
        false
    }
}

mod linux {
    use super::*;
    #[cfg(target_os = "linux")]
    use std::os::fd::AsRawFd;
    use std::{
        fs::{self, File, OpenOptions},
        io::{self, Read},
        os::unix::fs::OpenOptionsExt,
        path::Path,
    };

    fn failure(e: io::Error) -> LidFailure {
        match e.kind() {
            io::ErrorKind::PermissionDenied => LidFailure::Denied,
            io::ErrorKind::NotFound => LidFailure::Unavailable,
            _ => LidFailure::Backend,
        }
    }
    // Linux asm-generic ioctl ABI (supported ARM64/x86_64 targets). Buffer length
    // is encoded, not pointer size. No EVIOCGRAB, writes, or input injection.
    #[cfg(target_os = "linux")]
    fn bits(file: &File, nr: u8, buf: &mut [u8]) -> Result<(), LidFailure> {
        let request =
            (2u32 << 30) | ((buf.len() as u32) << 16) | (u32::from(b'E') << 8) | u32::from(nr);
        // SAFETY: writable buffer is valid for exactly the encoded length; fd
        // remains owned/live for the synchronous ioctl.
        let rc = unsafe { libc::ioctl(file.as_raw_fd(), request as _, buf.as_mut_ptr()) };
        if rc < 0 {
            Err(failure(io::Error::last_os_error()))
        } else {
            Ok(())
        }
    }
    #[cfg(not(target_os = "linux"))]
    fn bits(_: &File, _: u8, _: &mut [u8]) -> Result<(), LidFailure> {
        Err(LidFailure::Unavailable)
    }
    // Linux UAPI input_event layout uses native timeval (ARM64/x86_64).
    #[repr(C)]
    struct InputEvent {
        time: libc::timeval,
        type_: u16,
        code: u16,
        value: i32,
    }
    fn query(file: &File) -> Result<bool, LidFailure> {
        let mut sw = [0u8; 16];
        bits(file, 0x1b, &mut sw)?; // EVIOCGSW, SW_LID=0
        Ok(sw[0] & 1 != 0)
    }
    fn discover(sys: &Path, dev: &Path, name: &str) -> Result<(File, PathBuf), LidFailure> {
        let mut found = None;
        for entry in fs::read_dir(sys).map_err(failure)? {
            let entry = entry.map_err(failure)?;
            let n = entry.file_name();
            let Some(n) = n.to_str() else { continue };
            if !n
                .strip_prefix("event")
                .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            {
                continue;
            }
            let actual = fs::read_to_string(entry.path().join("device/name")).map_err(failure)?;
            if actual.trim() != name {
                continue;
            }
            let path = dev.join(n);
            let file = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
                .open(&path)
                .map_err(failure)?;
            let mut ev = [0u8; 4];
            bits(&file, 0x20, &mut ev)?; // EVIOCGBIT(0): event types
            if ev[0] & (1 << 5) == 0 {
                continue;
            } // EV_SW
            let mut sw = [0u8; 16];
            bits(&file, 0x25, &mut sw)?; // EVIOCGBIT(EV_SW)
            if sw[0] & 1 == 0 {
                continue;
            }
            if found.is_some() {
                return Err(LidFailure::Ambiguous);
            }
            found = Some((file, path));
        }
        found.ok_or(LidFailure::Unavailable)
    }
    fn observe(
        reader: &mut impl Read,
        sync: &mut SyncState,
        query: impl FnOnce() -> Result<bool, LidFailure>,
    ) -> Result<bool, LidFailure> {
        // Native Linux UAPI layout, not a fixed-size byte offset decoder.
        // SAFETY: zero is valid for all integer/timeval fields.
        let mut event: InputEvent = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of_val(&event);
        for _ in 0..256 {
            // SAFETY: initialized integer-only repr(C) storage is writable for
            // exactly size bytes and remains live through the read.
            let bytes = unsafe {
                std::slice::from_raw_parts_mut((&mut event as *mut InputEvent).cast::<u8>(), size)
            };
            match reader.read(bytes) {
                Ok(n) if n == size => {
                    sync.event(event.type_, event.code);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    return if sync.dropped {
                        Err(LidFailure::Resynchronizing)
                    } else {
                        query()
                    };
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Ok(0) => return Err(LidFailure::Unavailable),
                Ok(_) => return Err(LidFailure::Backend),
                Err(e) => return Err(failure(e)),
            }
        }
        Err(LidFailure::Resynchronizing)
    }
    pub struct LidInput {
        sys: PathBuf,
        dev: PathBuf,
        name: String,
        polarity: LidPolarity,
        file: Option<File>,
        node: PathBuf,
        sync: SyncState,
        sample: Result<LidSample, LidFailure>,
        next_discovery: u64,
    }
    impl LidInput {
        pub fn new(sys: PathBuf, dev: PathBuf, name: String) -> Self {
            Self {
                sys,
                dev,
                name,
                polarity: LidPolarity::Unqualified,
                file: None,
                node: PathBuf::new(),
                sync: SyncState::default(),
                sample: Err(LidFailure::Unavailable),
                next_discovery: 0,
            }
        }
        /// Trusted board-deployment setting. Never automatically qualify by
        /// device identity; raw switch_asserted is retained under either mode.
        pub fn with_polarity(mut self, polarity: LidPolarity) -> Self {
            self.polarity = polarity;
            self
        }
        fn fail(&mut self, e: LidFailure, now: u64) {
            self.sample = Err(e);
            self.file = None;
            self.sync = SyncState::default();
            self.next_discovery = now.saturating_add(1000);
        }
        /// Bounded draining in the existing provider loop. A fresh ioctl is the
        /// source of truth; event values/timestamps never manufacture a reading.
        pub fn poll(&mut self, now: u64) {
            if self.file.is_none() {
                if now < self.next_discovery {
                    return;
                }
                match discover(&self.sys, &self.dev, &self.name) {
                    Ok((f, p)) => {
                        self.file = Some(f);
                        self.node = p;
                    }
                    Err(e) => {
                        self.fail(e, now);
                        return;
                    }
                }
            }
            let result = {
                let f = self.file.as_ref().unwrap();
                observe(&mut &*f, &mut self.sync, || query(f))
            };
            match result {
                Ok(value) => {
                    self.sample = Ok(LidSample {
                        switch_asserted: value,
                        physical_position: self.polarity.position(value),
                        event_node: self.node.clone(),
                        observed_monotonic_ms: now,
                    })
                }
                Err(LidFailure::Resynchronizing) => self.sample = Err(LidFailure::Resynchronizing),
                Err(e) => self.fail(e, now),
            }
        }
        pub fn snapshot(&self, now: u64, max_age: u64) -> Result<LidSample, LidFailure> {
            let s = self.sample.as_ref().map_err(|e| *e)?;
            s.check_freshness(now, max_age)?;
            Ok(s.clone())
        }
    }
    #[cfg(test)]
    mod backend_tests {
        use super::*;
        use std::collections::VecDeque;
        struct Events(VecDeque<Result<Vec<u8>, io::Error>>);
        impl Read for Events {
            fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
                match self
                    .0
                    .pop_front()
                    .unwrap_or_else(|| Err(io::ErrorKind::WouldBlock.into()))
                {
                    Ok(b) => {
                        out[..b.len()].copy_from_slice(&b);
                        Ok(b.len())
                    }
                    Err(e) => Err(e),
                }
            }
        }
        fn event(ty: u16, code: u16, value: i32) -> Result<Vec<u8>, io::Error> {
            let mut b = vec![0; std::mem::size_of::<InputEvent>()];
            let offset = std::mem::size_of::<libc::timeval>();
            b[offset..offset + 2].copy_from_slice(&ty.to_ne_bytes());
            b[offset + 2..offset + 4].copy_from_slice(&code.to_ne_bytes());
            b[offset + 4..offset + 8].copy_from_slice(&value.to_ne_bytes());
            Ok(b)
        }
        #[test]
        fn initial_query_and_event_values_do_not_guess_polarity() {
            let mut sync = SyncState::default();
            for asserted in [false, true] {
                assert_eq!(
                    observe(&mut Events(VecDeque::new()), &mut sync, || Ok(asserted)),
                    Ok(asserted)
                );
            }
            let mut input = Events(VecDeque::from([event(5, 0, 1), event(0, 0, 0)]));
            assert_eq!(observe(&mut input, &mut sync, || Ok(false)), Ok(false));
        }
        #[test]
        fn dropped_resync_requires_report_and_successful_ioctl() {
            let mut sync = SyncState::default();
            let mut input = Events(VecDeque::from([event(0, 3, 0), event(5, 0, 1)]));
            assert_eq!(
                observe(&mut input, &mut sync, || panic!(
                    "query during dropped interval"
                )),
                Err(LidFailure::Resynchronizing)
            );
            assert_eq!(
                observe(&mut Events(VecDeque::new()), &mut sync, || Ok(true)),
                Err(LidFailure::Resynchronizing)
            );
            let mut report = Events(VecDeque::from([event(0, 0, 0)]));
            assert_eq!(
                observe(&mut report, &mut sync, || Err(LidFailure::Backend)),
                Err(LidFailure::Backend)
            );
            assert_eq!(
                observe(&mut Events(VecDeque::new()), &mut sync, || Ok(true)),
                Ok(true)
            );
        }
        #[test]
        fn read_errors_eof_short_and_flood_fail_closed() {
            let mut sync = SyncState::default();
            for (record, expected) in [
                (Ok(vec![]), LidFailure::Unavailable),
                (Ok(vec![0]), LidFailure::Backend),
                (
                    Err(io::ErrorKind::PermissionDenied.into()),
                    LidFailure::Denied,
                ),
            ] {
                assert_eq!(
                    observe(&mut Events(VecDeque::from([record])), &mut sync, || panic!(
                        "unexpected query"
                    )),
                    Err(expected)
                );
            }
            let mut flood = Events((0..256).map(|_| event(5, 0, 1)).collect());
            assert_eq!(
                observe(&mut flood, &mut sync, || panic!("unbounded drain")),
                Err(LidFailure::Resynchronizing)
            );
        }
        #[test]
        fn missing_discovery_and_failed_cache_are_typed() {
            let mut lid = LidInput::new(
                "/nonexistent/fungos-input".into(),
                "/dev/input".into(),
                "gpio-keys".into(),
            );
            lid.poll(0);
            assert_eq!(lid.snapshot(0, 1000), Err(LidFailure::Unavailable));
            lid.sample = Ok(LidSample {
                switch_asserted: true,
                physical_position: LidPosition::Unqualified,
                event_node: "event9".into(),
                observed_monotonic_ms: 0,
            });
            lid.fail(LidFailure::Denied, 10);
            assert_eq!(lid.snapshot(10, 1000), Err(LidFailure::Denied));
            assert!(lid.file.is_none());
        }
    }
}
pub use linux::LidInput;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn freshness_and_polarity_are_honest() {
        for value in [false, true] {
            let s = LidSample {
                switch_asserted: value,
                physical_position: LidPosition::Unqualified,
                event_node: "/dev/input/event9".into(),
                observed_monotonic_ms: 100,
            };
            assert_eq!(s.check_freshness(110, 10), Ok(()));
            assert_eq!(s.check_freshness(111, 10), Err(LidFailure::Stale));
            assert_eq!(s.check_freshness(99, 10), Err(LidFailure::InvalidTime));
            assert_eq!(s.physical_position, LidPosition::Unqualified);
        }
    }
    #[test]
    fn qualified_standard_initial_and_resync_mapping_preserves_raw_bit() {
        let mapping = LidPolarity::StandardClosedAsserted;
        // Same mapping for initial EVIOCGSW and every successful resync query.
        let mut sync = SyncState::default();
        for (asserted, expected) in [(true, LidPosition::Closed), (false, LidPosition::Open)] {
            assert_eq!(mapping.position(asserted), expected);
            sync.event(0, 3);
            assert!(sync.dropped);
            assert_eq!(
                LidPolarity::Unqualified.position(asserted),
                LidPosition::Unqualified
            );
            sync.event(0, 0);
            assert!(!sync.dropped);
            assert_eq!(mapping.position(asserted), expected);
            // Mapping is separate from and cannot change this raw bit.
            let sample = LidSample {
                switch_asserted: asserted,
                physical_position: mapping.position(asserted),
                event_node: "event1".into(),
                observed_monotonic_ms: 1,
            };
            assert_eq!(sample.switch_asserted, asserted);
        }
    }
    #[test]
    fn dropped_events_stay_unknown_until_report() {
        let mut s = SyncState::default();
        assert!(!s.dropped);
        s.event(0, 3);
        assert!(s.dropped);
        s.event(5, 0); // Ignore any SW_LID value during dropped interval.
        s.event(1, 1); // Keys are unrelated.
        assert!(s.dropped);
        assert!(s.event(0, 0));
        assert!(!s.dropped); // Still needs successful query in backend.
    }
}
