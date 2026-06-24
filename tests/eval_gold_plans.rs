use commandagent::agent::step_runner::parse_step_plan_yaml;
use commandagent::agent::step_runner::plan_lint::lint_step_plan;
use std::fs;
use std::path::Path;

#[test]
fn large_gold_plans_parse_and_lint() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("eval/gold_plans/large");
    let mut checked = 0usize;

    for entry in fs::read_dir(&root).expect("read eval/gold_plans/large") {
        let path = entry.expect("gold plan dir entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("yaml") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("read gold plan");
        let plan = parse_step_plan_yaml(&text)
            .unwrap_or_else(|err| panic!("{} failed to parse: {err}", path.display()));
        lint_step_plan(&plan).unwrap_or_else(|err| panic!("{} failed lint: {err}", path.display()));
        checked += 1;
    }

    assert_eq!(checked, 6, "expected one gold plan for each large case");
}
