use std::path::{Path, PathBuf};
use std::process::Command;

use commandagent::planner::pack::PackProfile;
use commandagent::planner::pack::catalog::{PackLocator, PackSource, profile_is_compatible};
use commandagent::planner::profile::ProfileId;
use commandagent::tui::boundary_shell::BoundaryShell;
use commandagent::tui::boundary_shell::ambiguity::{
    ClassifierProvenance, ProposalStatus, RouteProposal,
};
use commandagent::tui::boundary_shell::confirmation::{ExecutionPins, PackSelection};
use commandagent::tui::boundary_shell::pack_catalog;
use commandagent::tui::boundary_shell::presentation::render_gate_one;
use commandagent::tui::boundary_shell::route::{
    DeterministicResolution, ExplicitRouteBinding, RouteRequest, deterministic_route,
};

fn case_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/apps/issue249-draft-local-pack")
}

fn extension_root() -> PathBuf {
    case_root().join("extension-root")
}

fn proposal_for(workspace: &Path) -> RouteProposal {
    let deterministic = deterministic_route(RouteRequest {
        request: "Create the requested static site",
        workspace,
        explicit: ExplicitRouteBinding {
            profile: Some(ProfileId::parse("static-site")),
            ..ExplicitRouteBinding::default()
        },
    });
    assert_eq!(deterministic.resolution, DeterministicResolution::Unique);
    RouteProposal {
        selected: deterministic.candidates.first().cloned(),
        alternatives: deterministic.candidates,
        classifier: ClassifierProvenance {
            used: false,
            provider: "ollama".to_string(),
            model: "planner".to_string(),
            prompt_version: "issue249-test",
            candidate_keys: Vec::new(),
            raw_response_hash: None,
            parse_reason: "deterministic_unique".to_string(),
        },
        status: ProposalStatus::AwaitingConfirmation,
        confirmation_required: true,
    }
}

fn pins() -> ExecutionPins {
    ExecutionPins {
        planner_provider: "ollama".to_string(),
        planner_model: "planner".to_string(),
        executor_provider: "ollama".to_string(),
        executor_model: "executor".to_string(),
        preset: "profile".to_string(),
        think: None,
    }
}

#[test]
fn registered_draft_uses_only_its_exact_local_pack_and_stays_static() {
    let extension = extension_root();
    commandagent::planner::extension_profiles::register(&extension).unwrap();
    assert_eq!(
        PackProfile::parse("static-site"),
        Some(PackProfile::Draft("static-site"))
    );
    assert_eq!(PackProfile::parse("unknown-external-profile"), None);
    assert!(profile_is_compatible(
        PackSource::Local,
        "static-site",
        PackProfile::Draft("static-site")
    ));
    assert!(!profile_is_compatible(
        PackSource::Repository,
        "static-site",
        PackProfile::Draft("static-site")
    ));

    let locator =
        PackLocator::with_extension_root(env!("CARGO_MANIFEST_DIR"), Some(extension.clone()));
    let pack = pack_catalog::select_with_locator(
        "static-site",
        "create",
        "static-guidance@1.0.0",
        &locator,
    )
    .unwrap();
    assert!(matches!(
        pack,
        PackSelection::Pinned {
            source: PackSource::Local,
            ..
        }
    ));

    let state = tempfile::tempdir().unwrap();
    let mut shell = BoundaryShell::new(state.path().join("confirmations"), None);
    let identity = shell
        .begin_gate_one_with_locator(
            proposal_for(&case_root()),
            "Create the requested static site",
            &case_root(),
            pins(),
            pack,
            &locator,
        )
        .unwrap()
        .clone();
    let card = render_gate_one(&identity, &locator).unwrap();
    assert!(card.contains("static-site（draft / 未承認 / 保証上限 static）"));
    assert!(card.contains("追加の検証パック: static-guidance@1.0.0"));
    assert!(card.contains("検証パックの供給元: ローカル（未承認・帯域未計測）"));
    assert!(card.contains("検証パックの状態: バイト単位で一致"));

    let events = state.path().join("events.jsonl");
    std::fs::write(
        &events,
        include_str!("corpus/apps/issue249-draft-local-pack/fixtures/terminal.jsonl"),
    )
    .unwrap();
    let sheet =
        commandagent::tui::boundary_shell::sheet::generate(&identity, Some(&events), true).unwrap();
    assert!(!sheet.full);
    assert!(
        sheet
            .markdown
            .contains("Assurance: static (profile_not_admitted)")
    );
    assert!(
        sheet
            .markdown
            .contains("Pack source: ローカル（未承認・帯域未計測）")
    );

    let listed = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args(["--extension-root"])
        .arg(&extension)
        .args(["--profile", "static-site", "--intent", "create", "--packs"])
        .output()
        .unwrap();
    assert!(
        listed.status.success(),
        "{}",
        String::from_utf8_lossy(&listed.stderr)
    );
    assert!(
        String::from_utf8_lossy(&listed.stdout).contains(
            "static-guidance@1.0.0\tsha256:b1d8b936d3fee069583e8caf081a49cf3155223fbd552b0df23d07194c7bc90b\tlocal"
        )
    );

    let selected = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args(["--extension-root"])
        .arg(&extension)
        .args([
            "--profile",
            "static-site",
            "--intent",
            "create",
            "--pack",
            "static-guidance@1.0.0",
            "--runs",
        ])
        .output()
        .unwrap();
    assert!(
        selected.status.success(),
        "{}",
        String::from_utf8_lossy(&selected.stderr)
    );
}
