use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DuplicateSubmitOutcome {
    InFlight,
    Accepted,
    Stale,
    LowDiff,
    Bad,
}

struct DuplicateSubmitEntry {
    ts: Instant,
    outcome: DuplicateSubmitOutcome,
}

pub(crate) struct DuplicateSubmitGuard {
    ttl: Duration,
    max_entries: usize,
    entries: HashMap<String, DuplicateSubmitEntry>,
    order: VecDeque<String>,
}

impl DuplicateSubmitGuard {
    pub(crate) fn new(ttl: Duration, max_entries: usize) -> Self {
        Self { ttl, max_entries, entries: HashMap::new(), order: VecDeque::new() }
    }

    fn prune(&mut self, now: Instant) {
        while let Some(front) = self.order.front() {
            let remove = match self.entries.get(front) {
                Some(e) => now.duration_since(e.ts) > self.ttl,
                None => true,
            };
            if remove {
                if let Some(key) = self.order.pop_front() {
                    self.entries.remove(&key);
                }
            } else {
                break;
            }
        }

        while self.entries.len() > self.max_entries {
            if let Some(key) = self.order.pop_front() {
                self.entries.remove(&key);
            } else {
                break;
            }
        }
    }

    pub(crate) fn get(&mut self, key: &str, now: Instant) -> Option<DuplicateSubmitOutcome> {
        self.prune(now);
        self.entries.get(key).map(|e| e.outcome)
    }

    pub(crate) fn insert_inflight(&mut self, key: String, now: Instant) {
        self.prune(now);
        if self.entries.contains_key(&key) {
            return;
        }
        self.entries.insert(key.clone(), DuplicateSubmitEntry { ts: now, outcome: DuplicateSubmitOutcome::InFlight });
        self.order.push_back(key);
    }

    pub(crate) fn set_outcome(&mut self, key: &str, now: Instant, outcome: DuplicateSubmitOutcome) {
        self.prune(now);
        if let Some(e) = self.entries.get_mut(key) {
            e.ts = now;
            e.outcome = outcome;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn insert_then_get_inflight() {
        let mut g = DuplicateSubmitGuard::new(Duration::from_secs(60), 100);
        let now = Instant::now();
        g.insert_inflight("a".to_string(), now);
        assert_eq!(g.get("a", now), Some(DuplicateSubmitOutcome::InFlight));
    }

    #[test]
    fn duplicate_insert_is_idempotent() {
        let mut g = DuplicateSubmitGuard::new(Duration::from_secs(60), 100);
        let now = Instant::now();
        g.insert_inflight("k".to_string(), now);
        g.insert_inflight("k".to_string(), now);
        assert_eq!(g.get("k", now), Some(DuplicateSubmitOutcome::InFlight));
    }

    #[test]
    fn set_outcome_updates_entry() {
        let mut g = DuplicateSubmitGuard::new(Duration::from_secs(60), 100);
        let now = Instant::now();
        g.insert_inflight("h".to_string(), now);
        let later = now + Duration::from_millis(1);
        g.set_outcome("h", later, DuplicateSubmitOutcome::Accepted);
        assert_eq!(g.get("h", later), Some(DuplicateSubmitOutcome::Accepted));
    }

    #[test]
    fn ttl_prunes_old_entries() {
        let mut g = DuplicateSubmitGuard::new(Duration::from_millis(30), 100);
        let t0 = Instant::now();
        g.insert_inflight("old".to_string(), t0);
        thread::sleep(Duration::from_millis(80));
        let now = Instant::now();
        assert_eq!(g.get("old", now), None);
    }

    #[test]
    fn max_entries_evicts_oldest() {
        let mut g = DuplicateSubmitGuard::new(Duration::from_secs(600), 2);
        let mut now = Instant::now();
        g.insert_inflight("k1".to_string(), now);
        now += Duration::from_millis(1);
        g.insert_inflight("k2".to_string(), now);
        now += Duration::from_millis(1);
        g.insert_inflight("k3".to_string(), now);
        assert_eq!(g.get("k1", now), None);
        assert_eq!(g.get("k2", now), Some(DuplicateSubmitOutcome::InFlight));
        assert_eq!(g.get("k3", now), Some(DuplicateSubmitOutcome::InFlight));
    }
}
