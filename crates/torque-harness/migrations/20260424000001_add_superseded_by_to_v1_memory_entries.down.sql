-- Remove superseded_by column
ALTER TABLE v1_memory_entries DROP COLUMN IF EXISTS superseded_by;