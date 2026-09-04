#[test]
fn nextjs_api_route_imported_file_write_is_persistence_evidence() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src/app/api/tasks")).unwrap();
    std::fs::create_dir_all(dir.path().join("src/lib")).unwrap();
    std::fs::write(dir.path().join("package.json"), "{}").unwrap();
    std::fs::write(
        dir.path().join("src/app/api/tasks/route.ts"),
        r#"import { writeTasks } from "@/lib/tasks";
export async function POST(request: Request) {
  await writeTasks(await request.json());
  return Response.json({ ok: true });
}
"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("src/lib/tasks.ts"),
        r#"import { writeFile } from "fs/promises";
export async function writeTasks(tasks: unknown[]) {
  await writeFile("tasks.json", JSON.stringify(tasks));
}
"#,
    )
    .unwrap();

    let report = verify_runtime_acceptance(
        dir.path(),
        &[
            "src/app/api/tasks/route.ts".to_string(),
            "src/lib/tasks.ts".to_string(),
        ],
        &[],
        &["persistence".to_string()],
        &[],
        &[],
        &[],
    );

    assert_eq!(
        report.evidence_tiers.get("persistence_evidence"),
        Some(&"strong".to_string())
    );
    assert!(
        report
            .artifact_obligations
            .iter()
            .any(|obligation| obligation.path == "src/lib/tasks.ts" && obligation.route_bound),
        "{report:?}"
    );
}
