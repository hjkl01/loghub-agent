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

const BATCH_SIZE: i32 = 200;
const LEASE_SECONDS: i64 = 30;

fn retry_delay(retry_count: i64) -> Duration {
    let seconds = (2_i64.pow(retry_count.min(5) as u32)).min(60);
    Duration::from_secs(seconds as u64)
}

pub async fn run_sender(config: Config, queue: QueueStore) -> Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    loop {
        let items = queue.reserve_batch(BATCH_SIZE, LEASE_SECONDS)?;

        if items.is_empty() {
            sleep(Duration::from_secs(3)).await;
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
                info!(count = ids.len(), "acknowledged queued logs");
            }
            Ok(resp) => {
                warn!(status = %resp.status(), "log upload rejected");
                for item in items {
                    queue.increase_retry(item.id)?;
                }
                let max_retry = items.iter().map(|x| x.retry_count).max().unwrap_or(0);
                sleep(retry_delay(max_retry)).await;
            }
            Err(err) => {
                warn!(error = %err, "log upload failed");
                for item in items {
                    queue.increase_retry(item.id)?;
                }
                let max_retry = ids.len() as i64;
                sleep(retry_delay(max_retry)).await;
            }
        }
    }
}
