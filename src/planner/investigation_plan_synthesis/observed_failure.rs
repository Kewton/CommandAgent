use crate::config::Config;
use crate::planner::adjudication::investigate::InvestigationRunEvidence;

const MINIMUM_CLAIM_GUIDANCE: &str = "診断には最低1件のエラー引用（上記R出力からの正確な引用）と、可能な場合はfile:line参照を含めること。機械照合可能な主張が0件の診断はfullにならない（契約§4）。";

pub(super) fn render(config: &Config) -> String {
    let path = config
        .workspace_root
        .join("evidence/investigation-run.json");
    let Some(run) = std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<InvestigationRunEvidence>(&bytes).ok())
    else {
        return format!(
            "実行済みRの失敗観測: unavailable（reproduce-candidate段のevidence未生成）\n\n{MINIMUM_CLAIM_GUIDANCE}"
        );
    };
    let excerpt = deterministic_excerpt(&run);
    let traceback =
        crate::minimal_loop::python_traceback::extract(&run.stderr, &config.workspace_root)
            .traceback;
    let traceback = traceback.map_or_else(
        || "- traceback mapping: unavailable".to_string(),
        |traceback| {
            format!(
                "- traceback file:line: {}:{}\n- traceback exception: {}: {}",
                traceback
                    .target_path
                    .as_deref()
                    .unwrap_or(&traceback.final_frame.file),
                traceback.final_frame.line,
                traceback.exception_type,
                traceback.message
            )
        },
    );
    format!(
        "実行済みRの失敗観測（機械注入・各stream最大500文字のtail）:\n- reproducer: {}\n- failure summary:\n~~~text\n{}\n~~~\n- deterministic excerpt（診断へ正確に引用する候補）:\n~~~text\n{}\n~~~\n- stdout tail:\n~~~text\n{}\n~~~\n- stderr tail:\n~~~text\n{}\n~~~\n{}\n\n{MINIMUM_CLAIM_GUIDANCE}",
        run.reproducer,
        observed_text(&excerpt),
        observed_text(&excerpt),
        observed_text(&run.stdout),
        observed_text(&run.stderr),
        traceback,
    )
}

fn deterministic_excerpt(run: &InvestigationRunEvidence) -> String {
    [&run.stderr, &run.stdout]
        .into_iter()
        .find_map(|value| value.lines().rev().find(|line| !line.trim().is_empty()))
        .map(crate::eval_events::body_snippet)
        .unwrap_or_else(|| "(empty)".to_string())
}

fn observed_text(value: &str) -> &str {
    if value.trim().is_empty() {
        "(empty)"
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::Parser;

    #[test]
    fn elev002_run1_failure_output_is_injected_into_diagnose_snapshot() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("evidence")).unwrap();
        std::fs::write(
            root.path().join("evidence/investigation-run.json"),
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/corpus/apps/test0718_d3b_investigation_intent/fixtures/elev002-run1-investigation-run.json"
            )),
        )
        .unwrap();

        let instruction =
            super::super::guidance::diagnose_instruction(&config(root.path()), "origin failure");

        assert_eq!(
            instruction,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/corpus/apps/test0718_d3b_investigation_intent/fixtures/elev002-run1-diagnose-plan.txt"
            ))
            .trim_end()
        );
    }

    #[test]
    fn python_reproducer_output_reuses_b2d_traceback_mapping() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("evidence")).unwrap();
        std::fs::create_dir_all(root.path().join("pipeline")).unwrap();
        std::fs::write(root.path().join("pipeline/main.py"), "one\ntwo\nthree\n").unwrap();
        let mut run = InvestigationRunEvidence::new(
            "python3 -B pipeline/main.py",
            1,
            crate::planner::adjudication::contract::ProbeOutcome::Failure,
        );
        run.stderr = "Traceback (most recent call last):\n  File \"pipeline/main.py\", line 2, in <module>\n    raise ValueError('bad row')\nValueError: bad row\n".into();
        std::fs::write(
            root.path().join("evidence/investigation-run.json"),
            serde_json::to_vec_pretty(&run).unwrap(),
        )
        .unwrap();

        let rendered = render(&config(root.path()));

        assert!(rendered.contains("traceback file:line: pipeline/main.py:2"));
        assert!(rendered.contains("traceback exception: ValueError: bad row"));
        assert!(rendered.contains("ValueError: bad row"));
    }

    fn config(root: &std::path::Path) -> Config {
        Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            root.to_str().unwrap(),
            "--intent",
            "investigate",
            "--profile",
            "data",
        ]))
        .unwrap()
    }
}
