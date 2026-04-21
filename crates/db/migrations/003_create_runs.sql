-- 003_create_runs.sql
CREATE TABLE runs (
    id            UUID PRIMARY KEY,
    tenant_id     UUID REFERENCES tenants,
    status        TEXT NOT NULL DEFAULT 'planning',
    instruction   TEXT NOT NULL,
    failure_policy TEXT DEFAULT 'abort',
    created_at    TIMESTAMPTZ DEFAULT NOW(),
    started_at    TIMESTAMPTZ,
    completed_at  TIMESTAMPTZ,
    error         TEXT
);

CREATE INDEX idx_runs_tenant_id ON runs(tenant_id);
CREATE INDEX idx_runs_status ON runs(status);
