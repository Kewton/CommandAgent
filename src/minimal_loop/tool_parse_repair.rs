use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde::Serialize;
use serde_json::json;

use crate::config::Config;
use crate::evidence_envelope::{EvidenceEnvelopeSpec, EvidenceFamily};
use crate::providers::xml_repair::ToolCallRepair;

const CHANGE_EXCERPT_MAX_BYTES: usize = 256;
const EVIDENCE_DIR: &str = ".commandagent/evidence";
const EVIDENCE_STEM: &str = "tool-parse-repair";

#[derive(Debug, Serialize)]
struct ChangeExcerpt {
    operation: &'static str,
    text: String,
    max_bytes: usize,
    original_bytes: usize,
    truncated: bool,
}

#[derive(Debug, Serialize)]
struct RepairObservation<'a> {
    model: &'a str,
    protocol: &'static str,
    repair_kind: &'static str,
    change_excerpt: &'a ChangeExcerpt,
    phase: &'a str,
}

#[derive(Debug, Serialize)]
struct RepairClaim<'a> {
    claim: &'static str,
    observation: RepairObservation<'a>,
}

#[derive(Debug, Serialize)]
struct RepairRecord<'a> {
    model: &'a str,
    protocol: &'static str,
    repair_kind: &'static str,
    change_excerpt: &'a ChangeExcerpt,
    phase: &'a str,
    claims: [RepairClaim<'a>; 1],
}

pub(crate) fn record_applied(
    config: &Config,
    phase: Option<&str>,
    repair: &ToolCallRepair,
) -> anyhow::Result<PathBuf> {
    record(
        &config.workspace_root,
        config.eval_events_path.as_deref(),
        &config.model,
        phase.unwrap_or(""),
        repair,
    )
}

fn record(
    root: &Path,
    events_path: Option<&Path>,
    model: &str,
    phase: &str,
    repair: &ToolCallRepair,
) -> anyhow::Result<PathBuf> {
    let events_path = events_path.context("repair_applied requires an event sink")?;
    let relative = next_evidence_path(root)?;
    let absolute = root.join(&relative);
    let change_excerpt = bounded_scrubbed_change(repair);
    let repair_kind = repair.kind.as_str();
    let observation = RepairObservation {
        model,
        protocol: "text",
        repair_kind,
        change_excerpt: &change_excerpt,
        phase,
    };
    let record = RepairRecord {
        model,
        protocol: "text",
        repair_kind,
        change_excerpt: &change_excerpt,
        phase,
        claims: [RepairClaim {
            claim: repair_kind,
            observation,
        }],
    };
    let source_refs = events_path
        .strip_prefix(root)
        .ok()
        .map(|path| vec![path.to_string_lossy().into_owned()])
        .unwrap_or_default();
    crate::evidence_envelope::write_json(
        &absolute,
        &record,
        EvidenceEnvelopeSpec::new(EvidenceFamily::ToolParse, "repair_applied")
            .with_source_refs(source_refs),
        true,
    )
    .with_context(|| format!("write {}", relative.display()))?;

    let envelope = crate::evidence_envelope::event_envelope(
        EvidenceFamily::ToolParse,
        "repair_applied",
        crate::evidence_envelope::unix_epoch(),
        [relative.to_string_lossy().into_owned()],
    );
    crate::eval_events::emit(
        Some(events_path),
        json!({
            "event": "repair_applied",
            "model": model,
            "protocol": "text",
            "repair_kind": repair_kind,
            "change_excerpt": change_excerpt,
            "phase": phase,
            "evidence_path": relative,
            "evidence_recorded": true,
            "evidence_envelope": envelope,
        }),
    );
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
    bail!("tool parse repair evidence sequence exhausted")
}

fn bounded_scrubbed_change(repair: &ToolCallRepair) -> ChangeExcerpt {
    let scrubbed = super::tool_parse_failure::scrub_sensitive(&repair.change);
    let text = super::tool_parse_failure::truncate_utf8_bytes(&scrubbed, CHANGE_EXCERPT_MAX_BYTES);
    ChangeExcerpt {
        operation: repair.operation,
        text,
        max_bytes: CHANGE_EXCERPT_MAX_BYTES,
        original_bytes: repair.change.len(),
        truncated: scrubbed.len() > CHANGE_EXCERPT_MAX_BYTES,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::xml_repair::ToolCallRepairKind;

    #[test]
    fn repair_evidence_is_enveloped_bounded_and_scrubbed() {
        let root = tempfile::tempdir().unwrap();
        let events = root.path().join(".anvil/runs/test/events.jsonl");
        let repair = ToolCallRepair {
            kind: ToolCallRepairKind::FirstJsonValue,
            operation: "discarded",
            change: format!("}} sk-secret-value {}", "x".repeat(400)),
        };

        let path = record(
            root.path(),
            Some(&events),
            "gpt-fixture",
            "create-sample-data",
            &repair,
        )
        .unwrap();

        let evidence = std::fs::read_to_string(path).unwrap();
        let event = std::fs::read_to_string(events).unwrap();
        for text in [&evidence, &event] {
            assert!(text.contains("repair_applied"));
            assert!(text.contains("first_json_value"));
            assert!(text.contains("<redacted>"));
            assert!(!text.contains("sk-secret-value"));
        }
        let document: serde_json::Value = serde_json::from_str(&evidence).unwrap();
        assert_eq!(document["change_excerpt"]["max_bytes"], 256);
        assert_eq!(document["evidence_envelope"]["family"], "tool_parse");
        assert_eq!(document["evidence_envelope"]["kind"], "repair_applied");
    }
}
