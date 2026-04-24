-- Add superseded_by column to v1_memory_entries for memory deduplication/chaining
ALTER TABLE v1_memory_entries
ADD COLUMN superseded_by UUID REFERENCES v1_memory_entries(id);

CREATE INDEX idx_v1_memory_entries_superseded_by ON v1_memory_entries(superseded_by) WHERE superseded_by IS NOT NULL;