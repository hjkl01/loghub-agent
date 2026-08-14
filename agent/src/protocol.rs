use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct UploadRequest<T> {
    pub agent_id: String,
    pub logs: Vec<T>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UploadAck {
    pub accepted: Vec<i64>,
    pub failed: Vec<i64>,
}

impl UploadAck {
    pub fn is_success(&self, id: i64) -> bool {
        self.accepted.contains(&id)
    }

    pub fn should_retry(&self, id: i64) -> bool {
        self.failed.contains(&id)
    }
}
