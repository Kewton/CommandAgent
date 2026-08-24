use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExecutionProgress {
    New,
    Repeated,
    NotSuccessful,
}

#[derive(Debug, Default)]
pub(crate) struct ExecutionProgressTracker {
    successful_commands: BTreeSet<String>,
}

impl ExecutionProgressTracker {
    pub(crate) fn observe_bash(
        &mut self,
        command: Option<&str>,
        formatted_outcome: &str,
    ) -> ExecutionProgress {
        let Some(command) = command.map(str::trim).filter(|command| !command.is_empty()) else {
            return ExecutionProgress::NotSuccessful;
        };
        if !bash_succeeded(formatted_outcome) {
            return ExecutionProgress::NotSuccessful;
        }
        if self.successful_commands.insert(command.to_string()) {
            ExecutionProgress::New
        } else {
            // Command identity is deliberately stricter than output identity. A changing
            // timestamp or elapsed_ms must not turn the same command loop into progress.
            ExecutionProgress::Repeated
        }
    }
}

fn bash_succeeded(formatted_outcome: &str) -> bool {
    matches!(
        formatted_outcome.lines().next(),
        Some("outcome: Success" | "combined_outcome: Success")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUCCESS: &str =
        "outcome: Success\nstatus: exit status: 0\nelapsed_ms: 21\nstdout:\n8 records";
    const FAILURE: &str =
        "outcome: CommandFailed\nstatus: exit status: 1\nelapsed_ms: 21\nstdout:\n";

    #[test]
    fn distinct_successful_commands_are_progress() {
        let mut tracker = ExecutionProgressTracker::default();

        assert_eq!(
            tracker.observe_bash(Some("python3 pipeline/main.py"), SUCCESS),
            ExecutionProgress::New
        );
        assert_eq!(
            tracker.observe_bash(Some("python3 pipeline/check.py"), SUCCESS),
            ExecutionProgress::New
        );
    }

    #[test]
    fn identical_command_and_output_only_progress_once() {
        let mut tracker = ExecutionProgressTracker::default();

        assert_eq!(
            tracker.observe_bash(Some("python3 pipeline/main.py"), SUCCESS),
            ExecutionProgress::New
        );
        assert_eq!(
            tracker.observe_bash(Some("python3 pipeline/main.py"), SUCCESS),
            ExecutionProgress::Repeated
        );
        assert_eq!(
            tracker.observe_bash(
                Some("python3 pipeline/main.py"),
                &SUCCESS.replace("elapsed_ms: 21", "elapsed_ms: 99"),
            ),
            ExecutionProgress::Repeated
        );
    }

    #[test]
    fn failed_command_does_not_consume_later_success() {
        let mut tracker = ExecutionProgressTracker::default();

        assert_eq!(
            tracker.observe_bash(Some("python3 pipeline/main.py"), FAILURE),
            ExecutionProgress::NotSuccessful
        );
        assert_eq!(
            tracker.observe_bash(Some("python3 pipeline/main.py"), SUCCESS),
            ExecutionProgress::New
        );
    }
}
