//! Lightweight tab lifecycle scheduler for Nexus Engine 1.02.
//!
//! The Alpha scheduler intentionally does not execute background JavaScript.
//! It tracks lifecycle state so Android can suspend inactive tabs predictably
//! today and evolve toward throttled background execution later.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::browser::TabId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabLifecycle {
    Active,
    Suspended,
    Frozen,
    Discarded,
}

#[derive(Debug, Clone, Copy)]
pub struct SchedulerPolicy {
    pub freeze_after: Duration,
}

impl Default for SchedulerPolicy {
    fn default() -> Self {
        Self { freeze_after: Duration::from_secs(30) }
    }
}

#[derive(Debug, Clone)]
struct TabScheduleEntry {
    lifecycle: TabLifecycle,
    last_active: Instant,
}

#[derive(Debug)]
pub struct TabScheduler {
    policy: SchedulerPolicy,
    entries: HashMap<TabId, TabScheduleEntry>,
}

impl Default for TabScheduler {
    fn default() -> Self {
        Self::new(SchedulerPolicy::default())
    }
}

impl TabScheduler {
    #[must_use]
    pub fn new(policy: SchedulerPolicy) -> Self {
        Self { policy, entries: HashMap::new() }
    }

    pub fn register(&mut self, id: TabId, active: bool) {
        self.entries.insert(id, TabScheduleEntry {
            lifecycle: if active { TabLifecycle::Active } else { TabLifecycle::Suspended },
            last_active: Instant::now(),
        });
    }

    pub fn activate(&mut self, id: TabId) {
        let now = Instant::now();
        for (candidate, entry) in &mut self.entries {
            if *candidate == id {
                entry.lifecycle = TabLifecycle::Active;
                entry.last_active = now;
            } else if entry.lifecycle == TabLifecycle::Active {
                entry.lifecycle = TabLifecycle::Suspended;
            }
        }
    }

    pub fn remove(&mut self, id: TabId) {
        self.entries.remove(&id);
    }

    pub fn discard(&mut self, id: TabId) -> bool {
        let Some(entry) = self.entries.get_mut(&id) else { return false };
        if entry.lifecycle == TabLifecycle::Active { return false; }
        entry.lifecycle = TabLifecycle::Discarded;
        true
    }

    pub fn discard_inactive(&mut self, maximum: usize) -> Vec<TabId> {
        self.discard_inactive_excluding(maximum, &[])
    }

    pub fn discard_inactive_excluding(&mut self, maximum: usize, protected: &[TabId]) -> Vec<TabId> {
        let mut candidates = self.entries.iter()
            .filter(|(_, entry)| entry.lifecycle != TabLifecycle::Active && entry.lifecycle != TabLifecycle::Discarded)
            .filter(|(id, _)| !protected.contains(id))
            .map(|(id, entry)| (*id, entry.last_active)).collect::<Vec<_>>();
        candidates.sort_by_key(|(_, last_active)| *last_active);
        let mut discarded = Vec::new();
        for (id, _) in candidates.into_iter().take(maximum) {
            if self.discard(id) { discarded.push(id); }
        }
        discarded
    }

    pub fn refresh(&mut self) {
        let now = Instant::now();
        for entry in self.entries.values_mut() {
            if entry.lifecycle == TabLifecycle::Suspended
                && now.duration_since(entry.last_active) >= self.policy.freeze_after
            {
                entry.lifecycle = TabLifecycle::Frozen;
            }
        }
    }

    #[must_use]
    pub fn lifecycle(&self, id: TabId) -> TabLifecycle {
        self.entries.get(&id).map_or(TabLifecycle::Suspended, |entry| entry.lifecycle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activating_one_tab_suspends_previous() {
        let mut scheduler = TabScheduler::default();
        scheduler.register(1, true);
        scheduler.register(2, false);
        scheduler.activate(2);
        assert_eq!(scheduler.lifecycle(1), TabLifecycle::Suspended);
        assert_eq!(scheduler.lifecycle(2), TabLifecycle::Active);
    }

    #[test]
    fn memory_pressure_discards_only_inactive_tabs() {
        let mut scheduler = TabScheduler::default();
        scheduler.register(1, true);
        scheduler.register(2, false);
        assert_eq!(scheduler.discard_inactive(1), vec![2]);
        assert_eq!(scheduler.lifecycle(1), TabLifecycle::Active);
        assert_eq!(scheduler.lifecycle(2), TabLifecycle::Discarded);
    }

    #[test]
    fn protected_tabs_are_skipped_by_discard_selection() {
        let mut scheduler = TabScheduler::default();
        scheduler.register(1, true); scheduler.register(2, false); scheduler.register(3, false);
        assert_eq!(scheduler.discard_inactive_excluding(2, &[2]), vec![3]);
        assert_eq!(scheduler.lifecycle(2), TabLifecycle::Suspended);
    }
}
