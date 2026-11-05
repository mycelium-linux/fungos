//! Non-destructive multi-reader keyboard stream, no UI navigation policy.
use std::collections::VecDeque;
pub const RING_CAPACITY: usize = 256;
pub const BATCH_CAPACITY: usize = 32;
pub const MAX_AGE_MS: u64 = 1000;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyAction {
    Press,
    Release,
    Repeat,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeypadCursor {
    pub provider_instance: u64,
    pub sequence: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeypadEvent {
    pub sequence: u64,
    pub code: u16,
    pub action: KeyAction,
    pub observed_monotonic_ms: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeypadResync {
    NewClient,
    Overflow,
    Restart,
    Dropped,
    Stale,
    Reconnect,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeypadBatch {
    pub cursor: KeypadCursor,
    pub events: Vec<KeypadEvent>,
    pub resync: Option<KeypadResync>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeypadFailure {
    Denied,
    Disconnected,
    Protocol,
    Backend,
}
pub struct KeypadRing {
    instance: u64,
    sequence: u64,
    events: VecDeque<KeypadEvent>,
    invalidated: u64,
    dropped: bool,
}
impl KeypadRing {
    /// Deployment uses a random instance from the OS; tests inject deterministic IDs.
    pub fn new(provider_instance: u64) -> Self {
        Self {
            instance: provider_instance,
            sequence: 0,
            events: VecDeque::new(),
            invalidated: 0,
            dropped: false,
        }
    }
    pub fn head(&self) -> KeypadCursor {
        KeypadCursor {
            provider_instance: self.instance,
            sequence: self.sequence,
        }
    }
    /// Called only by the existing input reader. No synthetic releases on loss.
    pub fn input(&mut self, ty: u16, code: u16, value: i32, now: u64) {
        if ty == 0 && code == 3 {
            self.sequence = self
                .sequence
                .checked_add(1)
                .expect("keyboard sequence exhausted");
            self.invalidated = self.sequence;
            self.events.clear();
            self.dropped = true;
            return;
        }
        if ty == 0 && code == 0 {
            self.dropped = false;
            return;
        }
        if self.dropped || ty != 1 || code == 0 {
            return;
        }
        let action = match value {
            0 => KeyAction::Release,
            1 => KeyAction::Press,
            2 => KeyAction::Repeat,
            _ => return,
        };
        self.sequence = self
            .sequence
            .checked_add(1)
            .expect("keyboard sequence exhausted");
        self.events.push_back(KeypadEvent {
            sequence: self.sequence,
            code,
            action,
            observed_monotonic_ms: now,
        });
        if self.events.len() > RING_CAPACITY {
            self.events.pop_front();
        }
    }
    pub fn poll(&self, cursor: Option<KeypadCursor>, now: u64) -> KeypadBatch {
        let resync = |reason| KeypadBatch {
            cursor: self.head(),
            events: Vec::new(),
            resync: Some(reason),
        };
        let Some(c) = cursor else {
            return resync(KeypadResync::NewClient);
        };
        if c.provider_instance != self.instance {
            return resync(KeypadResync::Restart);
        }
        if self.dropped || c.sequence < self.invalidated {
            return resync(KeypadResync::Dropped);
        }
        if c.sequence > self.sequence {
            return resync(KeypadResync::Restart);
        }
        if self
            .events
            .front()
            .is_some_and(|e| c.sequence.saturating_add(1) < e.sequence)
        {
            return resync(KeypadResync::Overflow);
        }
        let pending = self.events.iter().filter(|e| e.sequence > c.sequence);
        if pending.clone().any(|e| {
            now.checked_sub(e.observed_monotonic_ms)
                .is_none_or(|age| age > MAX_AGE_MS)
        }) {
            return resync(KeypadResync::Stale);
        }
        let events: Vec<_> = pending.take(BATCH_CAPACITY).copied().collect();
        KeypadBatch {
            cursor: KeypadCursor {
                provider_instance: self.instance,
                sequence: events.last().map_or(c.sequence, |e| e.sequence),
            },
            events,
            resync: None,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_readers_order_repeat_release_and_no_history() {
        let mut r = KeypadRing::new(1);
        r.input(1, 28, 1, 0);
        let a = r.poll(None, 0);
        assert!(a.events.is_empty());
        let b = a.cursor;
        for v in [1, 2, 0] {
            r.input(1, 103, v, 10);
        }
        let first = r.poll(Some(b), 10);
        assert_eq!(
            first.events.iter().map(|e| e.action).collect::<Vec<_>>(),
            [KeyAction::Press, KeyAction::Repeat, KeyAction::Release]
        );
        assert_eq!(r.poll(Some(a.cursor), 10), first);
        assert!(r.poll(Some(first.cursor), 10).events.is_empty());
    }
    #[test]
    fn overflow_stale_restart_drop_and_bounds() {
        let mut r = KeypadRing::new(1);
        let c = r.head();
        for _ in 0..257 {
            r.input(1, 28, 1, 0);
        }
        assert_eq!(r.poll(Some(c), 0).resync, Some(KeypadResync::Overflow));
        let c = KeypadCursor {
            provider_instance: 1,
            sequence: 1,
        };
        assert_eq!(r.poll(Some(c), 1000).events.len(), 32);
        assert_eq!(r.poll(Some(c), 1001).resync, Some(KeypadResync::Stale));
        assert_eq!(
            KeypadRing::new(2).poll(Some(c), 0).resync,
            Some(KeypadResync::Restart)
        );
        let c = r.head();
        r.input(0, 3, 0, 0);
        r.input(1, 28, 0, 0);
        assert_eq!(r.poll(Some(c), 0).resync, Some(KeypadResync::Dropped));
        let head = r.head();
        r.input(0, 0, 0, 0);
        assert!(r.poll(Some(head), 0).events.is_empty());
    }
}
