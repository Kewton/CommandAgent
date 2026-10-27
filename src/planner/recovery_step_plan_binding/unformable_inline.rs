//! A closed impossibility proof for a frozen inline Node check, never a
//! readiness or ordering decision. Unknown shapes continue through the
//! existing admission/retry policy.
use super::Admission;
use super::bounded_repair::Infeasible;
use crate::config::Config;
use crate::minimal_loop::completion::CompletionContract;
use crate::minimal_loop::evidence::{command_diagnosis, import_check};
use crate::planner::recovery_contract_authority::verifier_obligations as scope;
use crate::planner::step_plan::StepPlan;

/// The admission proof that ends planning without another proposal.
#[derive(Debug, serde::Serialize, thiserror::Error)]
#[serde(untagged)]
pub(super) enum Terminal {
    #[error(transparent)]
    Bounded(Infeasible),
    #[error(transparent)]
    Unformable(UnformableInline),
}

impl Terminal {
    pub(super) fn reason(&self) -> &'static str {
        match self {
            Self::Bounded(_) => "bounded_repair_infeasible",
            Self::Unformable(_) => "unformable_inline_evidence",
        }
    }
}

#[derive(Debug, serde::Serialize)]
struct Frozen {
    command: String,
    reason: Option<String>,
    repairability: &'static str,
}

#[derive(Debug, serde::Serialize, thiserror::Error)]
#[error(
    "this inline check has no acceptable form and is frozen, so a later proposal cannot replace it: {listed}; stopped without requesting another proposal"
)]
pub(super) struct UnformableInline {
    listed: String,
    frozen: Vec<Frozen>,
    formation_shapes: [&'static str; 3],
    frozen_rule: &'static str,
}

impl Admission {
    pub(super) fn prove_terminal(
        &self,
        config: &Config,
        error: &anyhow::Error,
        plan: &StepPlan,
        attempt: usize,
        recovery: bool,
    ) -> Option<Terminal> {
        if let Some(proof) = self.prove_bounded_repair_infeasible(error, plan, attempt) {
            return Some(Terminal::Bounded(proof));
        }
        if recovery {
            return None;
        }
        self.prove_unformable_inline(config, plan, attempt)
            .map(Terminal::Unformable)
    }

    // The first proposal was rejected by `require_formed` for an inline check
    // that the frozen baseline keeps and no registered formation can replace.
    fn prove_unformable_inline(
        &self,
        config: &Config,
        plan: &StepPlan,
        attempt: usize,
    ) -> Option<UnformableInline> {
        let original = self.original.as_ref()?;
        if attempt != 1
            || original != plan
            || self.duplicate_strengthen
            || self.profile_instruction_provenance_error.is_some()
            || self.marker_capture_error.is_some()
            || !self.replacements.is_empty()
            || self.profile_instruction_failure(plan).is_some()
        {
            return None;
        }
        // Mirror the gate in `form`: only these profiles reach `require_formed`.
        if !matches!(
            config.profile.as_str(),
            crate::planner::profile_descriptor::GENERIC_PROFILE_ID
                | crate::planner::profile_descriptor::NEXTJS_PROFILE_ID
        ) || CompletionContract::configured_path_for_config(config)
            .ok()?
            .is_some()
        {
            return None;
        }
        let commands = scope::commands(plan);
        // The rejection being explained is the one `form` raised.
        super::super::verifier_formation::require_formed(config, &commands).err()?;
        let diagnoses = command_diagnosis::collect(&config.workspace_root, &commands);
        if diagnoses
            .iter()
            .any(|d| d.repairability == "frozen_verifier_evidence")
        {
            return None;
        }
        let frozen: Vec<_> = diagnoses
            .iter()
            .filter(|d| d.immutable_inline() && !self.formable(&d.command))
            .map(|d| Frozen {
                command: d.command.clone(),
                reason: d.reason.clone(),
                repairability: "immutable_inline_evidence",
            })
            .collect();
        if frozen.is_empty() {
            return None;
        }
        Some(UnformableInline {
            listed: frozen
                .iter()
                .map(|f| f.command.as_str())
                .collect::<Vec<_>>()
                .join("; "),
            frozen,
            formation_shapes: [
                "import_or_require_export_set",
                "package_script_literal",
                "literal_marker",
            ],
            frozen_rule: "preservation requires every original verify command to remain; require_formed rejects a weak inline command that remains",
        })
    }

    // The same three shapes `verifier_formation::project` can substitute.
    fn formable(&self, command: &str) -> bool {
        import_check::pure_target(command).is_some()
            || import_check::pure_require_target(command).is_some()
            || self
                .package_scripts
                .as_ref()
                .is_some_and(|p| p.obligations.iter().any(|o| o.original_command == command))
            || self
                .marker_checks
                .iter()
                .any(|o| o.original_command == command)
    }
}

#[cfg(test)]
#[path = "unformable_inline_tests.rs"]
mod tests;
