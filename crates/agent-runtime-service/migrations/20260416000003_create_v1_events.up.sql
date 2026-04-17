CREATE TABLE v1_events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_type TEXT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resource_type TEXT NOT NULL,
    resource_id UUID NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}',
    sequence_number BIGINT
);

CREATE INDEX idx_v1_events_resource ON v1_events(resource_type, resource_id, timestamp DESC);
CREATE INDEX idx_v1_events_type ON v1_events(event_type, timestamp DESC);
CREATE INDEX idx_v1_events_sequence ON v1_events(resource_type, resource_id, sequence_number);
