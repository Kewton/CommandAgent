use crate::agent::step_runner::correction_evidence::{ContractEvidence, failure_signature};
use crate::agent::step_runner::profiles::ProfileVerificationFailure;
use crate::agent::step_runner::recovery_orchestration::orchestrate_evidence;
use crate::agent::step_runner::recovery_policy::profile_failure_policy;
use crate::agent::step_runner::recovery_task::RecoveryTaskContract;
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
    pub active_profile_contract_facts: Vec<String>,
    pub contract_evidence: Vec<ContractEvidence>,
    pub verification_failures: Vec<VerificationFailure>,
    pub missing_expected_paths: Vec<String>,
    pub changed_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileRepairContext {
    pub phase_id: String,
    pub original_goal: String,
    pub phase_goal: String,
    pub profile: String,
    pub style: String,
    pub profile_failures: Vec<ProfileVerificationFailure>,
    pub phase_contract_facts: Vec<String>,
    pub profile_facts: Vec<String>,
    pub expected_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolProtocolCorrectionContext {
    pub tool: String,
    pub reason_code: String,
    pub missing_field: Option<String>,
    pub required_fields: Vec<String>,
    pub target_path: Option<String>,
    pub diagnostic: String,
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
    let active_contract = active_profile_contract_section(&context.active_profile_contract_facts);
    let contract_evidence = contract_evidence_section(&context.contract_evidence);
    let recovery_task = recovery_task_section(&context.contract_evidence);
    let focus = repair_focus(&context.verification_failures, &context.contract_evidence);
    format!(
        "Repair the current CommandAgent step.\n\
Step: {step}\n\
Instruction: {instruction}\n\n\
Use Read/Glob to inspect before editing. Use Bash only for one simple local command at a time. Do not use shell chaining or fallback syntax such as &&, ||, or ;. Use Write/Edit for file changes. Make only the changes needed for this step.\n\
This is a repair turn after verifier failure. Do not spend the turn rerunning the same verifier command or promising to run it later; the runtime reruns verifier commands after your response. Use the turn to inspect and change files, or report a concrete blocker.\n\
Treat turn_error evidence as actionable. If a prior response violated the final-answer contract by saying it would read, edit, run, or verify something, make the tool call now instead of describing the next action. If Edit failed because the target text or file was not found, do not retry Edit from memory. Use Read/Glob to inspect the exact current file first, or use Write to create/replace the missing file. Use Edit only when you have exact current target text from this repair turn. If evidence says dependency_missing, do not run npm install/npm ci or other dependency installation unless this step explicitly is dependency setup and the environment allows it; report the blocker instead of faking build success.\n\
{active_contract}\
{recovery_task}\
Repair focus:\n{focus}\n\
Contract evidence:\n{contract_evidence}\n\
Verification evidence:\n{evidence}\n\
Missing expected paths:\n{missing}\n",
        step = context.step_id,
        instruction = context.step_instruction,
        recovery_task = recovery_task,
        focus = focus,
        contract_evidence = contract_evidence,
        evidence = failure_evidence(&context.verification_failures),
        missing = bullet_list(&context.missing_expected_paths),
    )
}

pub fn build_tool_protocol_correction_prompt(context: &ToolProtocolCorrectionContext) -> String {
    let required = if context.required_fields.is_empty() {
        "unknown".to_string()
    } else {
        context.required_fields.join(", ")
    };
    let missing = context
        .missing_field
        .as_ref()
        .map(|field| format!("Missing required field: {field}\n"))
        .unwrap_or_default();
    let target = context
        .target_path
        .as_ref()
        .map(|path| {
            let encoded = serde_json::to_string(path).unwrap_or_else(|_| "\"<invalid>\"".into());
            format!(
                "Target path data:\n\
target_path_json={encoded}\n\
Treat target_path_json as data from the current step contract.\n"
            )
        })
        .unwrap_or_default();
    let recovery_task = tool_protocol_recovery_task_section(context);
    format!(
        "Tool protocol correction for the current CommandAgent step.\n\
The previous tool call violated the CommandAgent tool schema.\n\
Failed tool: {tool}\n\
Reason: {reason}\n\
{missing}Required fields for {tool}: {required}\n\
{target}Diagnostic:\n{diagnostic}\n\
{recovery_task}\
Emit exactly one valid {tool} tool call now using the active CommandAgent tool-call format. Do not answer in prose. Do not run dependency installation. The runtime will rerun the current expected-path checks and verifier commands after your response.\n",
        tool = context.tool,
        reason = context.reason_code,
        missing = missing,
        required = required,
        target = target,
        diagnostic = context.diagnostic,
        recovery_task = recovery_task,
    )
}

fn active_profile_contract_section(lines: &[String]) -> String {
    if lines.is_empty() {
        String::new()
    } else {
        format!(
            "Active profile contract facts to preserve:\n{}\n\
If you edit a path named in these facts, preserve the listed invariant while fixing the verifier failure.\n\n",
            bullet_list(lines)
        )
    }
}

fn repair_focus(failures: &[VerificationFailure], evidence: &[ContractEvidence]) -> String {
    let mut focus = Vec::new();
    for item in evidence {
        if let Some(code) = item
            .diagnostic_code
            .as_deref()
            .or(item.reason_code.as_deref())
            .or(item.violated_contract.as_deref())
        {
            push_unique(&mut focus, format!("- Current blocker: {code}"));
        }
        if let Some(target) = repair_focus_target(item) {
            push_unique(&mut focus, format!("- Repair target: {target}"));
        }
        if !item.candidate_artifacts.is_empty() {
            push_unique(
                &mut focus,
                format!(
                    "- Candidate artifacts: {}",
                    item.candidate_artifacts.join(", ")
                ),
            );
        }
        if let Some(failure_signature) = item.failure_signature.as_deref() {
            push_unique(
                &mut focus,
                format!("- Failure signature: {failure_signature}"),
            );
        }
        if let Some(repair_focus) = item.repair_focus.as_deref() {
            push_unique(&mut focus, format!("- {repair_focus}"));
        } else if let Some(action) = item.required_action.as_deref() {
            push_unique(&mut focus, format!("- {action}"));
        }
    }
    if failures.iter().any(has_concrete_source_failure) {
        push_unique(&mut focus, "- Concrete verifier failure: the verifier identified a source error or source excerpt. Fix that reported error first, before continuing feature work. Read the referenced file before editing; use Edit only with exact current target text, or Write for a coherent full-file replacement when exact target text is uncertain.".to_string());
    }
    if failures
        .iter()
        .any(|failure| failure.reason == "edit_target_not_found")
    {
        push_unique(&mut focus, "- Edit target not found: current file content did not match the attempted Edit. Do not call Edit from memory in the next repair turn. Call Read or Glob to inspect the current target file first. If exact target text is still uncertain, use Write to replace the full file with corrected content instead of retrying stale Edit text.".to_string());
    }
    if focus.is_empty() {
        "- none".to_string()
    } else {
        focus.join("\n")
    }
}

fn repair_focus_target(evidence: &ContractEvidence) -> Option<&str> {
    if evidence.guard == "step_policy" {
        return evidence.repair_target.as_deref();
    }
    evidence
        .repair_target
        .as_deref()
        .or(evidence.target_path.as_deref())
}

fn recovery_task_section(evidence: &[ContractEvidence]) -> String {
    let mut tasks = Vec::new();
    for item in evidence {
        if let Some(task) = RecoveryTaskContract::from_contract_evidence(item)
            && !tasks.contains(&task)
        {
            tasks.push(task);
        }
    }
    tasks.sort_by_key(recovery_task_priority);
    if tasks.is_empty() {
        String::new()
    } else {
        format!("Recovery task:\n{}\n", recovery_task_list(&tasks))
    }
}

fn recovery_task_priority(task: &RecoveryTaskContract) -> u8 {
    task.active_job_priority
        .as_deref()
        .and_then(|value| value.parse::<u8>().ok())
        .unwrap_or(u8::MAX)
}

fn tool_protocol_recovery_task_section(context: &ToolProtocolCorrectionContext) -> String {
    let required = if context.required_fields.is_empty() {
        "the required fields".to_string()
    } else {
        context.required_fields.join(", ")
    };
    let required_action = if context.tool == "Write"
        && context.missing_field.as_deref() == Some("path")
        && let Some(path) = context.target_path.as_deref()
    {
        format!(
            "Emit exactly one valid Write tool call with path={path} and required fields: {required}."
        )
    } else {
        format!(
            "Emit exactly one valid {} tool call with required fields: {required}.",
            context.tool
        )
    };
    let mut task = RecoveryTaskContract::new("tool_protocol")
        .with_contract_code(context.reason_code.clone())
        .with_blocker(format!("Tool call violated schema for {}", context.tool))
        .with_required_action(required_action)
        .with_allowed_tool(context.tool.clone())
        .with_disallowed_action("Do not answer in prose instead of a tool call.")
        .with_disallowed_action("Do not run dependency installation.")
        .with_success_check("tool schema validation");
    if let Some(path) = context.target_path.clone() {
        task = task
            .with_repair_target(path.clone())
            .with_candidate_artifact(path);
    }
    format!("Recovery task:\n{}\n", recovery_task_list(&[task]))
}

fn recovery_task_list(tasks: &[RecoveryTaskContract]) -> String {
    tasks
        .iter()
        .filter_map(RecoveryTaskContract::render)
        .map(|rendered| indent(&rendered, "  "))
        .enumerate()
        .map(|(index, rendered)| format!("- task {}:\n{}", index + 1, rendered))
        .collect::<Vec<_>>()
        .join("\n")
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
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
    let active_contract = if context.active_profile_contract_facts.is_empty() {
        String::new()
    } else {
        format!(
            "Active profile contract facts:\n{}\n",
            bullet_list(&context.active_profile_contract_facts)
        )
    };
    let contract_evidence = contract_evidence_section(&context.contract_evidence);
    let recovery_task = recovery_task_section(&context.contract_evidence);
    let focus = repair_focus(&context.verification_failures, &context.contract_evidence);
    let packet = format!(
        "Repair failed step: {step}\n\
Original goal: {goal}\n\
Profile: {profile}\n\
Style: {style}\n\
Step instruction: {instruction}\n\
{active_contract}\
{recovery_task}\
Repair focus:\n{focus}\n\
Missing expected paths:\n{missing}\n\
Contract evidence:\n{contract_evidence}\n\
Verification failures:\n{failures}\n\
Changed files in failed repair attempts:\n{changed}\n\
\n\
Replan focus:\n\
- Address the contract evidence first.\n\
- If guard=verifier, fix the command failure before feature work.\n\
- If guard=step_policy, do not put mutation in inspect/report steps.\n\
- If guard=tool_protocol, emit valid tool calls with required fields.\n\
\n\
Continuation semantics:\n\
- Running the suggested command starts a standalone repair plan for this failed step.\n\
- The original ultra plan remains incomplete until it is explicitly resumed or replanned.\n\
\n\
Task: Replan only this failed step. Keep scope narrow. Preserve completed work. Use Read/Glob for inspection, Write/Edit for file changes, and only one simple local verifier command at a time; do not use shell chaining or fallback syntax.",
        step = context.step_id,
        goal = context.original_goal,
        profile = context.profile,
        style = context.style,
        instruction = context.step_instruction,
        active_contract = active_contract,
        recovery_task = recovery_task,
        focus = focus,
        missing = bullet_list(&context.missing_expected_paths),
        contract_evidence = contract_evidence,
        failures = failure_evidence(&context.verification_failures),
        changed = bullet_list(&context.changed_files),
    );
    truncate_bytes(packet, MAX_REPLAN_PACKET_BYTES)
}

pub fn build_profile_replan_packet(context: &ProfileRepairContext) -> String {
    let route_targets = profile_route_integration_targets(&context.profile_failures);
    let profile_contract_evidence = profile_contract_evidence(context);
    let recovery_task = recovery_task_section(&profile_contract_evidence);
    let focus = repair_focus(&[], &profile_contract_evidence);
    let contract_evidence = contract_evidence_section(&profile_contract_evidence);
    let packet = format!(
        "Repair failed profile verification after ultra phase: {phase}\n\
Original goal: {goal}\n\
Phase goal: {phase_goal}\n\
Profile: {profile}\n\
Style: {style}\n\
Expected paths from completed phase:\n{expected}\n\
{recovery_task}\
Repair focus:\n{focus}\n\
Contract evidence:\n{contract_evidence}\n\
Profile verification failures:\n{failures}\n\
{route_targets}\
Phase contract facts:\n{phase_facts}\n\
Profile facts after phase:\n{profile_facts}\n\
\n\
Continuation semantics:\n\
- Running the suggested command starts a standalone repair plan for this profile verification failure.\n\
- The original ultra plan remains incomplete until it is explicitly resumed or replanned.\n\
\n\
Task: Replan only the failed profile contract. Keep scope narrow. Preserve completed work. Use Read/Glob for inspection, Write/Edit for file changes, and only one simple local verifier command at a time; do not use shell chaining or fallback syntax. Fix the reported profile contract directly before adding new feature work.",
        phase = context.phase_id,
        goal = context.original_goal,
        phase_goal = context.phase_goal,
        profile = context.profile,
        style = context.style,
        expected = bullet_list(&context.expected_paths),
        recovery_task = recovery_task,
        focus = focus,
        contract_evidence = contract_evidence,
        failures = profile_failure_evidence(&context.profile_failures),
        route_targets = route_targets,
        phase_facts = bullet_list(&context.phase_contract_facts),
        profile_facts = bullet_list(&context.profile_facts),
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

pub fn save_profile_repair_prompt(
    cwd: impl AsRef<Path>,
    context: &ProfileRepairContext,
) -> Result<SavedRepairPrompt, RepairError> {
    let dir = repairs_dir(cwd.as_ref());
    fs::create_dir_all(&dir).map_err(|err| RepairError::Io {
        path: dir.clone(),
        message: err.to_string(),
    })?;
    let relative_path = format!(
        ".commandagent/repairs/repair-profile-{}-{}.md",
        slug(&context.phase_id),
        now_ms()
    );
    let path = cwd.as_ref().join(&relative_path);
    let packet = build_profile_replan_packet(context);
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

fn contract_evidence_section(evidence: &[ContractEvidence]) -> String {
    if evidence.is_empty() {
        return "- none".to_string();
    }
    evidence
        .iter()
        .cloned()
        .map(orchestrate_evidence)
        .filter_map(|evidence| evidence.render())
        .map(|rendered| indent(&rendered, "  "))
        .enumerate()
        .map(|(index, rendered)| format!("- evidence {}:\n{}", index + 1, rendered))
        .collect::<Vec<_>>()
        .join("\n")
}

fn profile_failure_evidence(failures: &[ProfileVerificationFailure]) -> String {
    if failures.is_empty() {
        return "- none".to_string();
    }
    failures
        .iter()
        .map(|failure| format!("- {}", failure.render()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn profile_route_integration_targets(failures: &[ProfileVerificationFailure]) -> String {
    let targets = failures
        .iter()
        .filter(|failure| failure.code == "nextjs_route_not_integrated")
        .filter_map(|failure| {
            let route = failure.paths.first()?;
            let artifact = failure.paths.get(1)?;
            Some(format!(
                "- selected_route={route}\n  unintegrated_artifact={artifact}\n  expected=selected route imports or references the artifact module"
            ))
        })
        .collect::<Vec<_>>();
    if targets.is_empty() {
        String::new()
    } else {
        format!("Route integration targets:\n{}\n", targets.join("\n"))
    }
}

fn profile_contract_evidence(context: &ProfileRepairContext) -> Vec<ContractEvidence> {
    context
        .profile_failures
        .iter()
        .map(|failure| profile_failure_contract_evidence(&context.phase_id, failure))
        .collect()
}

fn profile_failure_contract_evidence(
    phase_id: &str,
    failure: &ProfileVerificationFailure,
) -> ContractEvidence {
    let policy = profile_failure_policy(failure);
    let repair_target = policy.repair_target.clone();
    let evidence = ContractEvidence::new("profile_verification")
        .with_failed_step(phase_id.to_string())
        .with_violated_contract(failure.code.clone())
        .with_reason_code(failure.code.clone())
        .with_failure_kind("profile_contract_failed")
        .with_diagnostic_code(failure.code.clone())
        .with_failure_signature(failure_signature([
            "profile_verification",
            phase_id,
            &failure.code,
            repair_target.as_deref().unwrap_or(""),
        ]))
        .with_candidate_artifacts(failure.paths.clone())
        .with_observed_expected_pairs(vec![profile_observed_expected_pair(failure)])
        .with_repair_focus(profile_repair_focus(failure))
        .with_diagnostic(failure.message.clone());
    orchestrate_evidence(policy.apply_to_evidence(evidence))
}

fn nextjs_tailwind_repair_target(failure: &ProfileVerificationFailure) -> Option<String> {
    let message = failure.message.to_ascii_lowercase();
    if message.contains("postcss.config") {
        Some("postcss.config.js".to_string())
    } else if message.contains("tailwind.config") {
        Some("tailwind.config.js".to_string())
    } else if message.contains("package.json") || message.contains("dependency") {
        Some("package.json".to_string())
    } else {
        failure.paths.first().cloned()
    }
}

fn profile_observed_expected_pair(failure: &ProfileVerificationFailure) -> String {
    match failure.code.as_str() {
        "nextjs_integration_artifact_missing" => {
            let artifact = failure.paths.first().map(String::as_str).unwrap_or("unknown");
            let route = failure.paths.get(1).map(String::as_str).unwrap_or("unknown");
            format!(
                "observed={artifact} is missing before route integration; expected={artifact} exists before checking selected route {route}"
            )
        }
        "nextjs_route_not_integrated" => {
            let route = failure.paths.first().map(String::as_str).unwrap_or("unknown");
            let artifact = failure.paths.get(1).map(String::as_str).unwrap_or("unknown");
            format!(
                "observed={artifact} is not referenced from {route}; expected=selected route imports or references artifact"
            )
        }
        "nextjs_app_root_ambiguous" => {
            "observed=app/ and src/app/ routes are mixed; expected=one selected Next.js app root"
                .to_string()
        }
        "nextjs_dev_port_drift" => {
            "observed=package.json scripts.dev does not preserve requested port; expected=next dev on requested port".to_string()
        }
        "nextjs_build_script_drift" => {
            "observed=package.json scripts.build is missing or drifted; expected=next build"
                .to_string()
        }
        "nextjs_dependency_version_conflict" => {
            "observed=package.json pins Next.js and React peer versions that are incompatible; expected=next/react/react-dom versions are mutually compatible".to_string()
        }
        "nextjs_missing_dependency" => {
            "observed=package.json is missing required Next.js runtime dependencies; expected=package.json includes next, react, and react-dom".to_string()
        }
        "nextjs_tailwind_contract" => {
            "observed=Tailwind/PostCSS contract is incomplete; expected=Tailwind dependencies and config files align with Next.js app styling".to_string()
        }
        "nextjs_alias_missing" => {
            "observed=tsconfig path alias is missing or incomplete; expected=@/* alias maps to the selected source root".to_string()
        }
        "nextjs_tsconfig_excludes_route" => {
            "observed=tsconfig excludes selected Next.js route root; expected=compiler options include the selected app route root".to_string()
        }
        _ => format!(
            "observed={}; expected=profile contract {} is satisfied",
            failure.message, failure.code
        ),
    }
}

fn profile_required_action(failure: &ProfileVerificationFailure) -> String {
    match failure.code.as_str() {
        "nextjs_integration_artifact_missing" => {
            let artifact = failure
                .paths
                .first()
                .map(String::as_str)
                .unwrap_or("the missing explicit artifact");
            format!("create {artifact} before editing selected route integration")
        }
        "nextjs_route_not_integrated" => {
            let route = failure
                .paths
                .first()
                .map(String::as_str)
                .unwrap_or("selected route");
            let artifact = failure
                .paths
                .get(1)
                .map(String::as_str)
                .unwrap_or("the unintegrated artifact");
            format!("edit {route} so it imports or references {artifact}")
        }
        "nextjs_app_root_ambiguous" => {
            "consolidate Next.js route files under one selected app root".to_string()
        }
        "nextjs_tsconfig_excludes_route" => {
            "align tsconfig rootDir with the selected Next.js route root".to_string()
        }
        "nextjs_alias_missing" => {
            "edit tsconfig.json so @/* resolves to the selected source root used by the Next.js app".to_string()
        }
        "nextjs_missing_dependency" => {
            "edit package.json to include required Next.js runtime dependencies without removing build or dev scripts".to_string()
        }
        "nextjs_tailwind_contract" => match nextjs_tailwind_repair_target(failure).as_deref() {
            Some("package.json") => {
                "edit package.json to include the required Tailwind/PostCSS dependencies without removing Next.js runtime dependencies".to_string()
            }
            Some("postcss.config.js") => {
                "create or edit postcss.config.js so it uses the required Tailwind/PostCSS plugin configuration".to_string()
            }
            Some("tailwind.config.js") => {
                "create or edit tailwind.config.js so it covers the selected Next.js app and component paths".to_string()
            }
            _ => "fix the reported Tailwind/PostCSS profile contract before adding feature work".to_string(),
        },
        "nextjs_dev_port_drift" => {
            "edit package.json so scripts.dev runs next dev on the requested port".to_string()
        }
        "nextjs_build_script_drift" => {
            "edit package.json so scripts.build runs next build".to_string()
        }
        "nextjs_dependency_version_conflict" => {
            "edit package.json so next, react, react-dom, TypeScript, and React type versions use a stable compatible generated-app dependency family; use a stable TypeScript 5.x range such as ^5.4.0 and @types/react 18.x with React 18/Next.js 14; do not switch generated setup repair to latest packages; preserve scripts.build=next build; do not keep exact React pins below 18.2 with Next.js 14, TypeScript 6, exact TypeScript pins such as 5.0.0, or @types/react 19".to_string()
        }
        _ => "fix the reported profile contract before adding feature work".to_string(),
    }
}

fn profile_repair_focus(failure: &ProfileVerificationFailure) -> String {
    match failure.code.as_str() {
        "nextjs_integration_artifact_missing" => {
            let artifact = failure
                .paths
                .first()
                .map(String::as_str)
                .unwrap_or("missing artifact");
            format!("create missing explicit artifact {artifact} before route integration work")
        }
        "nextjs_route_not_integrated" => {
            let route = failure
                .paths
                .first()
                .map(String::as_str)
                .unwrap_or("selected route");
            let artifact = failure
                .paths
                .get(1)
                .map(String::as_str)
                .unwrap_or("unintegrated artifact");
            format!(
                "integrate {artifact} through selected route {route} before adding new feature work"
            )
        }
        "nextjs_app_root_ambiguous" => {
            "choose one Next.js app root and remove or migrate the other root before continuing"
                .to_string()
        }
        "nextjs_dependency_version_conflict" => {
            "fix the package.json dependency version contract before adding feature work"
                .to_string()
        }
        "nextjs_missing_dependency" => {
            "fix the package.json dependency contract before build or feature work".to_string()
        }
        "nextjs_tailwind_contract" => {
            let target = nextjs_tailwind_repair_target(failure)
                .unwrap_or_else(|| "Tailwind/PostCSS config".to_string());
            format!("fix the Tailwind/PostCSS contract in {target} before styling work")
        }
        "nextjs_alias_missing" => {
            "fix tsconfig path aliases before importing shared app modules".to_string()
        }
        "nextjs_dev_port_drift" => {
            "restore the requested Next.js dev port in package.json before continuing".to_string()
        }
        "nextjs_build_script_drift" => {
            "restore scripts.build=next build in package.json before continuing".to_string()
        }
        "nextjs_tsconfig_excludes_route" => {
            "fix tsconfig route-root coverage before build or route integration repair".to_string()
        }
        _ => profile_required_action(failure),
    }
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
        let packet = fs::read_to_string(saved.path).unwrap();
        assert!(packet.contains("standalone repair plan"));
        assert!(packet.contains("original ultra plan remains incomplete"));
        assert!(packet.contains("Contract evidence"));
        assert!(packet.contains("Recovery task"));
        assert!(packet.contains("success_check: npm run build"));
        assert!(packet.contains("guard: verifier"));
        assert!(packet.contains("Replan focus"));
        assert!(packet.contains("Address the contract evidence first"));
    }

    #[test]
    fn saves_profile_repair_prompt_with_profile_evidence() {
        let root = temp_workspace("profile-save");
        let context = sample_profile_context();

        let saved = save_profile_repair_prompt(&root, &context).unwrap();

        assert!(saved.path.exists());
        assert!(
            saved
                .relative_path
                .starts_with(".commandagent/repairs/repair-profile-")
        );
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
        let packet = fs::read_to_string(saved.path).unwrap();
        assert!(packet.contains("Repair failed profile verification"));
        assert!(packet.contains("Contract evidence"));
        assert!(packet.contains("Recovery task"));
        assert!(packet.contains("success_check: profile verification"));
        assert!(packet.contains("guard: profile_verification"));
        assert!(packet.contains("nextjs_dev_port_drift"));
        assert!(packet.contains("profile_contract_failed"));
        assert!(packet.contains("Repair focus"));
        assert!(packet.contains("Current blocker: nextjs_dev_port_drift"));
        assert!(packet.contains("profile.obligation.nextjs_dev_port_required"));
        assert!(packet.contains("standalone repair plan"));
        assert!(packet.contains("original ultra plan remains incomplete"));
    }

    #[test]
    fn profile_replan_packet_names_nextjs_route_integration_targets() {
        let mut context = sample_profile_context();
        context.profile_failures = vec![ProfileVerificationFailure {
            code: "nextjs_route_not_integrated".to_string(),
            message: "explicit artifact `app/hooks/useGame.ts` is not referenced from selected route `app/page.tsx`"
                .to_string(),
            paths: vec!["app/page.tsx".to_string(), "app/hooks/useGame.ts".to_string()],
        }];

        let packet = build_profile_replan_packet(&context);

        assert!(packet.contains("Route integration targets"));
        assert!(packet.contains("Recovery task"));
        assert!(packet.contains(
            "required_action: edit app/page.tsx so it imports or references app/hooks/useGame.ts"
        ));
        assert!(packet.contains("selected_route=app/page.tsx"));
        assert!(packet.contains("unintegrated_artifact=app/hooks/useGame.ts"));
        assert!(packet.contains("selected route imports or references"));
        assert!(packet.contains("repair_target: app/page.tsx"));
        assert!(packet.contains("candidate_artifacts: app/page.tsx, app/hooks/useGame.ts"));
        assert!(packet.contains("active_job: route_integration_repair"));
        assert!(packet.contains("repair_kind: route_integration_repair"));
        assert!(packet.contains("repair_action: connect_artifact_to_selected_route"));
        assert!(packet.contains("setup_implication: none"));
        assert!(packet.contains("rerun_authority: profile_verification, npm run build"));
        assert!(
            packet.contains("integrate app/hooks/useGame.ts through selected route app/page.tsx")
        );
    }

    #[test]
    fn profile_replan_packet_can_target_route_graph_component() {
        let mut context = sample_profile_context();
        context.profile_failures = vec![ProfileVerificationFailure {
            code: "nextjs_route_not_integrated".to_string(),
            message: "explicit artifact `app/hooks/useGame.ts` is not referenced from selected route graph rooted at `app/page.tsx`; repair target `app/components/GameBoard.tsx`"
                .to_string(),
            paths: vec![
                "app/page.tsx".to_string(),
                "app/hooks/useGame.ts".to_string(),
                "app/components/GameBoard.tsx".to_string(),
            ],
        }];

        let packet = build_profile_replan_packet(&context);

        assert!(packet.contains("selected_route=app/page.tsx"));
        assert!(packet.contains("unintegrated_artifact=app/hooks/useGame.ts"));
        assert!(packet.contains("repair_target: app/components/GameBoard.tsx"));
        assert!(packet.contains("required_action: edit app/components/GameBoard.tsx"));
        assert!(packet.contains(
            "candidate_artifacts: app/page.tsx, app/hooks/useGame.ts, app/components/GameBoard.tsx"
        ));
        assert!(packet.contains("active_job: route_integration_repair"));
        assert!(packet.contains("repair_kind: route_integration_repair"));
        assert!(packet.contains("repair_action: connect_artifact_to_selected_route"));
        assert!(packet.contains("Do not create an unrelated replacement app or route tree"));
    }

    #[test]
    fn profile_replan_packet_names_missing_integration_artifact_target() {
        let mut context = sample_profile_context();
        context.profile_failures = vec![ProfileVerificationFailure {
            code: "nextjs_integration_artifact_missing".to_string(),
            message: "explicit artifact `components/SpaceInvaders.tsx` does not exist before route integration with selected route `app/page.tsx`"
                .to_string(),
            paths: vec![
                "components/SpaceInvaders.tsx".to_string(),
                "app/page.tsx".to_string(),
            ],
        }];

        let packet = build_profile_replan_packet(&context);

        assert!(packet.contains("nextjs_integration_artifact_missing"));
        assert!(packet.contains(
            "required_action: create components/SpaceInvaders.tsx before editing selected route integration"
        ));
        assert!(packet.contains("repair_target: components/SpaceInvaders.tsx"));
        assert!(packet.contains("candidate_artifacts: components/SpaceInvaders.tsx, app/page.tsx"));
        assert!(packet.contains("active_job: integration_artifact_creation"));
        assert!(packet.contains("repair_kind: integration_artifact_creation"));
        assert!(packet.contains("repair_action: create_missing_integration_artifact"));
        assert!(packet.contains("rerun_authority: profile_verification"));
        assert!(packet.contains("missing artifact path exists, then profile verification"));
        assert!(packet.contains(
            "create missing explicit artifact components/SpaceInvaders.tsx before route integration work"
        ));
        assert!(!packet.contains("selected_route=app/page.tsx"));
        assert!(!packet.contains("unintegrated_artifact=components/SpaceInvaders.tsx"));
    }

    #[test]
    fn profile_replan_packet_names_dependency_version_repair_action() {
        let mut context = sample_profile_context();
        context.profile_failures = vec![ProfileVerificationFailure {
            code: "nextjs_dependency_version_conflict".to_string(),
            message: "Next.js 14 requires React peer versions compatible with 18.2 or newer; incompatible exact pins: react@18.0.0, react-dom@18.0.0"
                .to_string(),
            paths: vec!["package.json".to_string()],
        }];

        let packet = build_profile_replan_packet(&context);

        assert!(packet.contains("nextjs_dependency_version_conflict"));
        assert!(packet.contains("repair_target: package.json"));
        assert!(packet.contains(
            "edit package.json so next, react, react-dom, TypeScript, and React type versions use a stable compatible generated-app dependency family"
        ));
        assert!(packet.contains("use a stable TypeScript 5.x range such as ^5.4.0"));
        assert!(packet.contains("Do not switch generated setup repair to latest packages"));
        assert!(packet.contains("Do not rewrite scripts.build away from next build"));
        assert!(packet.contains("Do not keep exact React pins below 18.2 with Next.js 14"));
        assert!(packet.contains("TypeScript 6"));
        assert!(packet.contains("@types/react 19"));
        assert!(packet.contains("repair_kind: manifest_dependency_repair"));
        assert!(packet.contains("repair_action: add_manifest_dependency"));
        assert!(packet.contains("setup_implication: setup_after_manifest_repair_required"));
        assert!(
            packet.contains("rerun_authority: profile_verification, npm install, npm run build")
        );
        assert!(packet.contains(
            "observed=package.json pins Next.js and React peer versions that are incompatible"
        ));
    }

    #[test]
    fn profile_replan_packet_names_missing_dependency_repair_action() {
        let mut context = sample_profile_context();
        context.profile_failures = vec![ProfileVerificationFailure {
            code: "nextjs_missing_dependency".to_string(),
            message: "package.json is missing required dependencies: react-dom".to_string(),
            paths: vec!["package.json".to_string()],
        }];

        let packet = build_profile_replan_packet(&context);

        assert!(packet.contains("nextjs_missing_dependency"));
        assert!(packet.contains("repair_target: package.json"));
        assert!(packet.contains("repair_kind: manifest_dependency_repair"));
        assert!(packet.contains("repair_action: add_manifest_dependency"));
        assert!(packet.contains("setup_implication: setup_after_manifest_repair_required"));
        assert!(
            packet.contains("edit package.json to include required Next.js runtime dependencies")
        );
        assert!(packet.contains("fix the package.json dependency contract"));
        assert!(
            packet
                .contains("observed=package.json is missing required Next.js runtime dependencies")
        );
    }

    #[test]
    fn profile_replan_packet_targets_tailwind_config_layers() {
        let mut context = sample_profile_context();
        context.profile_failures = vec![
            ProfileVerificationFailure {
                code: "nextjs_tailwind_contract".to_string(),
                message: "tailwind.config.js is missing content paths".to_string(),
                paths: vec!["tailwind.config.js".to_string()],
            },
            ProfileVerificationFailure {
                code: "nextjs_tailwind_contract".to_string(),
                message: "postcss.config.js must include @tailwindcss/postcss".to_string(),
                paths: vec!["postcss.config.js".to_string()],
            },
            ProfileVerificationFailure {
                code: "nextjs_tailwind_contract".to_string(),
                message: "package.json is missing Tailwind dependency".to_string(),
                paths: vec!["package.json".to_string()],
            },
        ];

        let packet = build_profile_replan_packet(&context);

        assert!(packet.contains("repair_target: tailwind.config.js"));
        assert!(packet.contains("repair_target: postcss.config.js"));
        assert!(packet.contains("repair_target: package.json"));
        assert!(packet.contains("create or edit tailwind.config.js"));
        assert!(packet.contains("create or edit postcss.config.js"));
        assert!(packet.contains("edit package.json to include the required Tailwind/PostCSS"));
        assert!(packet.contains("repair_kind: tailwind_contract_repair"));
        assert!(packet.contains("repair_action: repair_tailwind_contract"));
        assert!(packet.contains("setup_implication: setup_after_manifest_repair_required"));
    }

    #[test]
    fn profile_replan_packet_targets_tsconfig_alias() {
        let mut context = sample_profile_context();
        context.profile_failures = vec![ProfileVerificationFailure {
            code: "nextjs_alias_missing".to_string(),
            message: "tsconfig.json is missing @/* alias".to_string(),
            paths: vec!["tsconfig.json".to_string()],
        }];

        let packet = build_profile_replan_packet(&context);

        assert!(packet.contains("nextjs_alias_missing"));
        assert!(packet.contains("repair_target: tsconfig.json"));
        assert!(packet.contains("repair_action: repair_tsconfig_alias"));
        assert!(packet.contains("edit tsconfig.json so @/* resolves"));
        assert!(packet.contains("fix tsconfig path aliases"));
    }

    #[test]
    fn profile_replan_packet_targets_dev_and_build_script_drift() {
        let mut context = sample_profile_context();
        context.profile_failures = vec![
            ProfileVerificationFailure {
                code: "nextjs_dev_port_drift".to_string(),
                message: "scripts.dev does not use port 3011".to_string(),
                paths: vec!["package.json".to_string()],
            },
            ProfileVerificationFailure {
                code: "nextjs_build_script_drift".to_string(),
                message: "scripts.build must be next build".to_string(),
                paths: vec!["package.json".to_string()],
            },
        ];

        let packet = build_profile_replan_packet(&context);

        assert!(packet.contains("nextjs_dev_port_drift"));
        assert!(packet.contains("nextjs_build_script_drift"));
        assert!(packet.contains("repair_target: package.json"));
        assert!(packet.contains("scripts.dev runs next dev on the requested port"));
        assert!(packet.contains("scripts.build runs next build"));
        assert!(packet.contains("restore scripts.build=next build"));
    }

    #[test]
    fn profile_contract_evidence_names_mixed_roots_without_arbitrary_target() {
        let context = ProfileRepairContext {
            profile_failures: vec![ProfileVerificationFailure {
                code: "nextjs_app_root_ambiguous".to_string(),
                message: "both root app and src/app routes are present".to_string(),
                paths: vec!["app/page.tsx".to_string(), "src/app/page.tsx".to_string()],
            }],
            ..sample_profile_context()
        };

        let packet = build_profile_replan_packet(&context);

        assert!(packet.contains("guard: profile_verification"));
        assert!(packet.contains(
            "failure_signature: profile_verification|setup-canvas|nextjs_app_root_ambiguous"
        ));
        assert!(packet.contains("candidate_artifacts: app/page.tsx, src/app/page.tsx"));
        assert!(packet.contains("observed=app/ and src/app/ routes are mixed"));
        assert!(packet.contains("choose one Next.js app root"));
        assert!(!packet.contains("repair_target: app/page.tsx"));
    }

    #[test]
    fn repair_prompt_contains_deterministic_evidence() {
        let prompt = build_repair_prompt(&sample_context());

        assert!(prompt.contains("Verification evidence"));
        assert!(prompt.contains("Recovery task"));
        assert!(prompt.contains("blocker: Verifier command failed: npm run build"));
        assert!(prompt.contains("success_check: npm run build"));
        assert!(prompt.contains("Contract evidence"));
        assert!(prompt.contains("guard: verifier"));
        assert!(prompt.contains("command: npm run build"));
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
        assert!(prompt.contains("Active profile contract facts to preserve"));
        assert!(prompt.contains("nextjs.app_root=src/app"));
        assert!(prompt.contains("nextjs_dev_port_required"));
        assert!(prompt.contains("preserve the listed invariant"));
    }

    #[test]
    fn contract_evidence_section_renders_multiple_records() {
        let context = RepairContext {
            contract_evidence: vec![
                ContractEvidence::new("tool_protocol")
                    .with_failed_step("create-game-canvas")
                    .with_violated_contract("tool_args_missing_required_field")
                    .with_reason_code("tool_args_missing_required_field")
                    .with_tool("Write")
                    .with_target_field("path"),
                ContractEvidence::new("step_policy")
                    .with_failed_step("inspect-source")
                    .with_violated_contract("read_only_step_mutation")
                    .with_tool("Write")
                    .with_required_action("move mutation into edit/create/repair step"),
            ],
            ..sample_context()
        };

        let packet = build_replan_packet(&context);

        assert!(packet.contains("- evidence 1:"));
        assert!(packet.contains("Recovery task"));
        assert!(packet.contains("Tool call violated schema for Write"));
        assert!(packet.contains("Step tool policy rejected Write"));
        assert!(packet.contains("guard: tool_protocol"));
        assert!(packet.contains("tool: Write"));
        assert!(packet.contains("- evidence 2:"));
        assert!(packet.contains("guard: step_policy"));
        assert!(packet.len() <= MAX_REPLAN_PACKET_BYTES);
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

    #[test]
    fn tool_protocol_correction_prompt_includes_schema_and_target() {
        let prompt = build_tool_protocol_correction_prompt(&ToolProtocolCorrectionContext {
            tool: "Write".to_string(),
            reason_code: "tool_args_missing_required_field".to_string(),
            missing_field: Some("path".to_string()),
            required_fields: vec!["path".to_string(), "content".to_string()],
            target_path: Some("tailwind.config.js".to_string()),
            diagnostic: "Write requires: path, content".to_string(),
        });

        assert!(prompt.contains("Tool protocol correction"));
        assert!(prompt.contains("Recovery task"));
        assert!(prompt.contains("required_action: Emit exactly one valid Write tool call"));
        assert!(prompt.contains("repair_target: tailwind.config.js"));
        assert!(prompt.contains("allowed_tools: Write"));
        assert!(prompt.contains("success_check: tool schema validation"));
        assert!(prompt.contains("Failed tool: Write"));
        assert!(prompt.contains("Reason: tool_args_missing_required_field"));
        assert!(prompt.contains("Missing required field: path"));
        assert!(prompt.contains("Required fields for Write: path, content"));
        assert!(prompt.contains("target_path_json=\"tailwind.config.js\""));
        assert!(prompt.contains("exactly one valid Write tool call"));
        assert!(prompt.contains("Do not answer in prose"));
        assert!(prompt.contains("runtime will rerun"));
        assert!(!prompt.contains("Gemini"));
        assert!(!prompt.contains("npm install"));
    }

    #[test]
    fn tool_protocol_correction_prompt_handles_invalid_json_without_target() {
        let prompt = build_tool_protocol_correction_prompt(&ToolProtocolCorrectionContext {
            tool: "Write".to_string(),
            reason_code: "tool_args_invalid_json".to_string(),
            missing_field: None,
            required_fields: Vec::new(),
            target_path: None,
            diagnostic: "Write arguments are not valid JSON".to_string(),
        });

        assert!(prompt.contains("Reason: tool_args_invalid_json"));
        assert!(prompt.contains("Required fields for Write: unknown"));
        assert!(!prompt.contains("Missing required field"));
        assert!(!prompt.contains("target_path_json"));
    }

    fn sample_context() -> RepairContext {
        RepairContext {
            step_id: "verify-build".to_string(),
            original_goal: "Build a Next.js app".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            step_instruction: "Run npm run build and fix failures.".to_string(),
            active_profile_contract_facts: vec![
                "nextjs.app_root=src/app".to_string(),
                "profile.obligation.nextjs_dev_port_required=port; paths=package.json; expected=scripts.dev contains next dev and 3011".to_string(),
            ],
            contract_evidence: vec![ContractEvidence::new("verifier")
                .with_failed_step("verify-build")
                .with_violated_contract("command_failed:1")
                .with_reason_code("command_failed:1")
                .with_command("npm run build")
                .with_required_action(
                    "fix the reported verifier failure before adding feature work",
                )
                .with_diagnostic("Type error: mismatch")],
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

    fn sample_profile_context() -> ProfileRepairContext {
        ProfileRepairContext {
            phase_id: "setup-canvas".to_string(),
            original_goal: "Create a Next.js app on port 3011".to_string(),
            phase_goal: "Create package.json and app/page.tsx".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            profile_failures: vec![ProfileVerificationFailure {
                code: "nextjs_dev_port_drift".to_string(),
                message: "package.json is missing scripts.dev for requested port 3011"
                    .to_string(),
                paths: vec!["package.json".to_string()],
            }],
            phase_contract_facts: vec![
                "profile.obligation.nextjs_dev_port_required=port; paths=package.json; expected=scripts.dev contains next dev and 3011".to_string(),
            ],
            profile_facts: vec!["nextjs.scripts.dev=next dev".to_string()],
            expected_paths: vec!["package.json".to_string(), "app/page.tsx".to_string()],
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
