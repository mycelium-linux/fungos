//! Portable device capability contracts and keyboard illumination policy.
//! Input events intentionally carry activity only, never key identity/content.
use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LightCapability {
    Unavailable,
    Binary,
    Brightness { max: u32 },
}

pub trait KeyboardLight {
    fn capability(&self) -> LightCapability;
    /// Set an absolute level. Binary devices accept only 0 or 1; brightness
    /// devices accept 0..=max. Unavailable devices must return an error.
    fn set_level(&mut self, level: u32) -> Result<(), String>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputActivity {
    KeyPress,
    KeyRepeat,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedCapability {
    Lid,
    Torch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Lease {
    pub level: u32,
    pub expires_at_ms: u64,
}

/// Deterministic policy state machine; callers supply monotonic milliseconds.
pub struct KeyboardLightPolicy {
    timeout_ms: u64,
    last_activity_ms: Option<u64>,
    lease: Option<Lease>,
    applied: Option<u32>,
}
impl Default for KeyboardLightPolicy {
    fn default() -> Self {
        Self::new(Duration::from_secs(5))
    }
}
impl KeyboardLightPolicy {
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout_ms: timeout.as_millis().min(u64::MAX as u128) as u64,
            last_activity_ms: None,
            lease: None,
            applied: None,
        }
    }
    pub fn activity(&mut self, _event: InputActivity, now_ms: u64) {
        self.last_activity_ms = Some(now_ms);
    }
    /// Leases are capped at one hour and levels are checked against capability.
    pub fn lease(
        &mut self,
        level: u32,
        now_ms: u64,
        duration: Duration,
        cap: LightCapability,
    ) -> Result<(), &'static str> {
        let max = match cap {
            LightCapability::Unavailable => return Err("light unavailable"),
            LightCapability::Binary => 1,
            LightCapability::Brightness { max } => max,
        };
        if level > max {
            return Err("level outside capability");
        }
        let ms = duration.as_millis().min(3_600_000) as u64;
        self.lease = Some(Lease {
            level,
            expires_at_ms: now_ms.saturating_add(ms),
        });
        Ok(())
    }
    pub fn clear_lease(&mut self) {
        self.lease = None;
    }
    pub fn desired(&mut self, now_ms: u64) -> u32 {
        if self.lease.is_some_and(|l| now_ms >= l.expires_at_ms) {
            self.lease = None;
        }
        if let Some(l) = self.lease {
            return l.level;
        }
        if self
            .last_activity_ms
            .is_some_and(|t| now_ms.saturating_sub(t) < self.timeout_ms)
        {
            1
        } else {
            0
        }
    }
    /// Reconcile hardware. Call on orderly shutdown with `force_off` before exit.
    pub fn reconcile(
        &mut self,
        light: &mut impl KeyboardLight,
        now_ms: u64,
        force_off: bool,
    ) -> Result<(), String> {
        let cap = light.capability();
        let desired = if force_off { 0 } else { self.desired(now_ms) };
        let level = match cap {
            LightCapability::Unavailable => return Ok(()),
            LightCapability::Binary => u32::from(desired != 0),
            LightCapability::Brightness { max } => desired.min(max),
        };
        if self.applied != Some(level) {
            light.set_level(level)?;
            self.applied = Some(level);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        level: u32,
    }
    impl KeyboardLight for Fake {
        fn capability(&self) -> LightCapability {
            LightCapability::Binary
        }
        fn set_level(&mut self, l: u32) -> Result<(), String> {
            self.level = l;
            Ok(())
        }
    }
    #[test]
    fn unavailable_and_brightness_validation() {
        struct Nope;
        impl KeyboardLight for Nope {
            fn capability(&self) -> LightCapability {
                LightCapability::Unavailable
            }
            fn set_level(&mut self, _: u32) -> Result<(), String> {
                Err("unsupported".into())
            }
        }
        let mut p = KeyboardLightPolicy::default();
        assert!(p
            .lease(0, 0, Duration::from_secs(1), LightCapability::Unavailable)
            .is_err());
        assert!(p.reconcile(&mut Nope, 0, false).is_ok());
        assert!(p
            .lease(
                8,
                0,
                Duration::from_secs(1),
                LightCapability::Brightness { max: 7 }
            )
            .is_err());
    }
    #[test]
    fn lease_priority_activity_and_expiry() {
        let mut p = KeyboardLightPolicy::default();
        p.activity(InputActivity::KeyPress, 100);
        p.lease(
            0,
            100,
            Duration::from_millis(20),
            LightCapability::Brightness { max: 10 },
        )
        .unwrap();
        assert_eq!(p.desired(110), 0);
        p.activity(InputActivity::KeyRepeat, 110);
        assert_eq!(p.desired(119), 0);
        assert_eq!(p.desired(120), 1);
        p.lease(
            2,
            121,
            Duration::from_secs(1),
            LightCapability::Brightness { max: 10 },
        )
        .unwrap();
        assert_eq!(p.desired(130), 2);
        p.clear_lease();
        assert_eq!(p.desired(130), 1);
    }
    #[test]
    fn default_off_timeout_repeat_and_shutdown() {
        let mut p = KeyboardLightPolicy::default();
        let mut f = Fake { level: 9 };
        p.reconcile(&mut f, 0, false).unwrap();
        assert_eq!(f.level, 0);
        p.activity(InputActivity::KeyRepeat, 10);
        p.reconcile(&mut f, 10, false).unwrap();
        assert_eq!(f.level, 1);
        p.reconcile(&mut f, 5010, false).unwrap();
        assert_eq!(f.level, 0);
        p.activity(InputActivity::KeyPress, 6000);
        p.reconcile(&mut f, 6000, false).unwrap();
        p.reconcile(&mut f, 6001, true).unwrap();
        assert_eq!(f.level, 0);
    }
    #[test]
    fn bounded_lease() {
        let mut p = KeyboardLightPolicy::default();
        assert!(p
            .lease(2, 0, Duration::from_secs(9), LightCapability::Binary)
            .is_err());
        p.lease(1, 0, Duration::from_secs(7200), LightCapability::Binary)
            .unwrap();
        assert_eq!(p.desired(3_599_999), 1);
        assert_eq!(p.desired(3_600_000), 0);
    }
}
