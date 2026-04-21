ALTER TABLE sessions ADD COLUMN IF NOT EXISTS agent_definition_id UUID;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS agent_instance_id UUID;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS active_task_id UUID;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS checkpoint_id UUID;
