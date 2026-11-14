//! Exclusive read-only battery mode. Deliberately has no combined-provider imports.
use fungos_device_hal::{
    battery_transport::{self, BatteryFailure},
    power_supply::PowerSupply,
};
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
struct Signals {
    term: libc::sigaction,
    int: libc::sigaction,
}
impl Signals {
    fn install() -> Result<Self, String> {
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
struct OwnedSocket {
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
fn bind(path: PathBuf) -> Result<(UnixListener, OwnedSocket), String> {
    // Never unlink: occupied/stale paths need explicit owner intervention.
    // Restrict at creation as well as afterwards (single-threaded early dispatch).
    let old = unsafe { libc::umask(0o077) };
    let result = UnixListener::bind(&path);
    unsafe {
        libc::umask(old);
    }
    let listener = result.map_err(|e| format!("battery-only socket bind (no replacement): {e}"))?;
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
pub fn run() -> Result<(), String> {
    // Explicit socket avoids accidentally starting another provider at a default.
    let socket = env::var_os("FUNGOS_BATTERY_SOCKET")
        .map(PathBuf::from)
        .ok_or("--battery-only requires explicit FUNGOS_BATTERY_SOCKET")?;
    if !socket.is_absolute() {
        return Err("FUNGOS_BATTERY_SOCKET must be absolute".into());
    }
    let root = env::var_os("FUNGOS_POWER_SUPPLY_ROOT")
        .map(PathBuf::from)
        .unwrap_or("/sys/class/power_supply".into());
    let _signals = Signals::install()?;
    let (listener, _owned) = bind(socket)?;
    let battery = PowerSupply::new(root);
    let start = Instant::now();
    eprintln!("battery-only: read-only power_supply; only battery-v1; no charge controls; TERM/INT cleans owned socket");
    while !TERMINATE.load(Ordering::Relaxed) {
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
                let response = if read.is_ok() && q == b"battery-v1\n" {
                    let now = start.elapsed().as_millis() as u64;
                    let sample =
                        battery
                            .read(now)
                            .map_err(BatteryFailure::from)
                            .and_then(|mut samples| {
                                if samples.len() != 1 {
                                    Err(BatteryFailure::Ambiguous)
                                } else {
                                    Ok(samples.remove(0))
                                }
                            });
                    battery_transport::encode_response(sample, start.elapsed().as_millis() as u64)
                } else {
                    "FUNGOS-BATTERY/1\tERR\tprotocol\n".into()
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
        let source = include_str!("battery_only.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        for forbidden in [
            "KeyboardLight",
            "reconcile",
            "Sysfs",
            "Keypad",
            "charge_control",
            "LidInput",
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
        let p = env::temp_dir().join(format!("battery-exclusive-{}", std::process::id()));
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
        let p = env::temp_dir().join(format!("battery-replacement-{}", std::process::id()));
        let (listener, owner) = bind(p.clone()).unwrap();
        fs::remove_file(&p).unwrap();
        fs::write(&p, "replacement").unwrap();
        drop(owner);
        drop(listener);
        assert_eq!(fs::read(&p).unwrap(), b"replacement");
        fs::remove_file(p).unwrap();
    }
}
