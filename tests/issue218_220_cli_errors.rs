use std::path::Path;
use std::process::{Command, Output};

fn commandagent(workspace: &Path, arguments: &[&str]) -> Output {
    let state = workspace.join("state");
    std::fs::create_dir_all(&state).unwrap();
    Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args(arguments)
        .args([
            "--cwd",
            workspace.to_str().unwrap(),
            "--state-dir",
            state.to_str().unwrap(),
        ])
        .output()
        .unwrap()
}

fn output_text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn assert_single_path_aware_os_error(output: &Output, path: &Path) {
    let text = output_text(output);
    let path = path.display().to_string();
    let matching_lines = text
        .lines()
        .filter(|line| line.contains(&path) && line.contains("os error"))
        .collect::<Vec<_>>();
    assert_eq!(matching_lines.len(), 1, "{text}");
    assert_eq!(text.matches("os error 2").count(), 1, "{text}");
    assert!(text.contains("No such file or directory"), "{text}");
}

#[test]
fn missing_pack_and_plan_paths_report_one_path_aware_os_error() {
    let workspace = tempfile::tempdir().unwrap();
    let missing_pack = workspace.path().join("missing-pack");
    let pack = commandagent(
        workspace.path(),
        &["--pack-verify", missing_pack.to_str().unwrap()],
    );
    assert!(!pack.status.success(), "{}", output_text(&pack));
    assert_single_path_aware_os_error(&pack, &missing_pack);

    let missing_extension = workspace.path().join("missing-extension");
    let doctor = commandagent(
        workspace.path(),
        &[
            "--extension-root",
            missing_extension.to_str().unwrap(),
            "--doctor",
        ],
    );
    assert!(!doctor.status.success(), "{}", output_text(&doctor));
    assert_single_path_aware_os_error(&doctor, &missing_extension);

    let missing_plan = workspace.path().join("missing-plan.yaml");
    let plan = commandagent(
        workspace.path(),
        &["--run-plan", missing_plan.to_str().unwrap()],
    );
    assert!(!plan.status.success(), "{}", output_text(&plan));
    assert_single_path_aware_os_error(&plan, &missing_plan);
}

#[test]
fn packs_without_compatible_rows_suppress_the_empty_heading() {
    let workspace = tempfile::tempdir().unwrap();
    let output = commandagent(
        workspace.path(),
        &["--profile", "nextjs", "--intent", "fix", "--packs"],
    );
    assert!(output.status.success(), "{}", output_text(&output));
    assert!(output.stdout.is_empty(), "{}", output_text(&output));
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "no compatible packs for nextjs × fix\n"
    );
}
