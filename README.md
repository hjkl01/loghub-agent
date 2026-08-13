# LogHub Agent

基于 Rust 的主动日志采集与查询平台。本项目与现有的 `loghub` 仓库完全独立。

## 架构

```text
Docker / systemd journal
          |
          v
   loghub-agent (Rust)
          |
      批量 HTTP 上报
          |
          v
   loghub-server (Axum)
          |
          v
      PostgreSQL
          |
      REST / WebSocket
```

## 功能

- 通过 Docker API 主动采集 Docker 容器日志。
- 通过 `journalctl -o json` 主动采集 systemd journal 日志。
- 统一日志模型，包含主机、来源、分类、级别、消息和元数据。
- 支持批量日志上报接口。
- 使用 PostgreSQL 存储日志，并针对时间、来源、分类、级别建立索引。
- 提供基础的自动日志分类能力。
- 通过 WebSocket 实时推送新接收的日志。
- 支持 Server + PostgreSQL Docker Compose 部署。
- Agent 同时支持直接运行和 Docker 运行。
- 提供 Makefile 统一管理常用开发、构建和部署命令。
- 提供 GitHub Actions Rust CI。

## 快速开始

### 使用 Makefile

项目提供 Makefile，建议直接使用：

```bash
make help
```

常用命令：

```bash
# 编译 Server 和 Agent
make build

# 检查代码
make check

# 格式化代码
make fmt

# 运行测试
make test

# 启动 PostgreSQL + Server
a make up

# 停止服务
make down

# 查看 Server 日志
make logs

# 本地启动 Server
make server

# 本地启动 Agent
make agent
```

> `make up` 依赖 Docker 和 Docker Compose。

### 1. 启动 Server 和 PostgreSQL

直接使用 Docker Compose：

```bash
docker compose up -d --build
```

或者：

```bash
make up
```

API 默认监听：`http://127.0.0.1:8080`。

健康检查：

```bash
curl http://127.0.0.1:8080/health
```

## Agent 运行方式

Agent 支持两种运行方式：

1. **直接运行在 Linux 主机上**：推荐用于生产环境和权限要求较严格的场景。
2. **运行在 Docker 容器中**：适合已经使用 Docker 管理基础服务的服务器。

### 方式一：直接运行 Agent

编译：

```bash
cargo build --release -p loghub-agent
```

或者：

```bash
make build
```

配置：

```bash
export LOGHUB_SERVER_URL=http://127.0.0.1:8080
export LOGHUB_HOSTNAME=$(hostname)
export SYSTEMD_UNITS=docker.service,caddy.service,postgresql.service
```

启动：

```bash
./target/release/loghub-agent
```

采集 Docker 日志时，Agent 需要访问 `/var/run/docker.sock`。

采集 systemd 日志时，Agent 会调用 `journalctl`，运行用户需要具备读取 journal 的权限；很多发行版可以通过加入 `systemd-journal` 用户组解决。

### 方式二：使用 Docker 运行 Agent

**可以。** Agent 已提供独立的 Dockerfile，并且 Docker Compose 已提供 `agent` profile。

Docker 运行 Agent 时，需要把宿主机的 Docker Socket 和 systemd journal 目录挂载进去：

```text
/var/run/docker.sock
/run/log/journal
/var/log/journal
/etc/machine-id
```

这些挂载允许容器中的 Agent 读取宿主机 Docker 和 systemd 日志。

#### 使用 Compose 启动

先启动 Server 和 PostgreSQL：

```bash
make up
```

然后启动 Agent：

```bash
make docker-agent
```

等价于：

```bash
docker compose --profile agent up -d --build agent
```

查看 Agent 日志：

```bash
docker compose logs -f agent
```

#### 配置需要采集的 systemd 服务

Docker 环境下建议显式指定 `SYSTEMD_UNITS`：

```bash
export SYSTEMD_UNITS=docker.service,caddy.service,postgresql.service
make docker-agent
```

也可以写入 `.env`：

```env
LOGHUB_HOSTNAME=server-01
SYSTEMD_UNITS=docker.service,caddy.service,postgresql.service
RUST_LOG=info
```

然后：

```bash
make docker-agent
```

如果不设置 `SYSTEMD_UNITS`，Agent 会尝试通过宿主机 journal 中的 `_SYSTEMD_UNIT` 字段自动发现日志单元。

> Docker Agent 默认以 root 用户运行，因为需要读取 Docker Socket 和宿主机 journal。请注意，访问 `/var/run/docker.sock` 本身就相当于授予较高的宿主机权限。

## API

### 日志上报

```http
POST /api/agent/logs
Content-Type: application/json
```

请求示例：

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

### 日志查询

```http
GET /api/logs?category=database&level=error&limit=200
```

支持以下查询条件：

- `start`：开始时间
- `end`：结束时间
- `host`：主机
- `source_type`：日志来源类型，例如 `docker`、`systemd`
- `source_name`：具体来源名称，例如容器名或 systemd service 名称
- `category`：日志分类
- `level`：日志级别
- `keyword`：关键词
- `limit`：返回数量
- `offset`：分页偏移量

### 查询日志分类

```http
GET /api/logs/categories
```

### 实时日志流

连接：

```text
ws://127.0.0.1:8080/api/logs/ws
```

每接收一条新日志，Server 都会通过 WebSocket 广播 JSON 格式的日志数据。

## 日志分类

当前版本的基础分类器可以识别 PostgreSQL、Redis、Nginx、Caddy、SSH、Docker 等常见日志来源。

数据库中同时提供了 `log_rules` 表，用于后续实现可配置的“日志来源 → 日志分类”规则。

例如：

```text
docker / postgres*      → database
docker / redis*         → database
docker / nginx*         → web
systemd / ssh.service   → security
systemd / caddy.service → web
```

## 项目结构

```text
loghub-agent/
├── agent/              # Rust 日志采集 Agent
│   ├── Docker 采集器
│   └── systemd 采集器
│
├── server/             # Rust 日志服务端
│   ├── Axum API
│   ├── PostgreSQL
│   └── WebSocket
│
├── migrations/         # PostgreSQL 数据库迁移
├── deploy/             # Dockerfile / 部署文件
├── config/             # 配置文件示例
├── Makefile             # 常用开发和部署命令
└── .github/workflows/  # GitHub Actions
```

## 工作流程

```text
Docker 容器 / systemd journal
              |
              v
       loghub-agent
              |
        日志标准化
              |
        分类 / Level 提取
              |
              v
        批量 HTTP 上报
              |
              v
       loghub-server
          /         \
         v           v
   PostgreSQL    WebSocket
                     |
                     v
                  前端查询
```

## 当前版本范围

当前版本已经建立从“日志采集 → 标准化 → 上报 → PostgreSQL 存储 → REST 查询 → WebSocket 实时推送”的完整基础架构。

后续版本建议继续完善：

- Agent 本地持久化缓存和断线重试，避免 Server 暂时不可用时丢失日志。
- Docker / systemd 采集器自动重连。
- 基于 `log_rules` 的动态分类规则。
- Agent 与 Server 的身份认证。
- Web 前端日志查询界面。
- 前端按照筛选条件建立实时日志订阅。
- 日志保留策略和自动清理。
- 大规模日志场景下的批量写入和性能优化。
