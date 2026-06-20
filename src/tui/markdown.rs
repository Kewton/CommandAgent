const CODE_COLOR: &str = "\x1b[32m";
const INLINE_CODE_COLOR: &str = "\x1b[36m";
const H1_COLOR: &str = "\x1b[1m\x1b[35m";
const H2_COLOR: &str = "\x1b[1m\x1b[33m";
const H3_COLOR: &str = "\x1b[1m\x1b[34m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

pub const MAX_BUFFERED_LINE_CHARS: usize = 64 * 1024;

#[derive(Debug, Clone)]
pub struct MarkdownRenderer {
    line_buffer: String,
    in_code_block: bool,
    in_think_block: bool,
    color_enabled: bool,
    utf8: bool,
}

impl MarkdownRenderer {
    pub fn new(color_enabled: bool, utf8: bool) -> Self {
        Self {
            line_buffer: String::new(),
            in_code_block: false,
            in_think_block: false,
            color_enabled,
            utf8,
        }
    }

    pub fn push_chunk(&mut self, chunk: &str) -> String {
        let mut out = String::new();
        if self.line_buffer.chars().count() + chunk.chars().count() > MAX_BUFFERED_LINE_CHARS {
            self.force_drain_buffer(&mut out);
        }
        self.line_buffer.push_str(chunk);

        while let Some(idx) = self.line_buffer.find('\n') {
            let line: String = self.line_buffer.drain(..=idx).collect();
            let body = line.strip_suffix('\n').unwrap_or(&line);
            self.process_line(body, true, &mut out);
        }

        if self.line_buffer.chars().count() > MAX_BUFFERED_LINE_CHARS {
            self.force_drain_buffer(&mut out);
        }

        out
    }

    pub fn flush(&mut self) -> String {
        let mut out = String::new();
        if !self.line_buffer.is_empty() {
            let body = std::mem::take(&mut self.line_buffer);
            self.process_line(&body, false, &mut out);
        }
        self.line_buffer.clear();
        self.in_code_block = false;
        self.in_think_block = false;
        out
    }

    fn force_drain_buffer(&mut self, out: &mut String) {
        const SUFFIX_RETAIN: usize = 16;
        let len = self.line_buffer.len();
        let mut split = len.saturating_sub(SUFFIX_RETAIN);
        while split < len && !self.line_buffer.is_char_boundary(split) {
            split += 1;
        }
        let suffix = self.line_buffer.split_off(split);
        let body = std::mem::take(&mut self.line_buffer);
        self.line_buffer = suffix;
        let visible = self.strip_think(&body);
        out.push_str(&sanitize(&visible));
        out.push('\n');
    }

    fn process_line(&mut self, line: &str, append_newline: bool, out: &mut String) {
        let visible = self.strip_think(line);
        if visible.is_empty() && (self.in_think_block || contained_think_only(line)) {
            return;
        }

        if visible.trim_start().starts_with("```") {
            self.in_code_block = !self.in_code_block;
            return;
        }

        if self.in_code_block {
            out.push_str("  ");
            if self.color_enabled {
                out.push_str(CODE_COLOR);
            }
            out.push_str(&sanitize(&visible));
            if self.color_enabled {
                out.push_str(RESET);
            }
            if append_newline {
                out.push('\n');
            }
            return;
        }

        out.push_str(&render_line(&visible, self.color_enabled, self.utf8));
        if append_newline {
            out.push('\n');
        }
    }

    fn strip_think(&mut self, line: &str) -> String {
        let mut out = String::new();
        let mut remaining = line;
        loop {
            if self.in_think_block {
                match remaining.find("</think>") {
                    Some(idx) => {
                        self.in_think_block = false;
                        remaining = &remaining[idx + "</think>".len()..];
                        continue;
                    }
                    None => return out,
                }
            }

            match remaining.find("<think>") {
                Some(idx) => {
                    out.push_str(&remaining[..idx]);
                    self.in_think_block = true;
                    remaining = &remaining[idx + "<think>".len()..];
                }
                None => {
                    out.push_str(remaining);
                    return out;
                }
            }
        }
    }
}

pub fn render_line(line: &str, color_enabled: bool, utf8: bool) -> String {
    if let Some(rest) = line.strip_prefix("### ") {
        return wrap_heading(rest, H3_COLOR, color_enabled);
    }
    if let Some(rest) = line.strip_prefix("## ") {
        return wrap_heading(rest, H2_COLOR, color_enabled);
    }
    if let Some(rest) = line.strip_prefix("# ") {
        return wrap_heading(rest, H1_COLOR, color_enabled);
    }
    if let Some(rest) = line.strip_prefix("- ") {
        let bullet = if utf8 { "* " } else { "- " };
        return format!("{bullet}{}", render_inline(rest, color_enabled));
    }

    render_inline(line, color_enabled)
}

pub fn sanitize(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        let cp = ch as u32;
        let is_c0 = cp < 0x20 && ch != '\t' && ch != '\n';
        let is_bidi = matches!(
            cp,
            0x202a..=0x202e | 0x2066..=0x2069 | 0x200e | 0x200f | 0x061c
        );
        if is_c0 || cp == 0x7f || (0x80..=0x9f).contains(&cp) || is_bidi {
            out.push('?');
        } else {
            out.push(ch);
        }
    }
    out
}

fn wrap_heading(body: &str, color: &str, color_enabled: bool) -> String {
    let rendered = render_inline(body, color_enabled);
    if color_enabled {
        format!("{color}{rendered}{RESET}")
    } else {
        rendered
    }
}

fn render_inline(text: &str, color_enabled: bool) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0usize;
    let mut literal_start = 0usize;

    while i < bytes.len() {
        if i + 1 < bytes.len()
            && bytes[i] == b'*'
            && bytes[i + 1] == b'*'
            && let Some(end_rel) = find_subslice(&bytes[i + 2..], b"**")
        {
            if literal_start < i {
                out.push_str(&sanitize(&text[literal_start..i]));
            }
            let inner = sanitize(&text[i + 2..i + 2 + end_rel]);
            if color_enabled {
                out.push_str(BOLD);
            }
            out.push_str(&inner);
            if color_enabled {
                out.push_str(RESET);
            }
            i += 2 + end_rel + 2;
            literal_start = i;
            continue;
        }

        if bytes[i] == b'`'
            && let Some(end_rel) = bytes[i + 1..].iter().position(|b| *b == b'`')
        {
            if literal_start < i {
                out.push_str(&sanitize(&text[literal_start..i]));
            }
            let inner = sanitize(&text[i + 1..i + 1 + end_rel]);
            if color_enabled {
                out.push_str(INLINE_CODE_COLOR);
            }
            out.push_str(&inner);
            if color_enabled {
                out.push_str(RESET);
            }
            i += 1 + end_rel + 1;
            literal_start = i;
            continue;
        }

        i = next_char_boundary(text, i);
    }

    if literal_start < bytes.len() {
        out.push_str(&sanitize(&text[literal_start..]));
    }
    out
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    (0..=haystack.len() - needle.len())
        .find(|start| &haystack[*start..*start + needle.len()] == needle)
}

fn next_char_boundary(input: &str, from: usize) -> usize {
    let mut end = from + 1;
    while end < input.len() && !input.is_char_boundary(end) {
        end += 1;
    }
    end
}

fn contained_think_only(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("<think>") && trimmed.ends_with("</think>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_headings_bold_inline_and_bullets() {
        assert_eq!(render_line("# Title", false, true), "Title");
        assert_eq!(
            render_line("use **bold** and `code`", false, true),
            "use bold and code"
        );
        assert_eq!(render_line("- item", false, true), "* item");
        assert_eq!(
            render_line("use **bold**", true, true),
            format!("use {BOLD}bold{RESET}")
        );
    }

    #[test]
    fn renders_code_fences_as_indented_lines() {
        let mut renderer = MarkdownRenderer::new(false, true);
        let mut out = String::new();
        out.push_str(&renderer.push_chunk("```rust\nfn main() {}\n```\n"));
        out.push_str(&renderer.flush());

        assert_eq!(out, "  fn main() {}\n");
    }

    #[test]
    fn strips_think_blocks_across_chunks() {
        let mut renderer = MarkdownRenderer::new(false, true);
        let mut out = String::new();
        out.push_str(&renderer.push_chunk("a<think>hidden"));
        out.push_str(&renderer.push_chunk("</think>b\n"));
        out.push_str(&renderer.flush());

        assert_eq!(out, "ab\n");
    }

    #[test]
    fn sanitizes_controls_and_bidi_marks() {
        assert_eq!(sanitize("a\x1bb\u{202e}c"), "a?b?c");
    }

    #[test]
    fn output_uses_sgr_only() {
        let sample = render_line("# **Title** and `code`", true, true);
        let bytes = sample.as_bytes();
        for idx in 0..bytes.len() {
            if bytes[idx] == 0x1b {
                assert_eq!(bytes.get(idx + 1), Some(&b'['));
                let mut end = idx + 2;
                while end < bytes.len() && bytes[end].is_ascii_digit()
                    || bytes.get(end) == Some(&b';')
                {
                    end += 1;
                }
                assert_eq!(bytes.get(end), Some(&b'm'));
            }
        }
    }

    #[test]
    fn renderer_does_not_treat_tool_xml_as_command() {
        let mut renderer = MarkdownRenderer::new(false, true);
        let xml = r#"<commandagent_tool_call>{"name":"Read","args":{"path":"Cargo.toml"}}</commandagent_tool_call>"#;

        assert_eq!(renderer.push_chunk(xml), "");
        assert_eq!(renderer.flush(), xml);
    }
}
