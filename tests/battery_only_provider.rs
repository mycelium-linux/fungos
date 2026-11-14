//! Actual early-dispatch executable, no combined capabilities available.
#![cfg(unix)]
use std::{
    fs,
    io::{Read, Write},
    net::Shutdown,
    os::unix::{fs::PermissionsExt, net::UnixStream},
    process::{Child, Command},
    thread,
    time::Duration,
};
struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn launch(root: &std::path::Path) -> Process {
    Process(
        Command::new(env!("CARGO_BIN_EXE_fungos-keyboard-lightd"))
            .arg("--battery-only")
            .env_remove("FUNGOS_INPUT_NAME")
            .env("FUNGOS_LED_NAME", "../invalid-unused")
            .env("FUNGOS_SYS_LED_ROOT", root.join("forbidden-led"))
            .env("FUNGOS_POWER_SUPPLY_ROOT", root.join("power"))
            .env("FUNGOS_SYS_INPUT_ROOT", root.join("sys"))
            .env("FUNGOS_INPUT_ROOT", root.join("dev"))
            .env("FUNGOS_LID_NAME", "gpio-keys")
            .env("FUNGOS_BATTERY_SOCKET", root.join("provider.sock"))
            .spawn()
            .unwrap(),
    )
}
fn query(path: &std::path::Path, q: &[u8]) -> String {
    let mut s = UnixStream::connect(path).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    s.write_all(q).unwrap();
    s.shutdown(Shutdown::Write).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    out
}
#[test]
fn battery_only_absence_commands_collision_term_and_reconnect() {
    let root = std::env::temp_dir().join(format!("battery-only-e2e-{}", std::process::id()));
    fs::create_dir_all(root.join("power")).unwrap();
    fs::create_dir_all(root.join("sys")).unwrap();
    fs::create_dir_all(root.join("dev")).unwrap();
    // A regular file as a root would make any LED discovery fail. Keep sentinel
    // unchanged; no keyboard identity or LED capability is provided at all.
    fs::write(root.join("forbidden-led"), "untouched").unwrap();
    fs::write(root.join("forbidden-power"), "untouched").unwrap();
    let socket = root.join("provider.sock");
    let mut p = launch(&root);
    for _ in 0..200 {
        if socket.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        fs::metadata(&socket).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        query(&socket, b"battery-v1\n"),
        "FUNGOS-BATTERY/1\tERR\tunavailable\n"
    );
    for q in [
        "lid\n",
        "lid-v1\n",
        "status\n",
        "stop\n",
        "battery\n",
        "lease 1 100\n",
        "clear\n",
        "key-v1\n",
        "battery-v1 \n",
    ] {
        assert_eq!(
            query(&socket, q.as_bytes()),
            "FUNGOS-BATTERY/1\tERR\tprotocol\n"
        );
    }
    let mut duplicate = launch(&root);
    assert!(!duplicate.0.wait().unwrap().success());
    assert_eq!(
        query(&socket, b"battery-v1\n"),
        "FUNGOS-BATTERY/1\tERR\tunavailable\n"
    );
    fs::create_dir_all(root.join("power/cat-s22-battery")).unwrap();
    fs::write(root.join("power/cat-s22-battery/type"), "Battery\n").unwrap();
    fs::write(root.join("power/cat-s22-battery/voltage_now"), "3878682\n").unwrap();
    let client = fungos_device_hal::battery_transport::BatteryClient::new(&socket);
    let sample = client
        .read()
        .unwrap()
        .snapshot(Duration::from_secs(1))
        .unwrap();
    assert_eq!(sample.percent, None);
    assert_eq!(sample.voltage_microvolts, Some(3878682));
    assert_eq!(sample.current_microamps, None);
    assert_eq!(sample.temperature_deci_celsius, None);
    fs::write(root.join("power/cat-s22-battery/capacity"), "101").unwrap();
    assert_eq!(
        client.read().unwrap_err(),
        fungos_device_hal::battery_transport::BatteryFailure::Backend
    );
    fs::remove_file(root.join("power/cat-s22-battery/capacity")).unwrap();
    fs::create_dir_all(root.join("power/second")).unwrap();
    fs::write(root.join("power/second/type"), "Battery").unwrap();
    assert_eq!(
        client.read().unwrap_err(),
        fungos_device_hal::battery_transport::BatteryFailure::Ambiguous
    );
    fs::remove_dir_all(root.join("power/second")).unwrap();
    assert_eq!(client.read().unwrap().sample().percent, None);
    unsafe {
        libc::kill(p.0.id() as i32, libc::SIGTERM);
    }
    assert!(p.0.wait().unwrap().success());
    assert!(!socket.exists());
    assert_eq!(
        client.read().unwrap_err(),
        fungos_device_hal::battery_transport::BatteryFailure::Disconnected
    );
    let mut p2 = launch(&root);
    for _ in 0..200 {
        if socket.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(client.read().unwrap().sample().percent, None);
    unsafe {
        libc::kill(p2.0.id() as i32, libc::SIGTERM);
    }
    assert!(p2.0.wait().unwrap().success());
    for f in ["forbidden-led", "forbidden-power"] {
        assert_eq!(fs::read_to_string(root.join(f)).unwrap(), "untouched");
    }
    fs::remove_dir_all(root).unwrap();
}
