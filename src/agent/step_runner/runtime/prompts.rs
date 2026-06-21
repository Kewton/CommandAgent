use crate::agent::step_runner::correction_evidence::PlanCorrectionEvidence;
use crate::agent::step_runner::profiles::profile_contract_text;
use crate::agent::step_runner::recovery_task::RecoveryTaskContract;
use crate::agent::step_runner::runtime::phase_contract::ActiveStepContract;
use crate::agent::step_runner::{StepKind, StepPlan, StepPlanStep};

pub(super) fn plan_correction_prompt(
    original_goal: &str,
    invalid_plan: &str,
    error: &str,
    plan_kind: &str,
    correction_evidence: Option<&PlanCorrectionEvidence>,
) -> String {
    let evidence_section = correction_evidence_section(correction_evidence);
    format!(
        "The generated CommandAgent {plan_kind} is invalid and must be corrected.\n\
Original goal:\n{original_goal}\n\n\
Validation error:\n{error}\n\n\
{evidence_section}\
Invalid plan:\n{invalid_plan}\n\n\
If the error mentions shell scaffolding, replace that step with explicit file creation or editing instructions that can be completed with Write/Edit.\n\
If the error mentions optional inspection, inspect discovery, missing inspect expected_paths, test -d, or test -f on a non-required inspect step, use kind: inspect with expected_paths: [] and verify: [].\n\
If the error mentions invalid verifier commands, remove every invalid verifier from the corrected YAML; do not keep the rejected command unchanged.\n\
If the error mentions dependency installation, dependency caches, node_modules, .venv, or dependency_missing, do not plan npm install, npm ci, pip install, node_modules checks, or dependency-cache checks as required success work. Replace that work with a report step using expected_result: unavailable, expected_paths: [], and verify: [].\n\
If the error mentions source-code behavior, source grep, or grep over source files, remove every grep verifier targeting source files such as .rs, .ts, .tsx, .js, .jsx, .py, .go, or .java. Replace source-code semantic checks with canonical build/test/check commands such as cargo check, cargo test, npm run build, python -m py_compile, or pytest. Keep grep only for literal docs/data/content checks.\n\
If the error mentions mixed setup and verification, remove build/test/check commands from create/edit/setup steps and add a separate verify step.\n\
If the error mentions shell chaining, split the verifier into simple commands or choose one canonical check. Do not use &&, ||, ;, pipes, redirection, or fallback-to-true syntax.\n\
If the error mentions action/path/content/old/new fields, rewrite those tool-call fields into step instruction and expected_paths fields.\n\
Long text fields such as goal, phase goal, and instruction may use quoted strings or YAML block scalars; do not use anchors, aliases, merge keys, custom tags, or extra nested maps.\n\
Return only corrected YAML using the required CommandAgent schema."
    )
}

fn correction_evidence_section(evidence: Option<&PlanCorrectionEvidence>) -> String {
    let Some(evidence) = evidence else {
        return String::new();
    };
    let Some(rendered) = evidence.render() else {
        return String::new();
    };
    let plan_contract_guidance = plan_contract_correction_guidance(evidence);
    let recovery_task = RecoveryTaskContract::from_contract_evidence(evidence)
        .and_then(|task| task.render())
        .map(|rendered| format!("Recovery task:\n{}\n", indent(&rendered, "  ")))
        .unwrap_or_default();
    format!(
        "{rendered}\n\
{recovery_task}\
{plan_contract_guidance}\
Copy exact required literals and paths from this evidence into the corrected YAML. Do not paraphrase required literals or paths.\n\n"
    )
}

fn plan_contract_correction_guidance(evidence: &PlanCorrectionEvidence) -> String {
    match evidence.violated_contract.as_deref() {
        Some("nextjs_tailwind_plan_contract") => {
            "Next.js Tailwind plan correction:\n\
- active job: manifest_repair. Correct the package/setup contract, not source or gameplay behavior.\n\
- choose exactly one valid direction:\n\
  1. remove Tailwind, @tailwind, Tailwind directives, tailwind.config.js, and postcss.config.js from source/style steps and use plain CSS; or\n\
  2. keep Tailwind and write exact package.json dependency literals tailwindcss, postcss, autoprefixer plus setup/config outputs tailwind.config.js and postcss.config.js.\n\
- if keeping Tailwind, the target package step must literally contain this deterministic manifest obligation block: next, react, react-dom, typescript 5.x, @types/react 18.x, tailwindcss, postcss, autoprefixer, scripts.build=next build, tailwind.config.js, postcss.config.js.\n\
- do not leave any source/style step mentioning Tailwind unless direction 2 is complete.\n\
- do not write only Tailwind CSS dependencies; that phrase is not a substitute for tailwindcss, postcss, and autoprefixer.\n\
- do not add npm install, npm ci, pnpm install, yarn install, node_modules, or package-lock.json as required plan work.\n\n"
                .to_string()
        }
        Some("nextjs_verifier_command_required") => {
            "Next.js verifier plan correction:\n\
- replace the rejected npx verifier with the exact verifier command npm run build.\n\
- keep npm run build in a separate kind: verify step; do not put it on create/edit/setup steps that produce files.\n\
- do not add npm install, npm ci, pnpm install, yarn install, node_modules checks, package-lock.json checks, or npx commands.\n\
- verifier-owned setup recovery will handle missing local dependencies when approved.\n\n"
                .to_string()
        }
        Some("nextjs_typescript_toolchain_plan_contract") => {
            "Next.js TypeScript toolchain plan correction:\n\
- because this plan uses tsconfig.json, .ts, .tsx, or TypeScript, the package.json step must include exact literals typescript, @types/react, and 18, plus a stable TypeScript 5.x range such as ^5.4.0.\n\
- use a stable Next.js 14 compatible TypeScript toolchain such as TypeScript ^5.4.0 and @types/react 18.x when React is 18.x.\n\
- do not use TypeScript 6, exact TypeScript pins such as 5.0.0, @types/react 19, latest, npm install, npm ci, node_modules checks, package-lock.json checks, or npx commands in the plan.\n\n"
                .to_string()
        }
        Some("nextjs_alias_plan_contract") => {
            "Next.js alias plan correction:\n\
- choose exactly one valid direction:\n\
  1. replace @/* imports with relative imports; or\n\
  2. add a tsconfig.json create/edit step whose instruction literally includes compilerOptions.paths and @/* mapping to the selected source root.\n\
- if keeping @/* imports, include tsconfig.json in expected_paths for that setup/config step.\n\
- do not replace npm run build with a weaker verifier.\n\n"
                .to_string()
        }
        _ => String::new(),
    }
}

fn indent(text: &str, prefix: &str) -> String {
    text.lines()
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn step_prompt(
    plan: &StepPlan,
    step: &StepPlanStep,
    missing_expected_paths: &[String],
    active_contract: &ActiveStepContract,
) -> Result<String, String> {
    let profile_contract = profile_contract_text(&plan.profile).map_err(|err| err.to_string())?;
    let active_contract_section = active_profile_contract_section(active_contract);
    let missing_hint = missing_expected_paths_hint(step, missing_expected_paths);
    Ok(format!(
        "Run one CommandAgent step.\n\
Overall goal: {goal}\n\
Profile: {profile}\n\
Style: {style}\n\
Intent: {intent}\n\
Required final artifacts:\n{artifacts}\n\
Profile contract:\n{profile_contract}\n\n\
{active_contract_section}\
Step id: {step_id}\n\
Step kind: {kind}\n\
Step instruction: {instruction}\n\
Expected result: {expected_result}\n\
Expected paths:\n{expected}\n\
Verifier commands:\n{verify}\n\n\
{missing_hint}\
Step tool policy: {policy}\n\
{action_guidance}\n\
Preserve the active profile contract facts while doing only this step.\n\
The runtime executes verifier commands after your response. Do not run listed verifier commands yourself unless the step kind is verify and the command is a single allowed local check.\n\
Do not use compound Bash commands with &&, ||, or ;.\n\
Do not install network dependencies unless the step explicitly asks for dependency setup and the environment allows it.",
        goal = plan.goal,
        profile = plan.profile,
        style = plan.style,
        intent = plan.intent.as_str(),
        artifacts = bullet_list(&plan.required_artifacts),
        active_contract_section = active_contract_section,
        step_id = step.id,
        kind = step.kind.as_str(),
        instruction = step.instruction,
        expected_result = step.expected_result.as_str(),
        expected = bullet_list(&step.expected_paths),
        verify = bullet_list(&step.verify),
        missing_hint = missing_hint,
        policy = step_tool_policy_text(step.kind),
        action_guidance = step_action_guidance(step.kind),
    ))
}

fn active_profile_contract_section(active_contract: &ActiveStepContract) -> String {
    let lines = active_contract.rendered_lines();
    if lines.is_empty() {
        String::new()
    } else {
        format!(
            "Active profile contract facts:\n{}\n\n",
            bullet_list(&lines)
        )
    }
}

fn step_tool_policy_text(kind: StepKind) -> &'static str {
    match kind {
        StepKind::Inspect | StepKind::Report => {
            "read-only; use Read/Glob/Grep or read-only Bash only, and do not call Write/Edit"
        }
        StepKind::Verify => "no mutation; run/check only and do not call Write/Edit",
        StepKind::Setup => {
            "setup/config file mutation only; do not edit source routes/components and do not run dependency installation yourself"
        }
        StepKind::Create | StepKind::Edit | StepKind::Repair => {
            "file mutation allowed when needed; keep changes scoped to this step"
        }
    }
}

fn step_action_guidance(kind: StepKind) -> &'static str {
    match kind {
        StepKind::Inspect | StepKind::Report => {
            "Do only this step. Produce concrete repository read evidence with Read, Glob, Grep, or read-only Bash. Do not use Write/Edit."
        }
        StepKind::Verify => {
            "Do only this step. Run or report the requested local check only; do not change files."
        }
        StepKind::Setup => {
            "Do only this step. Use Write/Edit only for setup or configuration files; Write creates parent directories automatically."
        }
        StepKind::Create | StepKind::Edit | StepKind::Repair => {
            "Do only this step. Use Write/Edit for file changes; Write creates parent directories automatically."
        }
    }
}

fn missing_expected_paths_hint(step: &StepPlanStep, missing_expected_paths: &[String]) -> String {
    if missing_expected_paths.is_empty()
        || !matches!(
            step.kind,
            StepKind::Create | StepKind::Edit | StepKind::Repair
        )
    {
        return String::new();
    }

    format!(
        "Currently missing expected paths:\n{missing}\n\n\
This step is not complete until the missing expected paths are created or the step reports a concrete blocker.\n\
If this step is supposed to produce one of these paths, create or update it with Write/Edit.\n\
If the path should not be created by this step, report a concrete blocker instead of pretending the step is complete.\n\
If more context is required, use Read/Glob first, then continue to Write/Edit in the same turn when a file change is still required.\n\
Do not finish with only a plan for the next action.\n\n",
        missing = bullet_list(missing_expected_paths)
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::step_runner::{ExpectedResult, StepKind, StepPlan, StepPlanStep, WorkIntent};

    fn prompt_test_plan() -> StepPlan {
        StepPlan {
            goal: "Create command parser".to_string(),
            profile: "rust".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::Modify,
            required_artifacts: Vec::new(),
            steps: Vec::new(),
        }
    }

    fn prompt_test_step(kind: StepKind) -> StepPlanStep {
        StepPlanStep {
            id: "create-commands-module".to_string(),
            kind,
            instruction: "Create src/commands.rs with the command parser implementation."
                .to_string(),
            expected_result: ExpectedResult::Pass,
            expected_paths: vec!["src/commands.rs".to_string()],
            verify: vec!["cargo check".to_string()],
        }
    }

    fn empty_contract(profile: &str) -> ActiveStepContract {
        ActiveStepContract::empty(profile)
    }

    #[test]
    fn step_prompt_shows_missing_paths_for_create_step() {
        let plan = prompt_test_plan();
        let step = prompt_test_step(StepKind::Create);
        let prompt = step_prompt(
            &plan,
            &step,
            &["src/commands.rs".to_string()],
            &empty_contract("rust"),
        )
        .unwrap();

        assert!(
            prompt.contains("Currently missing expected paths"),
            "{prompt}"
        );
        assert!(prompt.contains("- src/commands.rs"), "{prompt}");
        assert!(
            prompt.contains("create or update it with Write/Edit"),
            "{prompt}"
        );
        assert!(
            prompt.contains("Do not finish with only a plan"),
            "{prompt}"
        );
    }

    #[test]
    fn step_prompt_shows_missing_paths_for_edit_step() {
        let plan = prompt_test_plan();
        let step = prompt_test_step(StepKind::Edit);
        let prompt = step_prompt(
            &plan,
            &step,
            &["src/commands.rs".to_string()],
            &empty_contract("rust"),
        )
        .unwrap();

        assert!(
            prompt.contains("Currently missing expected paths"),
            "{prompt}"
        );
        assert!(prompt.contains("concrete blocker"), "{prompt}");
        assert!(prompt.contains("Write/Edit"), "{prompt}");
    }

    #[test]
    fn step_prompt_omits_missing_hint_for_inspect_verify_report() {
        let plan = prompt_test_plan();
        for kind in [StepKind::Inspect, StepKind::Verify, StepKind::Report] {
            let step = prompt_test_step(kind);
            let prompt = step_prompt(
                &plan,
                &step,
                &["src/commands.rs".to_string()],
                &empty_contract("rust"),
            )
            .unwrap();

            assert!(
                !prompt.contains("Currently missing expected paths"),
                "kind={kind:?}\n{prompt}"
            );
        }
    }

    #[test]
    fn inspect_and_report_prompts_require_read_evidence() {
        let plan = prompt_test_plan();
        for kind in [StepKind::Inspect, StepKind::Report] {
            let step = prompt_test_step(kind);
            let prompt = step_prompt(&plan, &step, &[], &empty_contract("rust")).unwrap();

            assert!(
                prompt.contains("Produce concrete repository read evidence"),
                "kind={kind:?}\n{prompt}"
            );
            assert!(
                prompt.contains("Read, Glob, Grep, or read-only Bash"),
                "kind={kind:?}\n{prompt}"
            );
            assert!(
                !prompt.contains("Write creates parent directories automatically"),
                "kind={kind:?}\n{prompt}"
            );
        }
    }

    #[test]
    fn step_prompt_omits_missing_hint_when_no_missing_paths() {
        let plan = prompt_test_plan();
        let step = prompt_test_step(StepKind::Create);
        let prompt = step_prompt(&plan, &step, &[], &empty_contract("rust")).unwrap();

        assert!(
            !prompt.contains("Currently missing expected paths"),
            "{prompt}"
        );
    }

    #[test]
    fn step_prompt_includes_active_profile_contract_facts() {
        let mut plan = prompt_test_plan();
        plan.profile = "nextjs".to_string();
        let step = prompt_test_step(StepKind::Edit);
        let active = ActiveStepContract {
            profile: "nextjs".to_string(),
            base_phase_contract_facts: vec!["nextjs.app_root=src/app".to_string()],
            profile_obligations: vec![crate::agent::step_runner::profiles::ProfileObligation {
                code: "nextjs_dev_port_required".to_string(),
                message: "scripts.dev must preserve requested port".to_string(),
                paths: vec!["package.json".to_string()],
                expected: Some("scripts.dev contains next dev and 3011".to_string()),
            }],
            current_profile_facts: Vec::new(),
        };

        let prompt = step_prompt(&plan, &step, &[], &active).unwrap();

        assert!(prompt.contains("Active profile contract facts"));
        assert!(prompt.contains("nextjs.app_root=src/app"));
        assert!(prompt.contains("nextjs_dev_port_required"));
        assert!(prompt.contains("scripts.dev contains next dev and 3011"));
        assert!(prompt.contains("Preserve the active profile contract facts"));
    }

    #[test]
    fn step_prompt_omits_active_contract_when_empty() {
        let plan = prompt_test_plan();
        let step = prompt_test_step(StepKind::Edit);

        let prompt = step_prompt(&plan, &step, &[], &empty_contract("rust")).unwrap();

        assert!(!prompt.contains("Active profile contract facts"));
    }

    #[test]
    fn plan_correction_prompt_explicitly_removes_source_grep_verifiers() {
        let prompt = plan_correction_prompt(
            "Create Rust CLI",
            "verify:\n  - grep -q \"pub fn\" src/cli.rs",
            "plan lint failed: step `create-cli-module` has invalid verifier command `grep -q \"pub fn\" src/cli.rs`: source-code behavior must be verified with build/test/check commands",
            "phase step plan",
            None,
        );

        assert!(prompt.contains("remove every invalid verifier"));
        assert!(prompt.contains("remove every grep verifier targeting source files"));
        assert!(prompt.contains("do not keep the rejected command unchanged"));
        assert!(prompt.contains("cargo check"));
        assert!(prompt.contains("npm run build"));
        assert!(prompt.contains("Keep grep only for literal docs/data/content checks"));
    }

    #[test]
    fn plan_correction_prompt_converts_dependency_install_to_unavailable_report() {
        let prompt = plan_correction_prompt(
            "Add analytics panel",
            "kind: setup\ninstruction: Install analytics library with npm install\nverify:\n  - test -f node_modules/.package-lock.json",
            "plan lint failed: dependency installation must not be a required success step",
            "phase step plan",
            None,
        );

        assert!(prompt.contains("do not plan npm install"));
        assert!(prompt.contains("node_modules checks"));
        assert!(prompt.contains("Replace that work with a report step"));
        assert!(prompt.contains("expected_result: unavailable"));
        assert!(prompt.contains("verify: []"));
    }

    #[test]
    fn plan_correction_prompt_includes_contract_evidence() {
        let evidence = PlanCorrectionEvidence {
            guard: "plan_lint.profile_obligations".to_string(),
            failed_step: Some("create-package-json".to_string()),
            violated_contract: Some("nextjs_dependencies_required".to_string()),
            target_field: Some("instruction".to_string()),
            required_literals: vec![
                "next".to_string(),
                "react".to_string(),
                "react-dom".to_string(),
            ],
            missing_literals: vec!["react-dom".to_string()],
            required_action: Some(
                "include these exact package literals in the corrected package.json step instruction"
                    .to_string(),
            ),
            ..Default::default()
        };

        let prompt = plan_correction_prompt(
            "Create Next.js app",
            "steps:\n- id: create-package-json",
            "plan lint failed: missing dependency literals",
            "phase step plan",
            Some(&evidence),
        );

        assert!(prompt.contains("Contract correction evidence"));
        assert!(prompt.contains("- failed_step: create-package-json"));
        assert!(prompt.contains("- violated_contract: nextjs_dependencies_required"));
        assert!(prompt.contains("- required_literals: next, react, react-dom"));
        assert!(prompt.contains("- missing_literals: react-dom"));
        assert!(prompt.contains("Recovery task:"));
        assert!(prompt.contains("source: plan_lint.profile_obligations"));
        assert!(prompt.contains("Copy exact required literals and paths"));
        assert!(prompt.contains("Do not paraphrase required literals or paths"));
    }

    #[test]
    fn plan_correction_prompt_explains_nextjs_tailwind_plan_contract() {
        let evidence = PlanCorrectionEvidence::new("plan_lint.nextjs_tailwind_plan_contract")
            .with_failed_step("create-global-styles")
            .with_violated_contract("nextjs_tailwind_plan_contract")
            .with_missing_literals(vec![
                "tailwindcss",
                "postcss",
                "autoprefixer",
                "postcss.config",
            ])
            .with_required_action(
                "make Tailwind intent explicit and complete: either remove Tailwind directives from source/style instructions, or add package.json dependencies plus tailwind.config.js and postcss.config.js setup steps",
            );

        let prompt = plan_correction_prompt(
            "Create app",
            "steps: []",
            "plan lint failed",
            "phase step plan",
            Some(&evidence),
        );

        assert!(prompt.contains("Next.js Tailwind plan correction"));
        assert!(prompt.contains("choose exactly one valid direction"));
        assert!(prompt.contains("remove Tailwind"));
        assert!(prompt.contains("tailwindcss, postcss, autoprefixer"));
        assert!(prompt.contains("do not add npm install"));
    }

    #[test]
    fn plan_correction_prompt_explains_nextjs_verifier_contract() {
        let evidence = PlanCorrectionEvidence::new("plan_lint.nextjs_verifier_contract")
            .with_failed_step("verify-compilation")
            .with_violated_contract("nextjs_verifier_command_required")
            .with_rejected_value("npx tsc --noEmit")
            .with_required_literals(vec!["npm run build"])
            .with_required_action(
                "replace the npx verifier with npm run build in a separate verify step",
            );

        let prompt = plan_correction_prompt(
            "Create app",
            "steps: []",
            "plan lint failed",
            "phase step plan",
            Some(&evidence),
        );

        assert!(prompt.contains("Next.js verifier plan correction"));
        assert!(prompt.contains("replace the rejected npx verifier"));
        assert!(prompt.contains("npm run build"));
        assert!(prompt.contains("separate kind: verify step"));
        assert!(prompt.contains("verifier-owned setup recovery"));
        assert!(prompt.contains("do not add npm install"));
    }

    #[test]
    fn plan_correction_prompt_explains_nextjs_typescript_toolchain_contract() {
        let evidence = PlanCorrectionEvidence::new("plan_lint.nextjs_typescript_plan_contract")
            .with_failed_step("create-tsconfig")
            .with_violated_contract("nextjs_typescript_toolchain_plan_contract")
            .with_missing_literals(vec!["typescript", "5.x or ^5.", "@types/react", "18"])
            .with_required_action(
                "make the Next.js TypeScript toolchain explicit in the package.json step",
            );

        let prompt = plan_correction_prompt(
            "Create app",
            "steps: []",
            "plan lint failed",
            "phase step plan",
            Some(&evidence),
        );

        assert!(prompt.contains("Next.js TypeScript toolchain plan correction"));
        assert!(prompt.contains("exact literals typescript, @types/react, and 18"));
        assert!(prompt.contains("stable TypeScript 5.x range such as ^5.4.0"));
        assert!(prompt.contains("TypeScript ^5.4.0"));
        assert!(prompt.contains("@types/react 18.x"));
        assert!(prompt.contains("do not use TypeScript 6"));
        assert!(prompt.contains("exact TypeScript pins such as 5.0.0"));
        assert!(prompt.contains("latest"));
    }
}
