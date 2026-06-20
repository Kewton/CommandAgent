use crate::providers::{ToolCallMode, ToolSpec};

pub fn system_prompt(mode: ToolCallMode, tools: &[ToolSpec]) -> String {
    let mut prompt = "You are CommandAgent, a minimal local-first coding agent.\n\
Use tools for file inspection, file changes, search, and local verification.\n\
If you say you will create, edit, read, or verify something, call the tool in that same response.\n\
Final answers must describe completed work, not planned next steps.\n\
Do not use Bash to create directories before Write; Write creates parent directories automatically."
        .to_string();

    if mode == ToolCallMode::XmlFallback {
        prompt.push_str("\n\nNative tool calls are unavailable in this session. Emit exactly one complete XML fallback tool call when you need a tool.\n");
        prompt.push_str("Format:\n");
        prompt.push_str(xml_tool_call_format());
        prompt.push_str("\n\nUse `args` as the JSON object for tool arguments. Do not use Markdown fences around tool calls.\n");
        prompt.push_str("\nAvailable tools:\n");
        for tool in tools {
            prompt.push_str(&format!("- {}: {}\n", tool.name, tool.description));
        }
        prompt.push_str("\nArgument shapes:\n");
        prompt.push_str("- Read: {\"path\":\"README.md\"}\n");
        prompt.push_str("- Write: {\"path\":\"README.md\",\"content\":\"text\"}\n");
        prompt.push_str("- Edit: {\"path\":\"README.md\",\"old\":\"before\",\"new\":\"after\"}\n");
        prompt.push_str("- Glob: {\"pattern\":\"src/*.rs\"}\n");
        prompt.push_str("- Grep: {\"pattern\":\"TODO\"}\n");
        prompt.push_str("- Bash: {\"command\":\"cargo test\"}");
    }

    prompt
}

pub fn xml_tool_call_format() -> &'static str {
    "<commandagent_tool_call>{\"name\":\"Write\",\"args\":{\"path\":\"README.md\",\"content\":\"text\"}}</commandagent_tool_call>"
}

pub fn parser_failure_feedback(error: &str) -> String {
    format!(
        "The previous tool call was malformed: {error}\n\
Use XML fallback format exactly like this:\n{}",
        xml_tool_call_format()
    )
}

pub fn violates_final_answer_contract(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let future_markers = [
        "i will ",
        "i'll ",
        "let me ",
        "now i'll ",
        "now i will ",
        "next i will ",
        "i am going to ",
        "i’m going to ",
    ];
    let tool_action_words = [
        "create", "edit", "read", "verify", "run", "write", "inspect", "check",
    ];

    future_markers.iter().any(|marker| {
        lower.contains(marker)
            && tool_action_words
                .iter()
                .any(|word| lower.contains(&format!("{marker}{word}")) || lower.contains(word))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::registry::file_tool_specs;

    #[test]
    fn native_prompt_contains_final_answer_contract_without_xml_fallback() {
        let prompt = system_prompt(ToolCallMode::Native, &file_tool_specs());

        assert!(prompt.contains("Final answers must describe completed work"));
        assert!(prompt.contains("Write creates parent directories"));
        assert!(!prompt.contains("commandagent_tool_call"));
    }

    #[test]
    fn xml_fallback_prompt_contains_tool_contract() {
        let prompt = system_prompt(ToolCallMode::XmlFallback, &file_tool_specs());

        assert!(prompt.contains("commandagent_tool_call"));
        assert!(prompt.contains("\"args\""));
        assert!(prompt.contains("- Write: {\"path\":\"README.md\",\"content\":\"text\"}"));
        assert!(
            prompt
                .contains("- Edit: {\"path\":\"README.md\",\"old\":\"before\",\"new\":\"after\"}")
        );
        assert!(prompt.contains("- Bash: {\"command\":\"cargo test\"}"));
        assert!(prompt.contains("- Read: Read a text file inside the workspace."));
    }

    #[test]
    fn detects_planned_next_step_as_invalid_final() {
        assert!(violates_final_answer_contract("Now I'll create the files."));
        assert!(violates_final_answer_contract("Let me verify the build."));
        assert!(!violates_final_answer_contract(
            "Created the files and verified the build."
        ));
    }
}
