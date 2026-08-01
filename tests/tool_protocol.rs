use std::sync::{Arc, Mutex};

use clap::Parser;
use commandagent::cli::Cli;
use commandagent::config::Config;
use commandagent::minimal_loop::loop_run::run_session_with_required_paths;
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, SessionSnapshot};
use commandagent::tools::registry::ToolSpec;

#[derive(Clone)]
struct TextProtocolFake {
    request_shapes: Arc<Mutex<Vec<(bool, usize, bool)>>>,
}

impl ChatClient for TextProtocolFake {
    fn label(&self) -> &str {
        "text-protocol-fake"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn supports_native_tools(&self, _model: &str) -> bool {
        true
    }

    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        self.request_shapes.lock().unwrap().push((
            native_tools_enabled,
            tools.len(),
            messages.iter().any(|message| {
                message
                    .content
                    .contains("Native tool calls are unavailable")
            }),
        ));
        Ok(AssistantReply::text(
            r#"<anvil_tool_call name="Write">{"path":"a.txt","content":"ok"}</anvil_tool_call>"#,
        ))
    }
}

#[test]
fn declared_text_protocol_runs_existing_xml_tools_on_production_path() {
    let dir = tempfile::tempdir().unwrap();
    let request_shapes = Arc::new(Mutex::new(Vec::new()));
    let mut fake = TextProtocolFake {
        request_shapes: request_shapes.clone(),
    };
    let mut session = SessionSnapshot::new();
    let config = Config::from_cli(Cli::parse_from([
        "commandagent",
        "--yes",
        "--cwd",
        dir.path().to_str().unwrap(),
        "--tool-protocol",
        "text",
        "--prompt",
        "create a.txt",
    ]))
    .unwrap();

    let result = run_session_with_required_paths(
        &mut fake,
        &mut session,
        "create a.txt",
        &["a.txt".to_string()],
        &config,
    )
    .unwrap();

    assert_eq!(result, "required artifacts satisfied: a.txt");
    assert_eq!(*request_shapes.lock().unwrap(), vec![(false, 0, true)]);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "ok"
    );
}
