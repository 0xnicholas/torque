-- 001_create_tenants.sql
CREATE TABLE tenants (
    id            UUID PRIMARY KEY,
    name          TEXT NOT NULL,
    weight        INTEGER DEFAULT 1,
    max_concurrency INTEGER DEFAULT 10,
    monthly_token_quota BIGINT,
    created_at    TIMESTAMPTZ DEFAULT NOW()
);
