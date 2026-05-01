DROP TRIGGER IF EXISTS trg_sessions_updated_at ON sessions;
DROP FUNCTION IF EXISTS update_sessions_updated_at();
DROP TABLE IF EXISTS sessions;
DROP TYPE IF EXISTS session_status;
