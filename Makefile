.PHONY: help build check fmt test server agent web up down logs clean docker-agent docker-agent-build run-agent

help:
	@echo "LogHub Agent 命令："
	@echo "  make build              编译 Server 和 Agent"
	@echo "  make check              cargo check"
	@echo "  make fmt                格式化 Rust 代码"
	@echo "  make test               运行测试"
	@echo "  make up                 启动 PostgreSQL + Server + Web"
	@echo "  make down               停止 Docker Compose 服务"
	@echo "  make logs               查看 Server 日志"
	@echo "  make web                本地启动 Web 开发服务器"
	@echo "  make server             本地启动 Server"
	@echo "  make agent              本地启动 Agent"
	@echo "  make docker-agent-build  构建 Agent Docker 镜像"
	@echo "  make docker-agent        使用 Docker 启动 Agent"
	@echo "  make clean              清理构建产物"

build:
	cargo build --release --workspace

check:
	cargo check --workspace

fmt:
	cargo fmt --all

test:
	cargo test --workspace

up:
	docker compose up -d --build

down:
	docker compose down

logs:
	docker compose logs -f server

web:
	cd web && npm install && npm run dev

server:
	cargo run --release -p loghub-server

agent:
	cargo run --release -p loghub-agent

docker-agent-build:
	docker build -f deploy/Dockerfile.agent -t loghub-agent:latest .

docker-agent:
	docker compose --profile agent up -d --build agent

clean:
	cargo clean
