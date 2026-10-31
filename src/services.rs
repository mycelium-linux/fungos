/// Frontend-neutral read-only view of platform accessories and battery.
/// Implementations must use platform-observed data and return Stale instead of
/// serving a battery reading older than the caller's max age. BlueZ remains
/// authoritative for bonds; this interface never pairs or mutates them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessoryItem {
    pub identity: String,
    pub kind: AccessoryType,
    pub connected: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessoryType {
    Headphones,
    Charger,
    Wearable,
    Other,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatterySample {
    pub percent: u8,
    pub charging: bool,
    pub observed_monotonic_ms: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessoryView {
    pub items: Vec<AccessoryItem>,
    pub battery: Option<BatterySample>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessoryFailure {
    Unavailable,
    Stale,
    Denied,
    Backend,
}
pub trait AccessoryInventory: Send + Sync {
    fn snapshot(
        &self,
        now_monotonic_ms: u64,
        max_age_ms: u64,
    ) -> Result<AccessoryView, AccessoryFailure>;
}

/// Frontend-neutral audio route observation and volume control. This is only a
/// client seam to the platform's existing policy owner, never a competing mixer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioOutput {
    pub identity: String,
    pub available: bool,
    pub volume_percent: u8,
    pub muted: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioFailure {
    Unavailable,
    Denied,
    InvalidVolume,
    Backend,
}
pub trait AudioRouteControl: Send + Sync {
    fn outputs(&self) -> Result<Vec<AudioOutput>, AudioFailure>;
    fn set_volume(&self, identity: &str, volume_percent: u8) -> Result<(), AudioFailure>;
}

#[cfg(test)]
mod service_contract_tests {
    use super::*;
    struct Fake;
    impl AccessoryInventory for Fake {
        fn snapshot(&self, now: u64, age: u64) -> Result<AccessoryView, AccessoryFailure> {
            let battery = BatterySample {
                percent: 64,
                charging: true,
                observed_monotonic_ms: now.saturating_sub(4),
            };
            if now.saturating_sub(battery.observed_monotonic_ms) > age {
                return Err(AccessoryFailure::Stale);
            }
            Ok(AccessoryView {
                items: vec![AccessoryItem {
                    identity: "fake-accessory".into(),
                    kind: AccessoryType::Wearable,
                    connected: true,
                }],
                battery: Some(battery),
            })
        }
    }
    impl AudioRouteControl for Fake {
        fn outputs(&self) -> Result<Vec<AudioOutput>, AudioFailure> {
            Ok(vec![AudioOutput {
                identity: "platform-default".into(),
                available: true,
                volume_percent: 40,
                muted: false,
            }])
        }
        fn set_volume(&self, id: &str, level: u8) -> Result<(), AudioFailure> {
            if id != "platform-default" {
                return Err(AudioFailure::Unavailable);
            }
            if level > 100 {
                return Err(AudioFailure::InvalidVolume);
            }
            Ok(())
        }
    }
    struct Missing;
    impl AccessoryInventory for Missing {
        fn snapshot(&self, _: u64, _: u64) -> Result<AccessoryView, AccessoryFailure> {
            Err(AccessoryFailure::Unavailable)
        }
    }
    impl AudioRouteControl for Missing {
        fn outputs(&self) -> Result<Vec<AudioOutput>, AudioFailure> {
            Err(AudioFailure::Unavailable)
        }
        fn set_volume(&self, _: &str, _: u8) -> Result<(), AudioFailure> {
            Err(AudioFailure::Unavailable)
        }
    }
    #[test]
    fn accessory_inventory_freshness_and_absence() {
        let f = Fake;
        let v = f.snapshot(100, 4).unwrap();
        assert_eq!(v.items.len(), 1);
        assert_eq!(v.battery.unwrap().percent, 64);
        assert_eq!(f.snapshot(100, 3), Err(AccessoryFailure::Stale));
        assert_eq!(Missing.snapshot(0, 0), Err(AccessoryFailure::Unavailable));
    }
    #[test]
    fn audio_controls_validate_and_report_unavailable() {
        let f = Fake;
        assert_eq!(f.outputs().unwrap()[0].volume_percent, 40);
        assert_eq!(f.set_volume("platform-default", 100), Ok(()));
        assert_eq!(
            f.set_volume("platform-default", 101),
            Err(AudioFailure::InvalidVolume)
        );
        assert_eq!(Missing.outputs(), Err(AudioFailure::Unavailable));
    }
}
