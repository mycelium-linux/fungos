use fungos_device_hal::{InputActivity, KeyboardLight, KeyboardLightPolicy, LightCapability};
#[cfg(unix)]
use std::io::Write;
use std::{
    env, fs,
    io::{self, Read},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};
#[derive(Debug)]
struct Config {
    input_name: String,
    led_name: String,
    sysfs: PathBuf,
    dev_input: PathBuf,
    socket: PathBuf,
}
impl Config {
    fn from_env() -> Result<Self, String> {
        let c = Self {
            input_name: env::var("FUNGOS_INPUT_NAME")
                .map_err(|_| "FUNGOS_INPUT_NAME required (exact /proc name)".to_string())?,
            led_name: env::var("FUNGOS_LED_NAME").unwrap_or_default(),
            sysfs: env::var_os("FUNGOS_SYS_LED_ROOT")
                .map(PathBuf::from)
                .unwrap_or("/sys/class/leds".into()),
            dev_input: env::var_os("FUNGOS_INPUT_ROOT")
                .map(PathBuf::from)
                .unwrap_or("/dev/input".into()),
            socket: env::var_os("FUNGOS_SOCKET")
                .map(PathBuf::from)
                .unwrap_or("/run/fungos-keyboard-light.sock".into()),
        };
        if c.input_name.trim().is_empty() || c.input_name.contains('\n') || c.input_name.len() > 256
        {
            return Err("invalid input identity".into());
        }
        if c.led_name.contains('/') || c.led_name == "." || c.led_name == ".." {
            return Err("LED identity must be a sysfs name".into());
        }
        Ok(c)
    }
}
struct Sysfs {
    brightness: PathBuf,
    max: u32,
}
impl Sysfs {
    fn discover(c: &Config) -> Result<Self, String> {
        for e in fs::read_dir(&c.sysfs).map_err(|e| format!("LED discovery: {e}"))? {
            let p = e.map_err(|e| e.to_string())?.path();
            let n = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if (!c.led_name.is_empty() && n != c.led_name)
                || (!p.join("function").exists() && !n.contains("kbd_backlight"))
            {
                continue;
            }
            let b = p.join("brightness");
            let max = fs::read_to_string(p.join("max_brightness"))
                .map_err(|e| e.to_string())?
                .trim()
                .parse::<u32>()
                .map_err(|_| "invalid max_brightness")?;
            if max == 0 || !b.exists() {
                return Err("invalid LED capability".into());
            }
            return Ok(Self { brightness: b, max });
        }
        Err("no matching keyboard LED".into())
    }
}
impl KeyboardLight for Sysfs {
    fn capability(&self) -> LightCapability {
        if self.max == 1 {
            LightCapability::Binary
        } else {
            LightCapability::Brightness { max: self.max }
        }
    }
    fn set_level(&mut self, l: u32) -> Result<(), String> {
        if l > self.max {
            return Err("brightness out of range".into());
        }
        fs::write(&self.brightness, format!("{l}\n")).map_err(|e| format!("write LED: {e}"))
    }
}
fn input_path(root: &Path, w: &str) -> Result<PathBuf, String> {
    for e in fs::read_dir(root).map_err(|e| e.to_string())? {
        let p = e.map_err(|e| e.to_string())?.path();
        if !p
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .starts_with("event")
        {
            continue;
        }
        let n = fs::read_to_string(p.join("device/name"))
            .or_else(|_| fs::read_to_string(p.join("name")));
        if n.is_ok_and(|v| v.trim() == w) {
            return Ok(p);
        }
    }
    Err(format!("input identity not found: {w}"))
}
#[cfg(test)]
mod config_tests {
    use super::*;
    #[test]
    fn rejects_bad_identity() {
        env::set_var("FUNGOS_INPUT_NAME", " ");
        assert!(Config::from_env().is_err());
        env::set_var("FUNGOS_INPUT_NAME", "Keyboard");
        env::set_var("FUNGOS_LED_NAME", "../bad");
        assert!(Config::from_env().is_err());
        env::remove_var("FUNGOS_INPUT_NAME");
        env::remove_var("FUNGOS_LED_NAME");
    }
    #[test]
    fn input_matching_is_exact_and_not_event_zero_assumed() {
        let d = std::env::temp_dir().join(format!("fungos-input-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("event7/device")).unwrap();
        fs::write(d.join("event7/device/name"), "wanted\n").unwrap();
        assert_eq!(input_path(&d, "wanted").unwrap(), d.join("event7"));
        assert!(input_path(&d, "other").is_err());
        fs::remove_dir_all(d).unwrap();
    }
}
#[cfg(unix)]
fn run() -> Result<(), String> {
    use std::{
        fs::OpenOptions,
        os::unix::{fs::OpenOptionsExt, net::UnixListener},
    };
    let c = Config::from_env()?;
    let input = input_path(&c.dev_input, &c.input_name)?;
    let mut led = Sysfs::discover(&c)?;
    let mut policy = KeyboardLightPolicy::default();
    policy
        .reconcile(&mut led, 0, false)
        .map_err(|e| e.to_string())?;
    if let Some(d) = c.socket.parent() {
        fs::create_dir_all(d).map_err(|e| e.to_string())?
    }
    let _ = fs::remove_file(&c.socket);
    let listener = UnixListener::bind(&c.socket).map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let mut f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(&input)
        .map_err(|e| e.to_string())?;
    let start = Instant::now();
    let mut buf = [0u8; 24];
    loop {
        let now = start.elapsed().as_millis() as u64;
        match f.read(&mut buf) {
            Ok(n) => {
                for ev in buf[..n].chunks_exact(24) {
                    let ty = u16::from_ne_bytes([ev[16], ev[17]]);
                    let code = u16::from_ne_bytes([ev[18], ev[19]]);
                    let v = i32::from_ne_bytes(ev[20..24].try_into().unwrap());
                    if ty == 1 && code != 0 && (v == 1 || v == 2) {
                        policy.activity(
                            if v == 1 {
                                InputActivity::KeyPress
                            } else {
                                InputActivity::KeyRepeat
                            },
                            now,
                        )
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(e.to_string()),
        }
        loop {
            match listener.accept() {
                Ok((mut s, _)) => {
                    let mut q = String::new();
                    (&mut s)
                        .take(256)
                        .read_to_string(&mut q)
                        .map_err(|e| e.to_string())?;
                    let a: Vec<_> = q.split_whitespace().collect();
                    let out = match a.as_slice() {
                        ["status"] => format!(
                            "OK level={} capability={:?}\n",
                            policy.desired(now),
                            led.capability()
                        ),
                        ["lease", l, ms] => match (l.parse::<u32>(), ms.parse::<u64>()) {
                            (Ok(l), Ok(ms)) => match policy.lease(
                                l,
                                now,
                                Duration::from_millis(ms),
                                led.capability(),
                            ) {
                                Ok(()) => "OK\n".into(),
                                Err(e) => format!("ERR {e}\n"),
                            },
                            _ => "ERR invalid arguments\n".into(),
                        },
                        ["clear"] => {
                            policy.clear_lease();
                            "OK\n".into()
                        }
                        ["stop"] => {
                            policy.clear_lease();
                            policy
                                .reconcile(&mut led, now, true)
                                .map_err(|e| e.to_string())?;
                            s.write_all(b"OK\n").ok();
                            return Ok(());
                        }
                        _ => "ERR commands: status | lease LEVEL MS | clear | stop\n".into(),
                    };
                    s.write_all(out.as_bytes()).map_err(|e| e.to_string())?
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e.to_string()),
            }
        }
        policy
            .reconcile(&mut led, now, false)
            .map_err(|e| e.to_string())?;
        thread::sleep(Duration::from_millis(25));
    }
}
#[cfg(not(unix))]
fn run() -> Result<(), String> {
    Err("Linux/Unix backend required".into())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("fungos-keyboard-lightd: {e}");
        std::process::exit(1)
    }
}
