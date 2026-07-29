fn assert_single_recovery_ultra_plan(root: &Path) -> UltraPlan {
    let plans_dir = root.join(".anvil/plans");
    assert!(
        plans_dir.is_dir(),
        "missing plans dir: {}",
        plans_dir.display()
    );
    let mut paths = std::fs::read_dir(&plans_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("recovery-ultra-plan-") && name.ends_with(".yaml")
                })
        })
        .collect::<Vec<_>>();
    paths.sort();
    assert_eq!(paths.len(), 1, "recovery plan paths: {paths:#?}");
    parse_ultra_plan(&std::fs::read_to_string(&paths[0]).unwrap()).unwrap()
}

#[derive(Clone)]
struct FakeClient {
    state: Arc<Mutex<FakeClientState>>,
}

struct FakeClientState {
    replies: Vec<AssistantReply>,
    messages: Vec<Vec<ConversationMessage>>,
}

impl FakeClient {
    fn new(replies: Vec<AssistantReply>) -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeClientState {
                replies,
                messages: Vec::new(),
            })),
        }
    }

    fn messages(&self) -> Vec<Vec<ConversationMessage>> {
        self.state.lock().unwrap().messages.clone()
    }
}

impl ChatClient for FakeClient {
    fn label(&self) -> &str {
        "fake"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let mut state = self.state.lock().unwrap();
        state.messages.push(messages.to_vec());
        if state.replies.is_empty() {
            anyhow::bail!("fake client exhausted")
        }
        Ok(state.replies.remove(0))
    }
}

#[derive(Clone)]
struct CompactAwareCompileRepairClient {
    state: Arc<Mutex<CompactAwareCompileRepairState>>,
}

struct CompactAwareCompileRepairState {
    messages: Vec<Vec<ConversationMessage>>,
    initial_done: bool,
    appended_repair_calls: usize,
    compact_repair_calls: usize,
}

impl CompactAwareCompileRepairClient {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(CompactAwareCompileRepairState {
                messages: Vec::new(),
                initial_done: false,
                appended_repair_calls: 0,
                compact_repair_calls: 0,
            })),
        }
    }

    fn messages(&self) -> Vec<Vec<ConversationMessage>> {
        self.state.lock().unwrap().messages.clone()
    }

    fn appended_repair_calls(&self) -> usize {
        self.state.lock().unwrap().appended_repair_calls
    }

    fn compact_repair_calls(&self) -> usize {
        self.state.lock().unwrap().compact_repair_calls
    }
}

impl ChatClient for CompactAwareCompileRepairClient {
    fn label(&self) -> &str {
        "compact-aware"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let mut state = self.state.lock().unwrap();
        state.messages.push(messages.to_vec());
        let prompt = messages
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if !state.initial_done {
            state.initial_done = true;
            return Ok(api_mismatch_initial_reply(3011));
        }
        if prompt.contains("Repair session mode: compact") {
            state.compact_repair_calls += 1;
            return Ok(api_mismatch_poll_fix_reply());
        }
        if prompt.contains("Property 'onStateChange'") {
            state.appended_repair_calls += 1;
            if state.appended_repair_calls == 1 {
                return Ok(api_mismatch_read_only_reply());
            }
            return Ok(AssistantReply::text(
                "The failing call is engine.onStateChange, but no edit is needed.",
            ));
        }
        anyhow::bail!("compact-aware fake client received unexpected prompt")
    }
}

#[derive(Clone)]
struct RegenerationCompileRepairClient {
    state: Arc<Mutex<RegenerationCompileRepairState>>,
}

struct RegenerationCompileRepairState {
    messages: Vec<Vec<ConversationMessage>>,
    initial_done: bool,
    appended_repair_calls: usize,
    compact_repair_calls: usize,
    regeneration_calls: usize,
    regeneration_reply: AssistantReply,
}

impl RegenerationCompileRepairClient {
    fn new(regeneration_reply: AssistantReply) -> Self {
        Self {
            state: Arc::new(Mutex::new(RegenerationCompileRepairState {
                messages: Vec::new(),
                initial_done: false,
                appended_repair_calls: 0,
                compact_repair_calls: 0,
                regeneration_calls: 0,
                regeneration_reply,
            })),
        }
    }

    fn messages(&self) -> Vec<Vec<ConversationMessage>> {
        self.state.lock().unwrap().messages.clone()
    }

    fn regeneration_calls(&self) -> usize {
        self.state.lock().unwrap().regeneration_calls
    }
}

impl ChatClient for RegenerationCompileRepairClient {
    fn label(&self) -> &str {
        "regeneration-aware"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let mut state = self.state.lock().unwrap();
        state.messages.push(messages.to_vec());
        let prompt = messages
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if !state.initial_done {
            state.initial_done = true;
            return Ok(api_mismatch_initial_reply(3011));
        }
        if prompt.contains("Repair session mode: compact regeneration") {
            state.regeneration_calls += 1;
            return Ok(state.regeneration_reply.clone());
        }
        if prompt.contains("Repair session mode: compact") {
            state.compact_repair_calls += 1;
            return Ok(AssistantReply::text(
                "I understand the compile frame, but no edit is needed.",
            ));
        }
        if prompt.contains("Property 'onStateChange'") {
            state.appended_repair_calls += 1;
            if state.appended_repair_calls == 1 {
                return Ok(api_mismatch_read_only_reply());
            }
            return Ok(AssistantReply::text(
                "The failing source was inspected, but no edit is needed.",
            ));
        }
        anyhow::bail!("regeneration-aware fake client received unexpected prompt")
    }
}

#[derive(Clone)]
struct EditThenRegenerationCompileRepairClient {
    state: Arc<Mutex<EditThenRegenerationCompileRepairState>>,
}

struct EditThenRegenerationCompileRepairState {
    messages: Vec<Vec<ConversationMessage>>,
    initial_done: bool,
    read_followup_pending: bool,
    appended_repair_calls: usize,
    compact_repair_calls: usize,
    regeneration_calls: usize,
}

impl EditThenRegenerationCompileRepairClient {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(EditThenRegenerationCompileRepairState {
                messages: Vec::new(),
                initial_done: false,
                read_followup_pending: false,
                appended_repair_calls: 0,
                compact_repair_calls: 0,
                regeneration_calls: 0,
            })),
        }
    }

    fn appended_repair_calls(&self) -> usize {
        self.state.lock().unwrap().appended_repair_calls
    }

    fn compact_repair_calls(&self) -> usize {
        self.state.lock().unwrap().compact_repair_calls
    }

    fn regeneration_calls(&self) -> usize {
        self.state.lock().unwrap().regeneration_calls
    }
}

impl ChatClient for EditThenRegenerationCompileRepairClient {
    fn label(&self) -> &str {
        "edit-then-regeneration-aware"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let mut state = self.state.lock().unwrap();
        state.messages.push(messages.to_vec());
        let prompt = messages
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if !state.initial_done {
            state.initial_done = true;
            return Ok(api_mismatch_initial_reply(3011));
        }
        if state.read_followup_pending {
            state.read_followup_pending = false;
            return Ok(AssistantReply::text(
                "The file was inspected, but no source behavior changed.",
            ));
        }
        if prompt.contains("Repair session mode: compact regeneration") {
            state.regeneration_calls += 1;
            return Ok(api_mismatch_poll_fix_reply());
        }
        if prompt.contains("Repair session mode: compact") {
            state.compact_repair_calls += 1;
            state.read_followup_pending = true;
            return Ok(api_mismatch_read_only_reply());
        }
        if prompt.contains("Compile error frames and remedies")
            || prompt.contains("implementation_compile_error")
            || prompt.contains("Type error:")
        {
            state.appended_repair_calls += 1;
            if state.appended_repair_calls == 1 {
                return Ok(AssistantReply {
                    content: String::new(),
                    tool_calls: vec![crate::state::ToolCall::new(
                        "Write",
                        serde_json::json!({"path":"src/app/SpaceInvadersGame.tsx","content":api_mismatch_insufficient_game_source()}),
                    )],
                    prompt_tokens: None,
                    completion_tokens: None,
                });
            }
            state.read_followup_pending = true;
            return Ok(api_mismatch_read_only_reply());
        }
        anyhow::bail!("edit-then-regeneration fake client received unexpected prompt")
    }
}

#[derive(Clone)]
struct FlakyClient {
    state: Arc<Mutex<FlakyClientState>>,
}

struct FlakyClientState {
    replies: Vec<AssistantReply>,
    messages: Vec<Vec<ConversationMessage>>,
    failures_remaining: usize,
    failure_message: String,
}

impl FlakyClient {
    fn new(
        failures_remaining: usize,
        failure_message: impl Into<String>,
        replies: Vec<AssistantReply>,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(FlakyClientState {
                replies,
                messages: Vec::new(),
                failures_remaining,
                failure_message: failure_message.into(),
            })),
        }
    }

    fn messages(&self) -> Vec<Vec<ConversationMessage>> {
        self.state.lock().unwrap().messages.clone()
    }
}

impl ChatClient for FlakyClient {
    fn label(&self) -> &str {
        "flaky"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let mut state = self.state.lock().unwrap();
        state.messages.push(messages.to_vec());
        if state.failures_remaining > 0 {
            state.failures_remaining -= 1;
            anyhow::bail!("{}", state.failure_message);
        }
        if state.replies.is_empty() {
            anyhow::bail!("flaky client exhausted")
        }
        Ok(state.replies.remove(0))
    }
}

#[derive(Clone)]
struct EchoGoalPlanner {
    state: Arc<Mutex<EchoGoalPlannerState>>,
}

struct EchoGoalPlannerState {
    messages: Vec<Vec<ConversationMessage>>,
    calls: usize,
}

impl EchoGoalPlanner {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(EchoGoalPlannerState {
                messages: Vec::new(),
                calls: 0,
            })),
        }
    }

    fn messages(&self) -> Vec<Vec<ConversationMessage>> {
        self.state.lock().unwrap().messages.clone()
    }
}

impl ChatClient for EchoGoalPlanner {
    fn label(&self) -> &str {
        "echo-planner"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let mut state = self.state.lock().unwrap();
        state.messages.push(messages.to_vec());
        state.calls += 1;
        let echoed_goal = messages
            .iter()
            .rev()
            .find(|message| message.role == "user")
            .map(|message| message.content.clone())
            .unwrap_or_default();
        let path = format!("phase-{}.txt", state.calls);
        let plan = StepPlan {
            goal: echoed_goal,
            steps: vec![PlanStep {
                id: format!("phase-{}", state.calls),
                kind: "implement".to_string(),
                expected_result: "pass".to_string(),
                instruction: format!("Create {path} for this phase."),
                expected_paths: vec![path],
                verify: Vec::new(),
            }],
        };
        Ok(AssistantReply::text(serde_json::to_string(&plan).unwrap()))
    }
}

