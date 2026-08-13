use anyhow::Result;
use bollard::{container::LogsOptions, Docker};
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use reqwest::Client;
use serde::Serialize;
use serde_json::Value;
use std::{env, sync::Arc};
use tokio::{io::{AsyncBufReadExt, BufReader}, process::Command, sync::mpsc};
use tracing::{error, info, warn};

#[derive(Debug, Clone, Serialize)]
struct LogRecord {
    log_time: DateTime<Utc>, host: String, source_type: String, source_name: String,
    category: Option<String>, level: Option<String>, message: String, metadata: Value,
}

#[derive(Clone)]
struct Config { server_url: String, host: String, systemd_units: Vec<String> }

async fn send_batch(client: &Client, url: &str, batch: &[LogRecord]) {
    if batch.is_empty() { return; }
    if let Err(e) = client.post(format!("{url}/api/agent/logs")).json(&serde_json::json!({"logs": batch})).send().await {
        warn!(error=%e, "failed to send logs; batch is dropped in this MVP");
    }
}

async fn docker_collector(tx: mpsc::Sender<LogRecord>, host: String) -> Result<()> {
    let docker = Docker::connect_with_local_defaults()?;
    let containers = docker.list_containers(None).await?;
    for c in containers {
        let id = match c.id { Some(v) => v, None => continue };
        let name = c.names.unwrap_or_default().first().cloned().unwrap_or_else(|| id[..12.min(id.len())].to_string()).trim_start_matches('/').to_string();
        let mut stream = docker.logs::<String>(&id, Some(LogsOptions { follow: true, stdout: true, stderr: true, timestamps: true, tail: "all".into(), ..Default::default() }));
        let tx2 = tx.clone(); let host2 = host.clone();
        tokio::spawn(async move {
            while let Some(item) = stream.next().await {
                match item {
                    Ok(output) => {
                        let raw = output.to_string();
                        let (time, message) = parse_docker_line(&raw);
                        if tx2.send(LogRecord { log_time: time, host: host2.clone(), source_type: "docker".into(), source_name: name.clone(), category: None, level: None, message, metadata: serde_json::json!({"container_id": id}) }).await.is_err() { break; }
                    }
                    Err(e) => { warn!(container=%name, error=%e, "docker log stream ended"); break; }
                }
            }
        });
    }
    Ok(())
}

fn parse_docker_line(raw: &str) -> (DateTime<Utc>, String) {
    let line = raw.trim_matches(['\r','\n']);
    if let Some((ts, msg)) = line.split_once(' ') { if let Ok(t) = DateTime::parse_from_rfc3339(ts) { return (t.with_timezone(&Utc), msg.to_string()); } }
    (Utc::now(), line.to_string())
}

async fn systemd_collector(tx: mpsc::Sender<LogRecord>, host: String, units: Vec<String>) -> Result<()> {
    for unit in units {
        let mut child = Command::new("journalctl").args(["-u", &unit, "-f", "-o", "json", "--no-pager"]).stdout(std::process::Stdio::piped()).spawn()?;
        let stdout = child.stdout.take().unwrap(); let mut lines = BufReader::new(stdout).lines(); let tx2 = tx.clone(); let host2 = host.clone();
        tokio::spawn(async move {
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(v) = serde_json::from_str::<Value>(&line) {
                    let ts = v.get("__REALTIME_TIMESTAMP").and_then(Value::as_str).and_then(|x| x.parse::<i64>().ok()).and_then(|x| DateTime::from_timestamp_micros(x)).unwrap_or_else(Utc::now);
                    let msg = v.get("MESSAGE").and_then(Value::as_str).unwrap_or("").to_string();
                    let priority = v.get("PRIORITY").and_then(Value::as_str).map(|p| match p { "0"|"1"|"2"|"3" => "error", "4" => "warning", "5"|"6" => "info", _ => "debug" }.to_string());
                    let record = LogRecord { log_time: ts, host: host2.clone(), source_type: "systemd".into(), source_name: unit.clone(), category: None, level: priority, message: msg, metadata: v };
                    if tx2.send(record).await.is_err() { break; }
                }
            }
        });
    }
    Ok(())
}

async fn discover_units() -> Vec<String> {
    // Use journalctl instead of systemctl so the same discovery works inside a
    // container with the host journal mounted read-only.
    let output = Command::new("journalctl").args(["-F", "_SYSTEMD_UNIT", "--no-pager"]).output().await;
    output.ok().map(|o| String::from_utf8_lossy(&o.stdout).lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect()).unwrap_or_default()
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_| "info".into())).init();
    let config = Config { server_url: env::var("LOGHUB_SERVER_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".into()), host: env::var("LOGHUB_HOSTNAME").unwrap_or_else(|_| hostname()), systemd_units: env::var("SYSTEMD_UNITS").ok().map(|x| x.split(',').map(str::trim).filter(|x| !x.is_empty()).map(str::to_string).collect()).unwrap_or_default() };
    let (tx, mut rx) = mpsc::channel::<LogRecord>(10000);
    let client = Arc::new(Client::new());
    let server_url = config.server_url.clone(); let sender_client = client.clone();
    tokio::spawn(async move {
        let mut batch = Vec::with_capacity(200); let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
        loop { tokio::select! { Some(log) = rx.recv() => { batch.push(log); if batch.len() >= 200 { send_batch(&sender_client, &server_url, &batch).await; batch.clear(); } }, _ = interval.tick() => { if !batch.is_empty() { send_batch(&sender_client, &server_url, &batch).await; batch.clear(); } } } }
    });
    if let Err(e) = docker_collector(tx.clone(), config.host.clone()).await { error!(error=%e, "docker collector failed"); }
    let units = if config.systemd_units.is_empty() { discover_units().await } else { config.systemd_units.clone() };
    info!(count=units.len(), "starting systemd collectors");
    if let Err(e) = systemd_collector(tx, config.host.clone(), units).await { error!(error=%e, "systemd collector failed"); }
    tokio::signal::ctrl_c().await?; Ok(())
}

fn hostname() -> String { std::process::Command::new("hostname").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).filter(|x| !x.is_empty()).unwrap_or_else(|| "unknown".into()) }
