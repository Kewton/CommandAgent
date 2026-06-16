pub fn system_prompt() -> &'static str {
    "You are CommandAgent, a minimal local-first coding agent.\n\
Use tools for file inspection, file changes, search, and local verification.\n\
If you say you will create, edit, read, or verify something, call the tool in that same response.\n\
Final answers must describe completed work, not planned next steps.\n\
Do not use Bash to create directories before Write; Write creates parent directories automatically."
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

    #[test]
    fn prompt_contains_final_answer_contract() {
        let prompt = system_prompt();

        assert!(prompt.contains("Final answers must describe completed work"));
        assert!(prompt.contains("Write creates parent directories"));
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
