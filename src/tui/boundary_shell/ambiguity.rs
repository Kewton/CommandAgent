use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::planner::adjudication::contract::IntentId;
use crate::planner::profile::ProfileId;
use crate::provider_call::{self, ProviderCallScope, ProviderChatRequest};
use crate::providers::ChatClient;
use crate::state::ConversationMessage;

use super::family_catalog::TaskFamilyId;
use super::route::{DeterministicResolution, DeterministicRouteResult, RouteCandidate};

const CLASSIFIER_PROMPT_VERSION: &str = "d3c-route-v1";
const CLASSIFIER_MAX_RESPONSE_BYTES: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalStatus {
    AwaitingConfirmation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifierProvenance {
    pub used: bool,
    pub provider: String,
    pub model: String,
    pub prompt_version: &'static str,
    pub candidate_keys: Vec<String>,
    pub raw_response_hash: Option<String>,
    pub parse_reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteProposal {
    pub selected: Option<RouteCandidate>,
    pub alternatives: Vec<RouteCandidate>,
    pub classifier: ClassifierProvenance,
    pub status: ProposalStatus,
    pub confirmation_required: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClassifierOutput {
    profile: String,
    intent: String,
    family: String,
}

pub fn propose_route(
    deterministic: DeterministicRouteResult,
    request: &str,
    provider: &str,
    model: &str,
    classifier: &mut dyn ChatClient,
    config: &Config,
    is_cancelled: &dyn Fn() -> bool,
) -> RouteProposal {
    let candidate_keys = deterministic
        .candidates
        .iter()
        .map(candidate_key)
        .collect::<Vec<_>>();
    let mut provenance = ClassifierProvenance {
        used: false,
        provider: provider.to_string(),
        model: model.to_string(),
        prompt_version: CLASSIFIER_PROMPT_VERSION,
        candidate_keys,
        raw_response_hash: None,
        parse_reason: String::new(),
    };
    let selected = match deterministic.resolution {
        DeterministicResolution::Unique => {
            provenance.parse_reason = "deterministic_unique".to_string();
            deterministic.candidates.first().cloned()
        }
        DeterministicResolution::Ambiguous => {
            provenance.used = true;
            classify_closed_candidates(
                request,
                &deterministic.candidates,
                model,
                classifier,
                config,
                is_cancelled,
                &mut provenance,
            )
        }
        DeterministicResolution::Unknown => {
            provenance.parse_reason = "typed_unknown:no_deterministic_candidate".to_string();
            None
        }
        DeterministicResolution::ContradictoryExplicitBinding => {
            provenance.parse_reason = "typed_unknown:contradictory_explicit_binding".to_string();
            None
        }
    };
    RouteProposal {
        selected,
        alternatives: deterministic.candidates,
        classifier: provenance,
        status: ProposalStatus::AwaitingConfirmation,
        confirmation_required: true,
    }
}

fn classify_closed_candidates(
    request: &str,
    candidates: &[RouteCandidate],
    model: &str,
    classifier: &mut dyn ChatClient,
    config: &Config,
    is_cancelled: &dyn Fn() -> bool,
    provenance: &mut ClassifierProvenance,
) -> Option<RouteCandidate> {
    let keys = candidates.iter().map(candidate_key).collect::<Vec<_>>();
    let prompt = format!(
        "D-3c route proposal classifier ({CLASSIFIER_PROMPT_VERSION}).\n\
         Choose exactly one registered candidate. This is a proposal and cannot dispatch work.\n\
         Return one JSON object with exactly profile, intent, family.\n\
         Request:\n{request}\n\
         Registered candidates:\n{}",
        keys.join("\n")
    );
    let messages = [
        ConversationMessage::system(
            "Choose only from the closed candidate list. Do not add prose or IDs.",
        ),
        ConversationMessage::user(prompt),
    ];
    let response = match provider_call::chat_with_cancel_and_response_limit(
        classifier,
        config,
        ProviderChatRequest {
            scope: ProviderCallScope::PlannerStep,
            model,
            messages: &messages,
            tools: &[],
            native_tools_enabled: false,
        },
        is_cancelled,
        CLASSIFIER_MAX_RESPONSE_BYTES,
    )
    .result
    {
        Ok(response) => response.content,
        Err(error) => {
            provenance.parse_reason = format!("typed_unknown:classifier_error:{error}");
            return None;
        }
    };
    provenance.raw_response_hash = Some(sha256(response.as_bytes()));
    let output = match serde_json::from_str::<ClassifierOutput>(response.trim()) {
        Ok(output) => output,
        Err(error) => {
            provenance.parse_reason = format!("typed_unknown:invalid_output:{error}");
            return None;
        }
    };
    let parsed_intent = parse_intent(&output.intent);
    let parsed_family = TaskFamilyId::parse(&output.family);
    let matched = candidates.iter().find(|candidate| {
        candidate.profile == ProfileId::parse(&output.profile)
            && Some(candidate.intent) == parsed_intent
            && candidate.family == parsed_family
    });
    match matched {
        Some(candidate) => {
            provenance.parse_reason = "closed_candidate_match".to_string();
            Some(candidate.clone())
        }
        None => {
            provenance.parse_reason = "typed_unknown:unregistered_candidate".to_string();
            None
        }
    }
}

fn candidate_key(candidate: &RouteCandidate) -> String {
    format!(
        "{} × {} × {}",
        candidate.profile,
        candidate.intent.as_str(),
        candidate.family
    )
}

fn parse_intent(value: &str) -> Option<IntentId> {
    match value.trim().to_ascii_lowercase().as_str() {
        "create" => Some(IntentId::Create),
        "fix" => Some(IntentId::Fix),
        "investigate" => Some(IntentId::Investigate),
        _ => None,
    }
}

fn sha256(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("sha256:{:x}", digest.finalize())
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::providers::AssistantReply;
    use crate::state::ConversationMessage;
    use crate::tools::registry::ToolSpec;
    use crate::tui::boundary_shell::route::{ExplicitRouteBinding, RouteBasis};

    use super::*;

    #[derive(Clone)]
    struct StubClassifier {
        response: anyhow::Result<String, String>,
        calls: Arc<AtomicUsize>,
    }

    impl StubClassifier {
        fn responding(response: &str) -> Self {
            Self {
                response: Ok(response.to_string()),
                calls: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    impl ChatClient for StubClassifier {
        fn label(&self) -> &str {
            "stub"
        }

        fn boxed_clone(&self) -> Box<dyn ChatClient> {
            Box::new(self.clone())
        }

        fn chat(
            &mut self,
            _model: &str,
            _messages: &[ConversationMessage],
            _tools: &[ToolSpec],
            _native_tools_enabled: bool,
        ) -> anyhow::Result<AssistantReply> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.response
                .clone()
                .map(AssistantReply::text)
                .map_err(anyhow::Error::msg)
        }
    }

    fn candidate(family: TaskFamilyId) -> RouteCandidate {
        RouteCandidate {
            profile: ProfileId::Ingest,
            intent: IntentId::Create,
            family,
            bases: vec![RouteBasis {
                rule: "fixture",
                observation: family.to_string(),
            }],
            contract_ref: "docs/ingest-profile-contract.md",
        }
    }

    fn result(
        resolution: DeterministicResolution,
        candidates: Vec<RouteCandidate>,
    ) -> DeterministicRouteResult {
        DeterministicRouteResult {
            resolution,
            candidates,
            observations: Vec::new(),
            inventory_omitted: 0,
        }
    }

    fn config(root: &Path) -> Config {
        Config {
            workspace_root: root.to_path_buf(),
            state_dir: root.join("state"),
            eval_events_path: Some(root.join("events.jsonl")),
            completion_contract_path: None,
            yes: true,
            offline: false,
            context_budget: 1_000,
            model: "executor".to_string(),
            provider: crate::config::Provider::Ollama,
            tool_protocol: None,
            openai_api: crate::config::OpenAiApi::ChatCompletions,
            prompt_layout: crate::config::PromptLayout::Stable,
            plan_preset: crate::config::PlanPreset::None,
            intent_override: None,
            planner_model: "classifier".to_string(),
            planner_provider: crate::config::Provider::Ollama,
            ollama_host: "http://localhost:11434".to_string(),
            num_predict: 100,
            max_iterations: 1,
            chat_timeout_secs: 1,
            chat_timeout_source: "override:test".to_string(),
            field_sources: crate::config::ConfigFieldSources::default(),
            chat_retries: 0,
            stream: false,
            resume: None,
            fresh_session: false,
            no_footer: false,
            narration: crate::config::NarrationMode::Normal,
            profile: "generic".to_string(),
            profile_explicit: false,
            profile_inference: None,
            style: "default".to_string(),
            action: crate::config::Action::Repl,
        }
    }

    #[test]
    fn deterministic_unique_never_calls_the_classifier_but_still_requires_gate_one() {
        let dir = tempfile::tempdir().unwrap();
        let mut classifier = StubClassifier::responding("unused");
        let proposal = propose_route(
            result(
                DeterministicResolution::Unique,
                vec![candidate(TaskFamilyId::List)],
            ),
            "request",
            "stub",
            "stub-model",
            &mut classifier,
            &config(dir.path()),
            &|| false,
        );
        assert_eq!(classifier.calls.load(Ordering::SeqCst), 0);
        assert_eq!(proposal.selected.unwrap().family, TaskFamilyId::List);
        assert_eq!(proposal.status, ProposalStatus::AwaitingConfirmation);
        assert!(proposal.confirmation_required);
    }

    #[test]
    fn registered_ambiguous_output_selects_a_proposal_without_dispatch_authority() {
        let dir = tempfile::tempdir().unwrap();
        let mut classifier = StubClassifier::responding(
            r#"{"profile":"ingest","intent":"create","family":"table"}"#,
        );
        let proposal = propose_route(
            result(
                DeterministicResolution::Ambiguous,
                vec![
                    candidate(TaskFamilyId::List),
                    candidate(TaskFamilyId::Table),
                ],
            ),
            "request",
            "stub",
            "stub-model",
            &mut classifier,
            &config(dir.path()),
            &|| false,
        );
        assert_eq!(classifier.calls.load(Ordering::SeqCst), 1);
        assert_eq!(proposal.selected.unwrap().family, TaskFamilyId::Table);
        assert!(proposal.confirmation_required);
        assert_eq!(proposal.status, ProposalStatus::AwaitingConfirmation);
        assert_eq!(proposal.classifier.parse_reason, "closed_candidate_match");
        let events = std::fs::read_to_string(dir.path().join("events.jsonl")).unwrap();
        assert!(events.contains("\"event\":\"provider_turn_duration\""));
        assert!(events.contains("\"caller_scope\":\"planner_step\""));
    }

    #[test]
    fn unregistered_or_unstable_output_falls_to_typed_unknown() {
        let dir = tempfile::tempdir().unwrap();
        let config = config(dir.path());
        for response in [
            r#"{"profile":"ingest","intent":"create","family":"invented"}"#,
            r#"{"profile":"ingest","intent":"create","family":"list","confidence":1}"#,
            "I choose list",
        ] {
            let mut classifier = StubClassifier::responding(response);
            let proposal = propose_route(
                result(
                    DeterministicResolution::Ambiguous,
                    vec![
                        candidate(TaskFamilyId::List),
                        candidate(TaskFamilyId::Table),
                    ],
                ),
                "request",
                "stub",
                "stub-model",
                &mut classifier,
                &config,
                &|| false,
            );
            assert!(proposal.selected.is_none(), "{response}");
            assert!(
                proposal
                    .classifier
                    .parse_reason
                    .starts_with("typed_unknown:")
            );
            assert!(proposal.confirmation_required);
        }
    }

    #[test]
    fn no_candidate_is_unknown_without_giving_the_llm_an_open_vocabulary() {
        let dir = tempfile::tempdir().unwrap();
        let mut classifier = StubClassifier::responding(
            r#"{"profile":"invented","intent":"create","family":"invented"}"#,
        );
        let proposal = propose_route(
            result(DeterministicResolution::Unknown, Vec::new()),
            "request",
            "stub",
            "stub-model",
            &mut classifier,
            &config(dir.path()),
            &|| false,
        );
        assert_eq!(classifier.calls.load(Ordering::SeqCst), 0);
        assert!(proposal.selected.is_none());
        assert_eq!(
            proposal.classifier.parse_reason,
            "typed_unknown:no_deterministic_candidate"
        );
    }

    #[test]
    fn oversized_classifier_output_is_rejected_inside_the_provider_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let response = "x".repeat(CLASSIFIER_MAX_RESPONSE_BYTES + 1);
        let mut classifier = StubClassifier::responding(&response);
        let proposal = propose_route(
            result(
                DeterministicResolution::Ambiguous,
                vec![
                    candidate(TaskFamilyId::List),
                    candidate(TaskFamilyId::Table),
                ],
            ),
            "request",
            "stub",
            "stub-model",
            &mut classifier,
            &config(dir.path()),
            &|| false,
        );
        assert!(proposal.selected.is_none());
        assert!(
            proposal
                .classifier
                .parse_reason
                .contains("provider_response_limit")
        );
        let events = std::fs::read_to_string(dir.path().join("events.jsonl")).unwrap();
        assert!(events.contains("\"finish_reason\":\"error\""));
        assert!(events.contains("\"ok\":false"));
    }

    #[test]
    fn cancelled_classifier_call_falls_to_typed_unknown_without_provider_work() {
        let dir = tempfile::tempdir().unwrap();
        let mut classifier =
            StubClassifier::responding(r#"{"profile":"ingest","intent":"create","family":"list"}"#);
        let proposal = propose_route(
            result(
                DeterministicResolution::Ambiguous,
                vec![
                    candidate(TaskFamilyId::List),
                    candidate(TaskFamilyId::Table),
                ],
            ),
            "request",
            "stub",
            "stub-model",
            &mut classifier,
            &config(dir.path()),
            &|| true,
        );

        assert!(proposal.selected.is_none());
        assert_eq!(classifier.calls.load(Ordering::SeqCst), 0);
        assert!(proposal.classifier.parse_reason.contains("aborted_by_user"));
        let events = std::fs::read_to_string(dir.path().join("events.jsonl")).unwrap();
        assert!(events.contains("\"event\":\"provider_turn_aborted_by_user\""));
    }

    #[test]
    fn ambiguity_adapter_has_no_execution_callback() {
        let _binding = ExplicitRouteBinding::default();
        let source = include_str!("ambiguity.rs");
        let production = source.split("#[cfg(test)]").next().unwrap_or(source);
        let forbidden = [
            ["run_", "ultra_plan"],
            ["generate_", "and_run"],
            ["Action", "::"],
        ];
        for parts in forbidden {
            let needle = parts.concat();
            assert!(
                !production.contains(&needle),
                "unexpected execution authority: {needle}"
            );
        }
    }
}
