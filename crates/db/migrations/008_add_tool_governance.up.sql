CREATE TABLE IF NOT EXISTS v1_tool_policies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tool_name VARCHAR(255) NOT NULL UNIQUE,
    risk_level VARCHAR(50) NOT NULL DEFAULT 'medium',
    side_effects TEXT[] NOT NULL DEFAULT '{}',
    requires_approval BOOLEAN NOT NULL DEFAULT false,
    blocked BOOLEAN NOT NULL DEFAULT false,
    blocked_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_tool_policies_risk_level ON v1_tool_policies(risk_level);
CREATE INDEX idx_tool_policies_blocked ON v1_tool_policies(blocked) WHERE blocked = true;