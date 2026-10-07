use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN_EN: &str = "without making network requests";
const FORBIDDEN_JA: &str = "ネットワーク要求を行わず";

const ENGLISH_DOCS: &[&str] = &[
    "docs/guide/en/cli-reference.md",
    "docs/guide/en/slash-commands.md",
    "docs/user/getting-started-cli.md",
];

const JAPANESE_DOCS: &[&str] = &[
    "docs/guide/ja/cli-reference.md",
    "docs/guide/ja/slash-commands.md",
];

const README: &str = "README.md";

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn read_repo_file(relative: &str) -> String {
    fs::read_to_string(repo_path(relative))
        .unwrap_or_else(|err| panic!("failed to read {relative}: {err}"))
}

fn doctor_help() -> String {
    let command = commandagent::provider_cli::command();
    let argument = command
        .get_arguments()
        .find(|argument| argument.get_id() == "doctor")
        .expect("--doctor is a public CLI flag");
    argument
        .get_help()
        .expect("--doctor has a Clap help description")
        .to_string()
}

#[test]
fn doctor_help_states_reachability_and_drops_the_offline_claim() {
    let help = doctor_help();
    for marker in ["reachability check", "connect", "provider", "API key"] {
        assert!(
            help.contains(marker),
            "--doctor help is missing {marker:?}: {help}"
        );
    }
    assert!(
        !help.contains(FORBIDDEN_EN),
        "--doctor help still claims no network requests: {help}"
    );
}

#[test]
fn bilingual_doctor_docs_state_reachability_and_drop_the_offline_claim() {
    for path in ENGLISH_DOCS {
        let markdown = read_repo_file(path);
        for marker in ["reachability check", "connect", "API key"] {
            assert!(
                markdown.contains(marker),
                "{path} is missing English reachability marker {marker:?}"
            );
        }
        assert!(
            !markdown.contains(FORBIDDEN_EN),
            "{path} still claims no network requests"
        );
        assert!(
            !markdown.contains(FORBIDDEN_JA),
            "{path} still claims no network requests in Japanese"
        );
    }
    for path in JAPANESE_DOCS {
        let markdown = read_repo_file(path);
        for marker in ["到達確認", "API キー"] {
            assert!(
                markdown.contains(marker),
                "{path} is missing Japanese reachability marker {marker:?}"
            );
        }
        assert!(
            !markdown.contains(FORBIDDEN_JA),
            "{path} still claims no network requests"
        );
        assert!(
            !markdown.contains(FORBIDDEN_EN),
            "{path} still claims no network requests in English"
        );
    }
}

#[test]
fn readme_doctor_summary_states_provider_reachability() {
    let markdown = read_repo_file(README);
    assert!(
        markdown.contains("reachability check"),
        "{README} doctor summary is missing a reachability check statement"
    );
    assert!(
        !markdown.contains("offline doctor"),
        "{README} still describes the doctor as offline"
    );
    assert!(
        !markdown.contains(FORBIDDEN_EN),
        "{README} still claims no network requests"
    );
    assert!(
        !markdown.contains(FORBIDDEN_JA),
        "{README} still claims no network requests in Japanese"
    );
}

#[test]
fn english_cli_reference_doctor_row_equals_the_public_help_text() {
    // tests/doc_drift.rs already pins this pair; repeating it here keeps the
    // Issue #514 contract (help and docs agree) failing from one place when
    // either the Clap help or the reference row drifts.
    let help = doctor_help();
    let reference = read_repo_file("docs/guide/en/cli-reference.md");
    assert!(
        reference.contains(&help),
        "en cli-reference --doctor row must equal the Clap help text exactly:\n{help}"
    );
}
