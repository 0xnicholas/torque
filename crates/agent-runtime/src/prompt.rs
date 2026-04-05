use llm::Message;

pub struct PromptBuilder;

impl PromptBuilder {
    pub fn build_system_prompt(system_prompt: &str, tools: &[String]) -> String {
        let tools_json = tools
            .iter()
            .map(|t| format!("  - {}", t))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "{}\n\nAvailable tools:\n{}\n\nOutput format: {{\"output\": \"...\", \"__requires_approval\": false, \"__metadata\": {{}}}}",
            system_prompt,
            tools_json
        )
    }

    pub fn build_initial_message(instruction: &str, context: &[Message]) -> Vec<Message> {
        let mut messages = vec![Message {
            role: "system".to_string(),
            content: instruction.to_string(),
        }];
        messages.extend(context.iter().cloned());
        messages
    }
}
