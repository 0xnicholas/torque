-- Enable pgvector extension
CREATE EXTENSION IF NOT EXISTS vector;

-- Create v1_memory_entries table with embedding support
CREATE TABLE IF NOT EXISTS v1_memory_entries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_instance_id UUID,
    team_instance_id UUID,
    category VARCHAR(50) NOT NULL,
    key TEXT NOT NULL,
    value JSONB NOT NULL,
    source_candidate_id UUID,
    
    -- Embedding fields
    embedding vector(1536),
    embedding_model TEXT DEFAULT 'text-embedding-3-small',
    
    -- Usage tracking
    access_count INTEGER DEFAULT 0,
    last_accessed_at TIMESTAMPTZ,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_v1_memory_entries_category 
    ON v1_memory_entries(category);
CREATE INDEX IF NOT EXISTS idx_v1_memory_entries_agent 
    ON v1_memory_entries(agent_instance_id);
CREATE INDEX IF NOT EXISTS idx_v1_memory_entries_created_at 
    ON v1_memory_entries(created_at DESC);

-- pgvector HNSW index for semantic search
CREATE INDEX IF NOT EXISTS idx_v1_memory_entries_embedding 
    ON v1_memory_entries 
    USING hnsw (embedding vector_cosine_ops)
    WITH (m = 16, ef_construction = 64);

-- Composite index for category-filtered search
CREATE INDEX IF NOT EXISTS idx_v1_memory_entries_category_embedding 
    ON v1_memory_entries 
    USING hnsw (embedding vector_cosine_ops) 
    WHERE category = 'agent_profile_memory';

-- Create v1_memory_write_candidates table
CREATE TABLE IF NOT EXISTS v1_memory_write_candidates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_instance_id UUID NOT NULL,
    team_instance_id UUID,
    content JSONB NOT NULL,
    reasoning TEXT,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    memory_entry_id UUID,
    reviewed_by TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    reviewed_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT v1_memory_write_candidates_status_check 
        CHECK (status IN ('pending', 'review_required', 'auto_approved', 'approved', 'rejected', 'merged'))
);

CREATE INDEX IF NOT EXISTS idx_v1_candidates_agent 
    ON v1_memory_write_candidates(agent_instance_id);
CREATE INDEX IF NOT EXISTS idx_v1_candidates_status 
    ON v1_memory_write_candidates(status);
CREATE INDEX IF NOT EXISTS idx_v1_candidates_created_at 
    ON v1_memory_write_candidates(created_at DESC);

-- Create session_memory table (KV + TTL)
CREATE TABLE IF NOT EXISTS session_memory (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL,
    key TEXT NOT NULL,
    value JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    
    UNIQUE(session_id, key)
);

CREATE INDEX IF NOT EXISTS idx_session_memory_session 
    ON session_memory(session_id);
CREATE INDEX IF NOT EXISTS idx_session_memory_expires 
    ON session_memory(expires_at) 
    WHERE expires_at IS NOT NULL;

-- Create memory_decision_log table (audit trail)
CREATE TABLE IF NOT EXISTS memory_decision_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    candidate_id UUID REFERENCES v1_memory_write_candidates(id) ON DELETE SET NULL,
    entry_id UUID REFERENCES v1_memory_entries(id) ON DELETE SET NULL,
    decision_type VARCHAR(20) NOT NULL,
    decision_reason TEXT,
    factors JSONB NOT NULL DEFAULT '{}',
    processed_by VARCHAR(50) NOT NULL,
    processed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT memory_decision_log_type_check 
        CHECK (decision_type IN ('approve', 'reject', 'merge', 'review'))
);

CREATE INDEX IF NOT EXISTS idx_decision_log_candidate 
    ON memory_decision_log(candidate_id);
CREATE INDEX IF NOT EXISTS idx_decision_log_type 
    ON memory_decision_log(decision_type);
CREATE INDEX IF NOT EXISTS idx_decision_log_processed_at 
    ON memory_decision_log(processed_at);
