CREATE TABLE IF NOT EXISTS ephemeral_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plan_id UUID NOT NULL,
    task_id UUID NOT NULL,
    input TEXT,
    output TEXT,
    duration_ms INTEGER,
    status VARCHAR(20) NOT NULL DEFAULT 'completed',
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT ephemeral_logs_status_check
        CHECK (status IN ('completed', 'failed', 'timeout'))
);

CREATE INDEX IF NOT EXISTS idx_ephemeral_logs_plan
    ON ephemeral_logs(plan_id);
CREATE INDEX IF NOT EXISTS idx_ephemeral_logs_task
    ON ephemeral_logs(task_id);
CREATE INDEX IF NOT EXISTS idx_ephemeral_logs_created_at
    ON ephemeral_logs(created_at DESC);