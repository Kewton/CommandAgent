use commandagent::minimal_loop::evidence::verify_runtime_acceptance;
use commandagent::planner::profiles::nextjs;

#[test]
fn engine_owned_nextjs_page_requires_task_specific_rewrite() {
    let dir = tempfile::tempdir().unwrap();
    let page_path = "src/app/page.tsx".to_string();
    nextjs::complete_scaffold(dir.path(), std::slice::from_ref(&page_path)).unwrap();

    let scaffold_report = verify_runtime_acceptance(
        dir.path(),
        std::slice::from_ref(&page_path),
        &[],
        &[],
        &[],
        &["implementation".to_string()],
        &[],
    );
    assert!(!scaffold_report.passed, "{scaffold_report:?}");
    assert_eq!(scaffold_report.artifact_obligations[0].role, "scaffold");
    assert!(
        scaffold_report
            .missing_obligations
            .contains(&"implementation".to_string())
    );

    std::fs::write(
        dir.path().join(&page_path),
        "export default function Page(){ return <main>Agency projects</main>; }\n",
    )
    .unwrap();
    let implementation_report = verify_runtime_acceptance(
        dir.path(),
        &[page_path],
        &[],
        &[],
        &[],
        &["implementation".to_string()],
        &[],
    );
    assert!(implementation_report.passed, "{implementation_report:?}");
    assert_eq!(
        implementation_report.artifact_obligations[0].role,
        "implementation"
    );
}
