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

fn display_width_ansi(value: &str) -> usize {
    let mut width = 0usize;
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            let _ = chars.next();
            for next in chars.by_ref() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
        } else {
            width += char_display_width(ch);
        }
    }
    width
}

fn char_display_width(ch: char) -> usize {
    let cp = ch as u32;
    if cp < 0x20
        || (0x7f..=0x9f).contains(&cp)
        || matches!(
            cp,
            0x0300..=0x036f
                | 0x1ab0..=0x1aff
                | 0x1dc0..=0x1dff
                | 0x20d0..=0x20ff
                | 0xfe00..=0xfe0f
                | 0xfe20..=0xfe2f
                | 0xe0100..=0xe01ef
        )
    {
        0
    } else if matches!(
        cp,
        0x1100..=0x115f
            | 0x231a..=0x231b
            | 0x2329..=0x232a
            | 0x23e9..=0x23ec
            | 0x23f0
            | 0x23f3
            | 0x25fd..=0x25fe
            | 0x2614..=0x2615
            | 0x2648..=0x2653
            | 0x267f
            | 0x2693
            | 0x26a1
            | 0x26aa..=0x26ab
            | 0x26bd..=0x26be
            | 0x26c4..=0x26c5
            | 0x26ce
            | 0x26d4
            | 0x26ea
            | 0x26f2..=0x26f3
            | 0x26f5
            | 0x26fa
            | 0x26fd
            | 0x2705
            | 0x270a..=0x270b
            | 0x2728
            | 0x274c
            | 0x274e
            | 0x2753..=0x2755
            | 0x2757
            | 0x2795..=0x2797
            | 0x27b0
            | 0x27bf
            | 0x2b1b..=0x2b1c
            | 0x2b50
            | 0x2b55
            | 0x2e80..=0xa4cf
            | 0xac00..=0xd7a3
            | 0xf900..=0xfaff
            | 0xfe10..=0xfe19
            | 0xfe30..=0xfe6f
            | 0xff00..=0xff60
            | 0xffe0..=0xffe6
            | 0x1f004
            | 0x1f0cf
            | 0x1f18e
            | 0x1f191..=0x1f19a
            | 0x1f200..=0x1f202
            | 0x1f210..=0x1f23b
            | 0x1f240..=0x1f248
            | 0x1f250..=0x1f251
            | 0x1f300..=0x1f64f
            | 0x1f680..=0x1f6ff
            | 0x1f900..=0x1f9ff
            | 0x20000..=0x3fffd
    ) {
        2
    } else {
        1
    }
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
