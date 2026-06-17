use crate::agent::step_runner::{StepKind, StepPlan};
use std::path::Path;

pub fn lint_step_plan(plan: &StepPlan) -> Result<(), PlanLintError> {
    lint_step_plan_with_workspace(plan, None)
}

pub fn lint_step_plan_with_workspace(
    plan: &StepPlan,
    cwd: Option<&Path>,
) -> Result<(), PlanLintError> {
    for step in &plan.steps {
        for path in &plan.required_artifacts {
            lint_expected_path(path)?;
        }
        for path in &step.expected_paths {
            lint_expected_path(path)?;
        }
        for command in &step.verify {
            lint_verifier_command(&step.id, command)?;
        }
        lint_step_instruction(&step.id, step.kind, &step.instruction, &step.expected_paths)?;
        lint_optional_inspection_paths(
            &step.id,
            step.kind,
            &step.instruction,
            &step.expected_paths,
        )?;
        lint_setup_verify_boundary(
            &step.id,
            step.kind,
            &step.instruction,
            !step.expected_paths.is_empty(),
            paths_exist(cwd, &step.expected_paths),
        )?;
        lint_profile_scaffolding(plan.profile.as_str(), &step.id, &step.instruction)?;
    }
    Ok(())
}

fn lint_expected_path(path: &str) -> Result<(), PlanLintError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(PlanLintError::InvalidExpectedPath {
            path: path.to_string(),
            reason: "path is empty".to_string(),
        });
    }
    let path_obj = Path::new(trimmed);
    if path_obj.is_absolute() || trimmed.contains("..") {
        return Err(PlanLintError::InvalidExpectedPath {
            path: path.to_string(),
            reason: "path must be repository-relative and cannot contain parent traversal"
                .to_string(),
        });
    }
    if trimmed.contains(':') || trimmed.starts_with("$.") {
        return Err(PlanLintError::InvalidExpectedPath {
            path: path.to_string(),
            reason: "expected_paths must be file paths, not JSON/property selectors".to_string(),
        });
    }
    if trimmed.contains(" or ") || trimmed.contains("||") {
        return Err(PlanLintError::InvalidExpectedPath {
            path: path.to_string(),
            reason: "expected_paths must be one concrete file, not alternatives".to_string(),
        });
    }
    if trimmed.contains('*') || trimmed.contains('?') || trimmed.contains('{') {
        return Err(PlanLintError::InvalidExpectedPath {
            path: path.to_string(),
            reason: "expected_paths must be concrete files, not glob patterns".to_string(),
        });
    }
    if is_dependency_cache_path(trimmed) {
        return Err(PlanLintError::InvalidExpectedPath {
            path: path.to_string(),
            reason: "expected_paths must not name generated dependency caches".to_string(),
        });
    }
    if looks_like_version(trimmed) {
        return Err(PlanLintError::InvalidExpectedPath {
            path: path.to_string(),
            reason: "expected_paths must be file paths, not version strings".to_string(),
        });
    }
    if !looks_like_file_path(trimmed) {
        return Err(PlanLintError::InvalidExpectedPath {
            path: path.to_string(),
            reason: "expected_paths must name concrete files".to_string(),
        });
    }
    Ok(())
}

fn lint_step_instruction(
    step_id: &str,
    kind: StepKind,
    instruction: &str,
    expected_paths: &[String],
) -> Result<(), PlanLintError> {
    let lower = instruction.to_ascii_lowercase();
    if contains_any(
        &lower,
        &[
            "npm install",
            "npm ci",
            "pnpm install",
            "pip install",
            "python -m pip",
        ],
    ) {
        return Err(PlanLintError::InvalidStepInstruction {
            step_id: step_id.to_string(),
            reason: "dependency installation must not be a required success step; report dependency_missing when offline".to_string(),
        });
    }
    if matches!(kind, StepKind::Setup | StepKind::Create)
        && expected_paths.is_empty()
        && contains_any(&lower, &["create", "make", "initialize", "init"])
        && contains_any(
            &lower,
            &[" directory", " directories", " folder", " folders"],
        )
    {
        return Err(PlanLintError::InvalidStepInstruction {
            step_id: step_id.to_string(),
            reason: "directory-only steps are unnecessary because Write creates parent directories automatically".to_string(),
        });
    }
    Ok(())
}

fn lint_optional_inspection_paths(
    step_id: &str,
    kind: StepKind,
    instruction: &str,
    expected_paths: &[String],
) -> Result<(), PlanLintError> {
    if !matches!(kind, StepKind::Inspect) || expected_paths.is_empty() {
        return Ok(());
    }
    let lower = instruction.to_ascii_lowercase();
    if contains_any(
        &lower,
        &[
            "if it exists",
            "if exists",
            "if present",
            "if available",
            "when present",
            "if any",
            "if there is",
            "if there are",
        ],
    ) {
        return Err(PlanLintError::InvalidStepInstruction {
            step_id: step_id.to_string(),
            reason: "optional inspection targets must not be placed in expected_paths; use Read/Glob inspection and an empty expected_paths list"
                .to_string(),
        });
    }
    Ok(())
}

fn lint_verifier_command(step_id: &str, command: &str) -> Result<(), PlanLintError> {
    let trimmed = command.trim();
    if contains_unquoted_shell_control(trimmed) {
        return Err(PlanLintError::InvalidVerifierCommand {
            step_id: step_id.to_string(),
            command: command.to_string(),
            reason: "verifier commands must be one simple local check; split shell chaining into separate commands".to_string(),
        });
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower == "true" {
        return Err(PlanLintError::InvalidVerifierCommand {
            step_id: step_id.to_string(),
            command: command.to_string(),
            reason: "no-op verifier is not allowed; use an empty verify list for report-only steps"
                .to_string(),
        });
    }
    if is_source_grep_verifier(trimmed) {
        return Err(PlanLintError::InvalidVerifierCommand {
            step_id: step_id.to_string(),
            command: command.to_string(),
            reason: "source-code behavior must be verified with build/test/check commands; reserve grep -q for literal docs, data, or content checks"
                .to_string(),
        });
    }
    if starts_with_any(
        &lower,
        &[
            "npm install",
            "npm ci",
            "pnpm install",
            "pip install",
            "python -m pip",
            "cargo add",
            "cargo update",
        ],
    ) {
        return Err(PlanLintError::InvalidVerifierCommand {
            step_id: step_id.to_string(),
            command: command.to_string(),
            reason: "verifier commands must not install dependencies or mutate project state"
                .to_string(),
        });
    }
    Ok(())
}
fn contains_unquoted_shell_control(command: &str) -> bool {
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    let mut chars = command.chars().peekable();

    while let Some(ch) = chars.next() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && in_double {
            escaped = true;
            continue;
        }
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            ';' if !in_single && !in_double => return true,
            '&' if !in_single && !in_double && chars.peek() == Some(&'&') => return true,
            '|' if !in_single && !in_double && chars.peek() == Some(&'|') => return true,
            _ => {}
        }
    }

    false
}

fn is_source_grep_verifier(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    if !lower.starts_with("grep -q ") {
        return false;
    }
    let Some(path) = command.split_whitespace().last() else {
        return false;
    };
    let path = path.trim_matches(|ch| ch == '\'' || ch == '"');
    is_source_file_path(path)
}

fn is_source_file_path(path: &str) -> bool {
    matches!(
        Path::new(path).extension().and_then(|ext| ext.to_str()),
        Some(
            "c" | "cc"
                | "cpp"
                | "cs"
                | "go"
                | "h"
                | "hpp"
                | "java"
                | "js"
                | "jsx"
                | "kt"
                | "php"
                | "py"
                | "rb"
                | "rs"
                | "swift"
                | "ts"
                | "tsx"
        )
    )
}

fn lint_setup_verify_boundary(
    step_id: &str,
    kind: StepKind,
    instruction: &str,
    has_expected_paths: bool,
    expected_paths_already_exist: bool,
) -> Result<(), PlanLintError> {
    if matches!(kind, StepKind::Verify | StepKind::Report) {
        return Ok(());
    }
    let lower = instruction.to_ascii_lowercase();
    let setup = contains_any(
        &lower,
        &[
            "create ",
            "write ",
            "edit ",
            "add ",
            "install ",
            "scaffold ",
            "configure ",
            "implement ",
        ],
    );
    let verify = contains_any(
        &lower,
        &[
            "verify ",
            "validate ",
            "run build",
            "npm run build",
            "cargo check",
            "cargo test",
            "cargo build",
            "pytest",
            "test ",
        ],
    );

    let explicit_sequence = contains_any(
        &lower,
        &[
            " and run ",
            " then run ",
            " and verify ",
            " then verify ",
            " and validate ",
            " then validate ",
            " and test ",
            " then test ",
            " and build ",
            " then build ",
        ],
    );

    if has_expected_paths && !expected_paths_already_exist && setup && verify && explicit_sequence {
        return Err(PlanLintError::MixedSetupAndVerify {
            step_id: step_id.to_string(),
        });
    }
    Ok(())
}

fn paths_exist(cwd: Option<&Path>, paths: &[String]) -> bool {
    let Some(cwd) = cwd else {
        return false;
    };
    !paths.is_empty() && paths.iter().all(|path| cwd.join(path).exists())
}

fn lint_profile_scaffolding(
    profile: &str,
    step_id: &str,
    instruction: &str,
) -> Result<(), PlanLintError> {
    let lower = instruction.to_ascii_lowercase();
    if profile == "rust" && contains_any(&lower, &["cargo init", "cargo new"]) {
        return Err(PlanLintError::ShellScaffold {
            step_id: step_id.to_string(),
            command: "cargo init/new".to_string(),
            guidance: "create Cargo.toml and src/main.rs with Write/Edit".to_string(),
        });
    }
    Ok(())
}

fn is_dependency_cache_path(path: &str) -> bool {
    path == "node_modules"
        || path.starts_with("node_modules/")
        || path == ".venv"
        || path.starts_with(".venv/")
        || path == "target"
        || path.starts_with("target/")
}

fn starts_with_any(value: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| value.starts_with(prefix))
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn looks_like_file_path(path: &str) -> bool {
    if path.contains('/') {
        return !path.ends_with('/');
    }
    if matches!(
        path,
        "Dockerfile" | "Makefile" | "README" | "LICENSE" | "Cargo.lock"
    ) {
        return true;
    }
    let Some(extension) = Path::new(path).extension().and_then(|ext| ext.to_str()) else {
        return false;
    };
    matches!(
        extension,
        "cjs"
            | "css"
            | "go"
            | "html"
            | "js"
            | "json"
            | "jsx"
            | "lock"
            | "md"
            | "mjs"
            | "py"
            | "rs"
            | "toml"
            | "ts"
            | "tsx"
            | "txt"
            | "yaml"
            | "yml"
    )
}

fn looks_like_version(value: &str) -> bool {
    let mut saw_dot = false;
    let mut saw_digit = false;
    for ch in value.chars() {
        if ch == '.' {
            saw_dot = true;
        } else if ch.is_ascii_digit() {
            saw_digit = true;
        } else {
            return false;
        }
    }
    saw_dot && saw_digit
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanLintError {
    InvalidExpectedPath {
        path: String,
        reason: String,
    },
    MixedSetupAndVerify {
        step_id: String,
    },
    ShellScaffold {
        step_id: String,
        command: String,
        guidance: String,
    },
    InvalidVerifierCommand {
        step_id: String,
        command: String,
        reason: String,
    },
    InvalidStepInstruction {
        step_id: String,
        reason: String,
    },
}

impl std::fmt::Display for PlanLintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidExpectedPath { path, reason } => {
                write!(f, "invalid expected path `{path}`: {reason}")
            }
            Self::MixedSetupAndVerify { step_id } => write!(
                f,
                "step `{step_id}` mixes setup/editing work with verification; split it into separate steps"
            ),
            Self::ShellScaffold {
                step_id,
                command,
                guidance,
            } => write!(
                f,
                "step `{step_id}` uses shell scaffolding `{command}`; {guidance}"
            ),
            Self::InvalidVerifierCommand {
                step_id,
                command,
                reason,
            } => write!(
                f,
                "step `{step_id}` has invalid verifier command `{command}`: {reason}"
            ),
            Self::InvalidStepInstruction { step_id, reason } => {
                write!(f, "step `{step_id}` has invalid instruction: {reason}")
            }
        }
    }
}

impl std::error::Error for PlanLintError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::step_runner::{ExpectedResult, StepKind, StepPlan, StepPlanStep, WorkIntent};
    use std::fs;

    #[test]
    fn accepts_generic_file_paths() {
        let plan = plan_with_paths("generic", vec!["README.md", "src/main.rs"]);

        lint_step_plan(&plan).unwrap();
    }

    #[test]
    fn accepts_nextjs_verification_step_without_naming_every_file_in_instruction() {
        let plan = StepPlan {
            goal: "verify app".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::New,
            required_artifacts: Vec::new(),
            steps: vec![StepPlanStep {
                id: "final-build-check".to_string(),
                kind: StepKind::Verify,
                instruction: "Run npm run build.".to_string(),
                expected_result: ExpectedResult::Pass,
                expected_paths: vec![
                    "package.json".to_string(),
                    "app/page.tsx".to_string(),
                    "next.config.js".to_string(),
                ],
                verify: vec!["npm run build".to_string()],
            }],
        };

        lint_step_plan(&plan).unwrap();
    }

    #[test]
    fn rejects_package_identifiers_as_expected_paths() {
        let plan = plan_with_paths("nextjs", vec!["next"]);

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidExpectedPath { .. }));
    }

    #[test]
    fn rejects_json_property_paths_as_expected_paths() {
        let plan = plan_with_paths("nextjs", vec!["package.json:scripts.build"]);

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidExpectedPath { .. }));
    }

    #[test]
    fn rejects_globs_as_expected_paths() {
        let plan = plan_with_paths("python", vec!["app/routes/*.py"]);

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidExpectedPath { .. }));
    }

    #[test]
    fn rejects_alternative_paths_as_expected_paths() {
        let plan = plan_with_paths("nextjs", vec!["app/layout.tsx or app/layout.ts"]);

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidExpectedPath { .. }));
    }

    #[test]
    fn rejects_version_strings_as_expected_paths() {
        let plan = plan_with_paths("nextjs", vec!["18.2.0"]);

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidExpectedPath { .. }));
    }

    #[test]
    fn rejects_dependency_cache_paths_as_expected_paths() {
        let plan = plan_with_paths("nextjs", vec!["node_modules/.package-lock.json"]);

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidExpectedPath { .. }));
    }

    #[test]
    fn rejects_compound_verifier_commands() {
        let mut plan = plan_with_paths("generic", vec!["README.md"]);
        plan.steps[0].verify = vec!["test -f README.md && grep -q usage README.md".to_string()];

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidVerifierCommand { .. }));
    }

    #[test]
    fn accepts_quoted_semicolon_in_python_verifier() {
        let mut plan = plan_with_paths("python", vec!["app/main.py"]);
        plan.steps[0].verify =
            vec![r#"python -c "import ast; ast.parse(open('app/main.py').read())""#.to_string()];

        lint_step_plan(&plan).unwrap();
    }

    #[test]
    fn rejects_unquoted_semicolon_in_verifier() {
        let mut plan = plan_with_paths("generic", vec!["README.md"]);
        plan.steps[0].verify = vec!["test -f README.md; cat README.md".to_string()];

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidVerifierCommand { .. }));
    }

    #[test]
    fn rejects_unquoted_or_in_verifier() {
        let mut plan = plan_with_paths("generic", vec!["README.md"]);
        plan.steps[0].verify = vec!["test -f README.md || cat README.md".to_string()];

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidVerifierCommand { .. }));
    }

    #[test]
    fn rejects_noop_true_verifier() {
        let mut plan = plan_with_paths("generic", vec!["README.md"]);
        plan.steps[0].verify = vec!["true".to_string()];

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidVerifierCommand { .. }));
    }

    #[test]
    fn rejects_optional_inspection_expected_paths() {
        let mut plan = plan_with_paths("rust", vec!["src/lib.rs"]);
        plan.steps[0].kind = StepKind::Inspect;
        plan.steps[0].instruction = "Read src/lib.rs if it exists.".to_string();
        plan.steps[0].verify = vec!["test -f src/lib.rs".to_string()];

        let err = lint_step_plan(&plan).unwrap_err();

        assert_eq!(
            err,
            PlanLintError::InvalidStepInstruction {
                step_id: "step".to_string(),
                reason: "optional inspection targets must not be placed in expected_paths; use Read/Glob inspection and an empty expected_paths list".to_string(),
            }
        );
    }

    #[test]
    fn rejects_source_code_grep_verifier() {
        let mut plan = plan_with_paths("rust", vec!["src/main.rs"]);
        plan.steps[0].verify = vec![r##"grep -q "#\[derive(Parser)\]" src/main.rs"##.to_string()];

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidVerifierCommand { .. }));
    }

    #[test]
    fn accepts_literal_grep_for_docs() {
        let mut plan = plan_with_paths("docs", vec!["README.md"]);
        plan.steps[0].verify = vec!["grep -q Usage README.md".to_string()];

        lint_step_plan(&plan).unwrap();
    }

    #[test]
    fn rejects_dependency_install_steps() {
        let mut plan = plan_with_paths("nextjs", vec!["package.json"]);
        plan.steps[0].instruction = "Run npm install to download dependencies.".to_string();
        plan.steps[0].expected_paths.clear();

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidStepInstruction { .. }));
    }

    #[test]
    fn rejects_directory_only_steps() {
        let mut plan = plan_with_paths("rust", vec!["Cargo.toml"]);
        plan.steps[0].kind = StepKind::Setup;
        plan.steps[0].instruction = "Create the src directory.".to_string();
        plan.steps[0].expected_paths.clear();

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidStepInstruction { .. }));
    }

    #[test]
    fn rejects_obvious_setup_and_verify_mix() {
        let plan = StepPlan {
            goal: "build app".to_string(),
            profile: "generic".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::New,
            required_artifacts: Vec::new(),
            steps: vec![StepPlanStep {
                id: "create-and-build".to_string(),
                kind: StepKind::Create,
                instruction: "Create app/page.tsx and run build.".to_string(),
                expected_result: ExpectedResult::Pass,
                expected_paths: vec!["app/page.tsx".to_string()],
                verify: vec!["npm run build".to_string()],
            }],
        };

        let err = lint_step_plan(&plan).unwrap_err();

        assert_eq!(
            err,
            PlanLintError::MixedSetupAndVerify {
                step_id: "create-and-build".to_string()
            }
        );
    }

    #[test]
    fn workspace_lint_accepts_existing_paths_in_verify_like_step() {
        let root = temp_workspace("existing-paths");
        fs::create_dir_all(root.join("app")).unwrap();
        fs::write(root.join("package.json"), "{}").unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "export default function Page() {}",
        )
        .unwrap();
        let plan = StepPlan {
            goal: "verify app".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::Modify,
            required_artifacts: Vec::new(),
            steps: vec![StepPlanStep {
                id: "install-and-build".to_string(),
                kind: StepKind::Setup,
                instruction: "Install dependencies if needed and verify npm run build.".to_string(),
                expected_result: ExpectedResult::Pass,
                expected_paths: vec!["package.json".to_string(), "app/page.tsx".to_string()],
                verify: vec!["npm run build".to_string()],
            }],
        };

        lint_step_plan_with_workspace(&plan, Some(&root)).unwrap();
    }

    #[test]
    fn rejects_rust_cargo_init_scaffolding() {
        let plan = StepPlan {
            goal: "create rust app".to_string(),
            profile: "rust".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::New,
            required_artifacts: Vec::new(),
            steps: vec![StepPlanStep {
                id: "init-project".to_string(),
                kind: StepKind::Create,
                instruction: "Initialize a new Rust project using cargo init.".to_string(),
                expected_result: ExpectedResult::Pass,
                expected_paths: vec!["Cargo.toml".to_string(), "src/main.rs".to_string()],
                verify: vec!["test -f Cargo.toml".to_string()],
            }],
        };

        let err = lint_step_plan(&plan).unwrap_err();

        assert_eq!(
            err,
            PlanLintError::ShellScaffold {
                step_id: "init-project".to_string(),
                command: "cargo init/new".to_string(),
                guidance: "create Cargo.toml and src/main.rs with Write/Edit".to_string()
            }
        );
    }

    fn plan_with_paths(profile: &str, paths: Vec<&str>) -> StepPlan {
        StepPlan {
            goal: "goal".to_string(),
            profile: profile.to_string(),
            style: "default".to_string(),
            intent: WorkIntent::Unknown,
            required_artifacts: Vec::new(),
            steps: vec![StepPlanStep {
                id: "step".to_string(),
                kind: StepKind::Create,
                instruction: "Create files.".to_string(),
                expected_result: ExpectedResult::Pass,
                expected_paths: paths.into_iter().map(ToString::to_string).collect(),
                verify: Vec::new(),
            }],
        }
    }

    fn temp_workspace(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "commandagent-plan-lint-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
