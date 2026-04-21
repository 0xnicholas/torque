-- up
CREATE TABLE v1_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_type TEXT NOT NULL,
    parent_task_id UUID,
    agent_instance_id UUID,
    team_instance_id UUID,
    status TEXT NOT NULL,
    goal TEXT NOT NULL,
    instructions TEXT,
    input_artifacts JSONB NOT NULL DEFAULT '[]',
    produced_artifacts JSONB NOT NULL DEFAULT '[]',
    delegation_ids JSONB NOT NULL DEFAULT '[]',
    approval_ids JSONB NOT NULL DEFAULT '[]',
    checkpoint_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
