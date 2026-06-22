pub(crate) mod active_job;
pub(crate) mod artifact_completion;
pub(crate) mod artifact_graph;
pub(crate) mod artifact_ledger;
pub(crate) mod artifact_ownership;
pub(crate) mod completion_evidence;
pub mod correction_evidence;
pub(crate) mod deliverable_obligation;
pub mod evidence;
pub(crate) mod evidence_authority;
pub(crate) mod evidence_binding;
pub(crate) mod evidence_producer;
pub(crate) mod failure_observation;
pub(crate) mod integrity_guard;
pub(crate) mod mechanical_repair;
pub mod plan_lint;
pub(crate) mod profile_artifact;
pub mod profiles;
pub(crate) mod recovery_contract;
pub(crate) mod recovery_orchestration;
pub(crate) mod recovery_policy;
pub mod recovery_task;
pub mod repair;
pub(crate) mod repair_action_plan;
pub(crate) mod repair_brief;
pub(crate) mod repair_job;
pub mod runtime;
pub(crate) mod semantic_failure;
pub(crate) mod setup_artifact_validation;
pub(crate) mod setup_lifecycle;
pub(crate) mod target_admission;
pub(crate) mod task_contract;
pub mod ultra_plan;
pub mod ultra_run;
pub(crate) mod verifier_diagnostic;
pub(crate) mod verifier_selection;
pub mod verify;
pub(crate) mod workspace_scope;
pub(crate) mod workspace_snapshot;

mod plan;
mod plan_error;
mod plan_input;
mod plan_prompt;
mod plan_store;
mod plan_yaml;
mod yaml_scalar;

pub use plan::{ExpectedResult, StepKind, StepPlan, StepPlanStep, WorkIntent};
pub use plan_error::PlanError;
pub use plan_prompt::{detect_work_intent, invalid_plan_correction_prompt, plan_generation_prompt};
pub use plan_store::save_step_plan;
pub use plan_yaml::{
    extract_plan_from_response, parse_step_plan_yaml, render_step_plan_yaml, validate_step_plan,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn renders_and_loads_step_plan_yaml() {
        let plan = sample_plan();

        let yaml = render_step_plan_yaml(&plan);
        let parsed = parse_step_plan_yaml(&yaml).unwrap();

        assert_eq!(parsed, plan);
    }

    #[test]
    fn extracts_plan_from_yaml_code_fence() {
        let yaml = render_step_plan_yaml(&sample_plan());
        let response = format!("```yaml\n{yaml}```");

        let parsed = extract_plan_from_response(&response).unwrap();

        assert_eq!(parsed.goal, "Build docs");
        assert_eq!(parsed.steps.len(), 1);
    }

    #[test]
    fn accepts_inline_empty_step_lists() {
        let yaml = "goal: \"Run check\"\nprofile: \"python\"\nstyle: \"default\"\nsteps:\n  - id: \"inspect\"\n    instruction: \"Inspect workspace.\"\n    expected_paths: []\n    verify: []\n";

        let plan = parse_step_plan_yaml(yaml).unwrap();

        assert!(plan.steps[0].expected_paths.is_empty());
        assert!(plan.steps[0].verify.is_empty());
    }

    #[test]
    fn accepts_common_model_step_indentation_drift() {
        let yaml = "goal: \"Create Rust CLI\"\nprofile: \"rust\"\nstyle: \"default\"\nsteps:\n- id: create-cargo-toml\n  instruction: \"Create Cargo.toml.\"\n  expected_paths:\n    - Cargo.toml\n  verify:\n    - test -f Cargo.toml\n";

        let plan = parse_step_plan_yaml(yaml).unwrap();

        assert_eq!(plan.steps[0].id, "create-cargo-toml");
        assert_eq!(plan.steps[0].expected_paths, vec!["Cargo.toml"]);
        assert_eq!(plan.steps[0].verify, vec!["test -f Cargo.toml"]);
    }

    #[test]
    fn accepts_unindented_step_fields() {
        let yaml = "goal: \"Create schemas\"\nprofile: \"python\"\nstyle: \"default\"\nintent: modify\nrequired_artifacts:\n- app/main.py\nsteps:\n- id: create-schemas\nkind: create\ninstruction: Create app/schemas.py.\nexpected_result: pass\nexpected_paths:\n- app/schemas.py\nverify:\n- test -f app/schemas.py\n";

        let plan = parse_step_plan_yaml(yaml).unwrap();

        assert_eq!(plan.required_artifacts, vec!["app/main.py"]);
        assert_eq!(plan.steps[0].kind, StepKind::Create);
        assert_eq!(plan.steps[0].expected_paths, vec!["app/schemas.py"]);
    }

    #[test]
    fn accepts_literal_block_scalar_instruction() {
        let yaml = r#"
goal: Create Next app
profile: nextjs
style: default
steps:
  - id: create-package-json
    kind: create
    instruction: |
      Create the package.json file and configure the dev script to use port 3011.
      The package.json work must mention next, react, and react-dom.
    expected_paths:
      - package.json
    verify:
      - test -f package.json
"#;

        let plan = parse_step_plan_yaml(yaml).unwrap();

        assert_eq!(
            plan.steps[0].instruction,
            "Create the package.json file and configure the dev script to use port 3011.\nThe package.json work must mention next, react, and react-dom."
        );
    }

    #[test]
    fn accepts_folded_block_scalar_instruction() {
        let yaml = r#"
goal: Create docs
profile: docs
style: default
steps:
  - id: write-readme
    kind: create
    instruction: >
      Create README.md with usage notes.
      Include setup instructions.
    expected_paths:
      - README.md
    verify:
      - test -f README.md
"#;

        let plan = parse_step_plan_yaml(yaml).unwrap();

        assert_eq!(
            plan.steps[0].instruction,
            "Create README.md with usage notes. Include setup instructions."
        );
    }

    #[test]
    fn accepts_folded_strip_block_scalar_instruction() {
        let yaml = r#"
goal: Create Next app
profile: nextjs
style: default
steps:
  - id: create-package-json
    kind: create
    instruction: >-
      Create package.json.
      Include next, react, and react-dom.
    expected_paths:
      - package.json
    verify:
      - test -f package.json
"#;

        let plan = parse_step_plan_yaml(yaml).unwrap();
        let rendered = render_step_plan_yaml(&plan);
        let reparsed = parse_step_plan_yaml(&rendered).unwrap();

        assert_eq!(
            plan.steps[0].instruction,
            "Create package.json. Include next, react, and react-dom."
        );
        assert_eq!(reparsed, plan);
    }

    #[test]
    fn accepts_literal_block_scalar_goal_and_canonicalizes() {
        let yaml = r#"
goal: |
  Build docs.
  Keep the output concise.
profile: docs
style: default
steps:
  - id: write-readme
    instruction: Create README.md.
    expected_paths:
      - README.md
    verify:
      - test -f README.md
"#;

        let plan = parse_step_plan_yaml(yaml).unwrap();
        let rendered = render_step_plan_yaml(&plan);
        let reparsed = parse_step_plan_yaml(&rendered).unwrap();

        assert_eq!(plan.goal, "Build docs.\nKeep the output concise.");
        assert_eq!(reparsed, plan);
        assert!(rendered.contains("goal: \"Build docs.\\nKeep the output concise.\""));
    }

    #[test]
    fn accepts_literal_strip_block_scalar_goal_and_canonicalizes() {
        let yaml = r#"
goal: |-
  Build docs.
  Keep the output concise.
profile: docs
style: default
steps:
  - id: write-readme
    instruction: Create README.md.
    expected_paths:
      - README.md
    verify:
      - test -f README.md
"#;

        let plan = parse_step_plan_yaml(yaml).unwrap();
        let rendered = render_step_plan_yaml(&plan);
        let reparsed = parse_step_plan_yaml(&rendered).unwrap();

        assert_eq!(plan.goal, "Build docs.\nKeep the output concise.");
        assert_eq!(reparsed, plan);
    }

    #[test]
    fn accepts_common_expected_result_aliases_but_renders_canonical_values() {
        let yaml = "goal: \"Check availability\"
profile: \"generic\"
style: \"default\"
steps:
- id: inspect-tooling
kind: inspect
instruction: Inspect whether local tooling is available.
expected_result: available
expected_paths: []
verify: []
- id: red-test
kind: verify
instruction: Run an expected failing check.
expected_result: expected_failure
expected_paths: []
verify:
- cargo test
- id: dependency-blocker
kind: report
instruction: Report unavailable dependency.
expected_result: not_available
expected_paths: []
verify: []
";

        let plan = parse_step_plan_yaml(yaml).unwrap();
        let rendered = render_step_plan_yaml(&plan);

        assert_eq!(plan.steps[0].expected_result, ExpectedResult::Pass);
        assert_eq!(plan.steps[1].expected_result, ExpectedResult::Fail);
        assert_eq!(plan.steps[2].expected_result, ExpectedResult::Unavailable);
        assert!(rendered.contains("expected_result: \"pass\""));
        assert!(rendered.contains("expected_result: \"fail\""));
        assert!(rendered.contains("expected_result: \"unavailable\""));
        assert!(!rendered.contains("expected_result: \"available\""));
        assert!(!rendered.contains("expected_failure"));
        assert!(!rendered.contains("not_available"));
    }

    #[test]
    fn accepts_common_step_kind_aliases_but_renders_canonical_values() {
        let yaml = "goal: \"Inspect and verify\"\nprofile: \"rust\"\nstyle: \"default\"\nsteps:\n- id: inspect-source\nkind: read\ninstruction: Read src/main.rs.\nexpected_paths:\n- src/main.rs\nverify:\n- test -f src/main.rs\n- id: analyze-source\nkind: analyze\ninstruction: Analyze source layout.\nexpected_paths: []\nverify: []\n- id: run-tests\nkind: shell\ninstruction: Run cargo test.\nexpected_paths: []\nverify:\n- cargo test\n";

        let plan = parse_step_plan_yaml(yaml).unwrap();
        let rendered = render_step_plan_yaml(&plan);

        assert_eq!(plan.steps[0].kind, StepKind::Inspect);
        assert_eq!(plan.steps[1].kind, StepKind::Inspect);
        assert_eq!(plan.steps[2].kind, StepKind::Verify);
        assert!(rendered.contains("kind: \"inspect\""));
        assert!(rendered.contains("kind: \"verify\""));
        assert!(!rendered.contains("kind: \"shell\""));
    }

    #[test]
    fn accepts_common_model_list_item_indentation_drift() {
        let yaml = "goal: \"Create Next app\"\nprofile: \"nextjs\"\nstyle: \"default\"\nsteps:\n  - id: create-files\n    instruction: \"Create Next.js files.\"\n    expected_paths:\n  - package.json\n  - app/page.tsx\n    verify:\n  - cat package.json\n";

        let plan = parse_step_plan_yaml(yaml).unwrap();

        assert_eq!(
            plan.steps[0].expected_paths,
            vec!["package.json", "app/page.tsx"]
        );
        assert_eq!(plan.steps[0].verify, vec!["cat package.json"]);
    }

    #[test]
    fn accepts_arbitrary_list_item_indentation_drift() {
        let yaml = "goal: \"Create Python app\"\nprofile: \"python\"\nstyle: \"default\"\nsteps:\n- id: create-init\n  instruction: Create init files.\n  expected_paths:\n     - app/__init__.py\n     - tests/__init__.py\n  verify:\n     - test -f app/__init__.py\n";

        let plan = parse_step_plan_yaml(yaml).unwrap();

        assert_eq!(
            plan.steps[0].expected_paths,
            vec!["app/__init__.py", "tests/__init__.py"]
        );
    }

    #[test]
    fn accepts_three_space_list_item_indentation_drift() {
        let yaml = r#"
goal: Build app
profile: nextjs
style: default
steps:
  - id: create-files
    instruction: Create files.
    expected_paths:
   - package.json
   - app/page.tsx
    verify: []
"#;

        let plan = parse_step_plan_yaml(yaml).unwrap();

        assert_eq!(
            plan.steps[0].expected_paths,
            vec!["package.json", "app/page.tsx"]
        );
    }

    #[test]
    fn ignores_common_model_action_annotation() {
        let yaml = "goal: \"Create docs\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: create-readme\n    action: write\n    instruction: \"Create README.md.\"\n    expected_paths:\n      - README.md\n    verify:\n      - cat README.md\n";

        let plan = parse_step_plan_yaml(yaml).unwrap();

        assert_eq!(plan.steps[0].id, "create-readme");
        assert_eq!(plan.steps[0].instruction, "Create README.md.");
    }

    #[test]
    fn validates_duplicate_step_ids() {
        let mut plan = sample_plan();
        plan.steps.push(plan.steps[0].clone());

        let err = validate_step_plan(&plan).unwrap_err();

        assert_eq!(err, PlanError::DuplicateStepId("write-readme".to_string()));
    }

    #[test]
    fn saves_plan_under_commandagent_plans() {
        let root = temp_workspace("save");
        let plan = sample_plan();

        let path = save_step_plan(&root, &plan).unwrap();

        assert!(path.starts_with(root.join(".commandagent/plans")));
        assert!(path.exists());
        let loaded = parse_step_plan_yaml(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(loaded, plan);
    }

    #[test]
    fn generation_prompt_demands_yaml_only() {
        let prompt =
            plan_generation_prompt("Build docs", "docs", "default", WorkIntent::Document, &[]);

        assert!(prompt.contains("Return only YAML"));
        assert!(prompt.contains("Goal: Build docs"));
        assert!(prompt.contains("Profile: docs"));
        assert!(prompt.contains("Intent: document"));
        assert!(prompt.contains("Do not include tool-call fields"));
        assert!(prompt.contains("YAML block scalars"));
        assert!(prompt.contains("do not use anchors"));
    }

    #[test]
    fn generation_prompt_warns_rust_against_shell_scaffolding() {
        let prompt =
            plan_generation_prompt("Build Rust CLI", "rust", "default", WorkIntent::New, &[]);

        assert!(prompt.contains("Cargo.toml"));
        assert!(prompt.contains("src/main.rs"));
        assert!(prompt.contains("Do not plan cargo init or cargo new"));
    }

    #[test]
    fn generation_prompt_lists_canonical_verifier_commands() {
        let prompt =
            plan_generation_prompt("Build app", "generic", "default", WorkIntent::New, &[]);

        assert!(prompt.contains("test -f <path>"));
        assert!(prompt.contains("python -m py_compile"));
        assert!(prompt.contains("cargo test"));
        assert!(prompt.contains("npm run build"));
        assert!(prompt.contains("grep -q"));
        assert!(prompt.contains("use build/test/check commands"));
        assert!(prompt.contains("active profile"));
        assert!(prompt.contains("not source-code semantics"));
        assert!(prompt.contains("if it exists"));
        assert!(prompt.contains("Inspect steps are observation-only"));
        assert!(prompt.contains("Do not use true as a verifier"));
    }

    #[test]
    fn generation_prompt_includes_nextjs_tailwind_plan_guidance() {
        let prompt = plan_generation_prompt("Build app", "nextjs", "default", WorkIntent::New, &[]);

        assert!(prompt.contains("React 18.2 or newer compatibility"));
        assert!(prompt.contains("Do not use exact React pins below 18.2"));
        assert!(prompt.contains("typescript 5.x compatibility"));
        assert!(prompt.contains("stable TypeScript 5.x range such as ^5.4.0"));
        assert!(prompt.contains("exact TypeScript pins such as 5.0.0"));
        assert!(prompt.contains("@types/react 18.x compatibility"));
        assert!(prompt.contains("If source imports use @/*"));
        assert!(prompt.contains("latest as the compatibility strategy"));
        assert!(prompt.contains("Use plain CSS unless"));
        assert!(prompt.contains("same step plan must also include"));
        assert!(prompt.contains("tailwindcss, postcss, and autoprefixer"));
        assert!(prompt.contains("tailwind.config.js and postcss.config.js"));
        assert!(prompt.contains("Do not plan npm install"));
    }

    #[test]
    fn correction_prompt_contains_error_and_invalid_plan() {
        let err = PlanError::NoSteps;
        let prompt = invalid_plan_correction_prompt("goal", "goal: x", &err);

        assert!(prompt.contains("Validation error"));
        assert!(prompt.contains("step plan must contain at least one step"));
        assert!(prompt.contains("goal: x"));
    }

    fn sample_plan() -> StepPlan {
        StepPlan {
            goal: "Build docs".to_string(),
            profile: "docs".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::Document,
            required_artifacts: vec!["README.md".to_string()],
            steps: vec![StepPlanStep {
                id: "write-readme".to_string(),
                kind: StepKind::Create,
                instruction: "Create README.md with usage notes.".to_string(),
                expected_result: ExpectedResult::Pass,
                expected_paths: vec!["README.md".to_string()],
                verify: vec!["cat README.md".to_string()],
            }],
        }
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "commandagent-step-plan-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
