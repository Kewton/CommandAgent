//! Expectations come only from the first saved duties. Later proposals and
//! workspace contents are deliberately absent from expectation acquisition.
use super::{formation_scope::FormationScope, *};
use crate::minimal_loop::evidence::package_script_check as grammar;
use crate::planner::recovery_contract_authority::verifier_obligations as scope;
use sha2::{Digest, Sha256};

pub(super) fn hash(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

fn object_hash(value: &impl serde::Serialize) -> String {
    hash(&serde_json::to_vec(value).expect("serializable obligation"))
}

#[derive(Clone, serde::Serialize)]
pub(super) struct CapturedSources {
    pub(super) plans: FormationScope,
    source_file: Option<String>,
    acquisition_stage: &'static str,
    model_sha256: String,
    host_sha256: String,
}

impl CapturedSources {
    pub(super) fn new(config: &Config, plans: FormationScope, stage: &'static str) -> Self {
        Self {
            model_sha256: object_hash(&plans.model),
            host_sha256: object_hash(&plans.host),
            plans,
            source_file: config
                .eval_events_path
                .as_ref()
                .map(|p| p.display().to_string()),
            acquisition_stage: stage,
        }
    }
}

#[derive(Clone, PartialEq, Eq, serde::Serialize)]
struct Source {
    kind: &'static str,
    json_pointer: String,
    owner: String,
    step_index: usize,
    step_sha256: String,
    instruction_sha256: String,
    fixed_literals: Vec<String>,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize)]
pub(super) struct Obligation {
    pub(super) original_command: String,
    original_command_sha256: String,
    pub(super) expected_result: String,
    target: &'static str,
    script: String,
    expected_literal: String,
    original_check_owner: String,
    original_step_index: usize,
    sources: Vec<Source>,
    pub(super) replacement_command: String,
    replacement_command_sha256: String,
    output: &'static str,
    failure_propagation: &'static str,
}

#[derive(serde::Serialize)]
pub(super) struct PackageScripts {
    sources: CapturedSources,
    registered_contract: CompletionContract,
    registered_contract_sha256: String,
    original_verify_commands: Vec<String>,
    pub(super) obligations: Vec<Obligation>,
    refusals: Vec<String>,
}

impl PackageScripts {
    pub(super) fn has_profile_capture(
        &self,
        sources: &FormationScope,
        original: &StepPlan,
    ) -> bool {
        self.sources.acquisition_stage == "profile_augmentation_before_sanitization"
            && self.sources.plans == *sources
            && self.sources.model_sha256 == object_hash(&sources.model)
            && self.sources.host_sha256 == object_hash(&sources.host)
            && self.registered_contract_sha256 == object_hash(&self.registered_contract)
            && self.original_verify_commands
                == original
                    .steps
                    .iter()
                    .flat_map(|s| s.verify.clone())
                    .collect::<Vec<_>>()
            && self.refusals.is_empty()
    }

    pub(super) fn preserve_raw_commands(&self, model: &StepPlan) -> anyhow::Result<()> {
        for command in model.steps.iter().flat_map(|s| &s.verify) {
            let normalized = crate::planner::verify::normalize_planner_verify_command(command)
                .unwrap_or_default();
            if self.obligations.iter().any(|o| {
                command != &o.replacement_command && normalized.contains(&o.replacement_command)
            }) {
                return Err(crate::planner::recovery_inspection::verifier_obligations::failure(
                    crate::planner::recovery_inspection::verifier_obligations::FailureClass::ProposalRepairable,
                    "package-script proposal must preserve the literal comparison, stdout and nonzero failure without shell wrappers",
                ));
            }
        }
        Ok(())
    }
    pub(super) fn capture(
        config: &Config,
        original: &StepPlan,
        sources: CapturedSources,
        contract: &CompletionContract,
    ) -> Self {
        let mut captured = Self {
            sources,
            registered_contract: contract.clone(),
            registered_contract_sha256: object_hash(contract),
            original_verify_commands: original
                .steps
                .iter()
                .flat_map(|s| s.verify.clone())
                .collect(),
            obligations: Vec::new(),
            refusals: Vec::new(),
        };
        for (index, step) in original.steps.iter().enumerate() {
            for command in &step.verify {
                let Some(script) = grammar::printed_script(command) else {
                    continue;
                };
                match captured.resolve(config, &script) {
                    Ok((expected, sources)) if step.expected_result == "pass" => {
                        let replacement = grammar::formed_command(&script, &expected);
                        captured.obligations.push(Obligation {
                            original_command: command.clone(),
                            original_command_sha256: hash(command.as_bytes()),
                            expected_result: step.expected_result.clone(), target: "./package.json",
                            script, expected_literal: expected, original_check_owner: step.id.clone(),
                            original_step_index: index, sources,
                            replacement_command_sha256: hash(replacement.as_bytes()),
                            replacement_command: replacement,
                            output: "print actual string plus newline after strict comparison",
                            failure_propagation: "uncaught require/property/assertion failure exits nonzero",
                        });
                    }
                    Ok(_) => captured
                        .refusals
                        .push(format!("{command}: original expected result must be pass")),
                    Err(reason) => captured.refusals.push(format!("{command}: {reason}")),
                }
            }
        }
        captured
    }

    fn resolve(&self, config: &Config, script: &str) -> Result<(String, Vec<Source>), String> {
        let mut values = std::collections::BTreeSet::new();
        let mut sources = Vec::new();
        let mut port_constraints = Vec::new();
        let runtime = crate::planner::profile::resolve_profile_runtime(&config.profile);
        for (kind, plan) in [
            ("model", &self.sources.plans.model),
            ("host", &self.sources.plans.host),
        ] {
            for (index, step) in plan.steps.iter().enumerate() {
                if !step
                    .expected_paths
                    .iter()
                    .any(|p| scope::normalized(p).is_ok_and(|p| p == "package.json"))
                {
                    continue;
                }
                if step.id.is_empty()
                    || step.step_kind() != StepKind::Implement
                    || step.expected_result != "pass"
                {
                    return Err(
                        "package-script expected value requires an executable Implement/pass owner"
                            .into(),
                    );
                }
                let known_host = kind == "host"
                    && runtime.guidance(&plan.goal).as_deref() == Some(step.instruction.as_str());
                let literals = if known_host
                    && crate::planner::profile::is_nextjs_profile(&config.profile)
                {
                    if matches!(script, "dev" | "start") {
                        port_constraints.push(
                            crate::planner::profiles::nextjs::requested_or_default_port(&plan.goal),
                        );
                    }
                    if script == "build" {
                        vec!["next build".into()]
                    } else {
                        Vec::new()
                    }
                } else {
                    fixed_values(&step.instruction, script).map_err(str::to_string)?
                };
                values.extend(literals.iter().cloned());
                sources.push(Source {
                    kind,
                    json_pointer: format!(
                        "/package_script_formation/sources/plans/{kind}/steps/{index}"
                    ),
                    owner: step.id.clone(),
                    step_index: index,
                    step_sha256: object_hash(step),
                    instruction_sha256: hash(step.instruction.as_bytes()),
                    fixed_literals: literals,
                });
            }
        }
        if values.len() != 1 {
            return Err(if values.is_empty() {
                "no fixed package-script literal in saved original obligations"
            } else {
                "conflicting package-script literals in saved model/host obligations"
            }
            .into());
        }
        let expected = values.into_iter().next().expect("one literal");
        for command in self
            .registered_contract
            .verify_commands
            .iter()
            .chain(&self.original_verify_commands)
        {
            if let Some((registered_script, registered_value)) = grammar::comparison(command)
                && registered_script == script
                && registered_value != expected
            {
                return Err("saved literal conflicts with registered completion contract".into());
            }
            let known_package_check =
                crate::planner::profiles::nextjs::recovery_authority::is_package_check(command);
            if known_package_check
                && script == "build"
                && !command.contains("--port=")
                && expected != "next build"
            {
                return Err(
                    "saved literal conflicts with registered build-script constraint".into(),
                );
            }
            if command.contains("package.json")
                && command.contains("scripts")
                && !known_package_check
                && grammar::comparison(command).is_none()
                && grammar::printed_script(command).is_none()
            {
                return Err(
                    "unsupported registered package-script constraint; cannot prove consistency"
                        .into(),
                );
            }
            if crate::planner::profiles::nextjs::recovery_authority::is_package_check(command)
                && let Some((_, suffix)) = command.split_once("--port=")
                && let Some((port, _)) = suffix.split_once('\'')
                && let Ok(port) = port.parse::<u16>()
            {
                port_constraints.push(port);
            }
        }
        if matches!(script, "dev" | "start")
            && port_constraints.iter().any(|port| {
                expected != format!("next {script} -p {port}")
                    && expected != format!("next {script} --port {port}")
                    && expected != format!("next {script} --port={port}")
                    && expected != format!("next {script} -p{port}")
            })
        {
            return Err(
                "saved literal conflicts with original host/registered port constraint".into(),
            );
        }
        Ok((expected, sources))
    }

    pub(super) fn guidance(&self) -> String {
        let mut lines = self.refusals.clone();
        lines.extend(self.obligations.iter().map(|o| format!(
            "Saved package-script obligation {} (owner {}, expected {:?}): replace only with {}",
            o.original_command, o.original_check_owner, o.expected_literal, o.replacement_command
        )));
        if !self.obligations.is_empty() {
            lines.push("If complete model and host package instructions exceed one step, explicitly split into two ordered Implement/pass package.json writers: unchanged original model owner first, full original host instruction on a new unique owner second. Keep every original package check together in a Verify/pass after both writers. Do not truncate either instruction, add a third writer or omit host checks.".into());
        }
        lines.join("\n")
    }
}

/// The observed declaration is a deliberately closed template, not a natural
/// language interpreter. Extra mentions of the same script are ambiguous.
pub(super) fn fixed_values(instruction: &str, script: &str) -> Result<Vec<String>, &'static str> {
    let Some(mut rest) = instruction.strip_prefix("Update package.json scripts so that ") else {
        return if instruction == "Maintain package.json." {
            Ok(Vec::new())
        } else {
            Err("ambiguous or unsupported saved package-script declaration")
        };
    };
    let mut values = Vec::new();
    loop {
        let (name, tail) = rest
            .split_once(" is '")
            .ok_or("unsupported saved package-script declaration")?;
        let (value, tail) = tail
            .split_once('\'')
            .ok_or("unterminated saved package-script literal")?;
        if !grammar::identifier(name) || !grammar::literal(value) {
            return Err("unsupported saved package-script name/literal");
        }
        if name == script {
            values.push(value.into());
        }
        if let Some(tail) = tail.strip_prefix(" and ") {
            rest = tail;
            continue;
        }
        let tail = tail
            .strip_prefix('.')
            .ok_or("ambiguous saved package-script alternatives")?;
        values.extend(trailing_values(tail, script)?);
        return Ok(values);
    }
}

fn trailing_values(mut tail: &str, script: &str) -> Result<Vec<String>, &'static str> {
    let mut values = Vec::new();
    while let Some(declaration) = tail.strip_prefix(" Keep scripts.") {
        let (name, rest) = declaration
            .split_once(" as '")
            .ok_or("unsupported saved script constraint")?;
        let (value, rest) = rest
            .split_once('\'')
            .ok_or("unterminated saved script constraint")?;
        if !grammar::identifier(name) || !grammar::literal(value) {
            return Err("unsupported saved script constraint");
        }
        if name == script {
            values.push(value.into());
        }
        tail = rest
            .strip_prefix('.')
            .ok_or("ambiguous saved script constraint")?;
    }
    // The saved non-script duties are an explicit finite suffix, not an
    // invitation to interpret arbitrary natural-language overrides such as
    // "Or use any port" (which need not mention the script key again).
    const SAVED_CONFIGURATION_DUTIES: &str = " Ensure next, react, react-dom dependencies are present and tailwindcss, postcss, autoprefixer are present. Do not modify tsconfig.json unless it has moduleResolution=node10 or target=ES5—in that case set moduleResolution to 'bundler' and target to 'ES2017'. Keep postcss.config.js plugins including both tailwindcss and autoprefixer. Keep tailwind.config.ts coherent with content paths covering src/app/**/*.tsx.";
    if !tail.is_empty() && tail != SAVED_CONFIGURATION_DUTIES {
        return Err("ambiguous or unsupported additional saved package requirements");
    }
    Ok(values)
}
