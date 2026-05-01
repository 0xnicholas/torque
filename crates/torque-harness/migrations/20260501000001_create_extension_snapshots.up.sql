-- up
-- Extension snapshot persistence table.
-- Stores serialized Extension runtime state for recovery, debugging,
-- and observability.

CREATE TABLE v1_extension_snapshots (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    extension_id    UUID NOT NULL,
    extension_name  TEXT NOT NULL,
    version         TEXT NOT NULL,
    sequence        BIGINT NOT NULL,
    reason          TEXT NOT NULL,
    lifecycle       TEXT NOT NULL,
    config          JSONB NOT NULL DEFAULT '{}',
    registered_hooks JSONB NOT NULL DEFAULT '[]',
    bus_subscriptions JSONB NOT NULL DEFAULT '[]',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_v1_extension_snapshots_extension_id
    ON v1_extension_snapshots(extension_id);
CREATE INDEX idx_v1_extension_snapshots_created_at
    ON v1_extension_snapshots(created_at DESC);
CREATE INDEX idx_v1_extension_snapshots_reason
    ON v1_extension_snapshots(reason);
