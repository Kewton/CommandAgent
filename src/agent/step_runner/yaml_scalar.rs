pub(crate) fn parse_block_scalar_value(
    lines: &[&str],
    index: &mut usize,
    field_line: &str,
    value: &str,
    field_name: &str,
) -> Result<Option<String>, String> {
    let value = value.trim();
    let style = match value {
        "|" => BlockScalarStyle::Literal,
        ">" => BlockScalarStyle::Folded,
        _ if value.starts_with('|') || value.starts_with('>') => {
            return Err(format!(
                "unsupported block scalar style for {field_name}: {value}"
            ));
        }
        _ => return Ok(None),
    };

    let field_indent = leading_spaces(field_line);
    let mut raw_block_lines = Vec::new();
    while *index < lines.len() {
        let line = lines[*index].trim_end();
        if line.trim().is_empty() {
            raw_block_lines.push(String::new());
            *index += 1;
            continue;
        }
        let indent = leading_spaces(line);
        if indent <= field_indent {
            break;
        }
        raw_block_lines.push(line.to_string());
        *index += 1;
    }

    let content_indent = raw_block_lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| leading_spaces(line))
        .min()
        .unwrap_or(field_indent + 1);

    let deindented = raw_block_lines
        .into_iter()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else if line.len() >= content_indent {
                line[content_indent..].to_string()
            } else {
                String::new()
            }
        })
        .collect::<Vec<_>>();

    let parsed = match style {
        BlockScalarStyle::Literal => deindented.join("\n"),
        BlockScalarStyle::Folded => fold_block_scalar_lines(&deindented),
    };
    Ok(Some(parsed))
}

fn fold_block_scalar_lines(lines: &[String]) -> String {
    let mut out = String::new();
    for line in lines {
        if line.trim().is_empty() {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            if !out.ends_with("\n\n") {
                out.push('\n');
            }
            continue;
        }
        if !out.is_empty() && !out.ends_with('\n') {
            out.push(' ');
        }
        out.push_str(line.trim());
    }
    out.trim_end_matches('\n').to_string()
}

fn leading_spaces(line: &str) -> usize {
    line.as_bytes()
        .iter()
        .take_while(|byte| **byte == b' ')
        .count()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockScalarStyle {
    Literal,
    Folded,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_literal_block_scalar() {
        let lines = vec!["instruction: |", "  one", "  two", "expected_paths:"];
        let mut index = 1;

        let parsed = parse_block_scalar_value(&lines, &mut index, lines[0], "|", "instruction")
            .unwrap()
            .unwrap();

        assert_eq!(parsed, "one\ntwo");
        assert_eq!(index, 3);
    }

    #[test]
    fn parses_folded_block_scalar() {
        let lines = vec!["instruction: >", "  one", "  two", "expected_paths:"];
        let mut index = 1;

        let parsed = parse_block_scalar_value(&lines, &mut index, lines[0], ">", "instruction")
            .unwrap()
            .unwrap();

        assert_eq!(parsed, "one two");
        assert_eq!(index, 3);
    }

    #[test]
    fn rejects_chomping_indicators_for_now() {
        let lines = vec!["instruction: |-"];
        let mut index = 1;

        let err = parse_block_scalar_value(&lines, &mut index, lines[0], "|-", "instruction")
            .unwrap_err();

        assert!(err.contains("unsupported block scalar style"));
    }
}
