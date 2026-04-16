CREATE TABLE checkpoints (
    id UUID PRIMARY KEY,
    instance_id UUID,
    task_id UUID,
    snapshot JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_checkpoints_instance ON checkpoints(instance_id, created_at DESC);
