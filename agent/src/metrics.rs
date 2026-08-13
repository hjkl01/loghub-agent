use std::sync::{Arc, RwLock};
use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Default, Serialize, Clone)]
pub struct AgentMetrics {
    pub queue_size: u64,
    pub sent_total: u64,
    pub retry_total: u64,
    pub failed_total: u64,
    pub last_success: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
}

pub type SharedMetrics = Arc<RwLock<AgentMetrics>>;

impl AgentMetrics {
    pub fn new() -> Self {
        Self {
            started_at: Some(Utc::now()),
            ..Default::default()
        }
    }

    pub fn prometheus(&self) -> String {
        format!(
            "# HELP loghub_agent_queue_size Current queue size\n# TYPE loghub_agent_queue_size gauge\nloghub_agent_queue_size {}\n# HELP loghub_agent_sent_total Sent logs\n# TYPE loghub_agent_sent_total counter\nloghub_agent_sent_total {}\n# HELP loghub_agent_retry_total Retry count\n# TYPE loghub_agent_retry_total counter\nloghub_agent_retry_total {}\n# HELP loghub_agent_failed_total Failed logs\n# TYPE loghub_agent_failed_total counter\nloghub_agent_failed_total {}\n",
            self.queue_size,
            self.sent_total,
            self.retry_total,
            self.failed_total
        )
    }
}
