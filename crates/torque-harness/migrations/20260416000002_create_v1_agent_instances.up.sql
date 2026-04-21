CREATE TABLE v1_agent_instances (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_definition_id UUID NOT NULL REFERENCES v1_agent_definitions(id),
    status TEXT NOT NULL DEFAULT 'CREATED',
    external_context_refs JSONB NOT NULL DEFAULT '[]',
    current_task_id UUID,
    checkpoint_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
