-- up
CREATE TABLE v1_checkpoints (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_instance_id UUID NOT NULL,
    task_id UUID,
    snapshot JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_v1_checkpoints_instance ON v1_checkpoints(agent_instance_id, created_at DESC);
