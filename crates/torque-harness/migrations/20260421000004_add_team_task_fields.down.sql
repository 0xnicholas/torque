-- down
DROP INDEX IF EXISTS idx_v1_team_tasks_idempotency_key;
ALTER TABLE v1_team_tasks
    DROP COLUMN idempotency_key,
    DROP COLUMN parent_task_id,
    DROP COLUMN input_artifacts;