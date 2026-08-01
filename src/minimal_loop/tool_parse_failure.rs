use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde::Serialize;
use serde_json::json;

use crate::config::Config;
use crate::eval_events;
use crate::evidence_envelope::{EvidenceEnvelopeSpec, EvidenceFamily};

pub(crate) const RAW_EXCERPT_MAX_BYTES: usize = 512;
const EVIDENCE_DIR: &str = ".anvil/evidence";
const EVIDENCE_STEM: &str = "tool-parse-failure";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolParseFailureKind {
    MalformedXml,
    JsonTrailing,
    MissingCall,
    UnknownTool,
    MissingName,
    InvalidPayload,
    JsonSyntax,
    Other,
}

impl ToolParseFailureKind {
    fn from_error(error: &str) -> Self {
        let lower = error.to_ascii_lowercase();
        if lower.contains("malformed xml tool call") {
            Self::MalformedXml
        } else if lower.contains("trailing characters") {
            Self::JsonTrailing
        } else if lower.contains("missing tool call") {
            Self::MissingCall
        } else if lower.contains("unknown tool") {
            Self::UnknownTool
        } else if lower.contains("tool call missing name") {
            Self::MissingName
        } else if lower.contains("payload must be an object") {
            Self::InvalidPayload
        } else if lower.contains("line ") || lower.contains("column ") {
            Self::JsonSyntax
        } else {
            Self::Other
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::MalformedXml => "malformed_xml",
            Self::JsonTrailing => "json_trailing",
            Self::MissingCall => "missing_call",
            Self::UnknownTool => "unknown_tool",
            Self::MissingName => "missing_name",
            Self::InvalidPayload => "invalid_payload",
            Self::JsonSyntax => "json_syntax",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct RawExcerpt {
    text: String,
    max_bytes: usize,
    raw_response_bytes: usize,
    start_byte: usize,
    end_byte: usize,
    truncated_before: bool,
    truncated_after: bool,
}

#[derive(Debug, Clone, Serialize)]
struct CalibrationObservation<'a> {
    model: &'a str,
    protocol: &'static str,
    failure_kind: &'static str,
    parse_error: &'a str,
    raw_excerpt: &'a RawExcerpt,
    phase: &'a str,
}

#[derive(Debug, Clone, Serialize)]
struct CalibrationClaim<'a> {
    claim: &'static str,
    observation: CalibrationObservation<'a>,
}

#[derive(Debug, Clone, Serialize)]
struct ToolParseFailureRecord<'a> {
    model: &'a str,
    protocol: &'static str,
    failure_kind: &'static str,
    parse_error: &'a str,
    raw_excerpt: &'a RawExcerpt,
    phase: &'a str,
    claims: [CalibrationClaim<'a>; 1],
}

pub(crate) fn record_normalization_failure(
    config: &Config,
    phase: Option<&str>,
    raw_response: &str,
    error: &anyhow::Error,
) {
    record_best_effort(
        config,
        phase,
        raw_response,
        &error.to_string(),
        ToolParseFailureKind::from_error(&error.to_string()),
    );
}

pub(crate) fn record_missing_call(
    config: &Config,
    phase: Option<&str>,
    raw_response: &str,
    error: &str,
) {
    record_best_effort(
        config,
        phase,
        raw_response,
        error,
        ToolParseFailureKind::MissingCall,
    );
}

fn record_best_effort(
    config: &Config,
    phase: Option<&str>,
    raw_response: &str,
    error: &str,
    kind: ToolParseFailureKind,
) {
    if let Err(record_error) = record(
        &config.workspace_root,
        config.eval_events_path.as_deref(),
        &config.model,
        phase.unwrap_or(""),
        raw_response,
        error,
        kind,
    ) {
        eprintln!("warning: failed to record tool parse evidence: {record_error}");
    }
}

fn record(
    root: &Path,
    events_path: Option<&Path>,
    model: &str,
    phase: &str,
    raw_response: &str,
    error: &str,
    kind: ToolParseFailureKind,
) -> anyhow::Result<PathBuf> {
    let relative = next_evidence_path(root)?;
    let absolute = root.join(&relative);
    let parse_error = eval_events::body_snippet(error);
    let excerpt = bounded_scrubbed_excerpt(raw_response, error, kind);
    let failure_kind = kind.as_str();
    let observation = CalibrationObservation {
        model,
        protocol: "text",
        failure_kind,
        parse_error: &parse_error,
        raw_excerpt: &excerpt,
        phase,
    };
    let record = ToolParseFailureRecord {
        model,
        protocol: "text",
        failure_kind,
        parse_error: &parse_error,
        raw_excerpt: &excerpt,
        phase,
        claims: [CalibrationClaim {
            claim: failure_kind,
            observation,
        }],
    };
    let source_refs = events_path
        .and_then(|path| path.strip_prefix(root).ok())
        .map(|path| vec![path.to_string_lossy().into_owned()])
        .unwrap_or_default();
    let write_result = crate::evidence_envelope::write_json(
        &absolute,
        &record,
        EvidenceEnvelopeSpec::new(EvidenceFamily::ToolParse, "tool_parse_failure")
            .with_source_refs(source_refs),
        true,
    )
    .with_context(|| format!("write {}", relative.display()));

    let envelope = crate::evidence_envelope::event_envelope(
        EvidenceFamily::ToolParse,
        "tool_parse_failure",
        crate::evidence_envelope::unix_epoch(),
        [relative.to_string_lossy().into_owned()],
    );
    eval_events::emit(
        events_path,
        json!({
            "event": "tool_parse_failure",
            "model": model,
            "protocol": "text",
            "failure_kind": failure_kind,
            "parse_error": parse_error,
            "raw_excerpt": excerpt,
            "phase": phase,
            "evidence_path": relative,
            "evidence_recorded": write_result.is_ok(),
            "evidence_envelope": envelope,
        }),
    );
    write_result?;
    Ok(absolute)
}

fn next_evidence_path(root: &Path) -> anyhow::Result<PathBuf> {
    let directory = root.join(EVIDENCE_DIR);
    std::fs::create_dir_all(&directory)?;
    for index in 1..=9_999 {
        let relative = PathBuf::from(EVIDENCE_DIR).join(format!("{EVIDENCE_STEM}-{index:03}.json"));
        if !root.join(&relative).exists() {
            return Ok(relative);
        }
    }
    bail!("tool parse evidence sequence exhausted")
}

fn bounded_scrubbed_excerpt(raw: &str, error: &str, kind: ToolParseFailureKind) -> RawExcerpt {
    let raw_bytes = raw.len();
    let point = failure_byte_offset(raw, error, kind).min(raw_bytes);
    let mut start = point.saturating_sub(RAW_EXCERPT_MAX_BYTES / 2);
    let mut end = (start + RAW_EXCERPT_MAX_BYTES).min(raw_bytes);
    if end - start < RAW_EXCERPT_MAX_BYTES {
        start = end.saturating_sub(RAW_EXCERPT_MAX_BYTES);
    }
    while start < raw_bytes && !raw.is_char_boundary(start) {
        start += 1;
    }
    while end > start && !raw.is_char_boundary(end) {
        end -= 1;
    }
    let text = truncate_utf8_bytes(&scrub_sensitive(&raw[start..end]), RAW_EXCERPT_MAX_BYTES);
    RawExcerpt {
        text,
        max_bytes: RAW_EXCERPT_MAX_BYTES,
        raw_response_bytes: raw_bytes,
        start_byte: start,
        end_byte: end,
        truncated_before: start > 0,
        truncated_after: end < raw_bytes,
    }
}

fn failure_byte_offset(raw: &str, error: &str, kind: ToolParseFailureKind) -> usize {
    if let Some((line, column)) = line_and_column(error) {
        let payload_start = tool_payload_start(raw).unwrap_or(0);
        let line_start = raw[payload_start..]
            .split_inclusive('\n')
            .take(line.saturating_sub(1))
            .map(str::len)
            .sum::<usize>();
        return payload_start
            .saturating_add(line_start)
            .saturating_add(column.saturating_sub(1));
    }
    if kind == ToolParseFailureKind::MalformedXml {
        return raw.len();
    }
    0
}

fn tool_payload_start(raw: &str) -> Option<usize> {
    [
        "<anvil_tool_call",
        "<tool_call",
        "<function_call",
        "<function",
    ]
    .into_iter()
    .filter_map(|marker| raw.rfind(marker))
    .max()
    .and_then(|start| raw[start..].find('>').map(|end| start + end + 1))
}

fn line_and_column(error: &str) -> Option<(usize, usize)> {
    let line_tail = error.rsplit_once(" line ")?.1;
    let (line, column_tail) = line_tail.split_once(" column ")?;
    let column = column_tail.split(|ch: char| !ch.is_ascii_digit()).next()?;
    Some((line.parse().ok()?, column.parse().ok()?))
}

fn truncate_utf8_bytes(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}

fn scrub_sensitive(value: &str) -> String {
    let mut scrubbed = redact_prefixed_token(value, "sk-");
    scrubbed = redact_prefixed_token(&scrubbed, "AIza");
    redact_home_paths(&scrubbed)
}

fn redact_prefixed_token(value: &str, prefix: &str) -> String {
    let mut output = value.to_string();
    let mut search_from = 0usize;
    while let Some(relative) = output[search_from..].find(prefix) {
        let start = search_from + relative;
        let mut end = start + prefix.len();
        for (offset, ch) in output[end..].char_indices() {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                end = start + prefix.len() + offset + ch.len_utf8();
            } else {
                break;
            }
        }
        output.replace_range(start..end, "<redacted>");
        search_from = start + "<redacted>".len();
    }
    output
}

fn redact_home_paths(value: &str) -> String {
    let mut output = value.to_string();
    for prefix in ["/Users/", "/home/"] {
        let mut search_from = 0usize;
        while let Some(relative) = output[search_from..].find(prefix) {
            let start = search_from + relative;
            let name_start = start + prefix.len();
            let Some(name_len) = output[name_start..].find('/') else {
                break;
            };
            let name_end = name_start + name_len;
            output.replace_range(name_start..name_end, "<user>");
            search_from = name_start + "<user>".len();
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use crate::config::{
        Action, ConfigFieldSources, NarrationMode, PlanPreset, PromptLayout, Provider, ToolProtocol,
    };
    use crate::providers::{AssistantReply, ChatClient};
    use crate::state::{ConversationMessage, SessionSnapshot};
    use crate::tools::registry::ToolSpec;

    #[derive(Clone)]
    struct ScriptedClient {
        replies: Arc<Mutex<Vec<AssistantReply>>>,
    }

    impl ScriptedClient {
        fn new(replies: Vec<AssistantReply>) -> Self {
            Self {
                replies: Arc::new(Mutex::new(replies)),
            }
        }
    }

    impl ChatClient for ScriptedClient {
        fn label(&self) -> &str {
            "tool-parse-fixture"
        }

        fn boxed_clone(&self) -> Box<dyn ChatClient> {
            Box::new(self.clone())
        }

        fn supports_native_tools(&self, _model: &str) -> bool {
            true
        }

        fn chat(
            &mut self,
            _model: &str,
            _messages: &[ConversationMessage],
            _tools: &[ToolSpec],
            _native_tools_enabled: bool,
        ) -> anyhow::Result<AssistantReply> {
            Ok(self.replies.lock().unwrap().remove(0))
        }
    }

    fn text_config(root: &Path, events: PathBuf) -> Config {
        Config {
            workspace_root: root.to_path_buf(),
            state_dir: root.join("state"),
            eval_events_path: Some(events),
            completion_contract_path: None,
            yes: true,
            offline: false,
            context_budget: 1_000,
            model: "gpt-fixture".to_string(),
            provider: Provider::Openai,
            tool_protocol: Some(ToolProtocol::Text),
            prompt_layout: PromptLayout::Stable,
            plan_preset: PlanPreset::None,
            intent_override: None,
            planner_model: "planner-fixture".to_string(),
            planner_provider: Provider::Ollama,
            ollama_host: "http://localhost:11434".to_string(),
            num_predict: 100,
            max_iterations: 4,
            chat_timeout_secs: 1,
            chat_timeout_source: "override:test".to_string(),
            field_sources: ConfigFieldSources::default(),
            chat_retries: 1,
            stream: false,
            resume: None,
            fresh_session: false,
            no_footer: false,
            narration: NarrationMode::Normal,
            profile: "generic".to_string(),
            profile_explicit: false,
            profile_inference: None,
            style: "default".to_string(),
            action: Action::Repl,
        }
    }

    fn event_values(path: &Path) -> Vec<serde_json::Value> {
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn failure_kinds_are_stable_and_specific() {
        assert_eq!(
            ToolParseFailureKind::from_error("malformed XML tool call"),
            ToolParseFailureKind::MalformedXml
        );
        assert_eq!(
            ToolParseFailureKind::from_error("trailing characters at line 1 column 121"),
            ToolParseFailureKind::JsonTrailing
        );
        assert_eq!(
            ToolParseFailureKind::from_error("missing tool call for action prompt after feedback"),
            ToolParseFailureKind::MissingCall
        );
    }

    #[test]
    fn writes_additive_enveloped_event_and_scrubbed_bounded_evidence() {
        let root = tempfile::tempdir().unwrap();
        let events = root.path().join(".anvil/runs/test/events.jsonl");
        let raw = format!(
            "{}<anvil_tool_call>{{\"name\":\"Write\"}}</anvil_tool_call> trailing sk-secret-value /Users/example/project",
            "前".repeat(180)
        );

        let evidence = record(
            root.path(),
            Some(&events),
            "gpt-fixture",
            "implement-cli-tool",
            &raw,
            "trailing characters at line 1 column 560",
            ToolParseFailureKind::JsonTrailing,
        )
        .unwrap();

        let evidence_text = std::fs::read_to_string(evidence).unwrap();
        let event_text = std::fs::read_to_string(events).unwrap();
        for text in [&evidence_text, &event_text] {
            assert!(text.contains("tool_parse_failure"));
            assert!(text.contains("json_trailing"));
            assert!(text.contains("gpt-fixture"));
            assert!(text.contains("implement-cli-tool"));
            assert!(text.contains("<redacted>"));
            assert!(text.contains("/Users/<user>/project"));
            assert!(!text.contains("sk-secret-value"));
        }
        let document: serde_json::Value = serde_json::from_str(&evidence_text).unwrap();
        assert_eq!(document["evidence_envelope"]["family"], "tool_parse");
        assert_eq!(document["evidence_envelope"]["kind"], "tool_parse_failure");
        assert!(document["raw_excerpt"]["text"].as_str().unwrap().len() <= RAW_EXCERPT_MAX_BYTES);
    }

    #[test]
    fn multibyte_excerpt_respects_the_byte_cap() {
        let excerpt = bounded_scrubbed_excerpt(
            &"日本語".repeat(300),
            "malformed XML tool call",
            ToolParseFailureKind::MalformedXml,
        );
        assert!(excerpt.text.len() <= RAW_EXCERPT_MAX_BYTES);
        assert!(excerpt.truncated_before);
    }

    #[test]
    fn json_column_is_mapped_from_the_tool_payload_not_leading_prose() {
        let raw = format!(
            "{}<anvil_tool_call>{{\"name\":\"Write\"}} trailing-marker</anvil_tool_call>",
            "p".repeat(800)
        );
        let excerpt = bounded_scrubbed_excerpt(
            &raw,
            "trailing characters at line 1 column 17",
            ToolParseFailureKind::JsonTrailing,
        );
        assert!(excerpt.text.contains("trailing-marker"));
        assert!(excerpt.truncated_before);
    }

    #[test]
    fn production_loop_records_text_parser_failure() {
        let root = tempfile::tempdir().unwrap();
        let events = root.path().join("events.jsonl");
        let mut client = ScriptedClient::new(vec![AssistantReply::text(
            "<anvil_tool_call>{\"name\":\"Write\"",
        )]);
        let mut session = SessionSnapshot::new();

        let error = super::super::loop_run::run_session(
            &mut client,
            &mut session,
            "create a.txt",
            &text_config(root.path(), events.clone()),
        )
        .unwrap_err()
        .to_string();

        assert_eq!(error, "malformed XML tool call");
        assert!(event_values(&events).iter().any(|event| {
            event["event"] == "tool_parse_failure"
                && event["failure_kind"] == "malformed_xml"
                && event["evidence_envelope"]["family"] == "tool_parse"
        }));
    }

    #[test]
    fn production_loop_records_missing_call_after_feedback() {
        let root = tempfile::tempdir().unwrap();
        let events = root.path().join("events.jsonl");
        let mut client = ScriptedClient::new(vec![
            AssistantReply::text("I will create it."),
            AssistantReply::text("I will create it now."),
        ]);
        let mut session = SessionSnapshot::new();

        let error = super::super::loop_run::run_session(
            &mut client,
            &mut session,
            "create a.txt",
            &text_config(root.path(), events.clone()),
        )
        .unwrap_err()
        .to_string();

        assert_eq!(error, "missing tool call for action prompt after feedback");
        assert!(event_values(&events).iter().any(|event| {
            event["event"] == "tool_parse_failure"
                && event["failure_kind"] == "missing_call"
                && event["evidence_recorded"] == true
        }));
    }
}
