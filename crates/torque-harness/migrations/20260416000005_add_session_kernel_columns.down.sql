ALTER TABLE sessions DROP COLUMN IF EXISTS agent_definition_id;
ALTER TABLE sessions DROP COLUMN IF EXISTS agent_instance_id;
ALTER TABLE sessions DROP COLUMN IF EXISTS active_task_id;
ALTER TABLE sessions DROP COLUMN IF EXISTS checkpoint_id;
