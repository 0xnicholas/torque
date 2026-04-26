use crate::repository::SessionRepository;
use async_trait::async_trait;
use std::sync::Arc;
use torque_kernel::AgentInstanceId;
use torque_runtime::checkpoint::HydrationState;
use torque_runtime::environment::RuntimeHydrationSource;

pub struct HarnessHydrationSource {
    session_repo: Arc<dyn SessionRepository>,
}

impl HarnessHydrationSource {
    pub fn new(session_repo: Arc<dyn SessionRepository>) -> Self {
        Self { session_repo }
    }
}

#[async_trait]
impl RuntimeHydrationSource for HarnessHydrationSource {
    async fn load_instance_state(
        &self,
        instance_id: AgentInstanceId,
    ) -> anyhow::Result<Option<HydrationState>> {
        Ok(self
            .session_repo
            .get_kernel_state(instance_id.as_uuid())
            .await?
            .map(|state| HydrationState {
                agent_definition_id: state.agent_definition_id,
                status: state.status,
                active_task_id: state.active_task_id,
                checkpoint_id: state.checkpoint_id,
            }))
    }
}
