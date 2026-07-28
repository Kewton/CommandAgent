pub(crate) const SELECTOR_KINDS: [&str; 3] = ["css", "html_tag", "line_prefix"];

pub(crate) const SELECTOR_LITERAL: &str =
    r#"{"candidate_selector": {"kind": "css", "value": "ul.events > li"}}"#;

pub(crate) const CSS_SUPPORTED_FORMS: &str = "CSS supports tag, .class, #id, their compound \
forms, descendant chains such as table tbody tr, and direct-child chains such as \
ul.events > li, with at most 8 compounds; attribute, pseudo, sibling, and comma selectors \
are not supported";

pub(crate) const INSPECTION_LITERAL: &str = r#"{"candidate_selector":{"kind":"css","value":"ul.events > li"},"candidate_accounting":{"accepted":[{"candidate_id":"data/snapshots/events.html#0","record_index":0}],"excluded":[{"candidate_id":"data/snapshots/events.html#1","reason":"missing required date"}]},"record_format":{"fields":[{"name":"name","type":"string","normalizations":["identity"]},{"name":"date","type":"string","normalizations":["japanese_date_to_iso","document_year_context"]},{"name":"location","type":"string","normalizations":["identity"]},{"name":"source_file","type":"string","normalizations":["identity"]}]}}"#;

pub(crate) const PROVISIONAL_INSPECTION_LITERAL: &str = r#"{"candidate_selector":{"kind":"css","value":"ul.events > li"},"candidate_accounting":{"accepted":[],"excluded":[]},"record_format":{"fields":[{"name":"name","type":"string","normalizations":["identity"]},{"name":"date","type":"string","normalizations":["japanese_date_to_iso","document_year_context"]},{"name":"location","type":"string","normalizations":["identity"]},{"name":"source_file","type":"string","normalizations":["identity"]}]}}"#;

pub(crate) const RECORDS_LITERAL: &str = r#"[{"name":"observed event name","date":"2026-08-01","location":"observed location","source_file":"events.html"}]"#;

pub(crate) const GENERATION_RULES: &str = concat!(
    "- Profile ingest: implement an offline deterministic pipeline/main.py over ",
    "data/snapshots. Do not invent source values, splice candidate blocks, silently ",
    "drop candidates, or fetch network data.\n",
    "- The phase structure gate requires pipeline/main.py and output/report.md to ",
    "exist, output/records.json to be valid JSON, and output/inspection.json to ",
    "declare candidate_selector as a kind/value object.\n",
    "- Allowed candidate_selector kind values are exactly css, html_tag, and ",
    "line_prefix. Literal selector shape: ",
    r#"{"candidate_selector": {"kind": "css", "value": "ul.events > li"}}"#,
    ".\n",
    "- CSS selector declaration boundary: CSS supports tag, .class, #id, their compound ",
    "forms, descendant chains such as table tbody tr, and direct-child chains such as ",
    "ul.events > li, with at most 8 compounds; attribute, pseudo, sibling, and comma selectors ",
    "are not supported. Unsupported CSS is rejected by the structural gate before acceptance.\n",
    "- Literal output/inspection.json shape: ",
    r#"{"candidate_selector":{"kind":"css","value":"ul.events > li"},"candidate_accounting":{"accepted":[{"candidate_id":"data/snapshots/events.html#0","record_index":0}],"excluded":[{"candidate_id":"data/snapshots/events.html#1","reason":"missing required date"}]},"record_format":{"fields":[{"name":"name","type":"string","normalizations":["identity"]},{"name":"date","type":"string","normalizations":["japanese_date_to_iso","document_year_context"]},{"name":"location","type":"string","normalizations":["identity"]},{"name":"source_file","type":"string","normalizations":["identity"]}]}}"#,
    ".\n",
    "- document_year_context: declare with japanese_date_to_iso only for a partial candidate ",
    "value plus a unique title or candidate-external heading. Evidence records both source ",
    "fragments and positions. Never use another candidate.\n",
    "- Literal output/records.json shape: ",
    r#"[{"name":"observed event name","date":"2026-08-01","location":"observed location","source_file":"events.html"}]"#,
    ".\n",
    "- The values shown are examples only: inspect the actual snapshots, replace ",
    "every selector, candidate id, record value, exclusion reason, field, and ",
    "normalization with actual observed declarations, and never copy example values ",
    "as fixed data.\n",
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_guidance_covers_every_gate_shape_and_runtime_vocabulary() {
        for required in [
            "pipeline/main.py",
            "output/report.md",
            "output/records.json to be valid JSON",
            "output/inspection.json",
            SELECTOR_LITERAL,
            INSPECTION_LITERAL,
            RECORDS_LITERAL,
            "css, html_tag, and line_prefix",
            CSS_SUPPORTED_FORMS,
            "before acceptance",
            "document_year_context",
            "both source fragments",
            "Never use another candidate",
            "examples only",
            "actual snapshots",
            "never copy example values as fixed data",
        ] {
            assert!(GENERATION_RULES.contains(required), "missing {required}");
        }
        for kind in SELECTOR_KINDS {
            assert!(GENERATION_RULES.contains(kind));
        }
    }
}
