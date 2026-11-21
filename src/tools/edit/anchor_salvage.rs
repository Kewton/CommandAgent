//! Indentation guard for the whitespace-normalized Edit anchor salvage.
//!
//! The salvage replaces the one region whose whitespace-normalized text equals
//! `old_string`. Normalization hides indentation, so writing `new_string`
//! verbatim can move lines to another block level: a method sent without its
//! class indentation leaves the class. The replacement is accepted only when
//! its indentation keeps the region's levels:
//!
//! - (a) `new_string` already sits at the region's levels, or
//! - (b) every non-blank line of `old_string` differs from its region line by
//!   the same indentation shift, and that shift is applied to `new_string`.
//!
//! Anything else, including indentation that mixes tabs and spaces, returns
//! `None` so the caller reports `edit_anchor_not_found` with the current
//! excerpt. The rules read only leading whitespace, never the file type.

/// The text to write in place of `region`, or `None` when neither (a) nor (b)
/// shows that the replacement keeps the region's indentation levels.
pub(super) fn indentation_preserving_replacement(
    region: &str,
    old: &str,
    new: &str,
) -> Option<String> {
    let unit = indent_unit(&[region, old, new])?;
    if keeps_region_levels(new, region) {
        return Some(new.to_string());
    }
    let shift = uniform_shift(old, region)?;
    let shifted = shift_lines(new, shift, unit)?;
    keeps_region_levels(&shifted, region).then_some(shifted)
}

/// The single indentation character used by every non-blank line, `' '` when
/// no line is indented, or `None` when tabs and spaces are mixed.
fn indent_unit(texts: &[&str]) -> Option<char> {
    let mut seen: Option<char> = None;
    for line in texts.iter().flat_map(|text| text.lines()) {
        if is_blank(line) {
            continue;
        }
        for ch in leading(line).chars() {
            match seen {
                None => seen = Some(ch),
                Some(unit) if unit != ch => return None,
                Some(_) => {}
            }
        }
    }
    Some(seen.unwrap_or(' '))
}

/// The indentation shift from `old` to `region` when it is the same on every
/// non-blank line and the lines pair up one to one by normalized text.
fn uniform_shift(old: &str, region: &str) -> Option<isize> {
    // The region is a window of whole lines, so it splits on every newline;
    // `old` was windowed by its `lines()` count.
    let old_lines = old.lines().collect::<Vec<_>>();
    let region_lines = region
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect::<Vec<_>>();
    if old_lines.len() != region_lines.len() {
        return None;
    }
    let mut shift = None;
    for (old_line, region_line) in old_lines.iter().zip(&region_lines) {
        if normalize(old_line) != normalize(region_line) {
            return None;
        }
        if is_blank(old_line) {
            continue;
        }
        let delta = indent_width(region_line) as isize - indent_width(old_line) as isize;
        match shift {
            None => shift = Some(delta),
            Some(existing) if existing != delta => return None,
            Some(_) => {}
        }
    }
    shift
}

/// `text` with every non-blank line moved by `shift` indentation characters,
/// or `None` when a line has less indentation than a negative shift removes.
fn shift_lines(text: &str, shift: isize, unit: char) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        if is_blank(line) {
            out.push_str(line);
        } else if shift >= 0 {
            out.extend(std::iter::repeat_n(unit, shift.unsigned_abs()));
            out.push_str(line);
        } else {
            let remove = shift.unsigned_abs();
            if indent_width(line) < remove {
                return None;
            }
            out.push_str(&line[remove..]);
        }
    }
    Some(out)
}

/// Whether `candidate` keeps the region's levels: it starts at the region's
/// first level, never goes above the region's shallowest level, and the lines
/// it shares with the start and end of the region keep their indentation.
fn keeps_region_levels(candidate: &str, region: &str) -> bool {
    let candidate_lines = non_blank_lines(candidate);
    let region_lines = non_blank_lines(region);
    let (Some(first), Some(region_first)) = (candidate_lines.first(), region_lines.first()) else {
        return candidate_lines.is_empty();
    };
    if indent_width(first) != indent_width(region_first) {
        return false;
    }
    let region_min = region_lines.iter().map(|line| indent_width(line)).min();
    let candidate_min = candidate_lines.iter().map(|line| indent_width(line)).min();
    if candidate_min < region_min {
        return false;
    }
    let shared = candidate_lines.len().min(region_lines.len());
    let prefix = candidate_lines
        .iter()
        .zip(&region_lines)
        .take_while(|(candidate, region)| normalize(candidate) == normalize(region))
        .collect::<Vec<_>>();
    let suffix = candidate_lines
        .iter()
        .rev()
        .zip(region_lines.iter().rev())
        .take(shared - prefix.len())
        .take_while(|(candidate, region)| normalize(candidate) == normalize(region))
        .collect::<Vec<_>>();
    prefix
        .into_iter()
        .chain(suffix)
        .all(|(candidate, region)| indent_width(candidate) == indent_width(region))
}

fn non_blank_lines(text: &str) -> Vec<&str> {
    text.lines().filter(|line| !is_blank(line)).collect()
}

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

fn leading(line: &str) -> &str {
    let rest = line.trim_start_matches([' ', '\t']);
    &line[..line.len() - rest.len()]
}

fn indent_width(line: &str) -> usize {
    leading(line).len()
}

fn normalize(line: &str) -> String {
    line.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests;
