use crate::models::Session;
use torque_kernel::{AgentDefinition, ExecutionMode, ExecutionRequest, KernelError};

pub fn session_to_execution_request(
    session: &Session,
    user_message: &str,
) -> Result<ExecutionRequest, KernelError> {
    let agent_def_id = session
        .agent_definition_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| session.id.to_string());
    let agent_def = AgentDefinition::new(&agent_def_id, "MVP session adapter");

    Ok(ExecutionRequest::new(
        agent_def.id,
        user_message.to_string(),
        vec![format!("Session {}", session.id)],
    )
    .with_execution_mode(ExecutionMode::Sync))
}
