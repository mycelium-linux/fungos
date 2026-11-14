//! Full executable -> owner socket -> combined provider -> typed lid absence.
//! Successful evdev ioctls still require parent-owned hardware qualification.
#[cfg(unix)]
#[test]
fn combined_provider_lid_probe_preserves_existing_commands() {
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        process::{Command, Stdio},
        thread,
        time::Duration,
    };
    let root = std::env::temp_dir().join(format!("fungos-lid-e2e-{}", std::process::id()));
    fs::create_dir_all(root.join("sys/event7/device")).unwrap();
    fs::create_dir_all(root.join("dev")).unwrap();
    fs::create_dir_all(root.join("led/kbd_backlight")).unwrap();
    fs::create_dir_all(root.join("power")).unwrap();
    fs::write(root.join("sys/event7/device/name"), "Test Keyboard\n").unwrap();
    fs::write(root.join("dev/event7"), []).unwrap();
    fs::write(root.join("led/kbd_backlight/brightness"), "0\n").unwrap();
    fs::write(root.join("led/kbd_backlight/max_brightness"), "1\n").unwrap();
    let socket = root.join("provider.sock");
    let exe = env!("CARGO_BIN_EXE_fungos-keyboard-lightd");
    let mut provider = Command::new(exe)
        .env("FUNGOS_INPUT_NAME", "Test Keyboard")
        .env("FUNGOS_LED_NAME", "kbd_backlight")
        .env("FUNGOS_SYS_INPUT_ROOT", root.join("sys"))
        .env("FUNGOS_INPUT_ROOT", root.join("dev"))
        .env("FUNGOS_SYS_LED_ROOT", root.join("led"))
        .env("FUNGOS_POWER_SUPPLY_ROOT", root.join("power"))
        .env("FUNGOS_SOCKET", &socket)
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let result = std::panic::catch_unwind(|| {
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
        for (command, ok, expected) in [
            ("lid", false, "ERR lid=Unavailable"),
            ("lid-v1", false, "FUNGOS-LID/1\tERR\tunavailable"),
            ("status", true, "OK level="),
            ("battery", false, "ERR battery=Unavailable"),
        ] {
            let output = Command::new(exe)
                .args(["--request", command])
                .env("FUNGOS_SOCKET", &socket)
                .env_remove("FUNGOS_INPUT_NAME")
                .output()
                .unwrap();
            assert_eq!(output.status.success(), ok);
            assert!(String::from_utf8_lossy(&output.stdout).contains(expected));
        }
        let mut keys = fungos_device_hal::keypad_transport::KeypadClient::new(&socket);
        let initial = keys.poll(None).unwrap();
        assert!(initial.events.is_empty());
        assert_eq!(
            initial.resync,
            Some(fungos_device_hal::keypad::KeypadResync::NewClient)
        );
        let next = keys.poll(Some(initial.cursor)).unwrap();
        assert!(next.events.is_empty());
        assert_eq!(
            next.resync,
            Some(fungos_device_hal::keypad::KeypadResync::Dropped)
        );
        let client = fungos_device_hal::lid_transport::LidClient::new(&socket);
        assert_eq!(
            client.read().unwrap_err(),
            fungos_device_hal::lid::LidFailure::Unavailable
        );
        // Oversized and non-half-closed clients cannot kill the service.
        {
            use std::{io::Write, net::Shutdown, os::unix::net::UnixStream};
            let mut s = UnixStream::connect(&socket).unwrap();
            s.write_all(&[b'x'; 300]).unwrap();
            s.shutdown(Shutdown::Write).unwrap();
        }
        assert_eq!(
            client.read().unwrap_err(),
            fungos_device_hal::lid::LidFailure::Unavailable
        );
        assert!(provider_running(&socket));
    });
    provider.kill().unwrap();
    provider.wait().unwrap();
    fs::remove_dir_all(root).unwrap();
    result.unwrap();

    fn provider_running(path: &std::path::Path) -> bool {
        std::os::unix::net::UnixStream::connect(path).is_ok()
    }
}
