use crate::models::v1::agent_definition::AgentDefinition as V1AgentDefinition;
use crate::models::v1::run::RunRequest;
use torque_kernel::{AgentDefinition as KernelAgentDefinition, ExecutionMode, ExecutionRequest};

pub fn v1_agent_definition_to_kernel(def: &V1AgentDefinition) -> KernelAgentDefinition {
    let system_prompt = def.system_prompt.clone().unwrap_or_default();

    let mut kernel_def = KernelAgentDefinition::new(def.name.clone(), system_prompt);

    // Map policy fields as refs (serialized JSON)
    if !def.tool_policy.is_null() && def.tool_policy != serde_json::json!({}) {
        kernel_def.tool_policy_ref = Some(def.tool_policy.to_string());
    }
    if !def.memory_policy.is_null() && def.memory_policy != serde_json::json!({}) {
        kernel_def.memory_policy_ref = Some(def.memory_policy.to_string());
    }
    if !def.delegation_policy.is_null() && def.delegation_policy != serde_json::json!({}) {
        kernel_def.delegation_policy_ref = Some(def.delegation_policy.to_string());
    }
    if !def.default_model_policy.is_null() && def.default_model_policy != serde_json::json!({}) {
        kernel_def.default_model_policy_ref = Some(def.default_model_policy.to_string());
    }

    kernel_def
}

pub fn run_request_to_execution_request(
    agent_definition: &KernelAgentDefinition,
    run_request: &RunRequest,
    instance_id: Option<uuid::Uuid>,
) -> ExecutionRequest {
    let mode = match run_request.execution_mode.as_str() {
        "async" => ExecutionMode::Async,
        _ => ExecutionMode::Sync,
    };

    let instructions = run_request.instructions.clone().unwrap_or_default();

    let mut request = ExecutionRequest::new(
        agent_definition.id,
        run_request.goal.clone(),
        vec![instructions],
    )
    .with_execution_mode(mode);

    // Include instance_id if available
    if let Some(id) = instance_id {
        use torque_kernel::ids::AgentInstanceId;
        let kernel_id = AgentInstanceId::new();
        // Note: This creates a new ID. In a full implementation,
        // we'd need a way to map v1 UUID to kernel AgentInstanceId
        request = request.with_instance_id(kernel_id);
    }

    request
}
