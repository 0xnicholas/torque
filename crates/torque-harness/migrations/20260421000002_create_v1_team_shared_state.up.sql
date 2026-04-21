-- up
CREATE TABLE v1_team_shared_state (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_instance_id UUID NOT NULL REFERENCES v1_team_instances(id) UNIQUE,
    accepted_artifact_refs JSONB NOT NULL DEFAULT '[]',
    published_facts JSONB NOT NULL DEFAULT '[]',
    delegation_status JSONB NOT NULL DEFAULT '[]',
    open_blockers JSONB NOT NULL DEFAULT '[]',
    decisions JSONB NOT NULL DEFAULT '[]',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- down
DROP TABLE IF EXISTS v1_team_shared_state;