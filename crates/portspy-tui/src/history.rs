//! Session history for observed listener counts and process memory.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use portspy_core::domain::process::ProcessIdentity;
use portspy_core::snapshot::SystemSnapshot;

pub(crate) const LISTENER_WINDOW: Duration = Duration::from_secs(600);
pub(crate) const MEMORY_WINDOW: Duration = Duration::from_secs(60);
const MAX_SAMPLES: usize = 320;

#[derive(Debug, Clone, Default)]
pub(crate) struct History {
    samples: VecDeque<Sample>,
}

#[derive(Debug, Clone)]
struct Sample {
    at: Instant,
    listeners: u64,
    memory: HashMap<ProcessIdentity, u64>,
}

impl History {
    pub(crate) fn observe(&mut self, snapshot: &SystemSnapshot) {
        let at = snapshot.captured_at;
        let memory = snapshot
            .listeners
            .iter()
            .filter_map(|listener| {
                let identity = listener.process.identity();
                (identity.pid.as_u32() != 0 && identity.is_stable())
                    .then_some((identity, listener.process.memory_bytes?))
            })
            .collect();

        self.samples.push_back(Sample {
            at,
            listeners: snapshot.listeners.len() as u64,
            memory,
        });
        while self.samples.len() > MAX_SAMPLES
            || self
                .samples
                .front()
                .is_some_and(|sample| at.saturating_duration_since(sample.at) > LISTENER_WINDOW)
        {
            self.samples.pop_front();
        }
    }

    pub(crate) fn listeners(&self, now: Instant) -> Vec<(Duration, Option<u64>)> {
        self.series(now, LISTENER_WINDOW, |sample| Some(sample.listeners))
    }

    pub(crate) fn memory(
        &self,
        now: Instant,
        identity: ProcessIdentity,
    ) -> Vec<(Duration, Option<u64>)> {
        self.series(now, MEMORY_WINDOW, |sample| {
            sample.memory.get(&identity).copied()
        })
    }

    fn series(
        &self,
        now: Instant,
        window: Duration,
        value: impl Fn(&Sample) -> Option<u64>,
    ) -> Vec<(Duration, Option<u64>)> {
        let mut points: Vec<_> = self
            .samples
            .iter()
            .rev()
            .map(|sample| (now.saturating_duration_since(sample.at), sample))
            .take_while(|(age, _)| *age <= window)
            .map(|(age, sample)| (age, value(sample)))
            .collect();
        points.reverse();
        points
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use portspy_core::domain::process::ProcessId;
    use std::time::UNIX_EPOCH;

    #[test]
    fn history_keeps_gaps_and_distinguishes_recycled_process_ids() {
        let now = Instant::now();
        let original = ProcessIdentity::new(ProcessId(42), Some(UNIX_EPOCH));
        let replacement =
            ProcessIdentity::new(ProcessId(42), Some(UNIX_EPOCH + Duration::from_secs(30)));
        let samples = VecDeque::from([
            Sample {
                at: now - Duration::from_secs(25),
                listeners: 2,
                memory: HashMap::from([(original, 8)]),
            },
            Sample {
                at: now - Duration::from_secs(14),
                listeners: 1,
                memory: HashMap::from([(replacement, 16)]),
            },
            Sample {
                at: now,
                listeners: 1,
                memory: HashMap::new(),
            },
        ]);
        let history = History { samples };
        assert_eq!(
            history.memory(now, original),
            [
                (Duration::from_secs(25), Some(8)),
                (Duration::from_secs(14), None),
                (Duration::ZERO, None)
            ]
        );
        assert_eq!(
            history.memory(now, replacement),
            [
                (Duration::from_secs(25), None),
                (Duration::from_secs(14), Some(16)),
                (Duration::ZERO, None)
            ]
        );
    }
}
