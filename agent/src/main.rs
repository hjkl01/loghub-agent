mod config;

use anyhow::Result;
use bollard::{container::LogsOptions, Docker};
use chrono::{DateTime, Utc};
use clap::Parser;
use config::Config as FileConfig;
use futures_util::StreamExt;
use reqwest::Client;
use serde::Serialize;
use serde_json::Value;
use std::{env, sync::Arc};
use tokio::{io::{AsyncBufReadExt, BufReader}, process::Command, sync::mpsc};
use tracing::{error, info, warn};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    config: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct LogRecord {
    log_time: DateTime<Utc>,
    host: String,
    source_type: String,
    source_name: String,
    category: Option<String>,
    level: Option<String>,
    message: String,
    metadata: Value,
}

#[derive(Clone)]
struct RuntimeConfig {
    server_url: String,
    host: String,
    token: Option<String>,
    systemd_units: Vec<String>,
}

async fn send_batch(client: &Client, config: &RuntimeConfig, batch: &[LogRecord]) {
    if batch.is_empty() { return; }
    let mut request = client.post(format!("{}/api/agent/logs", config.server_url))
        .json(&serde_json::json!({"logs": batch}));
    if let Some(token) = &config.token {
        request = request.bearer_auth(token);
    }
    if let Err(e) = request.send().await {
        warn!(error=%e, "failed to send logs");
    }
}

fn build_config(file: FileConfig) -> RuntimeConfig {
    RuntimeConfig {
        server_url: file.server_url.or_else(|| env::var("LOGHUB_SERVER_URL").ok()).unwrap_or_else(|| "http://127.0.0.1:8080".into()),
        host: file.host.or_else(|| env::var("LOGHUB_HOSTNAME").ok()).unwrap_or_else(hostname),
        token: file.token.or_else(|| env::var("LOGHUB_TOKEN").ok()),
        systemd_units: file.systemd_units.or_else(|| env::var("SYSTEMD_UNITS").ok().map(|x| x.split(',').map(str::trim).map(str::to_string).collect())).unwrap_or_default(),
    }
}

async fn docker_collector(tx: mpsc::Sender<LogRecord>, host: String) -> Result<()> {
    let docker = Docker::connect_with_local_defaults()?;
    for c in docker.list_containers(None).await? {
        let id = match c.id { Some(v) => v, None => continue };
        let name = c.names.unwrap_or_default().first().cloned().unwrap_or_else(|| id[..12.min(id.len())].to_string()).trim_start_matches('/').to_string();
        let mut stream = docker.logs::<String>(&id, Some(LogsOptions { follow:true, stdout:true, stderr:true, timestamps:true, tail:"all".into(), ..Default::default() }));
        let tx2=tx.clone(); let host2=host.clone();
        tokio::spawn(async move {
            while let Some(item)=stream.next().await { if let Ok(output)=item { let (_, msg)=parse_docker_line(&output.to_string()); let _=tx2.send(LogRecord{log_time:Utc::now(),host:host2.clone(),source_type:"docker".into(),source_name:name.clone(),category:None,level:None,message:msg,metadata:serde_json::json!({"container_id":id})}).await; } }
        });
    }
    Ok(())
}

fn parse_docker_line(raw:&str)->(DateTime<Utc>,String){ (Utc::now(), raw.trim().to_string()) }

async fn systemd_collector(tx:mpsc::Sender<LogRecord>, host:String, units:Vec<String>)->Result<()> {
    for unit in units {
        let mut child=Command::new("journalctl").args(["-u",&unit,"-f","-o","json","--no-pager"]).stdout(std::process::Stdio::piped()).spawn()?;
        let stdout=child.stdout.take().unwrap(); let mut lines=BufReader::new(stdout).lines(); let tx2=tx.clone(); let host2=host.clone();
        tokio::spawn(async move { while let Ok(Some(line))=lines.next_line().await { if let Ok(v)=serde_json::from_str::<Value>(&line) { let msg=v.get("MESSAGE").and_then(Value::as_str).unwrap_or("").to_string(); let _=tx2.send(LogRecord{log_time:Utc::now(),host:host2.clone(),source_type:"systemd".into(),source_name:unit.clone(),category:None,level:None,message:msg,metadata:v}).await; } } });
    }
    Ok(())
}

#[tokio::main]
async fn main()->Result<()> {
    tracing_subscriber::fmt().with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_|"info".into())).init();
    let args=Args::parse();
    let config=build_config(FileConfig::load(args.config.as_deref())?);
    let (tx,mut rx)=mpsc::channel::<LogRecord>(10000);
    let client=Arc::new(Client::new()); let sender_config=config.clone(); let sender_client=client.clone();
    tokio::spawn(async move { let mut batch=Vec::new(); while let Some(log)=rx.recv().await { batch.push(log); if batch.len()>=200 { send_batch(&sender_client,&sender_config,&batch).await; batch.clear(); } } });
    if let Err(e)=docker_collector(tx.clone(),config.host.clone()).await { error!(error=%e,"docker collector failed"); }
    if let Err(e)=systemd_collector(tx,config.host.clone(),config.systemd_units.clone()).await { error!(error=%e,"systemd collector failed"); }
    info!("agent started");
    tokio::signal::ctrl_c().await?;
    Ok(())
}

fn hostname()->String{std::process::Command::new("hostname").output().ok().map(|o|String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_else(||"unknown".into())}
