CREATE TABLE IF NOT EXISTS runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL DEFAULT gen_random_uuid(),
    status VARCHAR(32) NOT NULL DEFAULT 'queued',
    agent_instance_id UUID NOT NULL,
    instruction TEXT NOT NULL DEFAULT '',
    request_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    failure_policy VARCHAR(32),
    webhook_url TEXT,
    async_execution BOOLEAN NOT NULL DEFAULT false,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    error TEXT,
    webhook_sent_at TIMESTAMPTZ,
    webhook_attempts INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
