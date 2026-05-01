-- Create session_status enum type
CREATE TYPE session_status AS ENUM (
    'active',
    'idle',
    'compacting',
    'error',
    'terminated'
);

-- Create sessions table
CREATE TABLE sessions (
    id              UUID PRIMARY KEY,
    tenant_id       UUID NOT NULL,
    agent_definition_id UUID NOT NULL,
    agent_instance_id   UUID,

    status          session_status NOT NULL DEFAULT 'active',
    title           TEXT,
    metadata        JSONB NOT NULL DEFAULT '{}',

    active_compaction_job_id UUID,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX idx_sessions_tenant_id ON sessions (tenant_id);
CREATE INDEX idx_sessions_tenant_status ON sessions (tenant_id, status);
CREATE INDEX idx_sessions_updated_at ON sessions (tenant_id, updated_at DESC);

-- Trigger to auto-update updated_at
CREATE OR REPLACE FUNCTION update_sessions_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_sessions_updated_at
    BEFORE UPDATE ON sessions
    FOR EACH ROW
    EXECUTE FUNCTION update_sessions_updated_at();
