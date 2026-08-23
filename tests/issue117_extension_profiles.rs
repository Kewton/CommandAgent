use std::path::{Path, PathBuf};

use commandagent::planner::profile::{ProfileRuntimeRegistry, verify_profile_final};
use commandagent::planner::profile_manifest::ManifestStatus;
use commandagent::planner::profile_manifest::overlay::load_extension_overlays;
use commandagent::planner::profile_manifest::source::load_extension_manifests;
use commandagent::tui::boundary_shell::BoundaryShell;
use commandagent::tui::boundary_shell::ambiguity::{
    ClassifierProvenance, ProposalStatus, RouteProposal,
};
use commandagent::tui::boundary_shell::confirmation::{ExecutionPins, PackSelection};
use commandagent::tui::boundary_shell::presentation::render_gate_one;
use commandagent::tui::boundary_shell::route::{
    DeterministicResolution, ExplicitRouteBinding, RouteRequest, deterministic_route,
};

fn case_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/apps/issue117-draft-profile")
}

fn extension_root() -> PathBuf {
    case_root().join("extension-root")
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

fn proposal_for(workspace: &Path, profile: &str) -> RouteProposal {
    let deterministic = deterministic_route(RouteRequest {
        request: "Create the requested site",
        workspace,
        explicit: ExplicitRouteBinding {
            profile: Some(commandagent::planner::profile::ProfileId::parse(profile)),
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
            prompt_version: "issue117-test",
            candidate_keys: Vec::new(),
            raw_response_hash: None,
            parse_reason: "deterministic_unique".to_string(),
        },
        status: ProposalStatus::AwaitingConfirmation,
        confirmation_required: true,
    }
}

#[test]
fn external_profiles_are_first_class_drafts_in_runtime_gate_and_sheet() {
    let root = extension_root();
    let manifests = load_extension_manifests(&root).unwrap();
    assert_eq!(manifests.len(), 1);
    let manifest = &manifests[0];
    assert_eq!(manifest.id(), "static-site");
    assert_eq!(manifest.status(), ManifestStatus::Draft);
    assert_eq!(manifest.warnings.len(), 1);
    assert_eq!(
        manifest.hash().unwrap(),
        include_str!("golden/issue117_static_site_manifest.sha256").trim()
    );

    let descriptors = commandagent::planner::extension_profiles::extension_descriptors(&root)
        .expect("fixture extension root must register");
    assert_eq!(descriptors.len(), 2);
    let static_site = commandagent::planner::extension_profiles::find("static-site").unwrap();
    assert_eq!(static_site.status(), ManifestStatus::Draft);
    assert_eq!(static_site.assurance_ceiling(), "static");
    assert_eq!(static_site.base_profile, None);
    assert!((static_site.descriptor.admission)() == ManifestStatus::Draft);
    assert_eq!(
        (commandagent::planner::profile_descriptor::descriptor_for_name("nextjs")
            .unwrap()
            .admission)(),
        ManifestStatus::Admitted
    );
    assert!(ProfileRuntimeRegistry::registered().any(|id| id.as_str() == "static-site"));

    let runtime_report = verify_profile_final(&case_root(), "static-site", "Create a static site");
    assert!(
        runtime_report.is_pass(),
        "{}",
        runtime_report.primary_reason()
    );

    let state = tempfile::tempdir().unwrap();
    let mut shell = BoundaryShell::new(state.path().join("confirmations"), None);
    let identity = shell
        .begin_gate_one(
            proposal_for(&case_root(), "static-site"),
            "Create the requested site",
            &case_root(),
            pins(),
            PackSelection::None,
        )
        .unwrap()
        .clone();
    let card = render_gate_one(
        &identity,
        &commandagent::planner::pack::catalog::PackLocator::new(Path::new(env!(
            "CARGO_MANIFEST_DIR"
        ))),
    )
    .unwrap();
    assert!(card.contains("- プロファイル: static-site（draft / 未承認 / 保証上限 static）"));
    assert!(card.contains(&format!("- manifest: {}", static_site.manifest_hash)));
    assert_eq!(identity.band_denominator, 0);
    assert!(matches!(identity.pack, PackSelection::None));

    let events = state.path().join("events.jsonl");
    std::fs::write(
        &events,
        serde_json::json!({
            "event": "tui_command_stop",
            "effective_profile": "static-site",
            "status": "completed",
            "assurance_level": "static",
            "assurance_reason": "profile_not_admitted",
            "runtime_acceptance_status": "full",
            "final_acceptance_status": "completed",
            "release_gate_status": "passed",
            "stop_reason": "completed"
        })
        .to_string(),
    )
    .unwrap();
    let sheet =
        commandagent::tui::boundary_shell::sheet::generate(&identity, Some(&events), true).unwrap();
    assert!(!sheet.full);
    assert!(sheet.markdown.contains("profile_not_admitted"));
    assert!(sheet.markdown.contains(static_site.manifest_hash));

    let mut overlay_shell = BoundaryShell::new(state.path().join("overlay-confirmations"), None);
    let overlay_identity = overlay_shell
        .begin_gate_one(
            proposal_for(&case_root(), "acme-nextjs"),
            "Create the requested site",
            &case_root(),
            pins(),
            PackSelection::None,
        )
        .unwrap();
    let overlay_card = render_gate_one(
        overlay_identity,
        &commandagent::planner::pack::catalog::PackLocator::new(Path::new(env!(
            "CARGO_MANIFEST_DIR"
        ))),
    )
    .unwrap();
    assert!(overlay_card.contains("- overlay: acme-nextjs / base: nextjs（admitted）"));
}

#[test]
fn external_loader_rejects_collision_fixture_leak_and_non_additive_overlay() {
    let source =
        std::fs::read_to_string(extension_root().join("profiles/static-site/manifest.toml"))
            .unwrap();

    let collision = tempfile::tempdir().unwrap();
    write_manifest(
        collision.path(),
        "nextjs",
        &source.replace("static-site", "nextjs"),
    );
    assert!(load_extension_manifests(collision.path()).is_err());

    let leaking = tempfile::tempdir().unwrap();
    write_manifest(
        leaking.path(),
        "static-site",
        &source.replace("Create the requested", "Create 売上 for the requested"),
    );
    assert!(load_extension_manifests(leaking.path()).is_err());

    let general = tempfile::tempdir().unwrap();
    write_manifest(
        general.path(),
        "landing-page",
        &source.replace("static-site", "landing-page"),
    );
    assert_eq!(
        load_extension_manifests(general.path()).unwrap()[0].id(),
        "landing-page"
    );

    let overlay_source =
        std::fs::read_to_string(extension_root().join("profiles/nextjs/overlay.toml")).unwrap();
    let replacement = tempfile::tempdir().unwrap();
    write_overlay(
        replacement.path(),
        &overlay_source.replace("mode = \"additive\"", "mode = \"replace\""),
    );
    assert!(load_extension_overlays(replacement.path()).is_err());

    let collision = tempfile::tempdir().unwrap();
    write_overlay(
        collision.path(),
        &overlay_source.replace("docs/security-review.md", "package.json"),
    );
    assert!(load_extension_overlays(collision.path()).is_err());

    let missing_evidence = tempfile::tempdir().unwrap();
    write_overlay(
        missing_evidence.path(),
        &format!(
            "{overlay_source}\n[[checks.security]]\nid = \"lint_config_present\"\n[checks.security.params]\npath = \"eslint.config.js\"\n"
        ),
    );
    let error = load_extension_overlays(missing_evidence.path())
        .unwrap_err()
        .to_string();
    assert!(error.contains("evidence_targets must map every added check"));
}

fn write_manifest(root: &Path, id: &str, source: &str) {
    let directory = root.join("profiles").join(id);
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("manifest.toml"), source).unwrap();
}

fn write_overlay(root: &Path, source: &str) {
    let directory = root.join("profiles/nextjs");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("overlay.toml"), source).unwrap();
}
