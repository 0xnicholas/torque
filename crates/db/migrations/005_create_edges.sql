-- 005_create_edges.sql
CREATE TABLE edges (
    id            UUID PRIMARY KEY,
    run_id        UUID REFERENCES runs,
    source_node   UUID REFERENCES nodes,
    target_node   UUID REFERENCES nodes
);

CREATE INDEX idx_edges_run_id ON edges(run_id);
