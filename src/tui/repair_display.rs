pub(crate) fn bounded_attempt(attempt: usize, max: usize) -> usize {
    if max == 0 { attempt } else { attempt.min(max) }
}

pub(crate) fn progress_label(attempt: usize, max: usize) -> String {
    let attempt = bounded_attempt(attempt, max);
    if max == 0 {
        attempt.to_string()
    } else {
        format!("{attempt}/{max}")
    }
}
