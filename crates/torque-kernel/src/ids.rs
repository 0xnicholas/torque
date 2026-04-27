use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            pub fn as_uuid(&self) -> Uuid {
                self.0
            }
        }
    };
}

define_id!(AgentDefinitionId);
define_id!(AgentInstanceId);
define_id!(TaskId);
define_id!(ExecutionRequestId);
define_id!(DelegationRequestId);
define_id!(ApprovalRequestId);
define_id!(ArtifactId);
define_id!(ExternalContextRefId);
define_id!(CheckpointId);
