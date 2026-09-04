use crate::planner::verify::VerificationReport;

pub(in crate::planner::runner) fn verification_report_signature(
    report: &VerificationReport,
) -> Vec<String> {
    let mut signature = Vec::new();
    signature.extend(
        report
            .missing_paths
            .iter()
            .map(|path| format!("missing:{path}")),
    );
    signature.extend(report.dependency_missing.iter().map(|reason| {
        format!(
            "dependency:{}",
            normalize_report_reason_for_signature(reason)
        )
    }));
    signature.extend(report.command_failures.iter().map(|failure| {
        format!(
            "command:{}:{}",
            failure.command,
            normalize_report_reason_for_signature(&failure.reason)
        )
    }));
    signature.extend(
        report
            .verifier_command_false_negatives
            .iter()
            .map(|failure| {
                format!(
                    "verifier_command:{}:{}",
                    failure.command,
                    normalize_report_reason_for_signature(&failure.reason)
                )
            }),
    );
    signature.extend(
        report
            .profile_failures
            .iter()
            .map(|reason| format!("profile:{reason}")),
    );
    signature.extend(
        report
            .python_tracebacks
            .iter()
            .map(|value| value.signature()),
    );
    signature.sort();
    signature
}

fn normalize_report_reason_for_signature(reason: &str) -> String {
    let mut normalized = Vec::new();
    let mut parts = reason.split_whitespace();
    while let Some(part) = parts.next() {
        if part == "elapsed_ms:" {
            let _ = parts.next();
            normalized.push("elapsed_ms:<n>".to_string());
        } else {
            normalized.push(part.to_string());
        }
    }
    normalized.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dependency_report(reason: &str) -> VerificationReport {
        let mut report = VerificationReport::pass();
        report.push_dependency_missing(reason);
        report
    }

    #[test]
    fn dependency_signature_normalizes_elapsed_time_only() {
        let first = dependency_report("npm run build elapsed_ms: 978 summary: missing next");
        let second = dependency_report("npm run build elapsed_ms: 1029 summary: missing next");
        let distinct = dependency_report("npm run build elapsed_ms: 1029 summary: missing react");

        assert_eq!(
            verification_report_signature(&first),
            verification_report_signature(&second)
        );
        assert_ne!(
            verification_report_signature(&first),
            verification_report_signature(&distinct)
        );
    }
}
