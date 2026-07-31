use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::Serialize;
use serde_json::{Value, json};

use crate::evidence_envelope::{EvidenceEnvelopeSpec, EvidenceFamily};
use crate::planner::failure_vocabulary::ViolationId;

pub(crate) const CHECK_ID: &str = "nextjs_testimony_binding";
pub(crate) const EVIDENCE_RELATIVE_PATH: &str = ".anvil/evidence/nextjs-testimony-binding.json";

const MAX_CLAIMS: usize = 128;
const MAX_NEAREST_MISS_CHARS: usize = 240;
const TESTIMONY_SOURCES: &[&str] = &[
    "README.md",
    "GOAL_RESPONSE.md",
    "goal-response.md",
    "output/response.md",
];

/// Closed T1 recognition vocabulary fixed by
/// `docs/nextjs-profile-contract.md` section 6.
///
/// Gate-to-guidance correspondence (complete):
///
/// | typed claim | registered lexical anchors | execution observation |
/// |---|---|---|
/// | `route` | route, page, path, URL, ルート, ページ | browser readiness route |
/// | `interaction` | interact, click, select, answer, input, 操作, クリック, 選択, 回答, 入力 | browser input/state observation |
/// | `score` | score, point, スコア, 得点 | changed `score` state dimension |
/// | `restart` | retry, restart, reset, リトライ, 再試行, 再開, リセット | recovery transition |
/// | `persistence` | persist, reload, localStorage, save, 保持, 永続, 保存, リロード | after-reload persistence |
///
/// Recognition shapes are likewise closed: Markdown paragraph, list item,
/// table cell, and `feature`/`result` labelled line. Headings, code fences,
/// commands, and metadata are never promoted to claims. Adding a gate without
/// adding its literal vocabulary here would recreate the DATA-1 guidance gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TestimonyClaimKind {
    Route,
    Interaction,
    Score,
    Restart,
    Persistence,
}

impl TestimonyClaimKind {
    fn anchors(self) -> &'static [&'static str] {
        match self {
            Self::Route => &["route", "page", "path", "url", "ルート", "ページ"],
            Self::Interaction => &[
                "interact",
                "click",
                "select",
                "answer",
                "input",
                "操作",
                "クリック",
                "選択",
                "回答",
                "入力",
            ],
            Self::Score => &["score", "point", "スコア", "得点"],
            Self::Restart => &[
                "retry",
                "restart",
                "reset",
                "リトライ",
                "再試行",
                "再開",
                "リセット",
            ],
            Self::Persistence => &[
                "persist",
                "reload",
                "localstorage",
                "save",
                "保持",
                "永続",
                "保存",
                "リロード",
            ],
        }
    }

    fn all() -> &'static [Self] {
        &[
            Self::Route,
            Self::Interaction,
            Self::Score,
            Self::Restart,
            Self::Persistence,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TestimonyRecognitionShape {
    Paragraph,
    ListItem,
    TableCell,
    LabelledFeature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TestimonyClaimResult {
    Matched,
    TestimonyBindingViolation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TestimonyViolationReason {
    #[serde(rename = "route_not_observed")]
    Route,
    #[serde(rename = "interaction_not_observed")]
    Interaction,
    #[serde(rename = "score_change_not_observed")]
    ScoreChange,
    #[serde(rename = "restart_not_observed")]
    Restart,
    #[serde(rename = "persistence_not_observed")]
    Persistence,
}

impl TestimonyViolationReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::Route => "route_not_observed",
            Self::Interaction => "interaction_not_observed",
            Self::ScoreChange => "score_change_not_observed",
            Self::Restart => "restart_not_observed",
            Self::Persistence => "persistence_not_observed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct TestimonyClaimEvidence {
    pub index: usize,
    pub claim: String,
    pub source_ref: String,
    pub line: usize,
    pub shape: TestimonyRecognitionShape,
    pub anchor: String,
    pub claim_kind: TestimonyClaimKind,
    pub matched: bool,
    pub result: TestimonyClaimResult,
    pub observation: Value,
    pub evidence_refs: Vec<String>,
    pub nearest_miss: Option<String>,
    pub violation: Option<String>,
    pub violation_reason: Option<TestimonyViolationReason>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct TestimonyBindingReport {
    pub schema_version: u8,
    pub check_id: &'static str,
    pub status: &'static str,
    pub claims_absent: bool,
    pub recognized_claim_count: usize,
    pub matched_claim_count: usize,
    pub violation_count: usize,
    pub unrecognized_prose: usize,
    pub claims: Vec<TestimonyClaimEvidence>,
    pub source_files: Vec<String>,
    pub compared_evidence: Vec<String>,
    pub violations: Vec<String>,
}

impl TestimonyBindingReport {
    pub(crate) fn failed(&self) -> bool {
        self.violation_count > 0
    }
}

#[derive(Debug)]
struct ExtractedClaim {
    claim: String,
    source_ref: String,
    line: usize,
    shape: TestimonyRecognitionShape,
    anchor: String,
    kind: TestimonyClaimKind,
}

#[derive(Debug, Default)]
struct ObservationSet {
    readiness: Value,
    interaction: Value,
    evidence_refs: Vec<String>,
}

pub(crate) fn evaluate(
    root: &Path,
    browser_readiness_path: Option<&str>,
    interaction_evidence_path: Option<&str>,
) -> anyhow::Result<TestimonyBindingReport> {
    let (extracted, source_files, unrecognized_prose) = extract_claims(root)?;
    let observations = observations(root, browser_readiness_path, interaction_evidence_path)?;
    let mut claims = Vec::with_capacity(extracted.len());
    for (index, extracted) in extracted.into_iter().enumerate() {
        let (matched, observation, nearest_miss, violation_reason) =
            compare_claim(extracted.kind, &observations);
        let violation = violation_reason.map(|reason| {
            ViolationId::testimony_binding(format!("claim={index}:{}", reason.as_str())).to_string()
        });
        claims.push(TestimonyClaimEvidence {
            index,
            claim: extracted.claim,
            source_ref: extracted.source_ref,
            line: extracted.line,
            shape: extracted.shape,
            anchor: extracted.anchor,
            claim_kind: extracted.kind,
            matched,
            result: if matched {
                TestimonyClaimResult::Matched
            } else {
                TestimonyClaimResult::TestimonyBindingViolation
            },
            observation,
            evidence_refs: observations.evidence_refs.clone(),
            nearest_miss,
            violation,
            violation_reason,
        });
    }
    let violations = claims
        .iter()
        .filter_map(|claim| claim.violation.clone())
        .collect::<Vec<_>>();
    let matched_claim_count = claims.iter().filter(|claim| claim.matched).count();
    let claims_absent = claims.is_empty();
    let status = if !violations.is_empty() {
        "failed"
    } else if claims_absent {
        "claims_absent"
    } else {
        "passed"
    };
    Ok(TestimonyBindingReport {
        schema_version: 1,
        check_id: CHECK_ID,
        status,
        claims_absent,
        recognized_claim_count: claims.len(),
        matched_claim_count,
        violation_count: violations.len(),
        unrecognized_prose,
        claims,
        source_files,
        compared_evidence: observations.evidence_refs,
        violations,
    })
}

pub(crate) fn write_evidence(root: &Path, report: &TestimonyBindingReport) -> anyhow::Result<()> {
    let path = root.join(EVIDENCE_RELATIVE_PATH);
    let parent = path
        .parent()
        .context("T1 evidence path must have a parent")?;
    std::fs::create_dir_all(parent)?;
    crate::evidence_envelope::write_json(
        &path,
        report,
        EvidenceEnvelopeSpec::new(EvidenceFamily::T, "testimony_binding")
            .with_source_refs(report.source_files.iter().chain(&report.compared_evidence)),
        true,
    )
}

fn extract_claims(root: &Path) -> anyhow::Result<(Vec<ExtractedClaim>, Vec<String>, usize)> {
    let mut claims = Vec::new();
    let mut source_files = Vec::new();
    let mut unrecognized_prose = 0;
    for relative in TESTIMONY_SOURCES {
        let path = root.join(relative);
        if !path.is_file() {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("read Next.js testimony source {relative}"))?;
        source_files.push((*relative).to_string());
        extract_document(relative, &text, &mut claims, &mut unrecognized_prose);
        if claims.len() >= MAX_CLAIMS {
            claims.truncate(MAX_CLAIMS);
            break;
        }
    }
    Ok((claims, source_files, unrecognized_prose))
}

fn extract_document(
    source: &str,
    text: &str,
    claims: &mut Vec<ExtractedClaim>,
    unrecognized_prose: &mut usize,
) {
    let mut in_code_fence = false;
    for (line_index, raw) in text.lines().enumerate() {
        let trimmed = raw.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code_fence = !in_code_fence;
            continue;
        }
        if in_code_fence || ignored_markdown_line(trimmed) {
            continue;
        }
        let Some((shape, claim_text)) = recognition_shape(trimmed) else {
            continue;
        };
        let Some((kind, anchor)) = recognized_anchor(claim_text) else {
            *unrecognized_prose += 1;
            continue;
        };
        if claims.len() >= MAX_CLAIMS {
            break;
        }
        claims.push(ExtractedClaim {
            claim: claim_text.to_string(),
            source_ref: format!("{source}:{}", line_index + 1),
            line: line_index + 1,
            shape,
            anchor: anchor.to_string(),
            kind,
        });
    }
}

fn ignored_markdown_line(line: &str) -> bool {
    line.is_empty()
        || line.starts_with('#')
        || line.starts_with('>')
        || line.starts_with("<!--")
        || line.starts_with("---")
        || line.starts_with("===")
        || line.starts_with('$')
        || line.starts_with("npm ")
        || line.starts_with("pnpm ")
        || line.starts_with("yarn ")
}

fn recognition_shape(line: &str) -> Option<(TestimonyRecognitionShape, &str)> {
    if line.starts_with('|') && line.ends_with('|') && !line.contains("---") {
        return Some((
            TestimonyRecognitionShape::TableCell,
            line.trim_matches('|').trim(),
        ));
    }
    if let Some(rest) = strip_list_marker(line) {
        return Some((TestimonyRecognitionShape::ListItem, rest));
    }
    let lower = line.to_lowercase();
    if [
        "feature:",
        "features:",
        "result:",
        "機能:",
        "機能：",
        "結果:",
        "結果：",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
    {
        return Some((TestimonyRecognitionShape::LabelledFeature, line));
    }
    if line.contains(char::is_alphabetic) {
        return Some((TestimonyRecognitionShape::Paragraph, line));
    }
    None
}

fn strip_list_marker(line: &str) -> Option<&str> {
    for prefix in ["- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(prefix) {
            return Some(rest.trim());
        }
    }
    let (number, rest) = line.split_once(". ")?;
    (!number.is_empty() && number.chars().all(|ch| ch.is_ascii_digit())).then(|| rest.trim())
}

fn recognized_anchor(line: &str) -> Option<(TestimonyClaimKind, &str)> {
    let lower = line.to_lowercase();
    let mut earliest: Option<(usize, TestimonyClaimKind, &str)> = None;
    for kind in TestimonyClaimKind::all() {
        for anchor in kind.anchors() {
            if let Some(position) = anchor_position(&lower, anchor)
                && earliest.is_none_or(|(current, _, _)| position < current)
            {
                earliest = Some((position, *kind, anchor));
            }
        }
    }
    earliest.map(|(_, kind, anchor)| (kind, anchor))
}

fn contains_anchor(line: &str, anchor: &str) -> bool {
    anchor_position(line, anchor).is_some()
}

fn anchor_position(line: &str, anchor: &str) -> Option<usize> {
    if !anchor.is_ascii() {
        return line.find(anchor);
    }
    line.match_indices(anchor).find_map(|(start, _)| {
        let before = line[..start].chars().next_back();
        let end = start + anchor.len();
        let after = line[end..].chars().next();
        (before.is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_')
            && after.is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_'))
        .then_some(start)
    })
}

fn observations(
    root: &Path,
    browser_readiness_path: Option<&str>,
    interaction_evidence_path: Option<&str>,
) -> anyhow::Result<ObservationSet> {
    let readiness_path = resolve_evidence_path(
        root,
        browser_readiness_path,
        ".anvil/evidence/browser-readiness.json",
    );
    let interaction_path = resolve_evidence_path(
        root,
        interaction_evidence_path,
        ".anvil/evidence/browser-interaction.json",
    );
    let (readiness, readiness_ref) = read_json_if_file(root, readiness_path.as_deref())?;
    let (interaction, interaction_ref) = read_json_if_file(root, interaction_path.as_deref())?;
    let evidence_refs = [readiness_ref, interaction_ref]
        .into_iter()
        .flatten()
        .collect();
    Ok(ObservationSet {
        readiness,
        interaction,
        evidence_refs,
    })
}

fn resolve_evidence_path(root: &Path, supplied: Option<&str>, fallback: &str) -> Option<PathBuf> {
    supplied
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                root.join(path)
            }
        })
        .or_else(|| {
            let path = root.join(fallback);
            path.is_file().then_some(path)
        })
}

fn read_json_if_file(root: &Path, path: Option<&Path>) -> anyhow::Result<(Value, Option<String>)> {
    let Some(path) = path.filter(|path| path.is_file()) else {
        return Ok((Value::Null, None));
    };
    let bytes = std::fs::read(path)
        .with_context(|| format!("read Next.js execution evidence {}", path.display()))?;
    let value = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse Next.js execution evidence {}", path.display()))?;
    let relative = path
        .strip_prefix(root)
        .unwrap_or(path.file_name().map(Path::new).unwrap_or(path))
        .to_string_lossy()
        .replace('\\', "/");
    Ok((value, Some(relative)))
}

fn compare_claim(
    kind: TestimonyClaimKind,
    observations: &ObservationSet,
) -> (
    bool,
    Value,
    Option<String>,
    Option<TestimonyViolationReason>,
) {
    let (matched, observation, reason) = match kind {
        TestimonyClaimKind::Route => {
            let route_rendered = observations
                .readiness
                .get("route_rendered")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                || observations
                    .readiness
                    .get("ok")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
            (
                route_rendered,
                json!({
                    "route_rendered": route_rendered,
                    "route": observations.readiness.get("route").cloned().unwrap_or(Value::Null),
                    "http_status": observations.readiness.get("http_status").cloned().unwrap_or(Value::Null),
                }),
                TestimonyViolationReason::Route,
            )
        }
        TestimonyClaimKind::Interaction => {
            let input_event_observed =
                bool_field(&observations.interaction, "input_event_observed");
            let input_state_change = bool_field(&observations.interaction, "input_state_change");
            let matched = bool_field(&observations.interaction, "interaction_success")
                && input_event_observed
                && input_state_change;
            (
                matched,
                json!({
                    "interaction_success": bool_field(&observations.interaction, "interaction_success"),
                    "input_event_observed": input_event_observed,
                    "input_state_change": input_state_change,
                }),
                TestimonyViolationReason::Interaction,
            )
        }
        TestimonyClaimKind::Score => {
            let dimensions = string_array(&observations.interaction, "state_dimensions_changed");
            let score_changed = dimensions.iter().any(|value| value == "score");
            (
                score_changed,
                json!({"state_dimensions_changed": dimensions}),
                TestimonyViolationReason::ScoreChange,
            )
        }
        TestimonyClaimKind::Restart => {
            let matched = bool_field(&observations.interaction, "recovery_transition");
            (
                matched,
                json!({
                    "recovery_transition": matched,
                    "recovery_transition_status": observations.interaction.get("recovery_transition_status").cloned().unwrap_or(Value::Null),
                    "action_hooks": string_array(&observations.interaction, "action_hooks"),
                }),
                TestimonyViolationReason::Restart,
            )
        }
        TestimonyClaimKind::Persistence => {
            let state = observations
                .interaction
                .get("persistence_after_reload")
                .and_then(Value::as_str)
                .unwrap_or("");
            let matched = bool_field(&observations.interaction, "token_echoed_after_reload")
                || matches!(state, "passed" | "persisted" | "observed");
            (
                matched,
                json!({
                    "persistence_after_reload": state,
                    "token_echoed_after_reload": bool_field(&observations.interaction, "token_echoed_after_reload"),
                }),
                TestimonyViolationReason::Persistence,
            )
        }
    };
    if matched {
        (true, observation, None, None)
    } else {
        let nearest = bounded_nearest_miss(&observation);
        (false, observation, Some(nearest), Some(reason))
    }
}

fn bool_field(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn string_array(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn bounded_nearest_miss(value: &Value) -> String {
    let rendered = serde_json::to_string(value).unwrap_or_else(|_| "null".to_string());
    if rendered.chars().count() <= MAX_NEAREST_MISS_CHARS {
        rendered
    } else {
        let mut truncated = rendered
            .chars()
            .take(MAX_NEAREST_MISS_CHARS)
            .collect::<String>();
        truncated.push('…');
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL_QUIZ_README: &str =
        include_str!("../../../../tests/corpus/apps/nextjs-testimony-quiz/README.md");
    const REAL_MATCHED_READINESS: &str =
        include_str!("../../../../tests/corpus/apps/nextjs-testimony-quiz/browser-readiness.json");
    const REAL_MATCHED_INTERACTION: &str = include_str!(
        "../../../../tests/corpus/apps/nextjs-testimony-quiz/browser-interaction.json"
    );

    #[test]
    fn registered_anchors_obey_word_boundaries() {
        assert_eq!(
            recognized_anchor("The score changes after an answer.").map(|value| value.0),
            Some(TestimonyClaimKind::Score)
        );
        assert_eq!(
            recognized_anchor("The scoreboard is decorative."),
            None,
            "score must not match inside an unregistered word"
        );
        assert_eq!(
            recognized_anchor("リトライでスコアをリセットできます。").map(|value| value.0),
            Some(TestimonyClaimKind::Restart)
        );
    }

    #[test]
    fn code_and_headings_are_not_promoted_to_claims() {
        let mut claims = Vec::new();
        let mut unrecognized = 0;
        extract_document(
            "README.md",
            "# Score\n```tsx\nconst score = 1;\n```\nA quiet description.",
            &mut claims,
            &mut unrecognized,
        );
        assert!(claims.is_empty());
        assert_eq!(unrecognized, 1);
    }

    #[test]
    fn real_quiz_readme_and_execution_evidence_cover_three_outcomes() {
        let root = measured_fixture(REAL_MATCHED_INTERACTION);
        let report = evaluate(root.path(), None, None).unwrap();
        assert!(!report.claims_absent);
        assert!(report.matched_claim_count >= 2);
        assert_eq!(report.violation_count, 2);
        assert!(
            report.unrecognized_prose > 0,
            "unregistered real README prose must remain counted, not inferred"
        );
        assert!(
            report.claims.iter().any(|claim| {
                claim.claim_kind == TestimonyClaimKind::Score
                    && claim.result == TestimonyClaimResult::TestimonyBindingViolation
                    && claim.violation_reason == Some(TestimonyViolationReason::ScoreChange)
            }),
            "the real Quiz observation has no score state dimension"
        );
        assert!(
            report.claims.iter().any(|claim| {
                claim.claim_kind == TestimonyClaimKind::Restart
                    && claim.result == TestimonyClaimResult::TestimonyBindingViolation
            }),
            "the real Quiz observation did not reach the documented retry"
        );
        assert!(
            report.claims.iter().any(|claim| claim.matched),
            "the same real evidence must retain matched route/interaction claims"
        );
    }

    #[test]
    fn evidence_is_enveloped_and_does_not_mutate_existing_projection() {
        let root = measured_fixture(REAL_MATCHED_INTERACTION);
        let report = evaluate(root.path(), None, None).unwrap();
        let mut snapshot = crate::eval_events::CompletionSnapshot::empty();
        snapshot.profile = "nextjs".to_string();
        snapshot.effective_profile = "nextjs".to_string();
        snapshot.assurance_level = "full".to_string();
        let projection = crate::eval_events::project_completion(true, &snapshot);
        let expected_projection = projection.clone();

        write_evidence(root.path(), &report).unwrap();
        let document: Value = serde_json::from_slice(
            &std::fs::read(root.path().join(EVIDENCE_RELATIVE_PATH)).unwrap(),
        )
        .unwrap();
        assert_eq!(document["check_id"], CHECK_ID);
        assert_eq!(document["evidence_envelope"]["family"], "T");
        assert_eq!(document["evidence_envelope"]["kind"], "testimony_binding");
        assert_eq!(projection, expected_projection);
    }

    fn measured_fixture(interaction: &str) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let evidence = root.path().join(".anvil/evidence");
        std::fs::create_dir_all(&evidence).unwrap();
        std::fs::write(root.path().join("README.md"), REAL_QUIZ_README).unwrap();
        std::fs::write(
            evidence.join("browser-readiness.json"),
            REAL_MATCHED_READINESS,
        )
        .unwrap();
        std::fs::write(evidence.join("browser-interaction.json"), interaction).unwrap();
        root
    }
}
