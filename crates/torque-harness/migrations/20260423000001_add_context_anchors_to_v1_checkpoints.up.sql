-- up
ALTER TABLE v1_checkpoints ADD COLUMN context_anchors JSONB DEFAULT '[]';

-- down
ALTER TABLE v1_checkpoints DROP COLUMN IF EXISTS context_anchors;