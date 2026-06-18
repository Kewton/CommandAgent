use super::{PlanError, WorkIntent};

pub fn detect_work_intent(goal: &str) -> WorkIntent {
    let lower = goal.to_ascii_lowercase();
    if contains_any(&lower, &["investigate", "triage", "debug", "原因", "調査"]) {
        WorkIntent::Investigate
    } else if contains_any(&lower, &["document", "docs", "readme", "ドキュメント"]) {
        WorkIntent::Document
    } else if contains_any(
        &lower,
        &["fix", "modify", "update", "repair", "修正", "改修"],
    ) {
        WorkIntent::Modify
    } else if contains_any(&lower, &["data", "csv", "report", "分析", "整形"]) {
        WorkIntent::Data
    } else if contains_any(
        &lower,
        &["create", "build", "implement", "new", "作成", "開発"],
    ) {
        WorkIntent::New
    } else {
        WorkIntent::Unknown
    }
}

pub fn plan_generation_prompt(
    goal: &str,
    profile: &str,
    style: &str,
    intent: WorkIntent,
    required_artifacts: &[String],
) -> String {
    format!(
        "Create a small step plan for CommandAgent.\n\
Return only YAML in this schema:\n\
goal: <string>\n\
profile: <string>\n\
style: <string>\n\
intent: <new|modify|investigate|document|data|unknown>\n\
required_artifacts:\n\
  - <repository-relative final artifact path>\n\
steps:\n\
  - id: <short-slug>\n\
    kind: <inspect|create|edit|setup|verify|repair|report>\n\
    instruction: <concrete action for one minimal-loop turn>\n\
    expected_result: <pass|fail|unavailable>\n\
    expected_paths:\n\
      - <repository-relative file path>\n\
    verify:\n\
      - <local verification command>\n\
\n\
Rules:\n\
- Keep steps small and executable.\n\
- Use only canonical kind values in output: inspect, create, edit, setup, verify, repair, report.\n\
- Use kind inspect instead of read/analyze, and kind verify instead of shell/run.\n\
- Do not mix setup and final verification in the same step.\n\
- If a create/edit/setup step produces new expected_paths, keep verifier commands to direct existence/syntax checks. Put npm run build, cargo check/test/build, pytest, or other integration checks in a separate verify step.\n\
- File creation or modification steps must be executable with Write/Edit, not shell scaffolding.\n\
- Do not create directory-only steps; Write creates parent directories automatically.\n\
- Do not plan dependency installation as a required success step; dependency installs may be unavailable offline.\n\
- expected_paths must be actual required file outputs for this step, not package names, concepts, directories, optional inspection targets, or dependency caches.\n\
- If a step says a file may exist, such as \"if it exists\" or \"if present\", do not put that file in expected_paths and do not require test -f for it. Inspect it with Read/Glob only when present.\n\
- Inspect steps are observation-only: use verify: [] unless the step is intentionally asserting a required existing file listed in expected_paths. Do not use test -d/test -f to make optional discovery fatal.\n\
- Verifier commands must be one simple local check each; split shell chaining into separate list items and avoid unquoted &&, ||, or ;.\n\
- Prefer canonical verifier commands: test -f <path>, python -m py_compile <path.py>, python -m pytest <path-or-test>, cargo check, cargo test, npm run build, or grep -q <literal> <path>.\n\
- For source-code behavior, use build/test/check commands: Rust uses cargo check/cargo test; Next.js uses npm run build; Python/FastAPI uses python -m py_compile or pytest. Use grep -q only for literal documentation, data, or content requirements, not source-code semantics.\n\
- If no file path is expected for a step, use an empty list.\n\
- required_artifacts are final user-requested outputs and must be preserved exactly.\n\
- setup prepares local dependencies or configuration; verify runs deterministic checks and must not change files.\n\
- report steps explicitly report blockers such as dependency_missing or verifier_unavailable and should use verify: [].\n\
- Do not use true as a verifier; use an empty verify list for report-only steps.\n\
- Do not include tool-call fields such as action, path, content, old, or new in the plan.\n\
\n\
Goal: {goal}\n\
Profile: {profile}\n\
Style: {style}\n\
Intent: {intent}\n\
Required final artifacts:\n{artifacts}\n\
Profile guidance:\n{profile_guidance}",
        intent = intent.as_str(),
        artifacts = bullet_list(required_artifacts),
        profile_guidance = plan_profile_guidance(profile)
    )
}

fn plan_profile_guidance(profile: &str) -> &'static str {
    match profile {
        "rust" => {
            "For new Rust projects, plan explicit file creation for Cargo.toml and src/main.rs. Do not plan cargo init or cargo new shell scaffolding."
        }
        _ => "No additional profile-specific plan guidance.",
    }
}

pub fn invalid_plan_correction_prompt(
    original_goal: &str,
    invalid_plan: &str,
    error: &PlanError,
) -> String {
    format!(
        "The generated step plan is invalid and must be corrected.\n\
Original goal:\n{original_goal}\n\n\
Validation error:\n{error}\n\n\
Invalid plan:\n{invalid_plan}\n\n\
If the invalid plan includes tool-call fields such as action, path, content, old, or new, rewrite them into instruction and expected_paths fields.\n\
Return only corrected YAML using the required CommandAgent step plan schema."
    )
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
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
