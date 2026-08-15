use std::path::{Path, PathBuf};

#[test]
fn gui_server_is_a_read_only_island() {
    let mut sources = vec![PathBuf::from("src/bin/gui_server.rs")];
    collect_rust_files(Path::new("src/bin/gui_server"), &mut sources);
    assert!(
        sources.len() >= 3,
        "GUI server modules are missing: {sources:?}"
    );

    let forbidden = [
        "commandagent::",
        "provider_call",
        "providers::",
        "reqwest",
        "std::process",
        "tokio::process",
        "Command::",
        "fs::write",
        "fs::create_dir",
        "OpenOptions",
        "File::create",
        ".write_all(",
        ".write(",
        "post(",
        "put(",
        "patch(",
        "delete(",
        "X-Forwarded",
        "x-forwarded",
    ];
    for path in sources {
        let source = std::fs::read_to_string(&path).unwrap();
        for token in forbidden {
            assert!(
                !source.contains(token),
                "{} contains forbidden GUI capability {token:?}",
                path.display()
            );
        }
    }
}

#[test]
fn gui_dependencies_stay_out_of_the_default_rust_build() {
    let manifest = std::fs::read_to_string("Cargo.toml").unwrap();
    for dependency in ["axum", "tokio"] {
        let declaration = manifest
            .lines()
            .find(|line| line.starts_with(&format!("{dependency} =")))
            .unwrap_or_else(|| panic!("missing {dependency} declaration"));
        assert!(
            declaration.contains("optional = true"),
            "{dependency} must remain optional: {declaration}"
        );
    }
    assert!(manifest.contains("required-features = [\"gui\"]"));
    assert!(manifest.contains("default = []"));
}

#[test]
fn next_export_and_base_path_audit_are_pinned() {
    let config = std::fs::read_to_string("gui/next.config.ts").unwrap();
    assert!(config.contains("output: \"export\""));
    assert!(config.contains("process.env.GUI_BASE_PATH"));
    assert!(config.contains("NEXT_PUBLIC_GUI_BASE_PATH: basePath"));

    let package = std::fs::read_to_string("gui/package.json").unwrap();
    assert!(package.contains("scripts/lint-internal-paths.mjs"));
    assert!(Path::new("gui/package-lock.json").is_file());
}

fn collect_rust_files(root: &Path, output: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(root).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, output);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            output.push(path);
        }
    }
}
