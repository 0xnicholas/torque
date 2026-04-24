-- down
ALTER TABLE runs DROP COLUMN IF EXISTS webhook_url;
ALTER TABLE runs DROP COLUMN IF EXISTS async_execution;