-- down
ALTER TABLE v1_delegations
DROP COLUMN IF EXISTS result_artifact_id,
DROP COLUMN IF EXISTS error_message,
DROP COLUMN IF EXISTS rejection_reason,
DROP COLUMN IF EXISTS updated_at;