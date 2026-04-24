-- up
CREATE TABLE v1_escalations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instance_id UUID NOT NULL,
    team_instance_id UUID,
    escalation_type TEXT NOT NULL,
    severity TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    description TEXT NOT NULL,
    context JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMPTZ,
    resolved_by UUID,
    resolution TEXT
);

CREATE INDEX idx_v1_escalations_instance_id ON v1_escalations(instance_id);
CREATE INDEX idx_v1_escalations_team_instance_id ON v1_escalations(team_instance_id);
CREATE INDEX idx_v1_escalations_status ON v1_escalations(status);