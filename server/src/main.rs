use axum::{extract::{Path, Query, State, WebSocketUpgrade, ws::{Message, WebSocket}}, response::IntoResponse, routing::{delete, get, post}, Json, Router};
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{env, net::SocketAddr, sync::Arc};
use tokio::sync::broadcast;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;
use uuid::Uuid;

#[derive(Clone)] struct AppState { pool: PgPool, tx: broadcast::Sender<LogEvent> }
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)] struct LogEvent { id: Uuid, log_time: DateTime<Utc>, ingest_time: DateTime<Utc>, host: String, source_type: String, source_name: String, category: String, level: Option<String>, message: String, metadata: Value }
#[derive(Debug, Deserialize)] struct IngestRequest { logs: Vec<IncomingLog> }
#[derive(Debug, Deserialize)] struct IncomingLog { log_time: DateTime<Utc>, host: String, source_type: String, source_name: String, category: Option<String>, level: Option<String>, message: String, #[serde(default)] metadata: Value }
#[derive(Debug, Deserialize)] struct LogQuery { start: Option<DateTime<Utc>>, end: Option<DateTime<Utc>>, host: Option<String>, source_type: Option<String>, source_name: Option<String>, category: Option<String>, level: Option<String>, keyword: Option<String>, limit: Option<i64>, offset: Option<i64> }
#[derive(Debug, Serialize, sqlx::FromRow)] struct LogRule { id: Uuid, source_type: Option<String>, source_pattern: Option<String>, category: String, priority: i32, enabled: bool }
#[derive(Debug, Deserialize)] struct RuleRequest { source_type: Option<String>, source_pattern: Option<String>, category: String, priority: Option<i32>, enabled: Option<bool> }
#[derive(Serialize)] struct ListResponse { logs: Vec<LogEvent> }

async fn ingest(State(state): State<Arc<AppState>>, Json(req): Json<IngestRequest>) -> impl IntoResponse {
    let mut inserted = Vec::with_capacity(req.logs.len());
    for item in req.logs {
        let id = Uuid::new_v4();
        let fallback = item.category.unwrap_or_else(|| "system".into());
        let category = classify_with_rules(&state.pool, &item.source_type, &item.source_name).await.unwrap_or(fallback);
        let log = LogEvent { id, log_time: item.log_time, ingest_time: Utc::now(), host: item.host, source_type: item.source_type, source_name: item.source_name, category, level: item.level, message: item.message, metadata: item.metadata };
        let result = sqlx::query("INSERT INTO logs (id, log_time, ingest_time, host, source_type, source_name, category, level, message, metadata) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
            .bind(log.id).bind(log.log_time).bind(log.ingest_time).bind(&log.host).bind(&log.source_type).bind(&log.source_name).bind(&log.category).bind(&log.level).bind(&log.message).bind(&log.metadata).execute(&state.pool).await;
        if result.is_ok() { let _ = state.tx.send(log.clone()); inserted.push(log); }
    }
    Json(serde_json::json!({"accepted": inserted.len()}))
}

async fn classify_with_rules(pool: &PgPool, source_type: &str, source_name: &str) -> Option<String> {
    let rules: Vec<LogRule> = sqlx::query_as("SELECT id, source_type, source_pattern, category, priority, enabled FROM log_rules WHERE enabled = TRUE ORDER BY priority DESC, id")
        .fetch_all(pool).await.ok()?;
    rules.into_iter().find(|r| {
        let type_ok = r.source_type.as_deref().map(|v| v == source_type).unwrap_or(true);
        let pattern_ok = r.source_pattern.as_deref().map(|p| wildcard_match(p, source_name)).unwrap_or(true);
        type_ok && pattern_ok
    }).map(|r| r.category)
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    let p = pattern.to_ascii_lowercase(); let v = value.to_ascii_lowercase();
    if p == "*" { return true; }
    let parts: Vec<&str> = p.split('*').collect();
    if parts.len() == 1 { return p == v; }
    let mut pos = 0usize;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() { continue; }
        if i == 0 { if !v.starts_with(part) { return false; } pos = part.len(); }
        else if i == parts.len() - 1 { return v[pos..].ends_with(part); }
        else if let Some(found) = v[pos..].find(part) { pos += found + part.len(); } else { return false; }
    }
    true
}

async fn logs(State(state): State<Arc<AppState>>, Query(q): Query<LogQuery>) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(200).clamp(1, 1000); let offset = q.offset.unwrap_or(0).max(0);
    let rows = sqlx::query_as::<_, LogEvent>("SELECT id, log_time, ingest_time, host, source_type, source_name, category, level, message, metadata FROM logs WHERE ($1::timestamptz IS NULL OR log_time >= $1) AND ($2::timestamptz IS NULL OR log_time <= $2) AND ($3::text IS NULL OR host = $3) AND ($4::text IS NULL OR source_type = $4) AND ($5::text IS NULL OR source_name = $5) AND ($6::text IS NULL OR category = $6) AND ($7::text IS NULL OR level = $7) AND ($8::text IS NULL OR message ILIKE '%' || $8 || '%') ORDER BY log_time DESC LIMIT $9 OFFSET $10")
        .bind(q.start).bind(q.end).bind(q.host).bind(q.source_type).bind(q.source_name).bind(q.category).bind(q.level).bind(q.keyword).bind(limit).bind(offset).fetch_all(&state.pool).await.unwrap_or_default();
    Json(ListResponse { logs: rows })
}

async fn categories(State(state): State<Arc<AppState>>) -> impl IntoResponse { let rows: Vec<(String,)> = sqlx::query_as("SELECT DISTINCT category FROM logs ORDER BY category").fetch_all(&state.pool).await.unwrap_or_default(); Json(rows.into_iter().map(|x| x.0).collect::<Vec<_>>()) }
async fn rules(State(state): State<Arc<AppState>>) -> impl IntoResponse { let rows: Vec<LogRule> = sqlx::query_as("SELECT id, source_type, source_pattern, category, priority, enabled FROM log_rules ORDER BY priority DESC, category, source_pattern").fetch_all(&state.pool).await.unwrap_or_default(); Json(rows) }
async fn create_rule(State(state): State<Arc<AppState>>, Json(req): Json<RuleRequest>) -> impl IntoResponse { let id=Uuid::new_v4(); let priority=req.priority.unwrap_or(0); let enabled=req.enabled.unwrap_or(true); let result=sqlx::query("INSERT INTO log_rules (id,source_type,source_pattern,category,priority,enabled) VALUES ($1,$2,$3,$4,$5,$6)").bind(id).bind(req.source_type).bind(req.source_pattern).bind(req.category).bind(priority).bind(enabled).execute(&state.pool).await; Json(serde_json::json!({"success":result.is_ok(),"id":id})) }
async fn delete_rule(State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> impl IntoResponse { let result=sqlx::query("DELETE FROM log_rules WHERE id=$1").bind(id).execute(&state.pool).await; Json(serde_json::json!({"success":result.is_ok()})) }

async fn reclassify(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let rows: Vec<LogEvent> = match sqlx::query_as("SELECT id, log_time, ingest_time, host, source_type, source_name, category, level, message, metadata FROM logs")
        .fetch_all(&state.pool).await { Ok(rows) => rows, Err(e) => { tracing::error!(error=%e, "读取历史日志失败"); return Json(serde_json::json!({"success":false,"updated":0,"error":"读取历史日志失败"})); } };
    let mut updated = 0usize;
    for log in rows {
        if let Some(category) = classify_with_rules(&state.pool, &log.source_type, &log.source_name).await {
            if category != log.category {
                if sqlx::query("UPDATE logs SET category=$1 WHERE id=$2").bind(&category).bind(log.id).execute(&state.pool).await.is_ok() { updated += 1; }
            }
        }
    }
    let _ = state.tx.send(LogEvent { id: Uuid::nil(), log_time: Utc::now(), ingest_time: Utc::now(), host: "__system__".into(), source_type: "system".into(), source_name: "reclassify".into(), category: "system".into(), level: Some("info".into()), message: format!("历史日志重新分类完成，共更新 {} 条", updated), metadata: serde_json::json!({"updated": updated}) });
    Json(serde_json::json!({"success":true,"updated":updated,"total":rows.len()}))
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse { ws.on_upgrade(move |socket| websocket(socket, state)) }
async fn websocket(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx=state.tx.subscribe(); let mut filter: Option<WsFilter>=None;
    loop { tokio::select! {
        msg=rx.recv()=>match msg { Ok(log)=>{ if log.id != Uuid::nil() && filter.as_ref().map(|f|f.matches(&log)).unwrap_or(true) { if socket.send(Message::Text(serde_json::to_string(&log).unwrap().into())).await.is_err(){break;} } }, Err(broadcast::error::RecvError::Lagged(_))=>continue, Err(_)=>break },
        incoming=socket.next()=>match incoming { Some(Ok(Message::Text(text)))=>{ if let Ok(f)=serde_json::from_str::<WsFilter>(&text){filter=Some(f);} }, Some(Ok(Message::Close(_)))|None=>break, _=>{} }
    }}
}
#[derive(Debug,Default,Deserialize)] struct WsFilter { host: Option<String>, source_type: Option<String>, source_name: Option<String>, category: Option<String>, level: Option<String>, keyword: Option<String> }
impl WsFilter { fn matches(&self,l:&LogEvent)->bool { self.host.as_ref().map(|x|x==&l.host).unwrap_or(true)&&self.source_type.as_ref().map(|x|x==&l.source_type).unwrap_or(true)&&self.source_name.as_ref().map(|x|x==&l.source_name).unwrap_or(true)&&self.category.as_ref().map(|x|x==&l.category).unwrap_or(true)&&self.level.as_ref().map(|x|x==l.level.as_ref().unwrap_or(&String::new())).unwrap_or(true)&&self.keyword.as_ref().map(|x|l.message.to_ascii_lowercase().contains(&x.to_ascii_lowercase())).unwrap_or(true) } }

#[tokio::main] async fn main()->anyhow::Result<()> { tracing_subscriber::fmt().with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_|"info".into())).init(); let database_url=env::var("DATABASE_URL")?; let pool=PgPoolOptions::new().max_connections(20).connect(&database_url).await?; let(tx,_)=broadcast::channel(2048); let state=Arc::new(AppState{pool,tx}); let app=Router::new().route("/health",get(||async{"ok"})).route("/api/agent/logs",post(ingest)).route("/api/logs",get(logs)).route("/api/logs/categories",get(categories)).route("/api/logs/rules",get(rules).post(create_rule)).route("/api/logs/rules/:id",delete(delete_rule)).route("/api/logs/reclassify",post(reclassify)).route("/api/logs/ws",get(ws_handler)).layer(CorsLayer::permissive()).layer(TraceLayer::new_for_http()).with_state(state); let addr:SocketAddr=env::var("LISTEN_ADDR").unwrap_or_else(|_|"0.0.0.0:8080".into()).parse()?; info!(%addr,"loghub server started"); let listener=tokio::net::TcpListener::bind(addr).await?; axum::serve(listener,app).await?; Ok(()) }
