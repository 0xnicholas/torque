-- up
CREATE TABLE v1_artifacts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kind TEXT NOT NULL,
    scope TEXT NOT NULL DEFAULT 'private',
    source_instance_id UUID,
    published_to_team_instance_id UUID,
    mime_type TEXT NOT NULL,
    size_bytes BIGINT NOT NULL DEFAULT 0,
    summary TEXT,
    content JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
