//! The request guidance text, one short block per pattern (Issue #918, D5).
//!
//! Guidance only suggests how to proceed. It names no task family, file, or
//! model, and it grants or removes no tool, completion, or evidence rule.

use super::RequestPattern;

/// Version of the guidance text below. Bump it whenever any block changes.
pub const GUIDANCE_VERSION: &str = "v1";

const ANSWER: &str = "This request asks for an answer, not a change.\n\
- Read only the evidence you need, then answer in the requested form.\n\
- Answer without creating or changing files unless the request asks for it.\n\
- If the evidence is missing, say what is unknown instead of guessing.";

const CREATE: &str = "This request asks you to create something new.\n\
- Check the target paths first and do not overwrite files the request does not name.\n\
- Write each requested artifact completely.\n\
- Run the program you wrote at least once with a small input derived from the request; if it fails, fix it and run it again.\n\
- Report only the results you observed.";

const MODIFY: &str = "This request asks you to change something that already exists.\n\
- Read the existing code and the interfaces around it before you edit, and keep the change within the request.\n\
- Run the program you changed at least once with a small input derived from the request; if it fails, fix it and run it again.\n\
- Do not weaken existing tests or checks.\n\
- Report only the results you observed.";

const INVESTIGATE: &str = "This request asks you to investigate, not to repair.\n\
- Do not change the subject you are investigating.\n\
- Gather evidence within the allowed scope.\n\
- Report observations separately from hypotheses, and say so when you cannot reproduce a problem.";

pub(super) fn text(pattern: RequestPattern) -> &'static str {
    match pattern {
        RequestPattern::Answer => ANSWER,
        RequestPattern::Create => CREATE,
        RequestPattern::Modify => MODIFY,
        RequestPattern::Investigate => INVESTIGATE,
    }
}
