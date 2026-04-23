-- Create table for notification hooks configuration
CREATE TABLE IF NOT EXISTS memory_notification_hooks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    hook_type VARCHAR(20) NOT NULL,
    url TEXT,
    enabled BOOLEAN NOT NULL DEFAULT true,
    events TEXT[] NOT NULL DEFAULT ARRAY['candidate_needs_review'],
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT hook_type_check CHECK (hook_type IN ('webhook', 'sse'))
);

CREATE INDEX IF NOT EXISTS idx_notification_hooks_enabled ON memory_notification_hooks(enabled);
CREATE INDEX IF NOT EXISTS idx_notification_hooks_type ON memory_notification_hooks(hook_type);

-- Create table for notification delivery log (audit)
CREATE TABLE IF NOT EXISTS memory_notification_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    hook_id UUID REFERENCES memory_notification_hooks(id) ON DELETE SET NULL,
    event_type VARCHAR(50) NOT NULL,
    recipient_url TEXT,
    payload JSONB,
    delivery_status VARCHAR(20) NOT NULL,
    error_message TEXT,
    delivered_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT delivery_status_check CHECK (delivery_status IN ('pending', 'delivered', 'failed'))
);

CREATE INDEX IF NOT EXISTS idx_notification_log_hook ON memory_notification_log(hook_id);
CREATE INDEX IF NOT EXISTS idx_notification_log_status ON memory_notification_log(delivery_status);