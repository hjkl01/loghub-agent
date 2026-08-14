use anyhow::Result;
use reqwest::Client;
use serde::Serialize;
use tokio::time::{sleep, Duration};

#[derive(Debug, Serialize)]
struct HeartbeatPayload {
    agent_id: String,
    hostname: String,
    version: String,
}

pub async fn run_heartbeat(
    server_url: String,
    token: Option<String>,
    agent_id: String,
    hostname: String,
) -> Result<()> {
    let client = Client::new();

    loop {
        let mut request = client
            .post(format!("{}/api/agent/heartbeat", server_url))
            .json(&HeartbeatPayload {
                agent_id: agent_id.clone(),
                hostname: hostname.clone(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            });

        if let Some(value) = &token {
            request = request.bearer_auth(value);
        }

        if let Err(err) = request.send().await {
            tracing::warn!(error = %err, "heartbeat failed");
        }

        sleep(Duration::from_secs(30)).await;
    }
}
