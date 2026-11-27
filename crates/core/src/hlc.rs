//! Hybrid logical clock: wall time that never goes backwards, so a device with a
//! slow clock still produces edits newer than the ones it has already seen.

use serde::{Deserialize, Serialize};

/// Field order is the comparison order.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Stamp {
    pub ms: u64,
    pub counter: u32,
    pub device: String,
}

#[derive(Debug, Default)]
pub struct Clock {
    ms: u64,
    counter: u32,
}

impl Clock {
    pub fn observe(&mut self, stamp: &Stamp) {
        if (stamp.ms, stamp.counter) > (self.ms, self.counter) {
            self.ms = stamp.ms;
            self.counter = stamp.counter;
        }
    }

    pub fn next(&mut self, now_ms: u64, device: &str) -> Stamp {
        if now_ms > self.ms {
            self.ms = now_ms;
            self.counter = 0;
        } else if let Some(c) = self.counter.checked_add(1) {
            self.counter = c;
        } else {
            self.ms = self.ms.saturating_add(1);
            self.counter = 0;
        }
        Stamp {
            ms: self.ms,
            counter: self.counter,
            device: device.to_owned(),
        }
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stays_ahead_of_a_peer_with_a_fast_clock() {
        let mut clock = Clock::default();
        let remote = Stamp {
            ms: 5_000,
            counter: 3,
            device: "b".into(),
        };
        clock.observe(&remote);
        assert!(clock.next(1_000, "a") > remote);
    }

    #[test]
    fn is_strictly_increasing_within_one_millisecond() {
        let mut clock = Clock::default();
        let first = clock.next(10, "a");
        assert!(clock.next(10, "a") > first);
    }
}
