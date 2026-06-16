use crate::providers::ToolSpec;

pub fn file_tool_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "Read".to_string(),
            description: "Read a text file inside the workspace.".to_string(),
        },
        ToolSpec {
            name: "Write".to_string(),
            description:
                "Write a file inside the workspace. Parent directories are created automatically."
                    .to_string(),
        },
        ToolSpec {
            name: "Edit".to_string(),
            description:
                "Replace one exact text match in a workspace file. Ambiguous matches are rejected."
                    .to_string(),
        },
        ToolSpec {
            name: "Glob".to_string(),
            description: "Find workspace files with a simple wildcard pattern.".to_string(),
        },
        ToolSpec {
            name: "Grep".to_string(),
            description: "Search workspace text files for a literal string.".to_string(),
        },
        ToolSpec {
            name: "Bash".to_string(),
            description:
                "Run classified local read-only, script-run, or build-test commands in the workspace."
                    .to_string(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_file_tool_contracts() {
        let specs = file_tool_specs();

        assert_eq!(specs.len(), 6);
        assert!(specs.iter().any(|spec| {
            spec.name == "Write" && spec.description.contains("Parent directories")
        }));
    }
}
