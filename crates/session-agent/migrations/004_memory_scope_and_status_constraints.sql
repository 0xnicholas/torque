-- Normalize legacy rows before installing scoped integrity constraints.
UPDATE memory_entries AS entry
SET source_candidate_id = NULL
WHERE source_candidate_id IS NOT NULL
  AND NOT EXISTS (
      SELECT 1
      FROM memory_candidates AS candidate
      WHERE candidate.id = entry.source_candidate_id
        AND candidate.project_scope = entry.project_scope
  );

ALTER TABLE memory_entries
    DROP CONSTRAINT IF EXISTS memory_entries_source_candidate_id_fkey;

ALTER TABLE memory_entries
    DROP CONSTRAINT IF EXISTS memory_entries_source_candidate_fk;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'memory_candidates_layer_check'
          AND conrelid = 'memory_candidates'::regclass
    ) THEN
        ALTER TABLE memory_candidates
            ADD CONSTRAINT memory_candidates_layer_check
            CHECK (layer IN ('l0', 'l1'));
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'memory_candidates_status_check'
          AND conrelid = 'memory_candidates'::regclass
    ) THEN
        ALTER TABLE memory_candidates
            ADD CONSTRAINT memory_candidates_status_check
            CHECK (status IN ('pending', 'accepted', 'rejected'));
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'memory_candidates_project_scope_id_unique'
          AND conrelid = 'memory_candidates'::regclass
    ) THEN
        ALTER TABLE memory_candidates
            ADD CONSTRAINT memory_candidates_project_scope_id_unique
            UNIQUE (project_scope, id);
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'memory_entries_layer_check'
          AND conrelid = 'memory_entries'::regclass
    ) THEN
        ALTER TABLE memory_entries
            ADD CONSTRAINT memory_entries_layer_check
            CHECK (layer IN ('l0', 'l1'));
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'memory_entries_status_check'
          AND conrelid = 'memory_entries'::regclass
    ) THEN
        ALTER TABLE memory_entries
            ADD CONSTRAINT memory_entries_status_check
            CHECK (status IN ('active', 'invalidated', 'replaced'));
    END IF;
END $$;

ALTER TABLE memory_entries
    ADD CONSTRAINT memory_entries_source_candidate_fk
    FOREIGN KEY (project_scope, source_candidate_id)
    REFERENCES memory_candidates(project_scope, id)
    ON DELETE RESTRICT;
