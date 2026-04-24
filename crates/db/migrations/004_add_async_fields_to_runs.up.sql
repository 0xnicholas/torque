-- up
ALTER TABLE runs ADD COLUMN webhook_url TEXT;
ALTER TABLE runs ADD COLUMN async_execution BOOLEAN NOT NULL DEFAULT false;