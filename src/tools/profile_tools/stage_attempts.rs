//! What a stage tool's answer says of G4's counts (Issue #1004).
//!
//! G4 counts the attempts of each target and of the run, and ends the run at either limit. The
//! model hears of a limit only through its answers, so every answer that goes back as data says,
//! at its top, how many attempts the target it called and the run have used, and their limits.
//! `used` is the count as this call was judged: a call that started a process is in it. A new
//! upstream version gives no attempt back, so a reopened target goes on from where it was.
//!
//! The numbers are the host's own, read from the ledger under the same lock that judged the
//! call. No operator or process chooses them, so they stay in an answer the history cut down.

use serde_json::{Value, json};

use crate::planner::structured_stages::state::{AttemptCount, AttemptCounts};

/// The top-level field of a stage answer that carries the counts.
pub(crate) const ATTEMPTS_FIELD: &str = "attempts";

/// `answer` with the counts at its top. With no counts, or an answer that is not an object,
/// it is returned as it was.
pub(super) fn with_attempts(mut answer: Value, counts: Option<AttemptCounts>) -> Value {
    if let (Some(counts), Some(fields)) = (counts, answer.as_object_mut()) {
        fields.insert(ATTEMPTS_FIELD.to_string(), attempts_value(counts));
    }
    answer
}

fn attempts_value(counts: AttemptCounts) -> Value {
    json!({"target": count_value(counts.target), "run": count_value(counts.run)})
}

fn count_value(count: AttemptCount) -> Value {
    json!({"used": count.used, "limit": count.limit})
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ATTEMPTS_FIELD, with_attempts};
    use crate::planner::structured_stages::state::{AttemptCount, AttemptCounts};

    fn counts(target: (u32, u32), run: (u32, u32)) -> AttemptCounts {
        AttemptCounts {
            target: AttemptCount {
                used: target.0,
                limit: target.1,
            },
            run: AttemptCount {
                used: run.0,
                limit: run.1,
            },
        }
    }

    #[test]
    fn the_counts_go_at_the_top_of_an_answer_and_nothing_else_moves() {
        let answer = json!({"status": "resubmit", "code": "check_failed", "truncated": false});
        let with = with_attempts(answer.clone(), Some(counts((2, 4), (9, 32))));
        assert_eq!(
            with[ATTEMPTS_FIELD],
            json!({"target": {"used": 2, "limit": 4}, "run": {"used": 9, "limit": 32}})
        );
        let mut without = with.clone();
        without.as_object_mut().unwrap().remove(ATTEMPTS_FIELD);
        assert_eq!(without, answer);
    }

    #[test]
    fn no_counts_or_no_object_leaves_the_answer_as_it_was() {
        let answer = json!({"status": "stale", "revision": "g1-0123456789abcdef"});
        assert_eq!(with_attempts(answer.clone(), None), answer);
        let list = json!(["not", "an", "object"]);
        assert_eq!(
            with_attempts(list.clone(), Some(counts((1, 1), (1, 1)))),
            list
        );
    }

    #[test]
    fn the_counts_are_numbers_the_host_holds_and_no_text() {
        let with = with_attempts(json!({}), Some(counts((64, 64), (64, 64))));
        let text = with.to_string();
        assert_eq!(
            text,
            r#"{"attempts":{"run":{"limit":64,"used":64},"target":{"limit":64,"used":64}}}"#
        );
    }
}
