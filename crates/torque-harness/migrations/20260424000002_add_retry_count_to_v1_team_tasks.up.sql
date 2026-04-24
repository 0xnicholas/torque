-- Add retry_count column to v1_team_tasks for task retry tracking
ALTER TABLE v1_team_tasks
ADD COLUMN retry_count INTEGER DEFAULT 0;