//! A closed impossibility proof, never a readiness or ordering decision.
//! Unknown shapes continue through the existing admission/retry policy.
use super::*;
use std::collections::BTreeSet;

#[derive(Debug, thiserror::Error)]
pub(super) enum InstructionFailure {
    #[error("{0}")]
    Provenance(String),
    #[error(
        "profile instruction exceeds the 2500-character capacity after deterministic sanitization: owner {owner} has {chars} characters; preserve the complete model and host duties without truncation and use only owner splits allowed by the existing ownership rules"
    )]
    Capacity { owner: String, chars: usize },
}

#[derive(Debug, serde::Serialize, thiserror::Error)]
#[error(
    "profile instruction exceeds the {limit_chars}-character capacity: bounded repair is not representable under current preservation and registered split rules for owner {owner}; frozen duties require at least {minimum_chars} Unicode scalar values; stopped without requesting another proposal"
)]
pub(super) struct Infeasible {
    owner: String,
    acquisition_stage: &'static str,
    normalized_paths: BTreeSet<String>,
    model_instruction_sha256: String,
    host_instruction_sha256: String,
    model_chars: usize,
    host_chars: usize,
    model_to_host_overlap: usize,
    host_to_model_overlap: usize,
    minimum_chars: usize,
    limit_chars: usize,
    excess_chars: usize,
    registered_split: &'static str,
    split_inapplicable_reason: &'static str,
    preservation_rule: &'static str,
}

impl Admission {
    pub(super) fn prove_bounded_repair_infeasible(
        &self,
        error: &anyhow::Error,
        plan: &StepPlan,
        attempt: usize,
    ) -> Option<Infeasible> {
        let InstructionFailure::Capacity { owner, .. } =
            error.downcast_ref::<InstructionFailure>()?
        else {
            return None;
        };
        let original = self.original.as_ref()?;
        let sources = self.sources.as_ref()?;
        let preserved = self.preservation_sources.as_ref()?;
        let addition = self.profile_addition.as_ref()?;
        if attempt != 1
            || original != plan
            || self.duplicate_strengthen
            || self.profile_instruction_provenance_error.is_some()
            || self.marker_capture_error.is_some()
            || !self.replacements.is_empty()
            || !self
                .package_scripts
                .as_ref()?
                .has_profile_capture(sources, original)
            || *preserved != FormationScope::capture(original, Some(addition))
            || [
                &sources.model,
                &sources.host,
                &preserved.model,
                &preserved.host,
                original,
            ]
            .iter()
            .any(|p| !unique_ids(p))
        {
            return None;
        }
        let [host] = sources.host.steps.as_slice() else {
            return None;
        };
        let model = sources.model.steps.iter().find(|s| s.id == host.id)?;
        let uncut = original.steps.iter().find(|s| s.id == host.id)?;
        // Goal canonicalization is independent of owner obligations. Match the
        // complete captured steps; keep both goal views in the event evidence.
        if &host.id != owner
            || sources.host.steps != preserved.host.steps
            || preserved.model.steps.iter().find(|s| s.id == host.id)? != model
            || [model, host, uncut].iter().any(|s| {
                s.step_kind() != StepKind::Implement
                    || s.expected_result != "pass"
                    || s.instruction.is_empty()
            })
            || !uncut.instruction.contains(&model.instruction)
            || !uncut.instruction.contains(&host.instruction)
            || addition
                .instruction_protection(original, true)
                .error
                .is_some()
        {
            return None;
        }
        let paths = normalized_paths(model)?;
        if paths.len() < 2 || paths != normalized_paths(host)? || paths != normalized_paths(uncut)?
        {
            return None;
        }
        // This is the only registered source-specific ownership split. Keep
        // its source gate shared with real admission, never infer a new split.
        if super::super::package_owner_scope::split_duties(&preserved.model, &preserved.host)
            .is_some()
        {
            return None;
        }
        // Projection only substitutes checks. Reader exceptions require Verify,
        // so neither can remove the full-text duty of these Implement owners.
        let lengths = common_superstring_lengths(&model.instruction, &host.instruction);
        let limit = crate::planner::sanitizer::STEP_PLAN_INSTRUCTION_LINT_LIMIT_CHARS;
        if lengths.minimum <= limit {
            return None;
        }
        Some(Infeasible {
            owner: owner.clone(),
            acquisition_stage: "profile_augmentation_before_sanitization",
            normalized_paths: paths,
            model_instruction_sha256: super::super::package_script_formation::hash(
                model.instruction.as_bytes(),
            ),
            host_instruction_sha256: super::super::package_script_formation::hash(
                host.instruction.as_bytes(),
            ),
            model_chars: lengths.model,
            host_chars: lengths.host,
            model_to_host_overlap: lengths.forward,
            host_to_model_overlap: lengths.reverse,
            minimum_chars: lengths.minimum,
            limit_chars: limit,
            excess_chars: lengths.minimum - limit,
            registered_split: "package_owner_scope",
            split_inapplicable_reason: "both duties must own exactly [package.json]; frozen scope has multiple normalized paths",
            preservation_rule: "each matching Implement owner contains both complete instructions; checks and owner order remain separately required",
        })
    }
}

fn unique_ids(plan: &StepPlan) -> bool {
    let mut ids = BTreeSet::new();
    plan.steps
        .iter()
        .all(|s| !s.id.is_empty() && ids.insert(&s.id))
}

fn normalized_paths(step: &PlanStep) -> Option<BTreeSet<String>> {
    step.expected_paths
        .iter()
        .map(|p| scope::normalized(p).ok())
        .collect()
}

struct Lengths {
    model: usize,
    host: usize,
    forward: usize,
    reverse: usize,
    minimum: usize,
}

fn common_superstring_lengths(model: &str, host: &str) -> Lengths {
    let m: Vec<_> = model.chars().collect();
    let h: Vec<_> = host.chars().collect();
    let forward = overlap(&m, &h);
    let reverse = overlap(&h, &m);
    let minimum = if model.contains(host) || host.contains(model) {
        m.len().max(h.len())
    } else {
        m.len() + h.len() - forward.max(reverse)
    };
    Lengths {
        model: m.len(),
        host: h.len(),
        forward,
        reverse,
        minimum,
    }
}

// KMP computes the longest suffix/prefix match in linear time, in scalars.
fn overlap(left: &[char], right: &[char]) -> usize {
    if right.is_empty() {
        return 0;
    }
    let mut prefix = vec![0; right.len()];
    let mut matched = 0;
    for i in 1..right.len() {
        while matched > 0 && right[i] != right[matched] {
            matched = prefix[matched - 1];
        }
        if right[i] == right[matched] {
            matched += 1;
        }
        prefix[i] = matched;
    }
    matched = 0;
    for ch in left {
        while matched > 0 && (matched == right.len() || *ch != right[matched]) {
            matched = prefix[matched - 1];
        }
        if *ch == right[matched] {
            matched += 1;
        }
    }
    matched
}

#[cfg(test)]
#[path = "issue498_tests.rs"]
mod tests;
