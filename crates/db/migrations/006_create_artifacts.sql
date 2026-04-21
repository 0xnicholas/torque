-- 006_create_artifacts.sql
CREATE TABLE artifacts (
    id            UUID PRIMARY KEY,
    node_id       UUID REFERENCES nodes,
    tenant_id     UUID REFERENCES tenants,
    storage       TEXT NOT NULL,
    location      TEXT NOT NULL,
    size_bytes    BIGINT,
    content_type  TEXT,
    created_at    TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_artifacts_node_id ON artifacts(node_id);
CREATE INDEX idx_artifacts_tenant_id ON artifacts(tenant_id);
