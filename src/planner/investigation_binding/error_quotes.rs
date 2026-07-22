use crate::planner::adjudication::investigate::{DiagnosisClaim, DiagnosisClaimKind};

pub(super) fn fenced_claim(output: &str, value: String) -> Option<DiagnosisClaim> {
    output_contains_quote(output, &value).then(|| bind_error_quote(output, value))
}

pub(super) fn line_claims(line: &str, output: &str) -> Vec<DiagnosisClaim> {
    let trimmed = line.trim();
    let inline_quotes = inline_code_values(line);
    let explicit_error_line = error_line_value(trimmed);
    let mut claims = inline_quotes
        .iter()
        .filter(|quoted| {
            output_contains_quote(output, quoted)
                || looks_like_error_quote(quoted)
                || explicit_error_line.is_some()
        })
        .cloned()
        .map(|quote| bind_error_quote(output, quote))
        .collect::<Vec<_>>();
    if inline_quotes.is_empty() {
        if let Some(quote) = explicit_error_line {
            claims.push(bind_error_quote(output, quote));
        } else if let Some(quote) = quote_form_value(trimmed)
            .filter(|quote| output_contains_quote(output, quote) || looks_like_error_quote(quote))
        {
            claims.push(bind_error_quote(output, quote));
        }
    }
    claims
}

fn bind_error_quote(output: &str, value: String) -> DiagnosisClaim {
    let matched = output_contains_quote(output, &value);
    DiagnosisClaim {
        kind: DiagnosisClaimKind::ErrorQuote,
        value: value.clone(),
        subject_path: None,
        line: None,
        matched,
        nearest: (!matched).then(|| nearest_output_line(output, &value)),
    }
}

fn inline_code_values(line: &str) -> Vec<String> {
    let marker = char::from(96);
    let mut values = Vec::new();
    let mut rest = line;
    while let Some((_, tail)) = rest.split_once(marker) {
        let Some((value, after)) = tail.split_once(marker) else {
            break;
        };
        let value = value.trim();
        if !value.is_empty() {
            values.push(value.to_string());
        }
        rest = after;
    }
    values
}

fn looks_like_error_quote(value: &str) -> bool {
    value.contains("Error") || value.contains("Exception") || value.contains("Traceback")
}

fn output_contains_quote(output: &str, value: &str) -> bool {
    !value.trim().is_empty() && output.contains(value.trim())
}

fn error_line_value(line: &str) -> Option<String> {
    let line = line.trim_start_matches(['-', '*']).trim();
    let (label, value) = line.split_once(':').or_else(|| line.split_once('：'))?;
    let label = label.trim().to_ascii_lowercase();
    let explicit = matches!(
        label.as_str(),
        "error" | "error quote" | "observed error" | "exception" | "traceback"
    ) || matches!(
        label.as_str(),
        "エラー" | "エラー引用" | "例外" | "トレースバック"
    );
    explicit.then(|| trim_quote_markers(value))
}

fn quote_form_value(line: &str) -> Option<String> {
    if let Some(value) = line.strip_prefix('>') {
        return Some(trim_quote_markers(value));
    }
    let value = line.trim();
    let paired = [
        ('"', '"'),
        ('\'', '\''),
        ('“', '”'),
        ('「', '」'),
        ('『', '』'),
    ];
    paired.iter().find_map(|(open, close)| {
        value
            .strip_prefix(*open)
            .and_then(|value| value.strip_suffix(*close))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn trim_quote_markers(value: &str) -> String {
    value
        .trim()
        .trim_matches(|ch: char| {
            matches!(ch, '`' | '"' | '\'' | '“' | '”' | '「' | '」' | '『' | '』')
        })
        .trim()
        .to_string()
}

fn nearest_output_line(output: &str, claim: &str) -> String {
    let needle = claim.split(':').next().unwrap_or(claim);
    output
        .lines()
        .find(|line| line.contains(needle))
        .or_else(|| output.lines().find(|line| !line.trim().is_empty()))
        .unwrap_or("no reproducer output")
        .to_string()
}
