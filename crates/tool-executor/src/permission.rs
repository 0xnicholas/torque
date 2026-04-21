use crate::error::ToolError;

pub struct PermissionChecker;

impl PermissionChecker {
    pub fn check(
        agent_tools: &[String],
        tool_name: &str,
        allowed_tools: Option<&[String]>,
    ) -> Result<(), ToolError> {
        let tools = allowed_tools.unwrap_or(agent_tools);

        if !tools.contains(&tool_name.to_string()) {
            return Err(ToolError::PermissionDenied(format!(
                "Tool '{}' not allowed",
                tool_name
            )));
        }

        Ok(())
    }
}
