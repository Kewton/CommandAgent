#[test]
fn binds_registered_pipeline_arguments() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("data")).unwrap();
    std::fs::create_dir_all(dir.path().join("pipeline")).unwrap();
    std::fs::write(dir.path().join("data/task-02.csv"), "value\n2\n").unwrap();
    std::fs::write(
        dir.path().join("pipeline/main.py"),
        "import pathlib, sys\npathlib.Path('selected.txt').write_text(sys.argv[1])\n",
    )
    .unwrap();

    let report = run(
        dir.path(),
        PipelineProbeConfig::new("pipeline/main.py").with_args(["data/task-02.csv".to_string()]),
    )
    .unwrap();

    assert!(report.ok, "{report:?}");
    assert_eq!(
        report.command,
        ["python3", "-B", "pipeline/main.py", "data/task-02.csv"]
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("selected.txt")).unwrap(),
        "data/task-02.csv"
    );
}
