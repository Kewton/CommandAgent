use crate::providers::ToolCallMode;

pub const MAX_EVENT_TEXT_CHARS: usize = 160;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeEvent {
    PlanGenerationStarted {
        kind: PlanKind,
        goal: String,
        profile: String,
    },
    PlanGenerationFinished {
        kind: PlanKind,
        item_count: usize,
    },
    PlanSaved {
        kind: PlanKind,
        path: String,
        item_ids: Vec<String>,
    },
    UltraPhaseStarted {
        index: usize,
        total: usize,
        phase_id: String,
    },
    UltraPhaseFinished {
        index: usize,
        total: usize,
        phase_id: String,
    },
    UltraPhaseFailed {
        index: usize,
        total: usize,
        phase_id: String,
        error: String,
    },
    ProfileVerificationFailed {
        profile: String,
        failures: Vec<String>,
    },
    StepStarted {
        index: usize,
        total: usize,
        step_id: String,
    },
    StepFinished {
        index: usize,
        total: usize,
        step_id: String,
    },
    StepFailed {
        index: usize,
        total: usize,
        step_id: String,
        error: String,
        missing_expected_paths: Vec<String>,
    },
    VerifierStarted {
        step_id: String,
        command: String,
    },
    VerifierFinished {
        step_id: String,
        command: String,
        ok: bool,
        failure_count: usize,
    },
    DependencySetupStarted {
        step_id: String,
        command: String,
    },
    DependencySetupFinished {
        step_id: String,
        command: String,
        ok: bool,
        elapsed_ms: u128,
        status: String,
    },
    RepairAttemptStarted {
        step_id: String,
        attempt: usize,
        max_attempts: usize,
        missing_expected_paths: Vec<String>,
    },
    RepairExhausted {
        step_id: String,
        repair_path: String,
        suggested_command: String,
        missing_expected_paths: Vec<String>,
    },
    ModelRequestStarted {
        iteration: usize,
        model: String,
        tool_call_mode: ToolCallMode,
    },
    ModelResponseReceived {
        iteration: usize,
        tool_call_mode: ToolCallMode,
        tool_call_count: usize,
        content_chars: usize,
        elapsed_ms: u128,
    },
    ParserFeedbackSent {
        iteration: usize,
        previous_tool_call_mode: ToolCallMode,
        next_tool_call_mode: ToolCallMode,
        error: String,
    },
    GuardFeedbackSent {
        iteration: usize,
        kind: GuardFeedbackKind,
        tool_call_mode: ToolCallMode,
        missing_artifacts: Vec<String>,
    },
    ArtifactStatus {
        scope: ArtifactScope,
        path: String,
        status: ArtifactStatus,
    },
    ToolCallStarted {
        iteration: usize,
        tool_name: String,
        args_summary: String,
    },
    ToolCallFinished {
        iteration: usize,
        tool_name: String,
        ok: bool,
        output_chars: usize,
        error: Option<String>,
    },
    ToolResultTruncated {
        iteration: usize,
        tool_name: String,
        original_chars: usize,
        returned_chars: usize,
        reason: String,
    },
    FinalAnswerAccepted {
        iteration: usize,
        answer_chars: usize,
    },
    SessionError {
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardFeedbackKind {
    FutureAction,
    RequestedArtifacts,
    ActionRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanKind {
    StepPlan,
    UltraPlan,
    PhaseStepPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactScope {
    StepExpectedPath,
    FinalRequiredArtifact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactStatus {
    Ok,
    Missing,
    Unchecked,
}

pub trait RuntimeObserver {
    fn on_event(&mut self, _event: RuntimeEvent) {}
}

#[derive(Debug, Default)]
pub struct NoopRuntimeObserver;

impl RuntimeObserver for NoopRuntimeObserver {}

pub fn bounded_event_text(input: impl AsRef<str>) -> String {
    let input = input.as_ref();
    let mut out = String::with_capacity(input.len().min(MAX_EVENT_TEXT_CHARS + 3));
    for ch in input.chars().take(MAX_EVENT_TEXT_CHARS) {
        let cp = ch as u32;
        if cp < 0x20 || cp == 0x7f || (0x80..=0x9f).contains(&cp) {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    if input.chars().count() > MAX_EVENT_TEXT_CHARS {
        out.push_str("...");
    }
    out.trim_end().to_string()
}

#[cfg(test)]
#[derive(Debug, Default)]
pub(crate) struct CaptureObserver {
    events: Vec<RuntimeEvent>,
}

#[cfg(test)]
impl CaptureObserver {
    pub(crate) fn events(&self) -> &[RuntimeEvent] {
        &self.events
    }
}

#[cfg(test)]
impl RuntimeObserver for CaptureObserver {
    fn on_event(&mut self, event: RuntimeEvent) {
        self.events.push(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_event_text_sanitizes_and_caps() {
        let long = format!("a\n{}", "b".repeat(MAX_EVENT_TEXT_CHARS + 8));
        let bounded = bounded_event_text(long);

        assert!(!bounded.contains('\n'));
        assert!(bounded.ends_with("..."));
        assert!(bounded.chars().count() <= MAX_EVENT_TEXT_CHARS + 3);
    }
}
