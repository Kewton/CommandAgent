use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;

use anyhow::{Context, bail};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

mod css_selector;

pub const INSPECTION_PATH: &str = "output/inspection.json";
pub const FREEZE_EVIDENCE_PATH: &str = "evidence/ingest-candidate-freeze.json";
pub const ACCOUNTING_EVIDENCE_PATH: &str = "evidence/candidate-accounting.json";

const SNAPSHOT_ROOT: &str = "data/snapshots";
const MAX_SNAPSHOTS: usize = 256;
const MAX_SNAPSHOT_BYTES: u64 = 2 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectorKind {
    Css,
    LinePrefix,
    HtmlTag,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateSelector {
    pub kind: SelectorKind,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptedCandidate {
    pub candidate_id: String,
    pub record_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExcludedCandidate {
    pub candidate_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateAccounting {
    pub accepted: Vec<AcceptedCandidate>,
    pub excluded: Vec<ExcludedCandidate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InspectionDocument {
    pub candidate_selector: CandidateSelector,
    pub candidate_accounting: CandidateAccounting,
    #[serde(default)]
    pub record_format: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotSeal {
    pub path: String,
    pub bytes: u64,
    pub fnv1a64: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrozenCandidate {
    pub id: String,
    pub source_path: String,
    pub ordinal: usize,
    pub byte_start: usize,
    pub byte_end: usize,
    pub fnv1a64: String,
    #[serde(skip, default)]
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateFreeze {
    pub capability_id: String,
    pub selector: CandidateSelector,
    pub record_format: Value,
    pub snapshots: Vec<SnapshotSeal>,
    pub candidates: Vec<FrozenCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateAccountingEvidence {
    pub capability_id: String,
    pub status: String,
    pub ok: bool,
    pub selector: CandidateSelector,
    pub detected: usize,
    pub accepted: usize,
    pub excluded_by_reason: BTreeMap<String, usize>,
    pub equation: String,
    pub candidate_ids: Vec<String>,
    pub failure_kinds: Vec<String>,
}

pub fn freeze(root: &Path) -> anyhow::Result<CandidateFreeze> {
    let frozen = build_freeze(root)?;
    write_json(root, FREEZE_EVIDENCE_PATH, &frozen)?;
    Ok(frozen)
}

pub fn check(root: &Path, frozen: &CandidateFreeze) -> anyhow::Result<CandidateAccountingEvidence> {
    let inspection = load_inspection(root)?;
    let mut failure_kinds = Vec::new();
    if inspection.candidate_selector != frozen.selector {
        failure_kinds.push("candidate_set_violation:selector_changed".to_string());
    }
    match build_freeze(root) {
        Ok(current) if candidate_seals(&current) != candidate_seals(frozen) => {
            failure_kinds.push("candidate_set_violation:frozen_candidates_changed".to_string());
        }
        Ok(_) => {}
        Err(error) => {
            failure_kinds.push(format!("candidate_set_violation:reenumeration:{error}"));
        }
    }

    let known = frozen
        .candidates
        .iter()
        .map(|candidate| candidate.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut accounted = BTreeSet::new();
    let mut accepted_indices = BTreeSet::new();
    for accepted in &inspection.candidate_accounting.accepted {
        validate_candidate_id(
            &accepted.candidate_id,
            &known,
            &mut accounted,
            &mut failure_kinds,
        );
        if !accepted_indices.insert(accepted.record_index) {
            failure_kinds.push(format!(
                "accounting_violation:duplicate_record_index:{}",
                accepted.record_index
            ));
        }
    }
    let mut excluded_by_reason = BTreeMap::new();
    for excluded in &inspection.candidate_accounting.excluded {
        validate_candidate_id(
            &excluded.candidate_id,
            &known,
            &mut accounted,
            &mut failure_kinds,
        );
        let reason = excluded.reason.trim();
        if reason.is_empty() {
            failure_kinds.push(format!(
                "accounting_violation:empty_exclusion_reason:{}",
                excluded.candidate_id
            ));
        } else {
            *excluded_by_reason.entry(reason.to_string()).or_insert(0) += 1;
        }
    }
    for candidate_id in known.difference(&accounted) {
        failure_kinds.push(format!(
            "accounting_violation:unaccounted_candidate:{candidate_id}"
        ));
    }
    validate_record_indices(root, &accepted_indices, &mut failure_kinds);

    failure_kinds.sort();
    failure_kinds.dedup();
    let accepted = inspection.candidate_accounting.accepted.len();
    let excluded = inspection.candidate_accounting.excluded.len();
    let ok = failure_kinds.is_empty() && frozen.candidates.len() == accepted + excluded;
    if !ok && frozen.candidates.len() != accepted + excluded {
        failure_kinds.push(format!(
            "accounting_violation:equation:detected={}:accepted={accepted}:excluded={excluded}",
            frozen.candidates.len()
        ));
        failure_kinds.sort();
    }
    let evidence = CandidateAccountingEvidence {
        capability_id: "ingest_candidate_accounting".to_string(),
        status: if ok { "pass" } else { "failed" }.to_string(),
        ok,
        selector: frozen.selector.clone(),
        detected: frozen.candidates.len(),
        accepted,
        excluded_by_reason,
        equation: format!("{} = {} + {}", frozen.candidates.len(), accepted, excluded),
        candidate_ids: frozen
            .candidates
            .iter()
            .map(|candidate| candidate.id.clone())
            .collect(),
        failure_kinds,
    };
    write_json(root, ACCOUNTING_EVIDENCE_PATH, &evidence)?;
    Ok(evidence)
}

pub fn candidate_lineage_matches(root: &Path, frozen: &CandidateFreeze) -> bool {
    build_freeze(root).is_ok_and(|current| candidate_seals(&current) == candidate_seals(frozen))
}

pub fn load_inspection(root: &Path) -> anyhow::Result<InspectionDocument> {
    let path = crate::tools::path_guard::resolve_existing(root, INSPECTION_PATH)
        .context("candidate_set_violation:inspection_path")?;
    let text =
        std::fs::read_to_string(path).context("candidate_set_violation:inspection_unreadable")?;
    serde_json::from_str(&text).context("candidate_set_violation:inspection_invalid")
}

fn build_freeze(root: &Path) -> anyhow::Result<CandidateFreeze> {
    let inspection = load_inspection(root)?;
    validate_selector(&inspection.candidate_selector)?;
    let snapshot_root = crate::tools::path_guard::resolve_existing(root, SNAPSHOT_ROOT)
        .context("candidate_set_violation:snapshot_root")?;
    let mut paths = std::fs::read_dir(snapshot_root)
        .context("candidate_set_violation:snapshot_read_dir")?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .context("candidate_set_violation:snapshot_entry")?;
    paths.sort();
    if paths.is_empty() || paths.len() > MAX_SNAPSHOTS {
        bail!("candidate_set_violation:snapshot_count:{}", paths.len());
    }
    let mut snapshots = Vec::new();
    let mut candidates = Vec::new();
    let mut total_bytes = 0u64;
    for path in paths {
        let metadata = path
            .symlink_metadata()
            .context("candidate_set_violation:snapshot_metadata")?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            bail!("candidate_set_violation:snapshot_not_regular_file");
        }
        if metadata.len() > MAX_SNAPSHOT_BYTES {
            bail!("candidate_set_violation:snapshot_size_limit");
        }
        total_bytes = total_bytes
            .checked_add(metadata.len())
            .context("candidate_set_violation:snapshot_size_overflow")?;
        if total_bytes > MAX_TOTAL_BYTES {
            bail!("candidate_set_violation:total_snapshot_size_limit");
        }
        let text =
            std::fs::read_to_string(&path).context("candidate_set_violation:snapshot_not_utf8")?;
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .context("candidate_set_violation:snapshot_name_not_utf8")?;
        let relative = format!("{SNAPSHOT_ROOT}/{file_name}");
        snapshots.push(SnapshotSeal {
            path: relative.clone(),
            bytes: metadata.len(),
            fnv1a64: fingerprint(text.as_bytes()),
        });
        candidates.extend(enumerate(&relative, &text, &inspection.candidate_selector)?);
    }
    Ok(CandidateFreeze {
        capability_id: "ingest_candidate_freeze".to_string(),
        selector: inspection.candidate_selector,
        record_format: inspection.record_format,
        snapshots,
        candidates,
    })
}

fn enumerate(
    source_path: &str,
    text: &str,
    selector: &CandidateSelector,
) -> anyhow::Result<Vec<FrozenCandidate>> {
    let matches = match selector.kind {
        SelectorKind::LinePrefix => {
            let mut offset = 0usize;
            let mut matches = Vec::new();
            for line in text.split_inclusive('\n') {
                let raw = line.trim_end_matches(['\r', '\n']);
                if raw.starts_with(&selector.value) {
                    matches.push((offset, offset + raw.len(), raw.to_string()));
                }
                offset += line.len();
            }
            matches
        }
        SelectorKind::HtmlTag => {
            let tag = regex::escape(&selector.value);
            let pattern = Regex::new(&format!(r"(?is)<{tag}\b[^>]*>.*?</{tag}\s*>"))
                .context("candidate_set_violation:selector_regex")?;
            pattern
                .find_iter(text)
                .map(|found| (found.start(), found.end(), found.as_str().to_string()))
                .collect()
        }
        SelectorKind::Css => css_selector::enumerate(text, &selector.value)?,
    };
    Ok(matches
        .into_iter()
        .enumerate()
        .map(|(ordinal, (byte_start, byte_end, raw))| FrozenCandidate {
            id: format!("{source_path}#{ordinal}"),
            source_path: source_path.to_string(),
            ordinal,
            byte_start,
            byte_end,
            fnv1a64: fingerprint(raw.as_bytes()),
            raw,
        })
        .collect())
}

fn validate_selector(selector: &CandidateSelector) -> anyhow::Result<()> {
    if selector.value.is_empty() || selector.value.len() > 200 {
        bail!("candidate_set_violation:selector_value_invalid");
    }
    if selector.kind == SelectorKind::HtmlTag
        && (!selector.value.is_ascii()
            || !selector
                .value
                .chars()
                .enumerate()
                .all(|(index, ch)| ch.is_ascii_alphanumeric() || (index > 0 && ch == '-')))
    {
        bail!("candidate_set_violation:html_tag_invalid");
    }
    if selector.kind == SelectorKind::Css {
        css_selector::validate(&selector.value)?;
    }
    Ok(())
}

fn validate_candidate_id<'a>(
    candidate_id: &'a str,
    known: &BTreeSet<&'a str>,
    accounted: &mut BTreeSet<&'a str>,
    failures: &mut Vec<String>,
) {
    if !known.contains(candidate_id) {
        failures.push(format!(
            "candidate_set_violation:unknown_candidate:{candidate_id}"
        ));
    }
    if !accounted.insert(candidate_id) {
        failures.push(format!(
            "candidate_set_violation:duplicate_candidate:{candidate_id}"
        ));
    }
}

fn validate_record_indices(root: &Path, indices: &BTreeSet<usize>, failures: &mut Vec<String>) {
    let records = crate::tools::path_guard::resolve_existing(root, "output/records.json")
        .ok()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|value| value.as_array().map(Vec::len));
    let Some(record_count) = records else {
        failures.push("accounting_violation:records_not_array".to_string());
        return;
    };
    let expected = (0..record_count).collect::<BTreeSet<_>>();
    if *indices != expected {
        failures.push(format!(
            "accounting_violation:record_indices:expected={expected:?}:observed={indices:?}"
        ));
    }
}

fn candidate_seals(freeze: &CandidateFreeze) -> (&[SnapshotSeal], Vec<(&str, &str)>) {
    (
        &freeze.snapshots,
        freeze
            .candidates
            .iter()
            .map(|candidate| (candidate.id.as_str(), candidate.fnv1a64.as_str()))
            .collect(),
    )
}

fn fingerprint(bytes: &[u8]) -> String {
    let value = bytes.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    format!("{value:016x}")
}

fn write_json<T: Serialize>(root: &Path, relative: &str, value: &T) -> anyhow::Result<()> {
    let path = crate::tools::path_guard::resolve_optional_existing(root, relative)
        .with_context(|| format!("candidate evidence path escapes workspace: {relative}"))?;
    std::fs::create_dir_all(path.parent().context("candidate evidence parent missing")?)?;
    let mut file = std::fs::File::create(path)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn line_candidates_freeze_and_balance_accepted_plus_reasoned_exclusions() {
        let dir = fixture(
            SelectorKind::LinePrefix,
            "EVENT|",
            "EVENT|one\nnoise\nEVENT|two\n",
            json!({
                "accepted": [{"candidate_id":"data/snapshots/events.txt#0","record_index":0}],
                "excluded": [{"candidate_id":"data/snapshots/events.txt#1","reason":"cancelled"}]
            }),
            json!([{"name":"one"}]),
        );

        let frozen = freeze(dir.path()).unwrap();
        let evidence = check(dir.path(), &frozen).unwrap();

        assert_eq!(frozen.candidates.len(), 2);
        assert!(evidence.ok, "{evidence:?}");
        assert_eq!(evidence.equation, "2 = 1 + 1");
        assert_eq!(evidence.excluded_by_reason["cancelled"], 1);
        assert!(dir.path().join(FREEZE_EVIDENCE_PATH).is_file());
        assert!(dir.path().join(ACCOUNTING_EVIDENCE_PATH).is_file());
    }

    #[test]
    fn silent_drop_and_empty_reason_fail_accounting() {
        let dir = fixture(
            SelectorKind::LinePrefix,
            "EVENT|",
            "EVENT|one\nEVENT|two\n",
            json!({
                "accepted": [],
                "excluded": [{"candidate_id":"data/snapshots/events.txt#0","reason":" "}]
            }),
            json!([]),
        );

        let evidence = check(dir.path(), &freeze(dir.path()).unwrap()).unwrap();

        assert!(!evidence.ok);
        assert!(
            evidence
                .failure_kinds
                .iter()
                .any(|failure| failure.contains("empty_exclusion_reason"))
        );
        assert!(
            evidence
                .failure_kinds
                .iter()
                .any(|failure| failure.contains("unaccounted_candidate"))
        );
    }

    #[test]
    fn post_freeze_snapshot_change_is_candidate_set_violation() {
        let dir = fixture(
            SelectorKind::LinePrefix,
            "EVENT|",
            "EVENT|one\n",
            json!({
                "accepted": [{"candidate_id":"data/snapshots/events.txt#0","record_index":0}],
                "excluded": []
            }),
            json!([{"name":"one"}]),
        );
        let frozen = freeze(dir.path()).unwrap();
        std::fs::write(
            dir.path().join("data/snapshots/events.txt"),
            "EVENT|changed\n",
        )
        .unwrap();

        let evidence = check(dir.path(), &frozen).unwrap();

        assert!(!evidence.ok);
        assert!(
            evidence
                .failure_kinds
                .contains(&"candidate_set_violation:frozen_candidates_changed".to_string())
        );
    }

    #[test]
    fn html_tag_selector_enumerates_whole_candidate_blocks() {
        let dir = fixture(
            SelectorKind::HtmlTag,
            "article",
            "<article><h2>A</h2></article><aside>x</aside><article>B</article>",
            json!({
                "accepted": [
                    {"candidate_id":"data/snapshots/events.txt#0","record_index":0},
                    {"candidate_id":"data/snapshots/events.txt#1","record_index":1}
                ],
                "excluded": []
            }),
            json!([{"name":"A"},{"name":"B"}]),
        );

        let frozen = freeze(dir.path()).unwrap();

        assert_eq!(frozen.candidates.len(), 2);
        assert!(frozen.candidates[0].raw.contains("<h2>A</h2>"));
        assert!(check(dir.path(), &frozen).unwrap().ok);
    }

    #[test]
    fn css_literal_example_enumerates_direct_child_candidates() {
        let dir = fixture(
            SelectorKind::Css,
            "ul.events > li",
            "<ul class=\"events\"><li>A</li><li>B</li></ul><ul><li>C</li></ul>",
            json!({
                "accepted": [
                    {"candidate_id":"data/snapshots/events.txt#0","record_index":0}
                ],
                "excluded": [
                    {"candidate_id":"data/snapshots/events.txt#1","reason":"missing date"}
                ]
            }),
            json!([{"name":"A"}]),
        );

        let frozen = freeze(dir.path()).unwrap();

        assert_eq!(frozen.candidates.len(), 2);
        assert_eq!(frozen.candidates[0].raw, "<li>A</li>");
        assert!(check(dir.path(), &frozen).unwrap().ok);
    }

    fn fixture(
        kind: SelectorKind,
        selector: &str,
        snapshot: &str,
        accounting: Value,
        records: Value,
    ) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("data/snapshots")).unwrap();
        std::fs::create_dir_all(dir.path().join("output")).unwrap();
        std::fs::write(dir.path().join("data/snapshots/events.txt"), snapshot).unwrap();
        std::fs::write(
            dir.path().join(INSPECTION_PATH),
            serde_json::to_vec_pretty(&json!({
                "candidate_selector": {"kind": kind, "value": selector},
                "candidate_accounting": accounting,
                "record_format": {}
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("output/records.json"),
            serde_json::to_vec_pretty(&records).unwrap(),
        )
        .unwrap();
        dir
    }
}
