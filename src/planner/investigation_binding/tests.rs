#[cfg(test)]
mod cases {
    use super::super::*;
    use crate::planner::adjudication::contract::ProbeOutcome;

    const GUIDED_DIAGNOSIS: &str = include_str!(
        "../../../tests/corpus/apps/test0718_d3b_investigation_intent/fixtures/diagnosis-valid.md"
    );
    const RUN6_STYLE_DIAGNOSIS: &str = include_str!(
        "../../../tests/corpus/apps/test0718_d3b_investigation_intent/fixtures/diagnosis-run6-example-blocks.md"
    );

    fn run() -> InvestigationRunEvidence {
        let mut run =
            InvestigationRunEvidence::new("python3 pipeline/main.py", 1, ProbeOutcome::Failure);
        run.stderr = "ValueError: invalid region".into();
        run
    }

    #[test]
    fn binds_error_file_line_and_code_snippet() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("pipeline")).unwrap();
        std::fs::write(
            root.path().join("pipeline/main.py"),
            "value = int(row[0])\nraise ValueError('invalid region')\n",
        )
        .unwrap();
        let ticks = char::from(96).to_string();
        let diagnosis = format!(
            "Observed {ticks}ValueError: invalid region{ticks} at pipeline/main.py:2\n{ticks}{ticks}{ticks}python\nraise ValueError('invalid region')\n{ticks}{ticks}{ticks}"
        );
        let evidence = bind_diagnosis(root.path(), &diagnosis, &run());
        assert_eq!(evidence.claims.len(), 3);
        assert!(evidence.claims.iter().all(|claim| claim.matched));
    }

    #[test]
    fn violations_record_nearest_information() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("pipeline")).unwrap();
        std::fs::write(
            root.path().join("pipeline/main.py"),
            "raise ValueError('invalid region')\n",
        )
        .unwrap();
        let ticks = char::from(96).to_string();
        let diagnosis = format!(
            "Observed {ticks}TypeError: absent{ticks} at pipeline/missing.py:9\n{ticks}{ticks}{ticks}python\nraise RuntimeError('absent')\n{ticks}{ticks}{ticks}"
        );
        let evidence = bind_diagnosis(root.path(), &diagnosis, &run());
        assert!(evidence.claims.iter().all(|claim| !claim.matched));
        assert!(evidence.claims.iter().all(|claim| claim.nearest.is_some()));
    }

    #[test]
    fn run6_style_example_blocks_remain_unbound_while_guided_claims_bind() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("pipeline")).unwrap();
        let source = "# 1\n# 2\n# 3\n# 4\n# 5\n# 6\n# 7\namount = float(row[\"amount\"])\n";
        std::fs::write(root.path().join("pipeline/main.py"), source).unwrap();
        let mut measured = run();
        measured.stderr = "ValueError: invalid literal".into();

        let unbound = bind_diagnosis(root.path(), RUN6_STYLE_DIAGNOSIS, &measured);
        assert!(!unbound.claims.is_empty());
        assert!(unbound.claims.iter().any(|claim| !claim.matched));

        let guided = bind_diagnosis(root.path(), GUIDED_DIAGNOSIS, &measured);
        assert_eq!(guided.claims.len(), 3);
        assert!(guided.claims.iter().all(|claim| claim.matched));
    }

    include!("output_anchor_tests.rs");
}
