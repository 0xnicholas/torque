-- up
ALTER TABLE v1_delegations
ADD COLUMN result_artifact_id UUID,
ADD COLUMN error_message TEXT,
ADD COLUMN rejection_reason TEXT,
ADD COLUMN updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

-- Update updated_at to be consistent with created_at for existing rows
UPDATE v1_delegations SET updated_at = created_at WHERE updated_at IS NULL;