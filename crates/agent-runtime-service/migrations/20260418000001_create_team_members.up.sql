CREATE TABLE v1_team_members (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_instance_id UUID NOT NULL REFERENCES v1_team_instances(id),
    agent_instance_id UUID NOT NULL REFERENCES v1_agent_instances(id),
    role TEXT NOT NULL DEFAULT 'member',
    status TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(team_instance_id, agent_instance_id)
);

CREATE INDEX idx_v1_team_members_team ON v1_team_members(team_instance_id);
CREATE INDEX idx_v1_team_members_agent ON v1_team_members(agent_instance_id);