use crate::util::display_width_ansi;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Alignment {
    Left,
    Center,
    Right,
}

#[derive(Debug)]
struct RawLine {
    text: String,
    append_newline: bool,
}

#[derive(Debug)]
pub(super) struct Table {
    alignments: Vec<Alignment>,
    rows: Vec<Vec<String>>,
    raw_lines: Vec<RawLine>,
    buffered_chars: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AddRow {
    Added,
    NotTableRow,
    Invalid,
    Overflow,
}

impl Table {
    pub(super) fn from_header_and_delimiter(
        header: String,
        header_newline: bool,
        delimiter: String,
        delimiter_newline: bool,
        max_buffered_chars: usize,
    ) -> Option<Self> {
        let header_cells = parse_row(&header)?;
        let delimiter_cells = parse_row(&delimiter)?;
        if header_cells.len() != delimiter_cells.len() {
            return None;
        }
        let alignments = delimiter_cells
            .iter()
            .map(|cell| parse_alignment(cell))
            .collect::<Option<Vec<_>>>()?;
        let buffered_chars = header.chars().count() + delimiter.chars().count();
        if buffered_chars > max_buffered_chars {
            return None;
        }
        Some(Self {
            alignments,
            rows: vec![header_cells],
            raw_lines: vec![
                RawLine {
                    text: header,
                    append_newline: header_newline,
                },
                RawLine {
                    text: delimiter,
                    append_newline: delimiter_newline,
                },
            ],
            buffered_chars,
        })
    }

    pub(super) fn add_row(
        &mut self,
        line: String,
        append_newline: bool,
        max_buffered_chars: usize,
    ) -> AddRow {
        let Some(cells) = parse_row(&line) else {
            return if looks_like_table_row(&line) {
                self.push_raw(line, append_newline);
                AddRow::Invalid
            } else {
                AddRow::NotTableRow
            };
        };
        if cells.len() != self.alignments.len() {
            self.push_raw(line, append_newline);
            return AddRow::Invalid;
        }
        if self.buffered_chars + line.chars().count() > max_buffered_chars {
            self.push_raw(line, append_newline);
            return AddRow::Overflow;
        }
        self.buffered_chars += line.chars().count();
        self.raw_lines.push(RawLine {
            text: line,
            append_newline,
        });
        self.rows.push(cells);
        AddRow::Added
    }

    pub(super) fn render(&self, color_enabled: bool, utf8: bool) -> String {
        let rendered_rows: Vec<Vec<String>> = self
            .rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|cell| super::render_inline(cell, color_enabled))
                    .collect()
            })
            .collect();
        let mut widths = vec![0usize; self.alignments.len()];
        for row in &rendered_rows {
            for (column, cell) in row.iter().enumerate() {
                widths[column] = widths[column].max(display_width_ansi(cell));
            }
        }

        let (
            vertical,
            top_left,
            top_join,
            top_right,
            middle_left,
            middle_join,
            middle_right,
            bottom_left,
            bottom_join,
            bottom_right,
            horizontal,
        ) = if utf8 {
            ("│", "┌", "┬", "┐", "├", "┼", "┤", "└", "┴", "┘", "─")
        } else {
            ("|", "+", "+", "+", "+", "+", "+", "+", "+", "+", "-")
        };

        let mut lines = Vec::with_capacity(rendered_rows.len() + 3);
        lines.push(border(&widths, top_left, top_join, top_right, horizontal));
        for (row_index, row) in rendered_rows.iter().enumerate() {
            lines.push(render_row(row, &widths, &self.alignments, vertical));
            if row_index == 0 {
                lines.push(border(
                    &widths,
                    middle_left,
                    middle_join,
                    middle_right,
                    horizontal,
                ));
            }
        }
        lines.push(border(
            &widths,
            bottom_left,
            bottom_join,
            bottom_right,
            horizontal,
        ));

        let mut out = lines.join("\n");
        if self
            .raw_lines
            .last()
            .is_some_and(|line| line.append_newline)
        {
            out.push('\n');
        }
        out
    }

    pub(super) fn render_literal(&self, color_enabled: bool) -> String {
        let mut out = String::new();
        for line in &self.raw_lines {
            out.push_str(&super::render_inline(&line.text, color_enabled));
            if line.append_newline {
                out.push('\n');
            }
        }
        out
    }

    fn push_raw(&mut self, line: String, append_newline: bool) {
        self.buffered_chars = self.buffered_chars.saturating_add(line.chars().count());
        self.raw_lines.push(RawLine {
            text: line,
            append_newline,
        });
    }
}

pub(super) fn is_delimiter_for(header: &str, delimiter: &str) -> bool {
    let Some(header_cells) = parse_row(header) else {
        return false;
    };
    let Some(delimiter_cells) = parse_row(delimiter) else {
        return false;
    };
    header_cells.len() == delimiter_cells.len()
        && delimiter_cells
            .iter()
            .all(|cell| parse_alignment(cell).is_some())
}

pub(super) fn is_candidate_header(line: &str) -> bool {
    parse_row(line).is_some()
}

fn parse_row(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim();
    let inner = trimmed.strip_prefix('|')?.strip_suffix('|')?;
    let cells: Vec<String> = inner
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect();
    (cells.len() >= 2).then_some(cells)
}

fn looks_like_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') || trimmed.ends_with('|')
}

fn parse_alignment(cell: &str) -> Option<Alignment> {
    let trimmed = cell.trim();
    let left = trimmed.starts_with(':');
    let right = trimmed.ends_with(':');
    let dashes = trimmed.trim_start_matches(':').trim_end_matches(':');
    if dashes.len() < 3 || !dashes.bytes().all(|byte| byte == b'-') {
        return None;
    }
    Some(match (left, right) {
        (true, true) => Alignment::Center,
        (false, true) => Alignment::Right,
        _ => Alignment::Left,
    })
}

fn render_row(
    row: &[String],
    widths: &[usize],
    alignments: &[Alignment],
    vertical: &str,
) -> String {
    let mut out = String::new();
    out.push_str(vertical);
    for (column, cell) in row.iter().enumerate() {
        let padding = widths[column].saturating_sub(display_width_ansi(cell));
        let (before, after) = match alignments[column] {
            Alignment::Left => (0, padding),
            Alignment::Center => (padding / 2, padding - padding / 2),
            Alignment::Right => (padding, 0),
        };
        out.push(' ');
        out.push_str(&" ".repeat(before));
        out.push_str(cell);
        out.push_str(&" ".repeat(after));
        out.push(' ');
        out.push_str(vertical);
    }
    out
}

fn border(widths: &[usize], left: &str, join: &str, right: &str, horizontal: &str) -> String {
    let mut out = String::from(left);
    for (index, width) in widths.iter().enumerate() {
        if index > 0 {
            out.push_str(join);
        }
        out.push_str(&horizontal.repeat(width + 2));
    }
    out.push_str(right);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_width_counts_cjk_and_ignores_sgr() {
        assert_eq!(display_width_ansi("a日本"), 5);
        assert_eq!(display_width_ansi("\x1b[31m日本\x1b[0m"), 4);
        assert_eq!(display_width_ansi("e\u{301}"), 1);
    }
}
