use crate::providers::ToolSpec;
use serde_json::{Value, json};

pub fn file_tool_specs() -> Vec<ToolSpec> {
    vec![
        tool_spec("Read", "Read a text file inside the workspace."),
        tool_spec(
            "Write",
            "Write a file inside the workspace. Parent directories are created automatically.",
        ),
        tool_spec(
            "Edit",
            "Replace one exact text match in a workspace file. Ambiguous matches are rejected.",
        ),
        tool_spec(
            "Glob",
            "Find workspace files with a simple wildcard pattern.",
        ),
        tool_spec("Grep", "Search workspace text files for a literal string."),
        tool_spec(
            "Bash",
            "Run classified local read-only, script-run, or build-test commands in the workspace.",
        ),
    ]
}

pub fn tool_parameters_json_schema(name: &str) -> Value {
    match name {
        "Read" => object_schema(
            &[("path", "Repository-relative text file path to read.")],
            &["path"],
        ),
        "Write" => object_schema(
            &[
                ("path", "Repository-relative file path to write."),
                ("content", "Complete UTF-8 file content to write."),
            ],
            &["path", "content"],
        ),
        "Edit" => object_schema(
            &[
                ("path", "Repository-relative file path to edit."),
                ("old", "Exact existing text to replace once."),
                ("new", "Replacement text."),
            ],
            &["path", "old", "new"],
        ),
        "Bash" => object_schema(
            &[(
                "command",
                "Local read-only, script-run, or build-test command.",
            )],
            &["command"],
        ),
        "Glob" | "Grep" => object_schema(
            &[("pattern", "Pattern or literal text to search for.")],
            &["pattern"],
        ),
        _ => json!({
            "type": "object",
            "properties": {},
            "additionalProperties": true,
        }),
    }
}

fn tool_spec(name: &str, description: &str) -> ToolSpec {
    ToolSpec {
        name: name.to_string(),
        description: description.to_string(),
        parameters_json_schema: tool_parameters_json_schema(name),
    }
}

fn object_schema(properties: &[(&str, &str)], required: &[&str]) -> Value {
    let props = properties
        .iter()
        .map(|(name, description)| {
            (
                (*name).to_string(),
                json!({
                    "type": "string",
                    "description": description,
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    json!({
        "type": "object",
        "properties": props,
        "required": required,
        "additionalProperties": false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn exposes_file_tool_contracts() {
        let specs = file_tool_specs();

        assert_eq!(specs.len(), 6);
        assert!(specs.iter().any(|spec| {
            spec.name == "Write" && spec.description.contains("Parent directories")
        }));
    }

    #[test]
    fn exposes_required_argument_schema() {
        let specs = file_tool_specs();
        let write = specs.iter().find(|spec| spec.name == "Write").unwrap();
        let edit = specs.iter().find(|spec| spec.name == "Edit").unwrap();
        let bash = specs.iter().find(|spec| spec.name == "Bash").unwrap();

        assert_eq!(
            write.parameters_json_schema["required"],
            json!(["path", "content"])
        );
        assert_eq!(
            edit.parameters_json_schema["required"],
            json!(["path", "old", "new"])
        );
        assert_eq!(bash.parameters_json_schema["required"], json!(["command"]));
    }
}
