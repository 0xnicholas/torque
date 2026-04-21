-- up
ALTER TABLE v1_team_tasks
    ADD COLUMN input_artifacts JSONB NOT NULL DEFAULT '[]',
    ADD COLUMN parent_task_id UUID REFERENCES v1_team_tasks(id),
    ADD COLUMN idempotency_key TEXT;

CREATE UNIQUE INDEX idx_v1_team_tasks_idempotency_key ON v1_team_tasks(team_instance_id, idempotency_key) WHERE idempotency_key IS NOT NULL;

-- down
DROP INDEX IF EXISTS idx_v1_team_tasks_idempotency_key;
ALTER TABLE v1_team_tasks
    DROP COLUMN idempotency_key,
    DROP COLUMN parent_task_id,
    DROP COLUMN input_artifacts;