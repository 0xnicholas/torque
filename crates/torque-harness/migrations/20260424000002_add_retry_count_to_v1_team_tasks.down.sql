-- Remove retry_count column
ALTER TABLE v1_team_tasks DROP COLUMN IF EXISTS retry_count;