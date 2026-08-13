# LogHub Agent

Rust-based active log collection and query platform. The project is intentionally separate from the existing `loghub` repository.

## Architecture

```text
Docker / systemd journal
          |
          v
   loghub-agent (Rust)
          |
   batch HTTP ingest
          |
          v
   loghub-server (Axum)
          |
          v
      PostgreSQL
          |
     REST / WebSocket
```

## Features

- Docker container log collection through the Docker API.
- systemd journal collection through `journalctl -o json`.
- Unified log model with host, source, category, level, message and metadata.
- Batch ingestion endpoint.
- PostgreSQL storage with indexes for time/source/category/level.
- Basic automatic category classification.
- WebSocket stream for newly ingested logs.
- Docker Compose deployment for server + PostgreSQL.
- systemd service example for the agent.
- GitHub Actions Rust CI.

## Quick start

### 1. Start server and PostgreSQL

```bash
docker compose up -d --build
```

The API listens on `http://127.0.0.1:8080`.

Health check:

```bash
curl http://127.0.0.1:8080/health
```

### 2. Build the agent

The agent must run on the Linux host whose Docker and systemd logs are being collected.

```bash
cargo build --release -p loghub-agent
```

For Docker collection, the agent needs access to the Docker socket. When running as a normal host process, the usual `/var/run/docker.sock` access applies.

For systemd collection, the agent invokes `journalctl`. The service account therefore needs permission to read the journal; on many distributions this means membership in `systemd-journal`.

### 3. Configure the agent

```bash
export LOGHUB_SERVER_URL=http://127.0.0.1:8080
export LOGHUB_HOSTNAME=$(hostname)
# Optional. If omitted, all currently installed service units are discovered.
export SYSTEMD_UNITS=docker.service,caddy.service,postgresql.service
./target/release/loghub-agent
```

## API

### Ingest

```http
POST /api/agent/logs
Content-Type: application/json
```

Body:

```json
{
  "logs": [
    {
      "log_time": "2026-08-13T06:00:00Z",
      "host": "server-01",
      "source_type": "docker",
      "source_name": "nginx",
      "message": "GET /health 200",
      "level": "info",
      "metadata": {}
    }
  ]
}
```

### Query

```http
GET /api/logs?category=database&level=error&limit=200
```

Supported filters:

- `start`
- `end`
- `host`
- `source_type`
- `source_name`
- `category`
- `level`
- `keyword`
- `limit`
- `offset`

### Categories

```http
GET /api/logs/categories
```

### Realtime stream

Connect to:

```text
ws://127.0.0.1:8080/api/logs/ws
```

Each newly accepted log is broadcast as JSON.

## Categories

The initial classifier recognizes common sources such as PostgreSQL, Redis, Nginx, Caddy, SSH and Docker. The database also contains a `log_rules` table intended for configurable source-to-category rules.

## Current scope

This first implementation establishes the end-to-end architecture. The next iterations should add durable agent-side buffering/retry, reconnectable collector loops, rule-driven classification, authentication, and a Web UI with filtered realtime subscriptions.
