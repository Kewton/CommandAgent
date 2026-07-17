use serde_json::json;

use crate::eval_events;
use crate::planner::adjudication::contract::is_fix_intent;
use crate::planner::profile::{ProfileFixReproducerSuggestion, domain_profile};
use crate::planner::ultra_plan::{UltraPhase, UltraPlan};

pub(crate) type ReproducerSuggestion = ProfileFixReproducerSuggestion;

pub(crate) fn suggestion_for(plan: &UltraPlan) -> Option<ReproducerSuggestion> {
    if !is_fix_intent(&plan.intent) {
        return None;
    }
    domain_profile(&plan.profile).fix_reproducer_suggestion(&plan.goal)
}

pub(crate) fn attach_to_phase_prompt(
    plan: &UltraPlan,
    phase: &UltraPhase,
    eval_events_path: Option<&std::path::Path>,
    mut prompt: String,
) -> String {
    if plan.phases.first().is_none_or(|first| first.id != phase.id) {
        return prompt;
    }
    let Some(suggestion) = suggestion_for(plan) else {
        return prompt;
    };
    eval_events::emit(
        eval_events_path,
        json!({
            "event": "fix_reproducer_suggested",
            "basis": suggestion.basis,
            "suggestion": suggestion.suggestion,
        }),
    );
    prompt.push_str(
        "\n\nFix contract section 8 reproducer suggestion (guidance, not enforcement):\n- basis: ",
    );
    prompt.push_str(&suggestion.basis);
    prompt.push_str("\n- canonical candidate: ");
    prompt.push_str(&suggestion.suggestion);
    prompt.push_str("\nUse this candidate when it represents the stated failure. A different deterministic R remains permitted; the F1 baseline gate remains authoritative.");
    prompt
}

#[cfg(test)]
mod tests;
