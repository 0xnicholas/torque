-- sessions table
CREATE TABLE IF NOT EXISTS sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    api_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'idle',
    project_scope TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    error_message TEXT
);

-- session_messages table
CREATE TABLE IF NOT EXISTS session_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    tool_calls JSONB,
    artifacts JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- tools metadata table
CREATE TABLE IF NOT EXISTS tools (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT UNIQUE NOT NULL,
    description TEXT NOT NULL,
    parameters_schema JSONB NOT NULL,
    is_builtin BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- index for faster queries
CREATE INDEX IF NOT EXISTS idx_sessions_api_key ON sessions(api_key);
CREATE INDEX IF NOT EXISTS idx_sessions_project_scope ON sessions(project_scope);
CREATE INDEX IF NOT EXISTS idx_messages_session_id ON session_messages(session_id);
CREATE INDEX IF NOT EXISTS idx_messages_created_at ON session_messages(created_at);

-- memory_candidates table
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

-- memory_entries table
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

CREATE INDEX IF NOT EXISTS idx_memory_candidates_project_scope ON memory_candidates(project_scope);
CREATE INDEX IF NOT EXISTS idx_memory_candidates_status ON memory_candidates(status);
CREATE INDEX IF NOT EXISTS idx_memory_candidates_created_at ON memory_candidates(created_at);
CREATE INDEX IF NOT EXISTS idx_memory_entries_project_scope ON memory_entries(project_scope);
CREATE INDEX IF NOT EXISTS idx_memory_entries_status ON memory_entries(status);
CREATE INDEX IF NOT EXISTS idx_memory_entries_created_at ON memory_entries(created_at);
