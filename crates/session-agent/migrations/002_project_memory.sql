ALTER TABLE sessions ADD COLUMN project_scope TEXT;
UPDATE sessions SET project_scope = 'torque' WHERE project_scope IS NULL;
ALTER TABLE sessions ALTER COLUMN project_scope SET NOT NULL;
CREATE INDEX IF NOT EXISTS idx_sessions_project_scope ON sessions(project_scope);
