use super::{
    MinimalRecoveryPaths, RunSessionOptions, RunSessionStepKind,
    render_minimal_recovery_stop_reason,
};
use crate::config::Config;
use crate::eval_events;
use crate::planner::repair::{self, RecoveryHandoff};
use crate::providers::AssistantReply;
use serde_json::json;
use std::time::Duration;

const DEFAULT_LIMIT: usize = 2;
pub(super) const REASON: &str = "max_length_no_tool_call_repeated";

pub(super) struct MaxLengthGuard {
    limit: usize,
    count: usize,
    duration: Duration,
}

impl MaxLengthGuard {
    pub(super) fn new(limit: Option<usize>) -> Self {
        Self {
            limit: limit.unwrap_or(DEFAULT_LIMIT).max(1),
            count: 0,
            duration: Duration::ZERO,
        }
    }

    pub(super) fn observe(
        &mut self,
        reply: &AssistantReply,
        num_predict: usize,
        elapsed: Duration,
    ) -> bool {
        if num_predict > 0
            && reply
                .completion_tokens
                .is_some_and(|count| count >= num_predict as u64)
            && reply.tool_calls.is_empty()
        {
            self.count = self.count.saturating_add(1);
            self.duration = self.duration.saturating_add(elapsed);
            return self.count >= self.limit;
        }
        // Only a successful Write/Edit ends the sequence. In particular, I1 had
        // short Read batches between its maximum-length, tool-less replies.
        false
    }

    pub(super) fn write_or_edit_succeeded(&mut self) {
        self.count = 0;
        self.duration = Duration::ZERO;
    }

    pub(super) fn stop(
        &self,
        config: &Config,
        options: &RunSessionOptions,
        goal: &str,
        changed_paths: &[String],
    ) -> anyhow::Error {
        let duration_ms = self.duration.as_millis();
        let detail = format!(
            "{REASON}: {} output-limit responses without tool calls since the last successful Write/Edit (limit={}, total_duration_ms={duration_ms})",
            self.count, self.limit
        );
        let recovery = save_handoff(config, options, goal, changed_paths, &detail);
        if let Err(error) = &recovery {
            eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event": "recovery_prompt_save_failed",
                    "recovery_handoff_kind": REASON,
                    "reason": eval_events::body_snippet(&error.to_string()),
                    "status": "incomplete",
                }),
            );
        }
        let paths = recovery.as_ref().ok();
        eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event": "loop_stop",
                "reason": REASON,
                "max_length_no_tool_call_count": self.count,
                "max_length_no_tool_call_limit": self.limit,
                "max_length_no_tool_call_duration_ms": duration_ms,
                "last_blocking_reason": detail,
                "last_provider_error": REASON,
                "session_scope": options.scope.as_str(),
                "step_kind": options.step_kind.map(RunSessionStepKind::as_str).unwrap_or(""),
                "step_id": options.step_id.as_deref().unwrap_or(""),
                "phase_scope": options.phase_scope.as_deref().unwrap_or(""),
                "recovery_prompt_path": paths.map(|paths| paths.prompt_path.as_str()).unwrap_or(""),
                "recovery_ultra_plan_path": paths.map(|paths| paths.yaml_path.as_str()).unwrap_or(""),
                "recovery_yaml_missing": paths.is_none(),
            }),
        );
        let detail = match &recovery {
            Ok(_) => detail,
            Err(error) => format!("{detail}; recovery handoff save failed: {error}"),
        };
        anyhow::anyhow!(render_minimal_recovery_stop_reason(detail, paths))
    }
}

fn save_handoff(
    config: &Config,
    options: &RunSessionOptions,
    goal: &str,
    changed_paths: &[String],
    detail: &str,
) -> anyhow::Result<MinimalRecoveryPaths> {
    let profile = if config.profile.trim().is_empty() {
        "generic"
    } else {
        &config.profile
    };
    let handoff = RecoveryHandoff {
        profile: profile.to_string(),
        original_goal: goal.to_string(),
        failed_phase: options
            .phase_scope
            .clone()
            .or_else(|| Some("minimal-loop".to_string())),
        failed_step: options
            .step_id
            .clone()
            .or_else(|| options.step_kind.map(|kind| kind.as_str().to_string())),
        failure_kind: REASON.to_string(),
        failure_evidence: vec![detail.to_string()],
        missing_paths: Vec::new(),
        missing_capabilities: Vec::new(),
        verify_commands: Vec::new(),
        changed_paths: changed_paths.to_vec(),
        repair_targets: vec!["resume_step_with_tool_calls".to_string()],
    };
    let prompt = repair::save_ultra_recovery_prompt(&config.workspace_root, REASON, &handoff)?;
    // This writer also registers the existing typed auto-Recovery candidate.
    let yaml = repair::save_recovery_ultra_plan(&config.workspace_root, REASON, &handoff)?;
    let paths = MinimalRecoveryPaths {
        prompt_path: repair::workspace_relative_handoff_path(&prompt),
        yaml_path: repair::workspace_relative_handoff_path(&yaml),
        suggested_prompt_command: repair::suggested_ultra_recovery_command(&prompt, profile),
        suggested_yaml_command: repair::suggested_recovery_ultra_plan_command(&yaml),
    };
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "recovery_prompt_saved",
            "recovery_handoff_kind": REASON,
            "recovery_prompt_path": paths.prompt_path,
            "recovery_ultra_plan_path": paths.yaml_path,
            "recovery_yaml_missing": false,
            "recovery_yaml_roundtrip_ok": true,
            "suggested_recovery_command": paths.suggested_prompt_command,
            "suggested_recovery_yaml_command": paths.suggested_yaml_command,
            "recovery_profile": profile,
            "local_repair_exhausted": true,
            "failure_kind": REASON,
            "status": "incomplete",
        }),
    );
    Ok(paths)
}
