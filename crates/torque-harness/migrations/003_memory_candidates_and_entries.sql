CREATE TABLE IF NOT EXISTS memory_candidates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_scope TEXT NOT NULL,
    layer TEXT NOT NULL,
    proposed_fact TEXT NOT NULL,
    source_type TEXT,
    source_ref TEXT,
    proposer TEXT,
    confidence DOUBLE PRECISION,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    accepted_at TIMESTAMPTZ,
    rejected_at TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS memory_entries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_scope TEXT NOT NULL,
    layer TEXT NOT NULL,
    content TEXT NOT NULL,
    source_candidate_id UUID REFERENCES memory_candidates(id) ON DELETE SET NULL,
    source_type TEXT,
    source_ref TEXT,
    proposer TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    invalidated_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_memory_candidates_project_scope
    ON memory_candidates(project_scope);
CREATE INDEX IF NOT EXISTS idx_memory_candidates_status
    ON memory_candidates(status);
CREATE INDEX IF NOT EXISTS idx_memory_candidates_created_at
    ON memory_candidates(created_at);

CREATE INDEX IF NOT EXISTS idx_memory_entries_project_scope
    ON memory_entries(project_scope);
CREATE INDEX IF NOT EXISTS idx_memory_entries_status
    ON memory_entries(status);
CREATE INDEX IF NOT EXISTS idx_memory_entries_created_at
    ON memory_entries(created_at);
