-- up
CREATE TABLE v1_team_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_instance_id UUID NOT NULL REFERENCES v1_team_instances(id),
    goal TEXT NOT NULL,
    instructions TEXT,
    status TEXT NOT NULL DEFAULT 'OPEN',
    triage_result JSONB,
    mode_selected TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX idx_v1_team_tasks_team_instance_id ON v1_team_tasks(team_instance_id);
CREATE INDEX idx_v1_team_tasks_status ON v1_team_tasks(status);

-- down
DROP TABLE IF EXISTS v1_team_tasks;