use std::collections::BTreeSet;

const AUDIT_PATH: &str = "workspace/management/runs/e5b-dispatch-audit.md";
const RUNNER_PATH: &str = "src/planner/runner.rs";

fn production_runner() -> String {
    let source = std::fs::read_to_string(RUNNER_PATH).expect("read runner");
    source
        .split_once("\n#[cfg(test)]\nmod tests")
        .map(|(production, _)| production.to_string())
        .expect("runner top-level test boundary")
}

#[test]
fn runner_profile_dispatch_is_confined_to_the_three_reviewed_identity_sites() {
    let source = production_runner();
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
            "runner allowlist marker must occur exactly once: {marker}"
        );
        assert!(
            source.contains(typed_site),
            "runner allowlist site lost its typed identity form: {typed_site}"
        );
    }
    assert_eq!(
        source.matches("E5B_PROFILE_DISPATCH_ALLOW:").count(),
        3,
        "new runner allowlist entries require explicit review"
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
            "runner reintroduced string profile dispatch: {forbidden}"
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
                "runner reintroduced a profile literal comparison: {profile}"
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
