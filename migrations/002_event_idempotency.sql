ALTER TABLE logs
    ADD COLUMN IF NOT EXISTS event_id UUID;

UPDATE logs
SET event_id = id
WHERE event_id IS NULL;

ALTER TABLE logs
    ALTER COLUMN event_id SET NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_logs_event_id_unique
    ON logs(event_id);
