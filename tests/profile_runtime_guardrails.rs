use std::collections::BTreeSet;
use std::path::Path;

const AUDIT_PATH: &str = "workspace/management/runs/e5b-dispatch-audit.md";
const RUNNER_PATH: &str = "src/planner/runner.rs";
const RUNNER_MODULE_ROOT: &str = "src/planner/runner";

fn production_runner_modules() -> String {
    let mut paths = vec![RUNNER_PATH.to_string()];
    collect_production_runner_modules(Path::new(RUNNER_MODULE_ROOT), &mut paths);
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|err| panic!("read runner module {path}: {err}"));
            format!("// {path}\n{source}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn collect_production_runner_modules(root: &Path, paths: &mut Vec<String>) {
    for entry in std::fs::read_dir(root)
        .unwrap_or_else(|err| panic!("read runner module directory {}: {err}", root.display()))
        .flatten()
    {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|name| name.to_str()) != Some("tests") {
                collect_production_runner_modules(&path, paths);
            }
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            paths.push(path.to_string_lossy().replace('\\', "/"));
        }
    }
}

#[test]
fn runner_profile_dispatch_is_confined_to_the_three_reviewed_identity_sites() {
    let source = production_runner_modules();
    let allowlist = [
        (
            "E5B_PROFILE_DISPATCH_ALLOW: inference-boundary",
            "infer_profile(None, &config.workspace_root)",
        ),
        (
            "E5B_PROFILE_DISPATCH_ALLOW: telemetry-profile",
            "\"profile\": ProfileId::parse(profile).to_string()",
        ),
        (
            "E5B_PROFILE_DISPATCH_ALLOW: telemetry-generic-contract",
            "ProfileId::parse(profile) == ProfileId::Generic",
        ),
    ];
    for (marker, typed_site) in allowlist {
        assert_eq!(
            source.matches(marker).count(),
            1,
            "runner module allowlist marker must occur exactly once: {marker}"
        );
        assert!(
            source.contains(typed_site),
            "runner module allowlist site lost its typed identity form: {typed_site}"
        );
    }
    assert_eq!(
        source.matches("E5B_PROFILE_DISPATCH_ALLOW:").count(),
        3,
        "new runner module allowlist entries require explicit review"
    );

    for forbidden in [
        "canonical_profile_name(",
        "domain_profile(",
        "is_nextjs_profile(",
        "profile_expected_paths(",
        "profile_setup_scaffold_paths(",
        "verify_profile_final(",
        "verify_profile_invariant(",
    ] {
        assert!(
            !source.contains(forbidden),
            "runner modules reintroduced string profile dispatch: {forbidden}"
        );
    }
    for profile in [
        "nextjs",
        "python-cli",
        "data",
        "ingest",
        "cli",
        "generic",
        "react",
        "vite",
        "web",
    ] {
        for operator in ["==", "!="] {
            let direct = format!("{operator} \"{profile}\"");
            let reversed = format!("\"{profile}\" {operator}");
            assert!(
                !source.contains(&direct) && !source.contains(&reversed),
                "runner modules reintroduced a profile literal comparison: {profile}"
            );
        }
    }
}

#[test]
fn dispatch_audit_accounts_for_all_110_sites_with_three_residuals() {
    let audit = std::fs::read_to_string(AUDIT_PATH).expect("read dispatch audit");
    let mut sites = BTreeSet::new();
    let mut listed = 0;
    let mut residual = Vec::new();
    let mut current_batch = None;
    for line in audit.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("batch = ") {
            current_batch = value.parse::<usize>().ok();
        } else if let Some(values) = line
            .strip_prefix("runner_sites = [")
            .and_then(|line| line.strip_suffix(']'))
        {
            let batch_sites = values
                .split(',')
                .filter_map(|value| value.trim().parse::<usize>().ok())
                .collect::<Vec<_>>();
            listed += batch_sites.len();
            for site in &batch_sites {
                assert!(sites.insert(*site), "duplicate audited runner site: {site}");
            }
            if current_batch == Some(6) {
                residual = batch_sites;
            }
        }
    }
    assert_eq!(listed, 110);
    assert_eq!(sites.len(), 110);
    assert_eq!(residual, vec![1301, 1755, 3639]);
    assert_eq!(audit.matches("status = \"complete\"").count(), 7);
    assert!(!audit.contains("status = \"pending\""));
}
