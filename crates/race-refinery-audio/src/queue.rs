use super::speech::SpeechPlan;

/// Alert priority ΓÇö higher values preempt lower in the queue.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct SpeechPriority(pub u8);

impl SpeechPriority {
    pub const PACE: Self = Self(0);
    pub const RACE: Self = Self(1);
    pub const PACK: Self = Self(2);
    pub const SAFETY: Self = Self(3);
    pub const CRITICAL: Self = Self(4);
}

pub struct QueuedSpeech {
    pub priority: SpeechPriority,
    pub plan: SpeechPlan,
}

/// Small priority queue for coach output (max 3 items).
pub struct SpeechQueue {
    items: Vec<QueuedSpeech>,
    capacity: usize,
}

impl SpeechQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::new(),
            capacity,
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn push(&mut self, priority: SpeechPriority, plan: SpeechPlan) {
        if self.items.len() >= self.capacity {
            if let Some(lowest_idx) = self
                .items
                .iter()
                .enumerate()
                .min_by_key(|(_, q)| q.priority)
                .map(|(i, _)| i)
            {
                if priority > self.items[lowest_idx].priority {
                    tracing::debug!(
                        "Speech queue full; dropping {:?}",
                        self.items[lowest_idx].plan.display_text()
                    );
                    self.items.remove(lowest_idx);
                } else {
                    tracing::debug!(
                        "Speech queue full; dropping incoming {:?}",
                        plan.display_text()
                    );
                    return;
                }
            }
        }
        self.items.push(QueuedSpeech { priority, plan });
        self.items
            .sort_by_key(|item| std::cmp::Reverse(item.priority));
    }

    pub fn pop(&mut self) -> Option<SpeechPlan> {
        if self.items.is_empty() {
            return None;
        }
        Some(self.items.remove(0).plan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(key: &str) -> SpeechPlan {
        SpeechPlan::clip(key)
    }

    #[test]
    fn drains_highest_priority_first() {
        let mut q = SpeechQueue::new(3);
        q.push(SpeechPriority::PACE, plan("pace"));
        q.push(SpeechPriority::CRITICAL, plan("critical"));
        q.push(SpeechPriority::PACK, plan("pack"));
        assert_eq!(q.pop().unwrap().display_text(), "critical");
        assert_eq!(q.pop().unwrap().display_text(), "pack");
        assert_eq!(q.pop().unwrap().display_text(), "pace");
    }

    #[test]
    fn overflow_drops_lowest() {
        let mut q = SpeechQueue::new(2);
        q.push(SpeechPriority::PACE, plan("pace"));
        q.push(SpeechPriority::PACK, plan("pack"));
        q.push(SpeechPriority::CRITICAL, plan("critical"));
        assert_eq!(q.len(), 2);
        assert_eq!(q.pop().unwrap().display_text(), "critical");
        assert_eq!(q.pop().unwrap().display_text(), "pack");
    }
}
