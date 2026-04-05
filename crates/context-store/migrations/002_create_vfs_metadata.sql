-- 002_create_vfs_metadata.sql
CREATE TABLE vfs_metadata (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id        UUID REFERENCES runs,
    node_id       UUID REFERENCES nodes,
    path          TEXT NOT NULL,
    artifact_id   UUID REFERENCES artifacts,
    is_directory  BOOLEAN DEFAULT false,
    created_at    TIMESTAMPTZ DEFAULT NOW(),
    modified_at   TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(run_id, path)
);

CREATE INDEX idx_vfs_run_id ON vfs_metadata(run_id);
CREATE INDEX idx_vfs_node_id ON vfs_metadata(node_id);
