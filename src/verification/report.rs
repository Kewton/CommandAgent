use super::diagnostics::{CompileError, PythonTraceback};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyStatus {
    Pass,
    MissingPath(String),
    CommandFailed(String),
    VerifierCommandFalseNegative(String),
    DependencyMissing(String),
    ProfileContractFailed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationReport {
    pub status: VerifyStatus,
    pub missing_paths: Vec<String>,
    pub command_failures: Vec<CommandFailure>,
    pub verifier_command_false_negatives: Vec<VerifierCommandFalseNegative>,
    pub runtime_command_normalizations: Vec<VerifyCommandRuntimeNormalization>,
    pub dependency_missing: Vec<String>,
    pub profile_failures: Vec<String>,
    pub compile_errors: Vec<CompileError>,
    pub python_tracebacks: Vec<PythonTraceback>,
    /// Issue #949: names the admitted artifact checks of this acceptance made
    /// repair candidates. Not a failure and not part of the status.
    pub artifact_repair_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandFailure {
    pub command: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct VerifierCommandFalseNegative {
    pub command: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct VerifyCommandRuntimeNormalization {
    pub original: String,
    pub repaired: String,
}

impl VerificationReport {
    pub fn pass() -> Self {
        Self {
            status: VerifyStatus::Pass,
            missing_paths: Vec::new(),
            command_failures: Vec::new(),
            verifier_command_false_negatives: Vec::new(),
            runtime_command_normalizations: Vec::new(),
            dependency_missing: Vec::new(),
            profile_failures: Vec::new(),
            compile_errors: Vec::new(),
            python_tracebacks: Vec::new(),
            artifact_repair_paths: Vec::new(),
        }
    }

    pub fn is_pass(&self) -> bool {
        self.status == VerifyStatus::Pass
            && self.missing_paths.is_empty()
            && self.command_failures.is_empty()
            && self.verifier_command_false_negatives.is_empty()
            && self.dependency_missing.is_empty()
            && self.profile_failures.is_empty()
            && self.compile_errors.is_empty()
            && self.python_tracebacks.is_empty()
    }

    pub fn missing_path(path: impl Into<String>) -> Self {
        let mut report = Self::pass();
        report.push_missing_path(path);
        report
    }

    pub fn command_failed(command: impl Into<String>, reason: impl Into<String>) -> Self {
        let mut report = Self::pass();
        report.push_command_failure(command, reason);
        report
    }

    pub fn dependency_missing(reason: impl Into<String>) -> Self {
        let mut report = Self::pass();
        report.push_dependency_missing(reason);
        report
    }

    pub fn profile_failed(reason: impl Into<String>) -> Self {
        let mut report = Self::pass();
        report.push_profile_failure(reason);
        report
    }

    pub fn push_missing_path(&mut self, path: impl Into<String>) {
        self.missing_paths.push(path.into());
        self.refresh_status();
    }

    pub fn push_command_failure(&mut self, command: impl Into<String>, reason: impl Into<String>) {
        self.command_failures.push(CommandFailure {
            command: command.into(),
            reason: reason.into(),
        });
        self.refresh_status();
    }

    pub fn push_verifier_command_false_negative(
        &mut self,
        command: impl Into<String>,
        reason: impl Into<String>,
    ) {
        self.verifier_command_false_negatives
            .push(VerifierCommandFalseNegative {
                command: command.into(),
                reason: reason.into(),
            });
        self.refresh_status();
    }

    pub(crate) fn push_runtime_command_normalization(
        &mut self,
        original: impl Into<String>,
        repaired: impl Into<String>,
    ) {
        self.runtime_command_normalizations
            .push(VerifyCommandRuntimeNormalization {
                original: original.into(),
                repaired: repaired.into(),
            });
    }

    pub fn push_dependency_missing(&mut self, reason: impl Into<String>) {
        self.dependency_missing.push(reason.into());
        self.refresh_status();
    }

    pub fn push_profile_failure(&mut self, reason: impl Into<String>) {
        self.profile_failures.push(reason.into());
        self.refresh_status();
    }

    pub fn push_python_traceback(&mut self, traceback: PythonTraceback) {
        if !self.python_tracebacks.contains(&traceback) {
            self.python_tracebacks.push(traceback);
        }
    }

    pub fn push_compile_errors(&mut self, command: impl Into<String>, errors: Vec<CompileError>) {
        if errors.is_empty() {
            return;
        }
        for error in &errors {
            if !self.compile_errors.contains(error) {
                self.compile_errors.push(error.clone());
            }
        }
        let reason = format!(
            "implementation_compile_error: {}",
            errors
                .iter()
                .map(CompileError::summary)
                .collect::<Vec<_>>()
                .join("; ")
        );
        self.push_command_failure(command, reason);
    }

    pub fn primary_reason(&self) -> String {
        self.missing_paths
            .first()
            .cloned()
            .or_else(|| self.dependency_missing.first().cloned())
            .or_else(|| {
                self.command_failures
                    .first()
                    .map(|failure| failure.reason.clone())
            })
            .or_else(|| {
                self.verifier_command_false_negatives
                    .first()
                    .map(|failure| failure.reason.clone())
            })
            .or_else(|| {
                self.compile_errors
                    .first()
                    .map(|error| format!("implementation_compile_error: {}", error.summary()))
            })
            .or_else(|| self.profile_failures.first().cloned())
            .unwrap_or_else(|| "pass".to_string())
    }

    pub fn refresh_status(&mut self) {
        self.status = if let Some(path) = self.missing_paths.first() {
            VerifyStatus::MissingPath(path.clone())
        } else if let Some(reason) = self.dependency_missing.first() {
            VerifyStatus::DependencyMissing(reason.clone())
        } else if let Some(failure) = self.command_failures.first() {
            VerifyStatus::CommandFailed(failure.reason.clone())
        } else if let Some(failure) = self.verifier_command_false_negatives.first() {
            VerifyStatus::VerifierCommandFalseNegative(failure.reason.clone())
        } else if let Some(error) = self.compile_errors.first() {
            VerifyStatus::CommandFailed(format!(
                "implementation_compile_error: {}",
                error.summary()
            ))
        } else if let Some(reason) = self.profile_failures.first() {
            VerifyStatus::ProfileContractFailed(reason.clone())
        } else {
            VerifyStatus::Pass
        };
    }
}

#[cfg(test)]
mod tests {
    use crate::minimal_loop::build_verifier::CompileError;
    use crate::minimal_loop::python_traceback::{PythonTraceback, PythonTracebackFrame, extract};
    use crate::minimal_loop::repair_target::{RepairTarget, classify_repair_target};
    use crate::planner::step_plan::PlanStep;
    use crate::planner::verify::{
        CommandFailure, VerificationReport, VerifierCommandFalseNegative,
        VerifyCommandRuntimeNormalization, VerifyStatus, verify_step,
    };

    fn compile_error(path: &str, line: usize, column: usize, message: &str) -> CompileError {
        CompileError {
            path: path.to_string(),
            line,
            column,
            message: message.to_string(),
            excerpt: String::new(),
            symbol: None,
            route_bound: None,
        }
    }

    fn traceback(file: &str, line: usize, target: Option<&str>) -> PythonTraceback {
        PythonTraceback {
            final_frame: PythonTracebackFrame {
                file: file.to_string(),
                line,
                function: "main".to_string(),
            },
            leading_frames: Vec::new(),
            exception_type: "ValueError".to_string(),
            message: "bad value".to_string(),
            target_path: target.map(str::to_string),
        }
    }

    #[test]
    fn constructors_pin_status_pass_flag_and_primary_reason() {
        let pass = VerificationReport::pass();
        assert_eq!(pass.status, VerifyStatus::Pass);
        assert!(pass.is_pass());
        assert_eq!(pass.primary_reason(), "pass");

        let missing = VerificationReport::missing_path("a.txt");
        assert_eq!(missing.status, VerifyStatus::MissingPath("a.txt".into()));
        assert_eq!(missing.missing_paths, vec!["a.txt".to_string()]);
        assert!(!missing.is_pass());
        assert_eq!(missing.primary_reason(), "a.txt");

        let failed = VerificationReport::command_failed("npm test", "tests failed");
        assert_eq!(
            failed.status,
            VerifyStatus::CommandFailed("tests failed".into())
        );
        assert_eq!(
            failed.command_failures,
            vec![CommandFailure {
                command: "npm test".into(),
                reason: "tests failed".into(),
            }]
        );
        assert!(!failed.is_pass());
        assert_eq!(failed.primary_reason(), "tests failed");

        let dependency = VerificationReport::dependency_missing("node_modules missing");
        assert_eq!(
            dependency.status,
            VerifyStatus::DependencyMissing("node_modules missing".into())
        );
        assert!(!dependency.is_pass());
        assert_eq!(dependency.primary_reason(), "node_modules missing");

        let profile = VerificationReport::profile_failed("contract broke");
        assert_eq!(
            profile.status,
            VerifyStatus::ProfileContractFailed("contract broke".into())
        );
        assert!(!profile.is_pass());
        assert_eq!(profile.primary_reason(), "contract broke");

        let mut negative = VerificationReport::pass();
        negative.push_verifier_command_false_negative("grep x", "grep masks exit");
        assert_eq!(
            negative.status,
            VerifyStatus::VerifierCommandFalseNegative("grep masks exit".into())
        );
        assert_eq!(
            negative.verifier_command_false_negatives,
            vec![VerifierCommandFalseNegative {
                command: "grep x".into(),
                reason: "grep masks exit".into(),
            }]
        );
        assert!(!negative.is_pass());
        assert_eq!(negative.primary_reason(), "grep masks exit");
    }

    #[test]
    fn constructors_leave_unrelated_collections_empty() {
        let report = VerificationReport::missing_path("a.txt");
        assert!(report.command_failures.is_empty());
        assert!(report.verifier_command_false_negatives.is_empty());
        assert!(report.runtime_command_normalizations.is_empty());
        assert!(report.dependency_missing.is_empty());
        assert!(report.profile_failures.is_empty());
        assert!(report.compile_errors.is_empty());
        assert!(report.python_tracebacks.is_empty());
    }

    #[test]
    fn status_and_primary_reason_priority_holds_for_every_failure_combination() {
        // Priority order: earliest entry wins for both `status` and `primary_reason`.
        let compile_reason = "implementation_compile_error: src/a.ts:3:5 bad type";
        let expected = [
            (VerifyStatus::MissingPath("m.txt".into()), "m.txt"),
            (
                VerifyStatus::DependencyMissing("dep missing".into()),
                "dep missing",
            ),
            (
                VerifyStatus::CommandFailed("cmd failed".into()),
                "cmd failed",
            ),
            (
                VerifyStatus::VerifierCommandFalseNegative("false negative".into()),
                "false negative",
            ),
            (
                VerifyStatus::CommandFailed(compile_reason.into()),
                compile_reason,
            ),
            (
                VerifyStatus::ProfileContractFailed("profile broke".into()),
                "profile broke",
            ),
        ];
        let apply = |report: &mut VerificationReport, kind: usize| match kind {
            0 => report.push_missing_path("m.txt"),
            1 => report.push_dependency_missing("dep missing"),
            2 => report.push_command_failure("cmd", "cmd failed"),
            3 => report.push_verifier_command_false_negative("vcmd", "false negative"),
            4 => {
                report
                    .compile_errors
                    .push(compile_error("src/a.ts", 3, 5, "bad type"));
                report.refresh_status();
            }
            _ => report.push_profile_failure("profile broke"),
        };
        for mask in 0u32..64 {
            let kinds = (0..6)
                .filter(|kind| mask & (1 << kind) != 0)
                .collect::<Vec<_>>();
            for reversed in [false, true] {
                let mut report = VerificationReport::pass();
                let order: Vec<usize> = if reversed {
                    kinds.iter().rev().copied().collect()
                } else {
                    kinds.clone()
                };
                for kind in order {
                    apply(&mut report, kind);
                }
                match kinds.first() {
                    None => {
                        assert_eq!(report.status, VerifyStatus::Pass, "mask {mask}");
                        assert!(report.is_pass(), "mask {mask}");
                        assert_eq!(report.primary_reason(), "pass", "mask {mask}");
                    }
                    Some(&winner) => {
                        let (status, reason) = &expected[winner];
                        assert_eq!(&report.status, status, "mask {mask} reversed {reversed}");
                        assert!(!report.is_pass(), "mask {mask} reversed {reversed}");
                        assert_eq!(
                            report.primary_reason(),
                            *reason,
                            "mask {mask} reversed {reversed}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn first_entry_of_each_collection_decides_status() {
        let mut report = VerificationReport::pass();
        report.push_missing_path("first.txt");
        report.push_missing_path("second.txt");
        report.push_command_failure("c1", "first failure");
        report.push_command_failure("c2", "second failure");
        assert_eq!(report.status, VerifyStatus::MissingPath("first.txt".into()));
        assert_eq!(report.primary_reason(), "first.txt");
        report.missing_paths.clear();
        report.refresh_status();
        assert_eq!(
            report.status,
            VerifyStatus::CommandFailed("first failure".into())
        );
        assert_eq!(report.primary_reason(), "first failure");
    }

    #[test]
    fn refresh_status_returns_to_pass_when_failures_are_cleared() {
        let mut report = VerificationReport::command_failed("cmd", "boom");
        report.command_failures.clear();
        report.refresh_status();
        assert_eq!(report.status, VerifyStatus::Pass);
        assert!(report.is_pass());
        assert_eq!(report.primary_reason(), "pass");
    }

    #[test]
    fn python_traceback_blocks_pass_without_changing_status() {
        let mut report = VerificationReport::pass();
        report.push_python_traceback(traceback("app.py", 3, Some("app.py")));
        assert_eq!(report.status, VerifyStatus::Pass);
        assert!(!report.is_pass());
        assert_eq!(report.primary_reason(), "pass");
    }

    #[test]
    fn runtime_normalizations_do_not_affect_pass() {
        let mut report = VerificationReport::pass();
        report
            .runtime_command_normalizations
            .push(VerifyCommandRuntimeNormalization {
                original: "npm test | tail".into(),
                repaired: "npm test".into(),
            });
        report.refresh_status();
        assert_eq!(report.status, VerifyStatus::Pass);
        assert!(report.is_pass());
        assert_eq!(report.primary_reason(), "pass");
    }

    #[test]
    fn push_compile_errors_deduplicates_errors_and_records_failure() {
        let first = compile_error("src/a.ts", 3, 5, "bad type");
        let second = compile_error("src/b.ts", 7, 0, "missing export");
        let mut report = VerificationReport::pass();
        report.push_compile_errors(
            "npm run build",
            vec![first.clone(), second.clone(), first.clone()],
        );
        assert_eq!(report.compile_errors, vec![first.clone(), second.clone()]);
        let reason = "implementation_compile_error: src/a.ts:3:5 bad type; src/b.ts:7 missing export; src/a.ts:3:5 bad type";
        assert_eq!(
            report.command_failures,
            vec![CommandFailure {
                command: "npm run build".into(),
                reason: reason.into(),
            }]
        );
        assert_eq!(report.status, VerifyStatus::CommandFailed(reason.into()));
        assert_eq!(report.primary_reason(), reason);

        report.push_compile_errors("npm run build", vec![first.clone()]);
        assert_eq!(report.compile_errors, vec![first, second]);
        assert_eq!(report.command_failures.len(), 2);
        assert_eq!(
            report.command_failures[1].reason,
            "implementation_compile_error: src/a.ts:3:5 bad type"
        );
    }

    #[test]
    fn push_compile_errors_with_no_errors_is_a_no_op() {
        let mut report = VerificationReport::pass();
        report.push_compile_errors("npm run build", Vec::new());
        assert_eq!(report, VerificationReport::pass());
    }

    #[test]
    fn compile_error_without_command_failure_still_fails_with_prefixed_reason() {
        let mut report = VerificationReport::pass();
        report
            .compile_errors
            .push(compile_error("src/a.ts", 1, 1, "oops"));
        report.refresh_status();
        assert_eq!(
            report.status,
            VerifyStatus::CommandFailed("implementation_compile_error: src/a.ts:1:1 oops".into())
        );
        assert_eq!(
            report.primary_reason(),
            "implementation_compile_error: src/a.ts:1:1 oops"
        );
        assert_eq!(
            classify_repair_target(&report),
            RepairTarget::Implementation
        );
    }

    #[test]
    fn push_python_traceback_deduplicates_identical_tracebacks() {
        let first = traceback("app.py", 3, Some("app.py"));
        let other = traceback("app.py", 9, Some("app.py"));
        let mut report = VerificationReport::pass();
        report.push_python_traceback(first.clone());
        report.push_python_traceback(first.clone());
        report.push_python_traceback(other.clone());
        assert_eq!(report.python_tracebacks, vec![first, other]);
    }

    #[test]
    fn compile_error_with_symbol_flows_into_the_report_with_stable_wording() {
        let error = CompileError {
            symbol: Some("Widget".into()),
            route_bound: Some(true),
            ..compile_error(
                "src/app/page.tsx",
                3,
                5,
                "Type error: Cannot find name 'Widget'.",
            )
        };
        let mut report = VerificationReport::pass();
        report.push_compile_errors("npm run build", vec![error.clone(), error.clone()]);
        assert_eq!(report.compile_errors, vec![error]);
        assert_eq!(
            report.primary_reason(),
            "implementation_compile_error: src/app/page.tsx:3:5 Type error: Cannot find name 'Widget'.; src/app/page.tsx:3:5 Type error: Cannot find name 'Widget'."
        );
        assert_eq!(
            classify_repair_target(&report),
            RepairTarget::Implementation
        );
    }

    #[test]
    fn parsed_python_traceback_flows_into_the_report_and_repair_target() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("app.py"), "raise ValueError('bad')\n").unwrap();
        let stderr = "Traceback (most recent call last):\n  File \"app.py\", line 1, in <module>\nValueError: bad\n";
        let extraction = extract(stderr, dir.path());
        let parsed = extraction.traceback.expect("traceback extracted");
        assert_eq!(parsed.final_frame.file, "app.py");
        assert_eq!(parsed.final_frame.line, 1);
        assert_eq!(parsed.final_frame.function, "<module>");
        assert_eq!(parsed.exception_type, "ValueError");
        assert_eq!(parsed.message, "bad");
        assert_eq!(parsed.target_path.as_deref(), Some("app.py"));
        let mut report = VerificationReport::command_failed("python app.py", "exit 1");
        report.push_python_traceback(parsed);
        assert_eq!(report.status, VerifyStatus::CommandFailed("exit 1".into()));
        assert!(!report.is_pass());
        assert_eq!(report.python_tracebacks.len(), 1);
    }

    fn step(expected_paths: &[&str], verify: &[&str]) -> PlanStep {
        PlanStep {
            id: "s".to_string(),
            kind: "work".to_string(),
            expected_result: "pass".to_string(),
            instruction: "x".to_string(),
            expected_paths: expected_paths.iter().map(|p| p.to_string()).collect(),
            verify: verify.iter().map(|c| c.to_string()).collect(),
        }
    }

    #[test]
    fn verify_step_missing_path_reports_status_reason_and_repair_target() {
        let dir = tempfile::tempdir().unwrap();
        let report = verify_step(dir.path(), &step(&["missing.txt"], &["false"]));
        assert_eq!(
            report.status,
            VerifyStatus::MissingPath("missing.txt".into())
        );
        assert_eq!(report.missing_paths, vec!["missing.txt".to_string()]);
        assert!(!report.is_pass());
        assert_eq!(report.primary_reason(), "missing.txt");
    }

    #[test]
    fn verify_step_passes_when_paths_exist_and_command_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("present.txt"), "ok\n").unwrap();
        let report = verify_step(
            dir.path(),
            &step(&["present.txt"], &["test -f present.txt"]),
        );
        assert_eq!(report, VerificationReport::pass());
        assert!(report.is_pass());
    }

    #[test]
    fn verify_step_failing_command_is_recorded_as_command_failure() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("present.txt"), "ok\n").unwrap();
        let report = verify_step(dir.path(), &step(&["present.txt"], &["false"]));
        assert!(!report.is_pass());
        assert_eq!(report.command_failures.len(), 1, "{report:?}");
        assert_eq!(report.command_failures[0].command, "false");
        assert_eq!(
            report.status,
            VerifyStatus::CommandFailed(report.command_failures[0].reason.clone())
        );
        assert_eq!(report.primary_reason(), report.command_failures[0].reason);
    }
}
