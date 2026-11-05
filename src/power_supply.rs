//! Read-only Linux battery backend hosted by the existing device provider.
//! No IIO/vendor probing, SOC estimation, charging control, or worker lifecycle.
use crate::services::{AccessoryFailure, BatteryProvenance, BatterySample, ChargingState};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub struct PowerSupply {
    root: PathBuf,
}
impl Default for PowerSupply {
    fn default() -> Self {
        Self::new("/sys/class/power_supply")
    }
}
impl PowerSupply {
    /// Root is provider/deployment configuration, never supplied by socket clients.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Reads all explicitly typed Battery entries, not USB/mains/accessory supplies.
    /// Empty/missing roots and absent batteries are Unavailable. A battery may have
    /// entirely unknown telemetry. Errors never reuse a previous successful read.
    /// `now_ms` must be from the provider's monotonic clock, sampled before reading.
    pub fn read(&self, now_ms: u64) -> Result<Vec<BatterySample>, AccessoryFailure> {
        let entries = fs::read_dir(&self.root).map_err(failure)?;
        let mut paths = entries
            .map(|e| e.map(|e| e.path()).map_err(failure))
            .collect::<Result<Vec<_>, _>>()?;
        paths.sort();
        let mut samples = Vec::new();
        for path in paths {
            if attribute(&path, "type")?.as_deref() != Some("Battery") {
                continue;
            }
            match attribute(&path, "present")?.as_deref() {
                Some("0") => continue,
                None | Some("1") => {}
                _ => return Err(AccessoryFailure::Backend),
            }
            // Device-scoped Bluetooth/peripheral batteries belong to accessory
            // inventory; do not claim them as the host battery.
            match attribute(&path, "scope")?.as_deref() {
                Some("Device") => continue,
                None | Some("System") | Some("Unknown") => {}
                _ => return Err(AccessoryFailure::Backend),
            }
            let percent = number::<u8>(&path, "capacity")?;
            if percent.is_some_and(|p| p > 100) {
                return Err(AccessoryFailure::Backend);
            }
            let charging = match attribute(&path, "status")?.as_deref() {
                None | Some("Unknown") => ChargingState::Unknown,
                Some("Charging") => ChargingState::Charging,
                Some("Discharging") => ChargingState::Discharging,
                Some("Not charging") => ChargingState::NotCharging,
                Some("Full") => ChargingState::Full,
                _ => return Err(AccessoryFailure::Backend),
            };
            samples.push(BatterySample {
                percent,
                charging,
                voltage_microvolts: number(&path, "voltage_now")?,
                current_microamps: number(&path, "current_now")?,
                temperature_deci_celsius: number(&path, "temp")?,
                provenance: BatteryProvenance::LinuxPowerSupply(path),
                observed_monotonic_ms: now_ms,
            });
        }
        if samples.is_empty() {
            Err(AccessoryFailure::Unavailable)
        } else {
            Ok(samples)
        }
    }
}
fn failure(e: io::Error) -> AccessoryFailure {
    match e.kind() {
        io::ErrorKind::NotFound => AccessoryFailure::Unavailable,
        io::ErrorKind::PermissionDenied => AccessoryFailure::Denied,
        _ => AccessoryFailure::Backend,
    }
}
fn attribute(path: &Path, name: &str) -> Result<Option<String>, AccessoryFailure> {
    match fs::read_to_string(path.join(name)) {
        Ok(s) => Ok(Some(s.trim().to_owned())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(failure(e)),
    }
}
fn number<T: std::str::FromStr>(path: &Path, name: &str) -> Result<Option<T>, AccessoryFailure> {
    attribute(path, name)?
        .map(|s| s.parse().map_err(|_| AccessoryFailure::Backend))
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    struct Root(PathBuf);
    impl Root {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let p = std::env::temp_dir().join(format!(
                "fungos-battery-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
        fn put(&self, file: &str, value: &str) {
            let p = self.0.join(file);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, value).unwrap();
        }
        fn read(&self) -> Result<Vec<BatterySample>, AccessoryFailure> {
            PowerSupply::new(&self.0).read(100)
        }
    }
    impl Drop for Root {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn qualified_071_voltage_only_standard_supply() {
        let r = Root::new();
        r.put("cat-s22-battery/type", "Battery\n");
        r.put("cat-s22-battery/voltage_now", "3830000\n");
        let samples = r.read().unwrap();
        assert_eq!(samples.len(), 1);
        let s = &samples[0];
        assert_eq!(s.voltage_microvolts, Some(3830000));
        assert_eq!(s.percent, None);
        assert_eq!(s.charging, ChargingState::Unknown);
        assert_eq!(s.current_microamps, None);
        assert_eq!(s.temperature_deci_celsius, None);
        assert_eq!(
            s.provenance,
            BatteryProvenance::LinuxPowerSupply(r.0.join("cat-s22-battery"))
        );
    }
    #[test]
    fn absence_unknowns_and_no_voltage_soc_inference() {
        let r = Root::new();
        assert_eq!(r.read(), Err(AccessoryFailure::Unavailable));
        assert_eq!(
            PowerSupply::new(r.0.join("missing")).read(0),
            Err(AccessoryFailure::Unavailable)
        );
        r.put("usb/type", "USB\n");
        assert_eq!(r.read(), Err(AccessoryFailure::Unavailable));
        r.put("BAT0/type", "Battery\n");
        let s = r.read().unwrap().remove(0);
        assert_eq!(s.percent, None);
        assert_eq!(s.charging, ChargingState::Unknown);
        assert_eq!(s.voltage_microvolts, None);
        r.put("BAT0/voltage_now", "4200000\n");
        assert_eq!(r.read().unwrap()[0].percent, None);
        r.put("BAT0/present", "0");
        assert_eq!(r.read(), Err(AccessoryFailure::Unavailable));
        r.put("BAT0/present", "1");
        r.put("BAT0/scope", "Device");
        assert_eq!(r.read(), Err(AccessoryFailure::Unavailable));
    }
    #[test]
    fn units_provenance_time_and_read_only() {
        let r = Root::new();
        for (k, v) in [
            ("type", "Battery"),
            ("capacity", "64\n"),
            ("status", "Not charging"),
            ("voltage_now", "3876000"),
            ("current_now", "-123456"),
            ("temp", "-51"),
            ("charge_control_limit", "80"),
        ] {
            r.put(&format!("BAT0/{k}"), v);
        }
        let s = r.read().unwrap().remove(0);
        assert_eq!(s.percent, Some(64));
        assert_eq!(s.charging, ChargingState::NotCharging);
        assert_eq!(s.voltage_microvolts, Some(3876000));
        assert_eq!(s.current_microamps, Some(-123456));
        assert_eq!(s.temperature_deci_celsius, Some(-51));
        assert_eq!(
            s.provenance,
            BatteryProvenance::LinuxPowerSupply(r.0.join("BAT0"))
        );
        assert_eq!(s.check_freshness(100, 0), Ok(()));
        assert_eq!(s.check_freshness(104, 4), Ok(()));
        assert_eq!(s.check_freshness(105, 4), Err(AccessoryFailure::Stale));
        assert_eq!(
            s.check_freshness(99, u64::MAX),
            Err(AccessoryFailure::InvalidTime)
        );
        assert_eq!(s.check_freshness(u64::MAX, 0), Err(AccessoryFailure::Stale));
        assert_eq!(
            fs::read_to_string(r.0.join("BAT0/charge_control_limit")).unwrap(),
            "80"
        );
        fs::remove_file(r.0.join("BAT0/capacity")).unwrap();
        assert_eq!(r.read().unwrap()[0].percent, None); // no cached success
    }
    #[test]
    fn malformed_and_out_of_range_fields_are_not_unknown_or_clamped() {
        let r = Root::new();
        r.put("BAT0/type", "Battery");
        for (field, bad) in [
            ("capacity", "101"),
            ("capacity", "-1"),
            ("capacity", "256"),
            ("capacity", ""),
            ("capacity", "4.2"),
            ("voltage_now", "-1"),
            ("voltage_now", "4294967296"),
            ("current_now", "2147483648"),
            ("temp", "-2147483649"),
            ("status", "bogus"),
            ("present", "2"),
        ] {
            r.put(&format!("BAT0/{field}"), bad);
            assert_eq!(r.read(), Err(AccessoryFailure::Backend), "{field}={bad}");
            fs::remove_file(r.0.join("BAT0").join(field)).unwrap();
        }
        for p in ["0", "100"] {
            r.put("BAT0/capacity", p);
            assert_eq!(r.read().unwrap()[0].percent, Some(p.parse().unwrap()));
        }
        for (v, expected) in [
            ("Unknown", ChargingState::Unknown),
            ("Charging", ChargingState::Charging),
            ("Discharging", ChargingState::Discharging),
            ("Full", ChargingState::Full),
        ] {
            r.put("BAT0/status", v);
            assert_eq!(r.read().unwrap()[0].charging, expected);
        }
    }
    #[test]
    fn permission_failures_are_distinct() {
        assert_eq!(
            failure(io::Error::from(io::ErrorKind::PermissionDenied)),
            AccessoryFailure::Denied
        );
    }
    #[cfg(unix)]
    #[test]
    fn filesystem_permission_boundary() {
        use std::os::unix::fs::PermissionsExt;
        let r = Root::new();
        r.put("BAT0/type", "Battery");
        r.put("BAT0/capacity", "50");
        let p = r.0.join("BAT0/capacity");
        fs::set_permissions(&p, fs::Permissions::from_mode(0o0)).unwrap();
        // Root can bypass DAC; error mapping is covered independently above.
        if unsafe { libc::geteuid() } != 0 {
            assert_eq!(r.read(), Err(AccessoryFailure::Denied));
        }
        fs::set_permissions(&p, fs::Permissions::from_mode(0o444)).unwrap();
        assert_eq!(r.read().unwrap()[0].percent, Some(50));
    }
}
