-- Drop indexes first
DROP INDEX IF EXISTS idx_decision_log_processed_at;
DROP INDEX IF EXISTS idx_decision_log_type;
DROP INDEX IF EXISTS idx_decision_log_candidate;

DROP INDEX IF EXISTS idx_session_memory_expires;
DROP INDEX IF EXISTS idx_session_memory_session;

DROP INDEX IF EXISTS idx_v1_candidates_created_at;
DROP INDEX IF EXISTS idx_v1_candidates_status;
DROP INDEX IF EXISTS idx_v1_candidates_agent;

DROP INDEX IF EXISTS idx_v1_memory_entries_category_embedding;
DROP INDEX IF EXISTS idx_v1_memory_entries_embedding;
DROP INDEX IF EXISTS idx_v1_memory_entries_created_at;
DROP INDEX IF EXISTS idx_v1_memory_entries_agent;
DROP INDEX IF EXISTS idx_v1_memory_entries_category;

-- Drop tables
DROP TABLE IF EXISTS memory_decision_log;
DROP TABLE IF EXISTS session_memory;
DROP TABLE IF EXISTS v1_memory_write_candidates;
DROP TABLE IF EXISTS v1_memory_entries;
