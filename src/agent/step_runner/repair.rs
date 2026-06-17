use crate::agent::step_runner::verify::VerificationFailure;
use crate::util::workspace_paths::repairs_dir;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_REPLAN_PACKET_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairBudget {
    pub max_file_changing_attempts: usize,
}

impl Default for RepairBudget {
    fn default() -> Self {
        Self {
            max_file_changing_attempts: 2,
        }
    }
}

impl RepairBudget {
    pub fn allows_next_attempt(&self, completed_file_changing_attempts: usize) -> bool {
        completed_file_changing_attempts < self.max_file_changing_attempts
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairContext {
    pub step_id: String,
    pub original_goal: String,
    pub profile: String,
    pub style: String,
    pub step_instruction: String,
    pub verification_failures: Vec<VerificationFailure>,
    pub missing_expected_paths: Vec<String>,
    pub changed_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairExhaustedReport {
    pub step_id: String,
    pub file_changing_attempts: usize,
    pub missing_expected_paths: Vec<String>,
    pub repeated_changed_files: Vec<String>,
    pub failure_summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedRepairPrompt {
    pub path: PathBuf,
    pub relative_path: String,
    pub suggested_command: String,
    pub bytes: usize,
}

pub fn build_repair_prompt(context: &RepairContext) -> String {
    format!(
        "Repair the current CommandAgent step.\n\
Step: {step}\n\
Instruction: {instruction}\n\n\
Use Read/Glob to inspect before editing. Use Bash only for one simple local command at a time. Do not use shell chaining or fallback syntax such as &&, ||, or ;. Use Write/Edit for file changes. Make only the changes needed for this step.\n\
This is a repair turn after verifier failure. Do not spend the turn rerunning the same verifier command or promising to run it later; the runtime reruns verifier commands after your response. Use the turn to inspect and change files, or report a concrete blocker.\n\
Treat turn_error evidence as actionable. If a prior response violated the final-answer contract by saying it would read, edit, run, or verify something, make the tool call now instead of describing the next action. If Edit failed because the target text or file was not found, do not retry Edit from memory. Use Read/Glob to inspect the exact current file first, or use Write to create/replace the missing file. Use Edit only when you have exact current target text from this repair turn. If evidence says dependency_missing, do not run npm install/npm ci or other dependency installation unless this step explicitly is dependency setup and the environment allows it; report the blocker instead of faking build success.\n\
Repair focus:\n{focus}\n\
Verification evidence:\n{evidence}\n\
Missing expected paths:\n{missing}\n",
        step = context.step_id,
        instruction = context.step_instruction,
        focus = repair_focus(&context.verification_failures),
        evidence = failure_evidence(&context.verification_failures),
        missing = bullet_list(&context.missing_expected_paths),
    )
}

fn repair_focus(failures: &[VerificationFailure]) -> String {
    let mut focus = Vec::new();
    if failures.iter().any(has_concrete_source_failure) {
        focus.push("- Concrete verifier failure: the verifier identified a source error or source excerpt. Fix that reported error first, before continuing feature work. Read the referenced file before editing; use Edit only with exact current target text, or Write for a coherent full-file replacement when exact target text is uncertain.".to_string());
    }
    if failures
        .iter()
        .any(|failure| failure.reason == "edit_target_not_found")
    {
        focus.push("- Edit target not found: current file content did not match the attempted Edit. Do not call Edit from memory in the next repair turn. Call Read or Glob to inspect the current target file first. If exact target text is still uncertain, use Write to replace the full file with corrected content instead of retrying stale Edit text.".to_string());
    }
    if focus.is_empty() {
        "- none".to_string()
    } else {
        focus.join("\n")
    }
}

fn has_concrete_source_failure(failure: &VerificationFailure) -> bool {
    failure.source_excerpt.is_some()
        || failure.diagnostic_excerpt.contains("error[")
        || failure.diagnostic_excerpt.contains("Type error")
        || failure.diagnostic_excerpt.contains("Failed to compile")
}

pub fn repair_exhausted_report(
    context: &RepairContext,
    file_changing_attempts: usize,
) -> RepairExhaustedReport {
    RepairExhaustedReport {
        step_id: context.step_id.clone(),
        file_changing_attempts,
        missing_expected_paths: context.missing_expected_paths.clone(),
        repeated_changed_files: repeated_values(&context.changed_files),
        failure_summary: failure_evidence(&context.verification_failures),
    }
}

pub fn build_replan_packet(context: &RepairContext) -> String {
    let packet = format!(
        "Repair failed step: {step}\n\
Original goal: {goal}\n\
Profile: {profile}\n\
Style: {style}\n\
Step instruction: {instruction}\n\
Missing expected paths:\n{missing}\n\
Verification failures:\n{failures}\n\
Changed files in failed repair attempts:\n{changed}\n\
\n\
Task: Replan only this failed step. Keep scope narrow. Preserve completed work. Use Read/Glob for inspection, Write/Edit for file changes, and only one simple local verifier command at a time; do not use shell chaining or fallback syntax.",
        step = context.step_id,
        goal = context.original_goal,
        profile = context.profile,
        style = context.style,
        instruction = context.step_instruction,
        missing = bullet_list(&context.missing_expected_paths),
        failures = failure_evidence(&context.verification_failures),
        changed = bullet_list(&context.changed_files),
    );
    truncate_bytes(packet, MAX_REPLAN_PACKET_BYTES)
}

pub fn save_repair_prompt(
    cwd: impl AsRef<Path>,
    context: &RepairContext,
) -> Result<SavedRepairPrompt, RepairError> {
    let dir = repairs_dir(cwd.as_ref());
    fs::create_dir_all(&dir).map_err(|err| RepairError::Io {
        path: dir.clone(),
        message: err.to_string(),
    })?;
    let relative_path = format!(
        ".commandagent/repairs/repair-{}-{}.md",
        slug(&context.step_id),
        now_ms()
    );
    let path = cwd.as_ref().join(&relative_path);
    let packet = build_replan_packet(context);
    fs::write(&path, &packet).map_err(|err| RepairError::Io {
        path: path.clone(),
        message: err.to_string(),
    })?;
    let suggested_command = format!(
        "/ultra-plan-run --profile {} \"$(cat {})\"",
        context.profile, relative_path
    );
    Ok(SavedRepairPrompt {
        path,
        relative_path,
        suggested_command,
        bytes: packet.len(),
    })
}

fn failure_evidence(failures: &[VerificationFailure]) -> String {
    if failures.is_empty() {
        return "- none".to_string();
    }
    let mut out = Vec::new();
    for failure in failures {
        let mut item = format!(
            "- command: {}\n  reason: {}",
            failure.command, failure.reason
        );
        if !failure.diagnostic_excerpt.trim().is_empty() {
            item.push_str(&format!(
                "\n  diagnostic:\n{}",
                indent(&failure.diagnostic_excerpt, "    ")
            ));
        }
        if let Some(source) = &failure.source_excerpt {
            item.push_str(&format!(
                "\n  source: {}:{}\n{}",
                source.path,
                source.line,
                indent(&source.excerpt, "    ")
            ));
        }
        out.push(item);
    }
    out.join("\n")
}

fn bullet_list(values: &[String]) -> String {
    if values.is_empty() {
        "- none".to_string()
    } else {
        values
            .iter()
            .map(|value| format!("- {value}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn repeated_values(values: &[String]) -> Vec<String> {
    let mut repeated = Vec::new();
    for value in values {
        if values
            .iter()
            .filter(|candidate| *candidate == value)
            .count()
            > 1
            && !repeated.contains(value)
        {
            repeated.push(value.clone());
        }
    }
    repeated
}

fn indent(text: &str, prefix: &str) -> String {
    text.lines()
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn truncate_bytes(mut value: String, max: usize) -> String {
    if value.len() <= max {
        return value;
    }
    value.truncate(max.saturating_sub(32));
    value.push_str("\n...[truncated]\n");
    value
}

fn slug(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if (ch.is_whitespace() || ch == '-' || ch == '_') && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "step".to_string()
    } else {
        out
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepairError {
    Io { path: PathBuf, message: String },
}

impl std::fmt::Display for RepairError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, message } => write!(f, "{}: {}", path.display(), message),
        }
    }
}

impl std::error::Error for RepairError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::slash_command::{SlashCommandKind, parse_slash_command};
    use crate::agent::step_runner::verify::SourceExcerpt;

    #[test]
    fn repair_budget_allows_two_file_changing_attempts() {
        let budget = RepairBudget::default();

        assert!(budget.allows_next_attempt(0));
        assert!(budget.allows_next_attempt(1));
        assert!(!budget.allows_next_attempt(2));
    }

    #[test]
    fn exhausted_report_lists_repeated_changed_files() {
        let context = sample_context();

        let report = repair_exhausted_report(&context, 2);

        assert_eq!(report.step_id, "verify-build");
        assert_eq!(report.file_changing_attempts, 2);
        assert_eq!(report.repeated_changed_files, vec!["app/page.tsx"]);
        assert!(report.failure_summary.contains("Type error"));
    }

    #[test]
    fn saves_short_repair_prompt_with_suggested_command() {
        let root = temp_workspace("save");
        let context = sample_context();

        let saved = save_repair_prompt(&root, &context).unwrap();

        assert!(saved.path.exists());
        assert!(saved.relative_path.starts_with(".commandagent/repairs/"));
        assert!(saved.bytes <= MAX_REPLAN_PACKET_BYTES);
        assert_eq!(
            saved.suggested_command,
            format!(
                "/ultra-plan-run --profile nextjs \"$(cat {})\"",
                saved.relative_path
            )
        );

        let parsed = parse_slash_command(&saved.suggested_command, &root)
            .unwrap()
            .unwrap();
        assert_eq!(parsed.kind, SlashCommandKind::UltraPlanRun);
        assert_eq!(parsed.profile.as_deref(), Some("nextjs"));
        assert!(parsed.argument.len() <= MAX_REPLAN_PACKET_BYTES);
    }

    #[test]
    fn repair_prompt_contains_deterministic_evidence() {
        let prompt = build_repair_prompt(&sample_context());

        assert!(prompt.contains("Verification evidence"));
        assert!(prompt.contains("app/page.tsx:3"));
        assert!(prompt.contains("Missing expected paths"));
        assert!(prompt.contains("Do not use shell chaining"));
        assert!(prompt.contains("Use Write/Edit for file changes"));
        assert!(prompt.contains("the runtime reruns verifier commands"));
        assert!(prompt.contains("Use the turn to inspect and change files"));
        assert!(prompt.contains("Treat turn_error evidence as actionable"));
        assert!(prompt.contains("do not retry Edit from memory"));
        assert!(prompt.contains("Use Edit only when you have exact current target text"));
        assert!(prompt.contains("If evidence says dependency_missing"));
        assert!(prompt.contains("Repair focus"));
        assert!(prompt.contains("Concrete verifier failure"));
        assert!(prompt.contains("Fix that reported error first"));
        assert!(prompt.contains("Write for a coherent full-file replacement"));
    }

    #[test]
    fn repair_prompt_includes_turn_error_recovery_guidance() {
        let mut context = sample_context();
        context.verification_failures.insert(
            0,
            VerificationFailure {
                command: "initial turn".to_string(),
                reason: "turn_error".to_string(),
                stdout_excerpt: String::new(),
                stderr_excerpt: String::new(),
                diagnostic_excerpt:
                    "assistant violated final answer contract: Now let me verify the build"
                        .to_string(),
                source_excerpt: None,
            },
        );
        context.verification_failures.push(VerificationFailure {
            command: "repair turn".to_string(),
            reason: "edit_target_not_found".to_string(),
            stdout_excerpt: String::new(),
            stderr_excerpt: String::new(),
            diagnostic_excerpt:
                "Edit target was not found. The file state is stale for this Edit attempt."
                    .to_string(),
            source_excerpt: None,
        });

        let prompt = build_repair_prompt(&context);

        assert!(prompt.contains("initial turn"));
        assert!(prompt.contains("assistant violated final answer contract"));
        assert!(prompt.contains("Edit target was not found"));
        assert!(prompt.contains("make the tool call now"));
        assert!(prompt.contains("do not retry Edit from memory"));
        assert!(prompt.contains("Use Edit only when you have exact current target text"));
        assert!(prompt.contains("If evidence says dependency_missing"));
        assert!(prompt.contains("Repair focus"));
        assert!(prompt.contains("Edit target not found"));
        assert!(prompt.contains("Do not call Edit from memory"));
        assert!(prompt.contains("use Write to replace the full file"));
    }

    fn sample_context() -> RepairContext {
        RepairContext {
            step_id: "verify-build".to_string(),
            original_goal: "Build a Next.js app".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            step_instruction: "Run npm run build and fix failures.".to_string(),
            verification_failures: vec![VerificationFailure {
                command: "npm run build".to_string(),
                reason: "command_failed:1".to_string(),
                stdout_excerpt: String::new(),
                stderr_excerpt: "Failed to compile".to_string(),
                diagnostic_excerpt: "Type error: mismatch".to_string(),
                source_excerpt: Some(SourceExcerpt {
                    path: "app/page.tsx".to_string(),
                    line: 3,
                    excerpt: " 2: before\n>3: broken\n 4: after".to_string(),
                }),
            }],
            missing_expected_paths: vec!["app/page.tsx".to_string()],
            changed_files: vec![
                "app/page.tsx".to_string(),
                "app/page.tsx".to_string(),
                "package.json".to_string(),
            ],
        }
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "commandagent-repair-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
