use thiserror::Error;

#[derive(Error, Debug)]
pub enum DagError {
    #[error("Cycle detected in DAG")]
    CycleDetected,

    #[error("Invalid edge reference: {0}")]
    InvalidEdgeReference(String),

    #[error("Orphan node detected: {0}")]
    OrphanNode(String),

    #[error("Empty node list")]
    EmptyNodeList,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DagErrorKind {
    CycleDetected,
    InvalidEdgeReference,
    OrphanNode,
    EmptyNodeList,
}
