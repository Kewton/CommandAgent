use commandagent::agent::minimal_loop::loop_run::ChatClient;
use commandagent::providers::{ChatRequest, ChatResponse};
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn temp_workspace(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "commandagent-it-{name}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

#[derive(Debug)]
pub struct MockChatClient {
    responses: VecDeque<ChatResponse>,
    requests: Vec<ChatRequest>,
}

impl MockChatClient {
    pub fn new(responses: Vec<ChatResponse>) -> Self {
        Self {
            responses: responses.into(),
            requests: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn requests(&self) -> &[ChatRequest] {
        &self.requests
    }
}

impl ChatClient for MockChatClient {
    fn chat(&mut self, request: &ChatRequest) -> Result<ChatResponse, String> {
        self.requests.push(request.clone());
        self.responses
            .pop_front()
            .ok_or_else(|| "mock response queue exhausted".to_string())
    }
}
