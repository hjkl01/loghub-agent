CREATE TABLE IF NOT EXISTS logs (
    id UUID PRIMARY KEY,
    log_time TIMESTAMPTZ NOT NULL,
    ingest_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    host VARCHAR(255) NOT NULL,
    source_type VARCHAR(32) NOT NULL,
    source_name VARCHAR(255) NOT NULL,
    category VARCHAR(64) NOT NULL,
    level VARCHAR(32),
    message TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE INDEX IF NOT EXISTS idx_logs_log_time ON logs(log_time DESC);
CREATE INDEX IF NOT EXISTS idx_logs_host_time ON logs(host, log_time DESC);
CREATE INDEX IF NOT EXISTS idx_logs_source ON logs(source_type, source_name, log_time DESC);
CREATE INDEX IF NOT EXISTS idx_logs_category ON logs(category, log_time DESC);
CREATE INDEX IF NOT EXISTS idx_logs_level ON logs(level, log_time DESC);
CREATE INDEX IF NOT EXISTS idx_logs_message_search ON logs USING GIN (to_tsvector('simple', message));

CREATE TABLE IF NOT EXISTS log_rules (
    id UUID PRIMARY KEY,
    source_type VARCHAR(32),
    source_pattern VARCHAR(255),
    category VARCHAR(64) NOT NULL,
    priority INT NOT NULL DEFAULT 0,
    enabled BOOLEAN NOT NULL DEFAULT TRUE
);

INSERT INTO log_rules (id, source_type, source_pattern, category, priority)
VALUES
    (gen_random_uuid(), 'docker', 'nginx*', 'web', 100),
    (gen_random_uuid(), 'docker', 'postgres*', 'database', 100),
    (gen_random_uuid(), 'docker', 'redis*', 'database', 100),
    (gen_random_uuid(), 'systemd', 'ssh*', 'security', 100),
    (gen_random_uuid(), 'systemd', 'docker*', 'container', 100),
    (gen_random_uuid(), 'systemd', 'caddy*', 'web', 100)
ON CONFLICT DO NOTHING;
