//! Select an existing template from phase intent, never incidental global rules.
use super::knowledge;

pub(super) enum Template {
    Scaffold,
    PortScripts,
    BuildVerification,
}

pub(super) fn select(prompt: &str) -> Option<Template> {
    let phase_id = phase_field(prompt, "Phase id:")
        .unwrap_or_default()
        .to_ascii_lowercase();
    // A port subtask cannot replace an explicitly broader implementation phase.
    if matches!(phase_id.as_str(), "core-implementation" | "contract-wiring") {
        return None;
    }

    let text = phase_id_and_task_text(prompt)
        .unwrap_or_else(|| prompt.to_string())
        .to_ascii_lowercase();
    let keywords = &knowledge::get().deterministic_keywords;
    // Preserve the legacy conservative veto, including natural inflections
    // such as scoreboard/player controlling. Only positive template evidence
    // uses strict boundaries; a missed optimization leaves work to the planner.
    if keywords
        .implementation_phase
        .iter()
        .any(|keyword| text.contains(keyword))
    {
        return None;
    }
    // Edited plans can reuse preset ids. Require the phase-local task to pass
    // the implementation guard above, without borrowing global goal/rule text.
    match phase_id.as_str() {
        "project-setup" => return Some(Template::Scaffold),
        "build-verification" => return Some(Template::BuildVerification),
        _ => {}
    }
    if contains_any(&text, &keywords.scaffold_phase)
        && contains_any(&phase_id, &keywords.scaffold_phase_id)
    {
        return Some(Template::Scaffold);
    }
    if contains_any(&text, &keywords.port_phase_markers)
        && contains_any(&text, &keywords.port_script_phase)
    {
        return Some(Template::PortScripts);
    }
    if contains_any(&text, &keywords.build_verify_phase) {
        return Some(Template::BuildVerification);
    }
    None
}

fn phase_id_and_task_text(prompt: &str) -> Option<String> {
    let fields = prompt
        .lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with("Phase id:") || line.starts_with("Phase task:"))
        .collect::<Vec<_>>();
    (!fields.is_empty()).then(|| fields.join("\n"))
}

fn phase_field<'a>(prompt: &'a str, prefix: &str) -> Option<&'a str> {
    prompt
        .lines()
        .find_map(|line| line.trim_start().strip_prefix(prefix).map(str::trim))
}

fn contains_any(text: &str, keywords: &[String]) -> bool {
    keywords.iter().any(|keyword| {
        // Japanese phrases do not require whitespace boundaries. ASCII words
        // must not borrow suffixes from TypeScript, import/export or identifiers.
        let word_start = keyword.starts_with(identifier_char);
        let word_end = keyword.ends_with(identifier_char);
        text.match_indices(keyword).any(|(start, matched)| {
            (!word_start || !text[..start].ends_with(identifier_char))
                && (!word_end || !text[start + matched.len()..].starts_with(identifier_char))
        })
    })
}

fn identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}
