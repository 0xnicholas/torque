-- 007_create_queue.sql
CREATE TABLE queue (
    id            UUID PRIMARY KEY,
    tenant_id     UUID REFERENCES tenants,
    run_id        UUID REFERENCES runs,
    node_id       UUID REFERENCES nodes UNIQUE,
    priority      INTEGER DEFAULT 0,
    status        TEXT NOT NULL DEFAULT 'pending',
    available_at  TIMESTAMPTZ DEFAULT NOW(),
    locked_at     TIMESTAMPTZ,
    locked_by     TEXT,
    created_at    TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_queue_tenant_id ON queue(tenant_id);
CREATE INDEX idx_queue_status ON queue(status);
CREATE INDEX idx_queue_available_at ON queue(available_at);
