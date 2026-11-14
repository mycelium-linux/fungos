//! Exclusive read-only lid mode. Deliberately has no combined-provider imports.
use fungos_device_hal::{lid::LidInput, lid_transport};
use std::{
    env, fs,
    io::{Read, Write},
    os::unix::{
        fs::{MetadataExt, PermissionsExt},
        net::UnixListener,
    },
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};
static TERMINATE: AtomicBool = AtomicBool::new(false);
extern "C" fn terminate(_: libc::c_int) {
    TERMINATE.store(true, Ordering::Relaxed);
}
pub(super) struct Signals {
    term: libc::sigaction,
    int: libc::sigaction,
}
impl Signals {
    pub(super) fn install() -> Result<Self, String> {
        // SAFETY: sigaction structs are initialized before use; handler only sets
        // a lock-free atomic flag. Previous dispositions are restored on exit.
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = terminate as *const () as usize;
            libc::sigemptyset(&mut action.sa_mask);
            let mut term = std::mem::zeroed();
            let mut int = std::mem::zeroed();
            TERMINATE.store(false, Ordering::Relaxed);
            if libc::sigaction(libc::SIGTERM, &action, &mut term) != 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            if libc::sigaction(libc::SIGINT, &action, &mut int) != 0 {
                libc::sigaction(libc::SIGTERM, &term, std::ptr::null_mut());
                return Err(std::io::Error::last_os_error().to_string());
            }
            Ok(Self { term, int })
        }
    }
}
impl Drop for Signals {
    fn drop(&mut self) {
        // SAFETY: saved initialized signal dispositions, valid for these signals.
        unsafe {
            libc::sigaction(libc::SIGTERM, &self.term, std::ptr::null_mut());
            libc::sigaction(libc::SIGINT, &self.int, std::ptr::null_mut());
        }
    }
}
pub(super) struct OwnedSocket {
    path: PathBuf,
    dev: u64,
    ino: u64,
}
impl Drop for OwnedSocket {
    fn drop(&mut self) {
        if let Ok(m) = fs::symlink_metadata(&self.path) {
            if m.dev() == self.dev && m.ino() == self.ino {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
}
pub(super) fn bind(path: PathBuf) -> Result<(UnixListener, OwnedSocket), String> {
    // Never unlink: occupied/stale paths need explicit owner intervention.
    // Restrict at creation as well as afterwards (single-threaded early dispatch).
    let old = unsafe { libc::umask(0o077) };
    let result = UnixListener::bind(&path);
    unsafe {
        libc::umask(old);
    }
    let listener = result.map_err(|e| format!("lid-only socket bind (no replacement): {e}"))?;
    let m = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    let owned = OwnedSocket {
        path,
        dev: m.dev(),
        ino: m.ino(),
    };
    fs::set_permissions(&owned.path, fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    Ok((listener, owned))
}
pub(super) fn terminated() -> bool {
    TERMINATE.load(Ordering::Relaxed)
}
pub fn run() -> Result<(), String> {
    // Explicit socket avoids accidentally starting another provider at a default.
    let socket = env::var_os("FUNGOS_SOCKET")
        .map(PathBuf::from)
        .ok_or("--lid-only requires explicit FUNGOS_SOCKET")?;
    if !socket.is_absolute() {
        return Err("FUNGOS_SOCKET must be absolute".into());
    }
    let name = env::var("FUNGOS_LID_NAME").unwrap_or("gpio-keys".into());
    if name.trim().is_empty() || name.contains('\n') || name.len() > 256 {
        return Err("invalid lid identity".into());
    }
    let polarity = super::parse_lid_polarity(env::var("FUNGOS_LID_POLARITY").ok().as_deref())?;
    let sys = env::var_os("FUNGOS_SYS_INPUT_ROOT")
        .map(PathBuf::from)
        .unwrap_or("/sys/class/input".into());
    let dev = env::var_os("FUNGOS_INPUT_ROOT")
        .map(PathBuf::from)
        .unwrap_or("/dev/input".into());
    let _signals = Signals::install()?;
    let (listener, _owned) = bind(socket)?;
    let mut lid = LidInput::new(sys, dev, name).with_polarity(polarity);
    let start = Instant::now();
    eprintln!("lid-only: read-only SW_LID; only lid-v1; TERM/INT cleans owned socket");
    while !TERMINATE.load(Ordering::Relaxed) {
        lid.poll(start.elapsed().as_millis() as u64);
        match listener.accept() {
            Ok((mut stream, _)) => {
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .map_err(|e| e.to_string())?;
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .map_err(|e| e.to_string())?;
                let mut q = Vec::new();
                let read = (&mut stream).take(257).read_to_end(&mut q);
                let response = if read.is_ok() && q == b"lid-v1\n" {
                    let now = start.elapsed().as_millis() as u64;
                    lid.poll(now);
                    lid_transport::encode_response(
                        lid.snapshot(now, 1000),
                        start.elapsed().as_millis() as u64,
                    )
                } else {
                    "ERR lid-only supports only lid-v1\n".into()
                };
                let _ = stream.write_all(response.as_bytes());
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(25))
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mode_has_no_combined_or_audio_backend_dependencies() {
        // Architectural regression guard supplements the executable sentinel
        // test: mode code cannot gain combined backend construction unnoticed.
        let source = include_str!("lid_only.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        for forbidden in [
            "KeyboardLight",
            "reconcile",
            "Sysfs",
            "Keypad",
            "PowerSupply",
            "PCM",
            "/dev/snd",
            "input_path(",
            "super::run(",
            "Config::",
        ] {
            assert!(
                !production.contains(forbidden),
                "forbidden dependency: {forbidden}"
            );
        }
    }
    #[test]
    fn collision_and_owned_cleanup() {
        let p = env::temp_dir().join(format!("lid-exclusive-{}", std::process::id()));
        let (listener, owner) = bind(p.clone()).unwrap();
        assert_eq!(
            fs::metadata(&p).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(bind(p.clone()).is_err());
        drop(owner);
        drop(listener);
        assert!(!p.exists());
    }
    #[test]
    fn cleanup_does_not_remove_replacement() {
        let p = env::temp_dir().join(format!("lid-replacement-{}", std::process::id()));
        let (listener, owner) = bind(p.clone()).unwrap();
        fs::remove_file(&p).unwrap();
        fs::write(&p, "replacement").unwrap();
        drop(owner);
        drop(listener);
        assert_eq!(fs::read(&p).unwrap(), b"replacement");
        fs::remove_file(p).unwrap();
    }
}
