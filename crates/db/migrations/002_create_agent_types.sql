-- 002_create_agent_types.sql
CREATE TABLE agent_types (
    id            UUID PRIMARY KEY,
    name          TEXT UNIQUE NOT NULL,
    description   TEXT,
    system_prompt TEXT NOT NULL,
    tools         JSONB DEFAULT '[]',
    max_tokens    INTEGER DEFAULT 4096,
    timeout_secs  INTEGER DEFAULT 300,
    created_at    TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_agent_types_name ON agent_types(name);
