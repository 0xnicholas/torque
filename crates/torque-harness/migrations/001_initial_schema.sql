-- sessions table
CREATE TABLE IF NOT EXISTS sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    api_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'idle',
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
CREATE INDEX IF NOT EXISTS idx_messages_session_id ON session_messages(session_id);
CREATE INDEX IF NOT EXISTS idx_messages_created_at ON session_messages(created_at);
