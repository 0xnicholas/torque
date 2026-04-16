pub mod error;
pub mod trait_def;

pub use error::{CheckpointerError, Result};
pub use trait_def::{Checkpointer, CheckpointId, CheckpointMeta, CheckpointState};
