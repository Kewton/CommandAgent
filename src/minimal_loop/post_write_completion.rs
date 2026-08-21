use std::collections::BTreeSet;

const CONFIRMING_READ_BATCH_LIMIT: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PostWriteCompletionEvidence {
    pub(crate) confirmation_batches: usize,
    pub(crate) read_paths: Vec<String>,
}

#[derive(Debug, Default)]
pub(crate) struct PostWriteCompletionTracker {
    written_paths: BTreeSet<String>,
    consecutive_confirming_reads: usize,
    unresolved_bash_failure: bool,
    unresolved_mutation_failure: bool,
    environment_failure_kind: Option<&'static str>,
}

impl PostWriteCompletionTracker {
    pub(crate) fn note_successful_write(&mut self, path: &str) {
        self.written_paths.insert(path.to_string());
        self.consecutive_confirming_reads = 0;
        self.unresolved_bash_failure = false;
        self.unresolved_mutation_failure = false;
        self.environment_failure_kind = None;
    }

    pub(crate) fn note_recoverable_tool_error(&mut self, tool_name: &str) {
        if self.written_paths.is_empty() {
            return;
        }
        match tool_name {
            "Bash" => self.unresolved_bash_failure = true,
            "Write" | "Edit" => self.unresolved_mutation_failure = true,
            _ => return,
        }
        self.consecutive_confirming_reads = 0;
    }

    pub(crate) fn note_bash_result(
        &mut self,
        succeeded: bool,
        environment_failure_kind: Option<&'static str>,
    ) {
        if self.written_paths.is_empty() {
            return;
        }
        self.consecutive_confirming_reads = 0;
        self.unresolved_bash_failure = !succeeded;
        self.environment_failure_kind = (!succeeded).then_some(environment_failure_kind).flatten();
    }

    pub(crate) fn environment_failure_reason(&self) -> Option<String> {
        self.environment_failure_kind.map(|kind| {
            format!("deterministic_environment_error:{kind}: command could not execute")
        })
    }

    pub(crate) fn observe_batch(
        &mut self,
        all_tools_are_reads: bool,
        read_paths: &[String],
        had_recoverable_error: bool,
    ) -> Option<PostWriteCompletionEvidence> {
        let confirms_written_paths = all_tools_are_reads
            && !had_recoverable_error
            && !self.unresolved_bash_failure
            && !self.unresolved_mutation_failure
            && !read_paths.is_empty()
            && read_paths
                .iter()
                .all(|path| self.written_paths.contains(path));
        if !confirms_written_paths {
            self.consecutive_confirming_reads = 0;
            return None;
        }
        self.consecutive_confirming_reads += 1;
        (self.consecutive_confirming_reads >= CONFIRMING_READ_BATCH_LIMIT).then(|| {
            PostWriteCompletionEvidence {
                confirmation_batches: self.consecutive_confirming_reads,
                read_paths: read_paths.to_vec(),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn two_consecutive_reads_of_written_paths_complete() {
        let mut tracker = PostWriteCompletionTracker::default();
        tracker.note_successful_write("README.md");
        tracker.note_successful_write("hello.py");

        assert_eq!(
            tracker.observe_batch(true, &paths(&["README.md"]), false),
            None
        );
        assert_eq!(
            tracker.observe_batch(true, &paths(&["hello.py"]), false),
            Some(PostWriteCompletionEvidence {
                confirmation_batches: 2,
                read_paths: paths(&["hello.py"]),
            })
        );
    }

    #[test]
    fn no_write_unrelated_read_and_failed_bash_do_not_complete() {
        let mut tracker = PostWriteCompletionTracker::default();
        assert_eq!(
            tracker.observe_batch(true, &paths(&["README.md"]), false),
            None
        );

        tracker.note_successful_write("hello.py");
        assert_eq!(
            tracker.observe_batch(true, &paths(&["README.md"]), false),
            None
        );
        tracker.note_bash_result(false, Some("exit_127"));
        assert_eq!(
            tracker.observe_batch(true, &paths(&["hello.py"]), false),
            None
        );
        assert_eq!(
            tracker.observe_batch(true, &paths(&["hello.py"]), false),
            None
        );
        assert_eq!(
            tracker.environment_failure_reason().as_deref(),
            Some("deterministic_environment_error:exit_127: command could not execute")
        );

        tracker.note_bash_result(true, None);
        assert_eq!(tracker.environment_failure_reason(), None);
        assert_eq!(
            tracker.observe_batch(true, &paths(&["hello.py"]), false),
            None
        );
        assert!(
            tracker
                .observe_batch(true, &paths(&["hello.py"]), false)
                .is_some()
        );

        tracker.note_recoverable_tool_error("Edit");
        tracker.note_bash_result(true, None);
        assert_eq!(
            tracker.observe_batch(true, &paths(&["hello.py"]), false),
            None
        );
        tracker.note_successful_write("hello.py");
        assert_eq!(
            tracker.observe_batch(true, &paths(&["hello.py"]), false),
            None
        );
        assert!(
            tracker
                .observe_batch(true, &paths(&["hello.py"]), false)
                .is_some()
        );
    }
}
