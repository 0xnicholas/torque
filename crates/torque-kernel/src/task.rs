use serde::{Deserialize, Serialize};
use std::fmt;

use crate::{
    error::{StateTransitionError, ValidationError},
    ids::{ArtifactId, ExternalContextRefId, TaskId},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    Open,
    InProgress,
    Blocked,
    Done,
    Failed,
}

impl fmt::Display for TaskState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open => write!(f, "open"),
            Self::InProgress => write!(f, "in_progress"),
            Self::Blocked => write!(f, "blocked"),
            Self::Done => write!(f, "done"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskConstraint {
    description: String,
}

impl TaskConstraint {
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
        }
    }

    pub fn description(&self) -> &str {
        &self.description
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedOutput {
    description: String,
}

impl ExpectedOutput {
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
        }
    }

    pub fn description(&self) -> &str {
        &self.description
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskInputRef {
    Artifact(ArtifactId),
    ExternalContext {
        context_ref_id: ExternalContextRefId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    id: TaskId,
    goal: String,
    instructions: Vec<String>,
    state: TaskState,
    input_refs: Vec<TaskInputRef>,
    constraints: Vec<TaskConstraint>,
    expected_outputs: Vec<ExpectedOutput>,
    artifact_ids: Vec<ArtifactId>,
    block_reason: Option<String>,
    completion_summary: Option<String>,
    failure_reason: Option<String>,
}

impl Task {
    pub fn new(goal: String, instructions: Vec<String>, constraints: Vec<TaskConstraint>) -> Self {
        Self {
            id: TaskId::new(),
            goal,
            instructions,
            state: TaskState::Open,
            input_refs: Vec::new(),
            constraints,
            expected_outputs: Vec::new(),
            artifact_ids: Vec::new(),
            block_reason: None,
            completion_summary: None,
            failure_reason: None,
        }
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.goal.trim().is_empty() {
            return Err(ValidationError::new("Task", "goal must not be empty"));
        }

        Ok(())
    }

    pub fn id(&self) -> TaskId {
        self.id
    }

    pub fn goal(&self) -> &str {
        &self.goal
    }

    pub fn state(&self) -> TaskState {
        self.state
    }

    pub fn instructions(&self) -> &[String] {
        &self.instructions
    }

    pub fn input_refs(&self) -> &[TaskInputRef] {
        &self.input_refs
    }

    pub fn constraints(&self) -> &[TaskConstraint] {
        &self.constraints
    }

    pub fn expected_outputs(&self) -> &[ExpectedOutput] {
        &self.expected_outputs
    }

    pub fn with_expected_output(mut self, expected_output: ExpectedOutput) -> Self {
        self.expected_outputs.push(expected_output);
        self
    }

    pub fn with_input_ref(mut self, input_ref: TaskInputRef) -> Self {
        self.input_refs.push(input_ref);
        self
    }

    pub fn with_input_ref_iter(
        mut self,
        input_refs: impl IntoIterator<Item = TaskInputRef>,
    ) -> Self {
        self.input_refs.extend(input_refs);
        self
    }

    pub fn start(&mut self) -> Result<(), StateTransitionError> {
        match self.state {
            TaskState::Open | TaskState::Blocked => {
                self.state = TaskState::InProgress;
                self.block_reason = None;
                Ok(())
            }
            other => Err(StateTransitionError::new(
                "Task",
                format!("{other:?}"),
                format!("{:?}", TaskState::InProgress),
            )),
        }
    }

    pub fn block(&mut self, reason: impl Into<String>) -> Result<(), StateTransitionError> {
        match self.state {
            TaskState::InProgress => {
                self.state = TaskState::Blocked;
                self.block_reason = Some(reason.into());
                Ok(())
            }
            other => Err(StateTransitionError::new(
                "Task",
                format!("{other:?}"),
                format!("{:?}", TaskState::Blocked),
            )),
        }
    }

    pub fn complete(&mut self, summary: impl Into<String>) -> Result<(), StateTransitionError> {
        match self.state {
            TaskState::InProgress => {
                self.state = TaskState::Done;
                self.completion_summary = Some(summary.into());
                Ok(())
            }
            other => Err(StateTransitionError::new(
                "Task",
                format!("{other:?}"),
                format!("{:?}", TaskState::Done),
            )),
        }
    }

    pub fn fail(&mut self, reason: impl Into<String>) -> Result<(), StateTransitionError> {
        match self.state {
            TaskState::Open | TaskState::InProgress | TaskState::Blocked => {
                self.state = TaskState::Failed;
                self.failure_reason = Some(reason.into());
                Ok(())
            }
            other => Err(StateTransitionError::new(
                "Task",
                format!("{other:?}"),
                format!("{:?}", TaskState::Failed),
            )),
        }
    }

    pub fn record_artifact(&mut self, artifact_id: ArtifactId) {
        self.artifact_ids.push(artifact_id);
    }
}
