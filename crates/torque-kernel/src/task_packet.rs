use crate::{ArtifactId, ExecutionRequest, ExternalContextRef, Task, TaskInputRef};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskPacket {
    pub goal: String,
    pub instructions: Vec<String>,
    pub constraints: Vec<String>,
    pub expected_outputs: Vec<String>,
    pub input_refs: Vec<TaskInputRef>,
    pub input_artifact_ids: Vec<ArtifactId>,
    pub external_context_refs: Vec<ExternalContextRef>,
}

impl TaskPacket {
    pub fn from_request_and_task(request: &ExecutionRequest, task: &Task) -> Self {
        let mut input_refs = task.input_refs().to_vec();
        input_refs.extend(
            request
                .input_artifact_ids()
                .iter()
                .copied()
                .map(TaskInputRef::Artifact),
        );
        input_refs.extend(request.external_context_refs().iter().map(|context_ref| {
            TaskInputRef::ExternalContext {
                context_ref_id: context_ref.id,
            }
        }));

        let mut constraints = task
            .constraints()
            .iter()
            .map(|constraint| constraint.description().to_string())
            .collect::<Vec<_>>();
        constraints.extend(request.constraints().iter().cloned());

        let mut expected_outputs = task
            .expected_outputs()
            .iter()
            .map(|expected_output| expected_output.description().to_string())
            .collect::<Vec<_>>();
        expected_outputs.extend(request.expected_outputs().iter().cloned());

        Self {
            goal: task.goal().to_string(),
            instructions: task.instructions().to_vec(),
            constraints,
            expected_outputs,
            input_refs,
            input_artifact_ids: request.input_artifact_ids().to_vec(),
            external_context_refs: request.external_context_refs().to_vec(),
        }
    }
}
