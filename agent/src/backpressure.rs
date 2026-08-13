use anyhow::{bail, Result};

#[derive(Debug, Clone)]
pub struct QueuePolicy {
    /// Maximum number of pending events allowed locally.
    pub max_events: i64,
    /// Whether the collector should reject new events when full.
    pub reject_when_full: bool,
}

impl Default for QueuePolicy {
    fn default() -> Self {
        Self {
            max_events: 500_000,
            reject_when_full: true,
        }
    }
}

impl QueuePolicy {
    pub fn check_capacity(&self, current_size: i64) -> Result<()> {
        if current_size >= self.max_events && self.reject_when_full {
            bail!("local queue capacity exceeded: {}/{}", current_size, self.max_events);
        }
        Ok(())
    }

    pub fn should_warn(&self, current_size: i64) -> bool {
        current_size >= (self.max_events as f64 * 0.8) as i64
    }
}
