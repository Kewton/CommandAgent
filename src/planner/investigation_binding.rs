use std::path::Path;

use crate::planner::adjudication::investigate::{
    DiagnosisClaim, DiagnosisClaimKind, InvestigationBindingEvidence, InvestigationRunEvidence,
};

pub(crate) fn bind_diagnosis(
    root: &Path,
    diagnosis: &str,
    run: &InvestigationRunEvidence,
) -> InvestigationBindingEvidence {
    let mut claims = Vec::new();
    let output = format!("{}\n{}", run.stdout, run.stderr);
    let mut current_file = None::<String>;
    let mut fenced = false;
    let mut snippet = Vec::new();
    for line in diagnosis.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("~~~") {
            if fenced {
                let value = snippet.join("\n").trim().to_string();
                if !value.is_empty() {
                    claims.push(bind_snippet(root, current_file.as_deref(), value));
                }
                snippet.clear();
            }
            fenced = !fenced;
            continue;
        }
        if trimmed.starts_with(char::from(96))
            && trimmed
                .chars()
                .take_while(|ch| *ch == char::from(96))
                .count()
                >= 3
        {
            if fenced {
                let value = snippet.join("\n").trim().to_string();
                if !value.is_empty() {
                    claims.push(bind_snippet(root, current_file.as_deref(), value));
                }
                snippet.clear();
            }
            fenced = !fenced;
            continue;
        }
        if fenced {
            snippet.push(line.to_string());
            continue;
        }
        for quoted in inline_code_values(line) {
            if looks_like_error_quote(&quoted) {
                let matched = output.contains(&quoted);
                claims.push(DiagnosisClaim {
                    kind: DiagnosisClaimKind::ErrorQuote,
                    value: quoted.clone(),
                    subject_path: None,
                    line: None,
                    matched,
                    nearest: (!matched).then(|| nearest_output_line(&output, &quoted)),
                });
            }
        }
        for token in line.split_whitespace() {
            if let Some((path, number)) = file_line_reference(token) {
                current_file = Some(path.clone());
                let file = root.join(&path);
                let line_count = std::fs::read_to_string(&file)
                    .map(|text| text.lines().count())
                    .unwrap_or(0);
                let matched = file.is_file() && number > 0 && number <= line_count;
                claims.push(DiagnosisClaim {
                    kind: DiagnosisClaimKind::FileLine,
                    value: format!("{path}:{number}"),
                    subject_path: Some(path.clone()),
                    line: Some(number),
                    matched,
                    nearest: (!matched).then(|| {
                        if file.is_file() {
                            format!("{path}:{}", line_count.max(1))
                        } else {
                            nearest_existing_path(root, &path)
                        }
                    }),
                });
            }
        }
    }
    InvestigationBindingEvidence::new(claims)
}

fn bind_snippet(root: &Path, path: Option<&str>, value: String) -> DiagnosisClaim {
    let contents = path.and_then(|path| std::fs::read_to_string(root.join(path)).ok());
    let matched = contents
        .as_deref()
        .is_some_and(|contents| contents.contains(&value));
    DiagnosisClaim {
        kind: DiagnosisClaimKind::CodeSnippet,
        value: value.clone(),
        subject_path: path.map(str::to_string),
        line: None,
        matched,
        nearest: (!matched).then(|| {
            contents
                .as_deref()
                .and_then(|contents| nearest_source_line(contents, &value))
                .unwrap_or_else(|| "no referenced existing file".to_string())
        }),
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

fn file_line_reference(token: &str) -> Option<(String, usize)> {
    let token = token.trim_matches(|ch: char| {
        matches!(ch, '\'' | '"' | '(' | ')' | '[' | ']' | ',' | '.') || ch == char::from(96)
    });
    let (path, line) = token.rsplit_once(':')?;
    let line = line.parse::<usize>().ok()?;
    if path.is_empty() || (!path.contains('/') && !path.contains('.')) {
        return None;
    }
    Some((path.to_string(), line))
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

fn nearest_existing_path(root: &Path, path: &str) -> String {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path);
    walk_files(root)
        .into_iter()
        .find(|candidate| candidate.ends_with(file_name))
        .unwrap_or_else(|| "no nearby existing path".to_string())
}

fn walk_files(root: &Path) -> Vec<String> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if let Ok(relative) = path.strip_prefix(root) {
                files.push(relative.to_string_lossy().to_string());
            }
        }
    }
    files.sort();
    files
}

fn nearest_source_line(contents: &str, snippet: &str) -> Option<String> {
    let needle = snippet.split_whitespace().next()?;
    contents
        .lines()
        .find(|line| line.contains(needle))
        .map(str::to_string)
        .or_else(|| contents.lines().next().map(str::to_string))
}

#[cfg(test)]
mod tests;
