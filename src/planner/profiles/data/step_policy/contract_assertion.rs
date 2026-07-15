const RESULTS_SCHEMA: &str = "data_results_schema";
const RECONCILIATION: &str = "data_reconciliation";
const INSPECTION_SCHEMA: &str = "data_inspection_schema";

pub(super) fn catalog_checks(command: &str) -> Option<Vec<&'static str>> {
    let body = inline_python_body(command)?;
    let lower = body.to_ascii_lowercase();
    if !has_token(&lower, "assert") {
        return None;
    }

    let mut checks = Vec::new();
    let reads_results = lower.contains("output/results.json");
    if reads_results && contains_all(&lower, &["reconciliation", "values"]) {
        checks.push(RESULTS_SCHEMA);
    }
    if reads_results
        && contains_all(
            &lower,
            &["reconciliation", "input_rows", "used_rows", "excluded"],
        )
    {
        checks.push(RECONCILIATION);
    }
    let names_inspection = lower.contains("output/inspection.json");
    let asserts_inspection_shape = contains_all(
        &lower,
        &[
            "column_names",
            "input_row_count",
            "type_summaries",
            "distinct_values",
            "sample_rows",
        ],
    );
    if names_inspection && (asserts_inspection_shape || lower.contains("os.path.exists")) {
        checks.push(INSPECTION_SCHEMA);
    }
    (!checks.is_empty()).then_some(checks)
}

fn inline_python_body(command: &str) -> Option<&str> {
    let command = command.trim();
    let separator = command.find(char::is_whitespace)?;
    let (program, args) = command.split_at(separator);
    if !matches!(program, "python" | "python3") {
        return None;
    }
    let body = args.trim_start().strip_prefix("-c")?;
    if !body.chars().next().is_some_and(char::is_whitespace) {
        return None;
    }
    let body = body.trim();
    let quote = body.chars().next()?;
    if !matches!(quote, '\'' | '"') || !body.ends_with(quote) {
        return None;
    }
    body.get(quote.len_utf8()..body.len() - quote.len_utf8())
}

fn contains_all(text: &str, tokens: &[&str]) -> bool {
    tokens.iter().all(|token| text.contains(token))
}

fn has_token(text: &str, expected: &str) -> bool {
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .any(|token| token == expected)
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use crate::planner::step_plan::{PlanStep, StepPlan};

    const UAT_REJECTIONS: &str = include_str!(
        "../../../../../tests/corpus/apps/test0715_data_b2f_verify_canonicalization/fixtures/python-c-rejections.jsonl"
    );
    const EXISTING_CANONICALIZATIONS: &str = include_str!(
        "../../../../../tests/corpus/apps/test0715_data_b2f_verify_canonicalization/fixtures/existing-canonicalizations.jsonl"
    );

    fn jsonl(raw: &str) -> Vec<Value> {
        raw.lines()
            .map(|line| serde_json::from_str(line).expect("valid measured JSONL fixture"))
            .collect()
    }

    #[test]
    fn measured_python_c_assertions_become_catalog_checks_and_events() {
        let measured = jsonl(UAT_REJECTIONS);
        let mut plan = StepPlan {
            goal: "Analyze sales".to_string(),
            steps: measured
                .iter()
                .map(|event| PlanStep {
                    id: event["step_id"].as_str().unwrap().to_string(),
                    kind: "verify".to_string(),
                    expected_result: "pass".to_string(),
                    instruction: "Verify the generated data contract artifacts.".to_string(),
                    expected_paths: Vec::new(),
                    verify: vec![event["original_command"].as_str().unwrap().to_string()],
                })
                .collect(),
        };
        let dir = tempfile::tempdir().unwrap();
        let events_path = dir.path().join("events.jsonl");

        assert_eq!(
            super::super::canonicalize_step_plan(&mut plan, Some(&events_path)),
            2
        );
        assert_eq!(
            plan.steps[0].verify,
            [
                super::super::catalog_check_command(RESULTS_SCHEMA),
                super::super::catalog_check_command(RECONCILIATION),
            ]
        );
        assert_eq!(
            plan.steps[1].verify,
            [
                super::super::catalog_check_command(RESULTS_SCHEMA),
                super::super::catalog_check_command(RECONCILIATION),
                super::super::catalog_check_command(INSPECTION_SCHEMA),
            ]
        );

        let emitted = jsonl(&std::fs::read_to_string(events_path).unwrap());
        assert_eq!(emitted.len(), 5);
        for (index, event) in emitted.iter().enumerate() {
            let measured_index = usize::from(index >= 2);
            assert_eq!(event["event"], "verify_canonicalized");
            assert_eq!(event["field"], "verify");
            assert_eq!(event["disposition"], "canonical");
            assert_eq!(
                event["original"],
                measured[measured_index]["original_command"]
            );
            assert!(
                event["replacement"]
                    .as_str()
                    .unwrap()
                    .starts_with(super::super::CATALOG_CHECK_PREFIX)
            );
        }
    }

    #[test]
    fn existing_thirteen_canonicalizations_do_not_enter_python_c_detection() {
        let events = jsonl(EXISTING_CANONICALIZATIONS);
        assert_eq!(events.len(), 13);
        assert_eq!(
            events
                .iter()
                .filter(|event| event["field"] == "expected_path")
                .count(),
            3
        );
        let verify_events = events
            .iter()
            .filter(|event| event["field"] == "verify")
            .collect::<Vec<_>>();
        assert_eq!(verify_events.len(), 10);
        assert!(verify_events.iter().all(|event| {
            let original = event["original"].as_str().unwrap();
            catalog_checks(original).is_none()
        }));
    }

    #[test]
    fn detection_requires_python_c_assertions_and_contract_paths() {
        assert_eq!(catalog_checks("python -c \"assert True\""), None);
        assert_eq!(
            catalog_checks("python -c \"print('output/results.json reconciliation values')\""),
            None
        );
        assert_eq!(
            catalog_checks("python3 -m json.tool output/results.json"),
            None
        );
        assert_eq!(
            catalog_checks(
                "python3 -c \"import json; d=json.load(open('output/inspection.json')); assert all(k in d for k in ['column_names','input_row_count','type_summaries','distinct_values','sample_rows'])\""
            ),
            Some(vec![INSPECTION_SCHEMA])
        );
    }
}
