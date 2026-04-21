-- up
CREATE TABLE v1_team_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_instance_id UUID NOT NULL REFERENCES v1_team_instances(id),
    event_type TEXT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    actor_ref TEXT NOT NULL,
    team_task_ref UUID REFERENCES v1_team_tasks(id),
    related_instance_refs JSONB NOT NULL DEFAULT '[]',
    related_artifact_refs JSONB NOT NULL DEFAULT '[]',
    payload JSONB NOT NULL DEFAULT '{}',
    causal_event_refs JSONB NOT NULL DEFAULT '[]'
);

CREATE INDEX idx_v1_team_events_team_instance_id ON v1_team_events(team_instance_id);
CREATE INDEX idx_v1_team_events_event_type ON v1_team_events(event_type);

-- down
DROP TABLE IF EXISTS v1_team_events;