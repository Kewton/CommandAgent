use commandagent::config::PromptLayout;
use commandagent::planner::profiles::data::manifest;
use commandagent::planner::repair::{RepairContext, build_repair_prompt_with_context};
use commandagent::planner::verify::VerificationReport;

#[test]
fn data_manifest_v1_knowledge_matches_golden() {
    let manifest = manifest::get();
    let phase = |id: &str| {
        manifest
            .plan
            .phases
            .iter()
            .find(|phase| phase.id == id)
            .unwrap_or_else(|| panic!("missing data manifest phase {id}"))
            .prompt
            .as_str()
    };
    let owned = &manifest.step_templates.ownership.template_owned_artifacts;
    let actual = format!(
        "data-inspection={}\n\
data-cleaning={}\n\
data-aggregation={}\n\
data-reporting={}\n\
data-validation={}\n\
guidance.generic={}\n\
guidance.inspection={}\n\
guidance.schema={}\n\
guidance.repair={}\n\
required_artifacts={}\n\
template_owned.package_phrases={}\n\
template_owned.scaffold_phrases={}\n\
template_owned.path_suffixes={}\n\
template_owned.path_contains={}\n",
        phase("data-inspection"),
        phase("data-cleaning"),
        phase("data-aggregation"),
        phase("data-reporting"),
        phase("data-validation"),
        manifest
            .guidance
            .message("generic", "generic_interaction")
            .unwrap(),
        manifest
            .guidance
            .message("inspection", "canvas_input_wiring_checklist")
            .unwrap(),
        manifest
            .guidance
            .message("contracts", "state_requirement")
            .unwrap(),
        manifest
            .guidance
            .message("contracts", "contract_attribute_guidance")
            .unwrap(),
        manifest::required_artifacts().join(","),
        owned.package_phrases.join(","),
        owned.scaffold_phrases.join(","),
        owned.artifact_path_suffixes.join(","),
        owned.artifact_path_contains.join(","),
    );

    assert_eq!(
        actual,
        include_str!("golden/data_manifest_v1_knowledge.txt")
    );
}

#[test]
fn inspection_literal_example_is_observation_bound_and_reaches_repair_prompt() {
    let manifest = manifest::get();
    let phase = &manifest.plan.phases[0].prompt;
    let guidance = manifest
        .guidance
        .message("inspection", "canvas_input_wiring_checklist")
        .unwrap();
    let literal = r#"{"column_names": ["recorded_at","category","metric"], "input_row_count": 3, "type_summaries": {"recorded_at":"string","category":"string","metric":"number"}, "distinct_values": {"category": ["alpha","beta"]}, "sample_rows": [{"recorded_at":"example-1","category":"alpha","metric":1}]}"#;
    for text in [phase, guidance] {
        assert!(text.contains(literal));
        assert!(text.contains("examples only"));
        assert!(text.contains("actual observed values"));
        assert!(text.contains("never copy the example values as fixed data"));
    }

    let mut report = VerificationReport::pass();
    report.push_profile_failure(
        "data_inspection_schema:inspection_schema_violation:missing_keys:column_names,input_row_count,type_summaries,distinct_values,sample_rows",
    );
    let context = RepairContext {
        profile: Some("data".to_string()),
        prompt_layout: PromptLayout::Stable,
        ..RepairContext::default()
    };
    let prompt = build_repair_prompt_with_context("verify-inspection", &report, &context);
    assert!(prompt.contains("Profile repair guidance:"));
    assert!(prompt.contains(literal));
    assert!(prompt.contains("actual observed values"));
}
