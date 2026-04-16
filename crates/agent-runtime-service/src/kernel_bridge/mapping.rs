use crate::models::Session;
use torque_kernel::{AgentDefinition, ExecutionMode, ExecutionRequest, KernelError};

pub fn session_to_execution_request(
    session: &Session,
    user_message: &str,
) -> Result<ExecutionRequest, KernelError> {
    let agent_def = AgentDefinition::new(&session.id.to_string(), "MVP session adapter");

    Ok(ExecutionRequest::new(
        agent_def.id,
        user_message.to_string(),
        vec![format!("Session {}", session.id)],
    )
    .with_execution_mode(ExecutionMode::Sync))
}
