use anyhow::Result;
use reqwest::Client;
use serde::Serialize;
use serde_json::Value;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};

use crate::config::Config;
use crate::storage::QueueStore;

#[derive(Debug, Serialize)]
struct QueuePayload {
    logs: Vec<Value>,
}

pub async fn run_sender(config: Config, queue: QueueStore) -> Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    loop {
        let items = queue.list(200)?;

        if items.is_empty() {
            sleep(Duration::from_secs(5)).await;
            continue;
        }

        let ids: Vec<i64> = items.iter().map(|item| item.id).collect();
        let payload = QueuePayload {
            logs: items.iter().map(|item| item.payload.clone()).collect(),
        };

        let mut request = client
            .post(format!("{}/api/agent/logs", config.server_url))
            .json(&payload);

        if let Some(token) = &config.token {
            request = request.bearer_auth(token);
        }

        match request.send().await {
            Ok(resp) if resp.status().is_success() => {
                queue.remove_batch(&ids)?;
                info!(count = ids.len(), "flushed queued logs");
            }
            Ok(resp) => {
                warn!(status = %resp.status(), "log upload rejected");
                for id in ids {
                    queue.increase_retry(id)?;
                }
                sleep(Duration::from_secs(5)).await;
            }
            Err(err) => {
                warn!(error = %err, "log upload failed");
                for id in ids {
                    queue.increase_retry(id)?;
                }
                sleep(Duration::from_secs(5)).await;
            }
        }
    }
}
