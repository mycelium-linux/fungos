#[cfg(unix)]
mod battery_only;
#[cfg(unix)]
mod keypad_only;
#[cfg(unix)]
mod lid_only;
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
    power_supply: PathBuf,
    lid_sys_input: PathBuf,
    lid_name: String,
    lid_polarity: fungos_device_hal::lid::LidPolarity,
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
            lid_sys_input: env::var_os("FUNGOS_SYS_INPUT_ROOT")
                .map(PathBuf::from)
                .unwrap_or("/sys/class/input".into()),
            lid_polarity: parse_lid_polarity(env::var("FUNGOS_LID_POLARITY").ok().as_deref())?,
            lid_name: env::var("FUNGOS_LID_NAME").unwrap_or("gpio-keys".into()),
            power_supply: env::var_os("FUNGOS_POWER_SUPPLY_ROOT")
                .map(PathBuf::from)
                .unwrap_or("/sys/class/power_supply".into()),
            socket: env::var_os("FUNGOS_SOCKET")
                .map(PathBuf::from)
                .unwrap_or("/run/fungos-keyboard-light.sock".into()),
        };
        if c.input_name.trim().is_empty() || c.input_name.contains('\n') || c.input_name.len() > 256
        {
            return Err("invalid input identity".into());
        }
        if c.lid_name.trim().is_empty() || c.lid_name.contains('\n') || c.lid_name.len() > 256 {
            return Err("invalid lid identity".into());
        }
        if c.led_name.contains('/') || c.led_name == "." || c.led_name == ".." {
            return Err("LED identity must be a sysfs name".into());
        }
        Ok(c)
    }
}
fn parse_lid_polarity(value: Option<&str>) -> Result<fungos_device_hal::lid::LidPolarity, String> {
    use fungos_device_hal::lid::LidPolarity;
    match value {
        None | Some("unqualified") => Ok(LidPolarity::Unqualified),
        Some("standard-closed-asserted") => Ok(LidPolarity::StandardClosedAsserted),
        _ => Err("FUNGOS_LID_POLARITY must be unqualified or standard-closed-asserted (qualified board only)".into()),
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
fn lid_status(lid: &mut fungos_device_hal::lid::LidInput, now: u64) -> String {
    lid.poll(now);
    match lid.snapshot(now, 1000) {
        Ok(sample) => format!("OK lid={sample:?}\n"),
        Err(e) => format!("ERR lid={e:?}\n"),
    }
}

fn battery_status(backend: &fungos_device_hal::power_supply::PowerSupply, now: u64) -> String {
    match backend.read(now) {
        Ok(samples) => format!("OK battery={samples:?}\n"),
        Err(e) => format!("ERR battery={e:?}\n"),
    }
}

#[cfg(unix)]
fn restrict_socket(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(unix)]
fn run() -> Result<(), String> {
    use std::{
        fs::OpenOptions,
        os::unix::{fs::OpenOptionsExt, net::UnixListener},
    };
    let c = Config::from_env()?;
    let battery = fungos_device_hal::power_supply::PowerSupply::new(&c.power_supply);
    let mut lid = fungos_device_hal::lid::LidInput::new(
        c.lid_sys_input.clone(),
        c.dev_input.clone(),
        c.lid_name.clone(),
    )
    .with_polarity(c.lid_polarity);
    let input_identity = input_path(&c.lid_sys_input, &c.input_name)?;
    let input = c
        .dev_input
        .join(input_identity.file_name().ok_or("missing event identity")?);
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
    restrict_socket(&c.socket).map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let mut f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(&input)
        .map_err(|e| e.to_string())?;
    let mut instance_bytes = [0u8; 8];
    fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut instance_bytes))
        .map_err(|e| e.to_string())?;
    let mut keypad = fungos_device_hal::keypad::KeypadRing::new(u64::from_ne_bytes(instance_bytes));
    let start = Instant::now();
    let mut buf = [0u8; 24];
    let mut input_lost = false;
    loop {
        let now = start.elapsed().as_millis() as u64;
        lid.poll(now);
        for _ in 0..256 {
            match f.read(&mut buf) {
                Ok(0) => {
                    if !input_lost {
                        keypad.input(0, 3, 0, now);
                        input_lost = true;
                    }
                    break;
                }
                Ok(n) => {
                    if n % 24 != 0 {
                        keypad.input(0, 3, 0, now);
                        break;
                    }
                    for ev in buf[..n].as_chunks::<24>().0 {
                        let ty = u16::from_ne_bytes([ev[16], ev[17]]);
                        let code = u16::from_ne_bytes([ev[18], ev[19]]);
                        let v = i32::from_ne_bytes(ev[20..24].try_into().unwrap());
                        keypad.input(ty, code, v, now);
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
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e.to_string()),
            }
        }
        loop {
            match listener.accept() {
                Ok((mut s, _)) => {
                    if s.set_read_timeout(Some(Duration::from_secs(2))).is_err()
                        || s.set_write_timeout(Some(Duration::from_secs(2))).is_err()
                    {
                        continue;
                    }
                    let mut q = String::new();
                    if (&mut s).take(257).read_to_string(&mut q).is_err() || q.len() > 256 {
                        let _ = s.write_all(b"ERR invalid bounded request\n");
                        continue;
                    }
                    let a: Vec<_> = q.split_whitespace().collect();
                    let out = if q == "lid-v1\n" {
                        let observed = start.elapsed().as_millis() as u64;
                        lid.poll(observed);
                        fungos_device_hal::lid_transport::encode_response(
                            lid.snapshot(observed, 1000),
                            start.elapsed().as_millis() as u64,
                        )
                    } else if q.starts_with("key-v1") {
                        match fungos_device_hal::keypad_transport::decode_request(&q) {
                            Ok(cursor) => {
                                let now = start.elapsed().as_millis() as u64;
                                fungos_device_hal::keypad_transport::encode_response(
                                    &keypad.poll(cursor, now),
                                    now,
                                )
                            }
                            Err(_) => "ERR invalid keypad request\n".into(),
                        }
                    } else {
                        match a.as_slice() {
                        ["lid"] => lid_status(&mut lid, start.elapsed().as_millis() as u64),
                        ["battery"] => battery_status(
                            &battery,
                            start.elapsed().as_millis().min(u64::MAX as u128) as u64,
                        ),
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
                        _ => {
                            "ERR commands: status | battery | lid | lease LEVEL MS | clear | stop\n"
                                .into()
                        }
                    }
                    };
                    let _ = s.write_all(out.as_bytes());
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
#[cfg(unix)]
fn request(socket: &Path, command: &str) -> Result<String, String> {
    use std::{net::Shutdown, os::unix::net::UnixStream};
    let mut stream = UnixStream::connect(socket).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    writeln!(stream, "{command}").map_err(|e| e.to_string())?;
    stream
        .shutdown(Shutdown::Write)
        .map_err(|e| e.to_string())?;
    let mut response = String::new();
    stream
        .take(65536)
        .read_to_string(&mut response)
        .map_err(|e| e.to_string())?;
    Ok(response)
}

fn entry() -> Result<(), String> {
    let args: Vec<_> = env::args().skip(1).collect();
    #[cfg(unix)]
    if args.as_slice() == ["--keypad-only"] {
        return keypad_only::run();
    }
    #[cfg(unix)]
    if args.as_slice() == ["--lid-only"] {
        return lid_only::run();
    }
    #[cfg(unix)]
    if args.as_slice() == ["--battery-only"] {
        return battery_only::run();
    }
    #[cfg(unix)]
    if args.as_slice() == ["--request", "battery-v1"] {
        let socket = env::var_os("FUNGOS_BATTERY_SOCKET")
            .map(PathBuf::from)
            .ok_or("battery-v1 requires explicit FUNGOS_BATTERY_SOCKET")?;
        let remote = fungos_device_hal::battery_transport::BatteryClient::new(socket)
            .read()
            .map_err(|e| format!("battery-v1 failed: {e:?}"))?;
        print!(
            "{}",
            fungos_device_hal::battery_transport::encode_response(
                Ok(remote.sample().clone()),
                remote
                    .sample()
                    .observed_monotonic_ms
                    .saturating_add(remote.provider_age().as_millis() as u64)
            )
        );
        return Ok(());
    }
    if args.is_empty() {
        return run();
    }
    #[cfg(unix)]
    if let [flag, command] = args.as_slice() {
        if flag == "--request"
            && matches!(command.as_str(), "battery" | "status" | "lid" | "lid-v1")
        {
            let socket = env::var_os("FUNGOS_SOCKET")
                .map(PathBuf::from)
                .unwrap_or("/run/fungos-keyboard-light.sock".into());
            let response = request(&socket, command)?;
            print!("{response}");
            return if response.starts_with("OK ") || response.starts_with("FUNGOS-LID/1\tOK\t") {
                Ok(())
            } else {
                Err("provider request failed (see response)".into())
            };
        }
    }
    Err("usage: fungos-keyboard-lightd [--lid-only | --battery-only | --request battery-v1|battery|status|lid|lid-v1]".into())
}

fn main() {
    if let Err(e) = entry() {
        eprintln!("fungos-keyboard-lightd: {e}");
        std::process::exit(1)
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;
    #[test]
    fn lid_mapping_requires_explicit_standard_qualification() {
        use fungos_device_hal::lid::LidPolarity;
        assert_eq!(parse_lid_polarity(None).unwrap(), LidPolarity::Unqualified);
        assert_eq!(
            parse_lid_polarity(Some("standard-closed-asserted")).unwrap(),
            LidPolarity::StandardClosedAsserted
        );
        assert!(parse_lid_polarity(Some("open-asserted")).is_err());
        assert!(parse_lid_polarity(Some("gpio-keys")).is_err());
    }
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
    #[cfg(unix)]
    #[test]
    fn request_client_half_closes_and_reads_existing_socket() {
        use std::os::unix::net::UnixListener;
        let p = std::env::temp_dir().join(format!("fungos-client-{}", std::process::id()));
        let listener = UnixListener::bind(&p).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut command = String::new();
            stream.read_to_string(&mut command).unwrap();
            assert_eq!(command, "battery\n");
            stream.write_all(b"OK battery=voltage-only\n").unwrap();
        });
        assert_eq!(request(&p, "battery").unwrap(), "OK battery=voltage-only\n");
        server.join().unwrap();
        fs::remove_file(p).unwrap();
    }
    #[test]
    fn battery_uses_existing_request_path_and_reports_absence() {
        let d = std::env::temp_dir().join(format!("fungos-battery-api-{}", std::process::id()));
        fs::create_dir_all(&d).unwrap();
        let backend = fungos_device_hal::power_supply::PowerSupply::new(&d);
        assert_eq!(battery_status(&backend, 25), "ERR battery=Unavailable\n");
        fs::create_dir_all(d.join("BAT0")).unwrap();
        fs::write(d.join("BAT0/type"), "Battery").unwrap();
        let response = battery_status(&backend, 25);
        assert!(response.contains("percent: None"));
        assert!(response.contains("charging: Unknown"));
        assert!(response.contains("observed_monotonic_ms: 25"));
        fs::remove_dir_all(d).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn provider_socket_is_owner_only() {
        use std::os::unix::{fs::PermissionsExt, net::UnixListener};
        let p = std::env::temp_dir().join(format!("fungos-socket-mode-{}", std::process::id()));
        let listener = UnixListener::bind(&p).unwrap();
        restrict_socket(&p).unwrap();
        assert_eq!(
            fs::metadata(&p).unwrap().permissions().mode() & 0o777,
            0o600
        );
        drop(listener);
        fs::remove_file(p).unwrap();
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
