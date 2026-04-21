CREATE TABLE v1_agent_definitions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    description TEXT,
    system_prompt TEXT,
    tool_policy JSONB NOT NULL DEFAULT '{}',
    memory_policy JSONB NOT NULL DEFAULT '{}',
    delegation_policy JSONB NOT NULL DEFAULT '{}',
    limits JSONB NOT NULL DEFAULT '{}',
    default_model_policy JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_v1_agent_definitions_name ON v1_agent_definitions(name);
