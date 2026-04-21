-- up
CREATE TABLE v1_team_definitions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    description TEXT,
    supervisor_agent_definition_id UUID NOT NULL,
    sub_agents JSONB NOT NULL DEFAULT '[]',
    policy JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE v1_team_instances (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_definition_id UUID NOT NULL REFERENCES v1_team_definitions(id),
    status TEXT NOT NULL DEFAULT 'CREATED',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
