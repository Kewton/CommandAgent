use std::fmt::Write;
use std::path::Path;

use anyhow::{Context, bail};

use super::accounting;

const MAX_CANDIDATES: usize = 1_024;
const MAX_TEXT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateIdGuidance {
    pub text: String,
    pub candidate_ids: Vec<String>,
    pub selector_kind: String,
    pub selector_value: String,
}

pub fn render(root: &Path) -> anyhow::Result<CandidateIdGuidance> {
    let frozen = accounting::freeze(root)
        .context("candidate set freeze before ingest delivery implementation failed")?;
    if frozen.candidates.len() > MAX_CANDIDATES {
        bail!(
            "candidate set has {} entries; canonical guidance limit is {MAX_CANDIDATES}",
            frozen.candidates.len()
        );
    }
    let candidate_ids = frozen
        .candidates
        .iter()
        .map(|candidate| candidate.id.clone())
        .collect::<Vec<_>>();
    let selector_kind = serde_json::to_value(frozen.selector.kind)?
        .as_str()
        .context("candidate selector kind is not a string")?
        .to_string();
    let selector_value = frozen.selector.value;
    let mut text = format!(
        "Machine-frozen canonical candidate IDs. This set was frozen after the selector \
declaration and before pipeline implementation or execution.\n\
Selector: kind={selector_kind}, value={selector_value:?}\n\
Every candidate reference in records-generation logic and output/inspection.json MUST use \
one of the following canonical IDs verbatim. Do not alter or omit any prefix. Account for \
every ID exactly once as accepted or excluded with a reason.\n"
    );
    if let Some(first) = candidate_ids.first() {
        writeln!(
            text,
            "Literal canonical candidate_id example: {first:?} (copy the applicable ID below verbatim)."
        )?;
    } else {
        text.push_str(
            "The frozen candidate set is empty; do not invent candidate references or records.\n",
        );
    }
    text.push_str("Frozen candidate IDs:\n");
    for candidate_id in &candidate_ids {
        writeln!(text, "- {candidate_id}")?;
    }
    if text.len() > MAX_TEXT_BYTES {
        bail!(
            "canonical candidate guidance is {} bytes; limit is {MAX_TEXT_BYTES}",
            text.len()
        );
    }
    Ok(CandidateIdGuidance {
        text,
        candidate_ids,
        selector_kind,
        selector_value,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn frozen_ids_are_rendered_verbatim_after_selector_declaration() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("data/snapshots")).unwrap();
        std::fs::create_dir_all(dir.path().join("output")).unwrap();
        std::fs::write(
            dir.path().join("data/snapshots/events.html"),
            "<article>A</article><article>B</article>",
        )
        .unwrap();
        std::fs::write(
            dir.path().join(accounting::INSPECTION_PATH),
            serde_json::to_vec_pretty(&json!({
                "candidate_selector": {"kind":"html_tag","value":"article"},
                "candidate_accounting": {"accepted":[],"excluded":[]},
                "record_format": {"fields":[]}
            }))
            .unwrap(),
        )
        .unwrap();

        let guidance = render(dir.path()).unwrap();

        assert_eq!(
            guidance.candidate_ids,
            [
                "data/snapshots/events.html#0",
                "data/snapshots/events.html#1"
            ]
        );
        assert_eq!(guidance.selector_kind, "html_tag");
        assert_eq!(guidance.selector_value, "article");
        for candidate_id in &guidance.candidate_ids {
            assert!(guidance.text.contains(candidate_id));
        }
        assert!(guidance.text.contains("MUST use"));
        assert!(guidance.text.contains("Do not alter or omit any prefix"));
        assert!(dir.path().join(accounting::FREEZE_EVIDENCE_PATH).is_file());
    }
}
