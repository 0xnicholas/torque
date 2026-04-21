CREATE TABLE IF NOT EXISTS rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    category VARCHAR(50) NOT NULL,
    pattern JSONB NOT NULL,
    action JSONB NOT NULL,
    priority INTEGER DEFAULT 0,
    success_count INTEGER DEFAULT 0,
    failure_count INTEGER DEFAULT 0,
    confidence_score DOUBLE PRECISION DEFAULT 0.5,
    embedding vector(1536),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_accessed_at TIMESTAMPTZ,

    CONSTRAINT rules_category_check
        CHECK (category IN ('tool_selection', 'task_pattern', 'execution', 'memory'))
);

CREATE INDEX IF NOT EXISTS idx_rules_category
    ON rules(category);
CREATE INDEX IF NOT EXISTS idx_rules_priority
    ON rules(priority DESC);
CREATE INDEX IF NOT EXISTS idx_rules_embedding
    ON rules USING hnsw (embedding vector_cosine_ops)
    WITH (m = 16, ef_construction = 64);
CREATE INDEX IF NOT EXISTS idx_rules_created_at
    ON rules(created_at DESC);