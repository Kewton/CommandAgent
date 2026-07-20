use std::io::{self, IsTerminal, Write};
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::tui::OutputRenderer;

mod highlight;
mod table;

pub mod capture {
    use super::*;

    static CAPTURE_SERIAL: OnceLock<Mutex<()>> = OnceLock::new();
    static CAPTURED: OnceLock<Mutex<Option<CaptureBuffer>>> = OnceLock::new();

    struct CaptureBuffer {
        owner: std::thread::ThreadId,
        output: String,
    }

    fn serial() -> &'static Mutex<()> {
        CAPTURE_SERIAL.get_or_init(|| Mutex::new(()))
    }

    fn captured() -> &'static Mutex<Option<CaptureBuffer>> {
        CAPTURED.get_or_init(|| Mutex::new(None))
    }

    pub struct Guard {
        _serial: MutexGuard<'static, ()>,
    }

    impl Guard {
        pub fn output(&self) -> String {
            captured()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .as_ref()
                .filter(|buffer| buffer.owner == std::thread::current().id())
                .map(|buffer| buffer.output.clone())
                .unwrap_or_default()
        }
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            *captured()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
        }
    }

    pub fn start() -> Guard {
        let serial = serial()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let buffer = CaptureBuffer {
            owner: std::thread::current().id(),
            output: String::new(),
        };
        *captured()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(buffer);
        Guard { _serial: serial }
    }

    pub fn is_active() -> bool {
        captured()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .is_some_and(|buffer| buffer.owner == std::thread::current().id())
    }

    pub(super) fn record(raw_text: &str) -> bool {
        let mut guard = captured()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(buffer) = guard
            .as_mut()
            .filter(|buffer| buffer.owner == std::thread::current().id())
        else {
            return false;
        };
        buffer.output.push_str(raw_text);
        if !raw_text.ends_with('\n') {
            buffer.output.push('\n');
        }
        true
    }

    pub(super) fn record_stream_chunk(raw_text: &str) -> bool {
        let mut guard = captured()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(buffer) = guard
            .as_mut()
            .filter(|buffer| buffer.owner == std::thread::current().id())
        else {
            return false;
        };
        buffer.output.push_str(raw_text);
        true
    }

    pub(super) fn finish_stream() {
        let mut guard = captured()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(buffer) = guard
            .as_mut()
            .filter(|buffer| buffer.owner == std::thread::current().id())
            && !buffer.output.is_empty()
            && !buffer.output.ends_with('\n')
        {
            buffer.output.push('\n');
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn capture_ignores_output_from_other_test_threads() {
            let capture = start();
            assert!(record("owner"));

            std::thread::spawn(|| {
                assert!(!is_active());
                assert!(!record("other"));
            })
            .join()
            .unwrap();

            assert_eq!(capture.output(), "owner\n");
        }
    }
}

const MD_CODE_FENCE_COLOR: &str = "\x1b[32m";
const MD_INLINE_CODE_COLOR: &str = "\x1b[36m";
const MD_H1_COLOR: &str = "\x1b[1m\x1b[35m";
const MD_H2_COLOR: &str = "\x1b[1m\x1b[33m";
const MD_H3_COLOR: &str = "\x1b[1m\x1b[34m";
const MD_BOLD: &str = "\x1b[1m";
const MD_RESET: &str = "\x1b[0m";

pub const MAX_BUFFERED_LINE_CHARS: usize = 64 * 1024;

pub struct MarkdownRenderer {
    line_buffer: String,
    in_code_block: bool,
    in_think_block: bool,
    code_language: Option<highlight::CodeLanguage>,
    highlight_state: highlight::HighlightState,
    pending_line: Option<PendingLine>,
    table: Option<table::Table>,
    list_state: ListState,
    color_enabled: bool,
    utf8: bool,
}

#[derive(Debug)]
struct PendingLine {
    text: String,
    append_newline: bool,
}

impl MarkdownRenderer {
    pub fn new(color_enabled: bool, utf8: bool) -> Self {
        Self {
            line_buffer: String::new(),
            in_code_block: false,
            in_think_block: false,
            code_language: None,
            highlight_state: highlight::HighlightState::default(),
            pending_line: None,
            table: None,
            list_state: ListState::default(),
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
        self.flush_deferred(&mut out);
        self.line_buffer.clear();
        self.in_code_block = false;
        self.in_think_block = false;
        self.code_language = None;
        self.highlight_state.reset();
        self.list_state.clear();
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
        self.flush_deferred(out);
        let visible = self.strip_think(&body);
        out.push_str(&sanitize(&visible));
        out.push('\n');
    }

    fn process_line(&mut self, line: &str, append_newline: bool, out: &mut String) {
        let visible = self.strip_think(line);
        if visible.is_empty() && (self.in_think_block || contained_think_only(line)) {
            return;
        }

        let is_fence = visible.trim_start().starts_with("```");
        if self.in_code_block && is_fence {
            self.in_code_block = false;
            self.code_language = None;
            self.highlight_state.reset();
            return;
        }
        if self.in_code_block {
            out.push_str("  ");
            let sanitized = sanitize(&visible);
            if self.color_enabled {
                out.push_str(MD_CODE_FENCE_COLOR);
                if let Some(language) = self.code_language {
                    out.push_str(&highlight::render(
                        &sanitized,
                        language,
                        &mut self.highlight_state,
                    ));
                } else {
                    out.push_str(&sanitized);
                }
                out.push_str(MD_RESET);
            } else {
                out.push_str(&sanitized);
            }
            if append_newline {
                out.push('\n');
            }
            return;
        }

        if is_fence {
            self.flush_deferred(out);
            self.in_code_block = true;
            self.code_language = highlight::CodeLanguage::from_fence(&visible);
            self.highlight_state.reset();
            self.list_state.clear();
            return;
        }

        if let Some(active_table) = self.table.as_mut() {
            match active_table.add_row(visible.clone(), append_newline, MAX_BUFFERED_LINE_CHARS) {
                table::AddRow::Added => return,
                table::AddRow::Invalid | table::AddRow::Overflow => {
                    let invalid = self.table.take().expect("active table exists");
                    out.push_str(&invalid.render_literal(self.color_enabled));
                    self.list_state.clear();
                    return;
                }
                table::AddRow::NotTableRow => {
                    let complete = self.table.take().expect("active table exists");
                    out.push_str(&complete.render(self.color_enabled, self.utf8));
                    self.list_state.clear();
                }
            }
        }

        if let Some(pending) = self.pending_line.take() {
            if table::is_delimiter_for(&pending.text, &visible) {
                if let Some(candidate) = table::Table::from_header_and_delimiter(
                    pending.text.clone(),
                    pending.append_newline,
                    visible.clone(),
                    append_newline,
                    MAX_BUFFERED_LINE_CHARS,
                ) {
                    self.table = Some(candidate);
                } else {
                    out.push_str(&render_inline(&pending.text, self.color_enabled));
                    if pending.append_newline {
                        out.push('\n');
                    }
                    out.push_str(&render_inline(&visible, self.color_enabled));
                    if append_newline {
                        out.push('\n');
                    }
                }
                self.list_state.clear();
                return;
            }
            self.render_normal_line(&pending.text, pending.append_newline, out);
        }

        if table::is_candidate_header(&visible) {
            self.pending_line = Some(PendingLine {
                text: visible,
                append_newline,
            });
        } else {
            self.render_normal_line(&visible, append_newline, out);
        }
    }

    fn render_normal_line(&mut self, line: &str, append_newline: bool, out: &mut String) {
        out.push_str(&render_line_with_state(
            line,
            self.color_enabled,
            self.utf8,
            &mut self.list_state,
        ));
        if append_newline {
            out.push('\n');
        }
    }

    fn flush_deferred(&mut self, out: &mut String) {
        if let Some(table) = self.table.take() {
            out.push_str(&table.render(self.color_enabled, self.utf8));
            self.list_state.clear();
        }
        if let Some(pending) = self.pending_line.take() {
            self.render_normal_line(&pending.text, pending.append_newline, out);
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

fn contained_think_only(line: &str) -> bool {
    let stripped = line.trim();
    stripped.starts_with("<think>") && stripped.ends_with("</think>")
}

pub fn render_line(line: &str, color_enabled: bool, utf8: bool) -> String {
    render_line_with_state(line, color_enabled, utf8, &mut ListState::default())
}

fn render_line_with_state(
    line: &str,
    color_enabled: bool,
    utf8: bool,
    list_state: &mut ListState,
) -> String {
    if let Some(rest) = line.strip_prefix("### ") {
        list_state.clear();
        return wrap_heading(rest, MD_H3_COLOR, color_enabled);
    }
    if let Some(rest) = line.strip_prefix("## ") {
        list_state.clear();
        return wrap_heading(rest, MD_H2_COLOR, color_enabled);
    }
    if let Some(rest) = line.strip_prefix("# ") {
        list_state.clear();
        return wrap_heading(rest, MD_H1_COLOR, color_enabled);
    }
    if let Some(item) = parse_list_item(line) {
        let depth = list_state.depth_for(item.indent);
        let indentation = "  ".repeat(depth);
        let marker = match item.marker {
            ListMarker::Unordered => unordered_marker(depth, utf8),
            ListMarker::Ordered(number) => format!("{number}. "),
        };
        return format!(
            "{indentation}{marker}{}",
            render_inline(item.body, color_enabled)
        );
    }
    list_state.clear();
    render_inline(line, color_enabled)
}

#[derive(Debug, Default)]
struct ListState {
    indents: Vec<usize>,
}

impl ListState {
    fn depth_for(&mut self, indent: usize) -> usize {
        if self.indents.is_empty() {
            if indent > 0 {
                self.indents.push(0);
            }
            self.indents.push(indent);
            return self.indents.len() - 1;
        }
        while self.indents.last().is_some_and(|current| *current > indent) {
            self.indents.pop();
        }
        if self.indents.last().is_none_or(|current| *current < indent) {
            self.indents.push(indent);
        }
        self.indents.len().saturating_sub(1)
    }

    fn clear(&mut self) {
        self.indents.clear();
    }
}

#[derive(Debug, Clone, Copy)]
enum ListMarker<'a> {
    Unordered,
    Ordered(&'a str),
}

#[derive(Debug, Clone, Copy)]
struct ListItem<'a> {
    indent: usize,
    marker: ListMarker<'a>,
    body: &'a str,
}

fn parse_list_item(line: &str) -> Option<ListItem<'_>> {
    let indent = line.bytes().take_while(|byte| *byte == b' ').count();
    let rest = &line[indent..];
    if let Some(body) = rest
        .strip_prefix("- ")
        .or_else(|| rest.strip_prefix("* "))
        .or_else(|| rest.strip_prefix("+ "))
    {
        return Some(ListItem {
            indent,
            marker: ListMarker::Unordered,
            body,
        });
    }

    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || !rest[digits..].starts_with(". ") {
        return None;
    }
    Some(ListItem {
        indent,
        marker: ListMarker::Ordered(&rest[..digits]),
        body: &rest[digits + 2..],
    })
}

fn unordered_marker(depth: usize, utf8: bool) -> String {
    if utf8 {
        ["● ", "○ ", "▪ ", "▫ "][depth % 4].to_string()
    } else {
        ["* ", "- ", "+ "][depth % 3].to_string()
    }
}

fn wrap_heading(body: &str, color: &str, color_enabled: bool) -> String {
    let inline = render_inline(body, color_enabled);
    if color_enabled {
        format!("{color}{inline}{MD_RESET}")
    } else {
        inline
    }
}

fn render_inline(text: &str, color_enabled: bool) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut literal_start = 0;
    while i < bytes.len() {
        if bytes[i] == b'['
            && let Some(label_end_rel) = find_byte(&bytes[i + 1..], b']')
        {
            let label_end = i + 1 + label_end_rel;
            if bytes.get(label_end + 1) == Some(&b'(')
                && let Some(url_end_rel) = find_byte(&bytes[label_end + 2..], b')')
            {
                if literal_start < i {
                    out.push_str(&sanitize(&text[literal_start..i]));
                }
                out.push_str(&render_inline(&text[i + 1..label_end], color_enabled));
                out.push_str(" (");
                out.push_str(&sanitize(&text[label_end + 2..label_end + 2 + url_end_rel]));
                out.push(')');
                i = label_end + 2 + url_end_rel + 1;
                literal_start = i;
                continue;
            }
        }
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
                out.push_str(MD_BOLD);
                out.push_str(&inner);
                out.push_str(MD_RESET);
            } else {
                out.push_str(&inner);
            }
            i += 2 + end_rel + 2;
            literal_start = i;
            continue;
        }
        if bytes[i] == b'`'
            && let Some(end_rel) = find_byte(&bytes[i + 1..], b'`')
        {
            if literal_start < i {
                out.push_str(&sanitize(&text[literal_start..i]));
            }
            let inner = sanitize(&text[i + 1..i + 1 + end_rel]);
            if color_enabled {
                out.push_str(MD_INLINE_CODE_COLOR);
                out.push_str(&inner);
                out.push_str(MD_RESET);
            } else {
                out.push_str(&inner);
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

fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }
    (0..=hay.len() - needle.len()).find(|&start| &hay[start..start + needle.len()] == needle)
}

fn find_byte(hay: &[u8], needle: u8) -> Option<usize> {
    hay.iter().position(|&b| b == needle)
}

fn next_char_boundary(s: &str, from: usize) -> usize {
    let bytes = s.as_bytes();
    let mut end = from + 1;
    while end < bytes.len() && !s.is_char_boundary(end) {
        end += 1;
    }
    end
}

pub fn sanitize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        let cp = ch as u32;
        let is_c0 = cp < 0x20 && ch != '\t' && ch != '\n';
        let is_del = cp == 0x7F;
        let is_c1 = (0x80..=0x9F).contains(&cp);
        let is_bidi = matches!(
            cp,
            0x202A..=0x202E | 0x2066..=0x2069 | 0x200E | 0x200F | 0x061C
        );
        if is_c0 || is_del || is_c1 || is_bidi {
            out.push('?');
        } else {
            out.push(ch);
        }
    }
    out
}

pub struct TerminalMarkdownRenderer {
    color_enabled: bool,
    utf8: bool,
    markdown_enabled: bool,
}

impl TerminalMarkdownRenderer {
    pub fn new(color_enabled: bool, utf8: bool) -> Self {
        Self {
            color_enabled,
            utf8,
            markdown_enabled: true,
        }
    }

    pub fn for_stdout() -> Self {
        let disabled = crate::tui::terminal::env_non_empty("COMMANDAGENT_NO_MARKDOWN");
        let color_enabled =
            !disabled && !crate::tui::terminal::no_color() && io::stdout().is_terminal();
        let utf8 = crate::tui::terminal::utf8_locale();
        Self {
            color_enabled,
            utf8,
            markdown_enabled: !disabled,
        }
    }

    pub fn render_to_string(&self, raw_text: &str) -> String {
        if !self.markdown_enabled || crate::tui::terminal::env_non_empty("COMMANDAGENT_NO_MARKDOWN")
        {
            return raw_text.to_string();
        }
        let mut renderer = MarkdownRenderer::new(self.color_enabled, self.utf8);
        let mut out = renderer.push_chunk(raw_text);
        out.push_str(&renderer.flush());
        out
    }

    pub fn render_chunks_to_string<'a>(&self, chunks: impl IntoIterator<Item = &'a str>) -> String {
        if !self.markdown_enabled || crate::tui::terminal::env_non_empty("COMMANDAGENT_NO_MARKDOWN")
        {
            return chunks.into_iter().collect();
        }
        let mut renderer = MarkdownRenderer::new(self.color_enabled, self.utf8);
        let mut out = String::new();
        for chunk in chunks {
            out.push_str(&renderer.push_chunk(chunk));
        }
        out.push_str(&renderer.flush());
        out
    }

    pub fn begin_stream(&self) -> TerminalMarkdownStream {
        TerminalMarkdownStream {
            renderer: self
                .markdown_enabled
                .then(|| MarkdownRenderer::new(self.color_enabled, self.utf8)),
            raw: !self.markdown_enabled,
            wrote_output: false,
            output_ends_with_newline: false,
            captured: false,
        }
    }
}

pub struct TerminalMarkdownStream {
    renderer: Option<MarkdownRenderer>,
    raw: bool,
    wrote_output: bool,
    output_ends_with_newline: bool,
    captured: bool,
}

impl TerminalMarkdownStream {
    pub fn push_chunk(&mut self, raw_text: &str) -> anyhow::Result<()> {
        if raw_text.is_empty() {
            return Ok(());
        }
        if capture::record_stream_chunk(raw_text) {
            self.captured = true;
            return Ok(());
        }
        if self.raw {
            let mut stdout = io::stdout().lock();
            stdout.write_all(raw_text.as_bytes())?;
            stdout.flush()?;
            self.wrote_output = true;
            self.output_ends_with_newline = raw_text.ends_with('\n');
            return Ok(());
        }
        let Some(renderer) = self.renderer.as_mut() else {
            return Ok(());
        };
        let rendered = renderer.push_chunk(raw_text);
        if rendered.is_empty() {
            return Ok(());
        }
        let mut stdout = io::stdout().lock();
        stdout.write_all(rendered.as_bytes())?;
        stdout.flush()?;
        self.wrote_output = true;
        self.output_ends_with_newline = rendered.ends_with('\n');
        Ok(())
    }

    pub fn finish(&mut self) -> anyhow::Result<()> {
        if self.captured {
            capture::finish_stream();
            return Ok(());
        }
        let rendered = self
            .renderer
            .take()
            .map(|mut renderer| renderer.flush())
            .unwrap_or_default();
        if rendered.is_empty() && !self.wrote_output {
            return Ok(());
        }
        let mut stdout = io::stdout().lock();
        if !rendered.is_empty() {
            stdout.write_all(rendered.as_bytes())?;
            self.output_ends_with_newline = rendered.ends_with('\n');
        }
        if !self.output_ends_with_newline {
            stdout.write_all(b"\n")?;
            self.output_ends_with_newline = true;
        }
        stdout.flush()?;
        self.wrote_output = true;
        Ok(())
    }
}

impl Drop for TerminalMarkdownStream {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

impl OutputRenderer for TerminalMarkdownRenderer {
    fn render_assistant(&self, raw_text: &str) -> anyhow::Result<()> {
        if capture::record(raw_text) {
            return Ok(());
        }
        let rendered = self.render_to_string(raw_text);
        let mut stdout = io::stdout().lock();
        stdout.write_all(rendered.as_bytes())?;
        if !rendered.ends_with('\n') {
            stdout.write_all(b"\n")?;
        }
        stdout.flush()?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PlainRenderer;

impl OutputRenderer for PlainRenderer {
    fn render_assistant(&self, raw_text: &str) -> anyhow::Result<()> {
        if capture::record(raw_text) {
            return Ok(());
        }
        println!("{raw_text}");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    fn render_all(raw: &str, color_enabled: bool, utf8: bool) -> String {
        let mut renderer = MarkdownRenderer::new(color_enabled, utf8);
        let mut out = renderer.push_chunk(raw);
        out.push_str(&renderer.flush());
        out
    }

    #[test]
    fn markdown_headings_and_inline_sgr() {
        let got = render_line("# hello **world** `code`", true, true);
        assert!(got.starts_with(MD_H1_COLOR));
        assert!(got.contains(MD_BOLD));
        assert!(got.contains(MD_INLINE_CODE_COLOR));
        assert!(!got.contains("**"));
        assert!(!got.contains('`'));
    }

    #[test]
    fn markdown_plain_strips_symbols() {
        assert_eq!(
            render_line("# hello **world** `code`", false, true),
            "hello world code"
        );
    }

    #[test]
    fn markdown_output_has_only_sgr_csi() {
        let samples = [
            render_line("# h1", true, true),
            render_line("## h2", true, true),
            render_line("### h3", true, true),
            render_line("- item", true, true),
            render_line("use **bold** and `code`", true, true),
        ];
        let csi_re = Regex::new(r"\x1b\[[0-9;]*([A-Za-z])").unwrap();
        for sample in samples {
            for cap in csi_re.captures_iter(&sample) {
                assert_eq!(&cap[1], "m", "non-SGR CSI detected in {sample:?}");
            }
        }
    }

    #[test]
    fn markdown_think_cross_chunk_is_hidden() {
        let mut r = MarkdownRenderer::new(true, true);
        let mut out = r.push_chunk("before <think>s");
        out.push_str(&r.push_chunk("ecret</think>after\n"));
        assert_eq!(out, "before after\n");
    }

    #[test]
    fn terminal_stream_matches_batch_for_arbitrary_chunks() {
        let raw = concat!(
            "<think>hidden across chunks</think># Heading\n\n",
            "| Name | Value |\n| --- | ---: |\n| 日本 | **2** |\n\n",
            "- parent\n  - child with `code`\n"
        );
        let renderer = TerminalMarkdownRenderer::new(false, true);
        let mut chunks = Vec::new();
        let mut start = 0;
        for (characters, (index, _)) in raw.char_indices().enumerate() {
            if characters > 0 && characters % 3 == 0 {
                chunks.push(&raw[start..index]);
                start = index;
            }
        }
        chunks.push(&raw[start..]);
        assert_eq!(
            renderer.render_chunks_to_string(chunks.iter().copied()),
            renderer.render_to_string(raw)
        );
    }

    #[test]
    fn markdown_buffer_cap_forces_bounded_memory() {
        let mut r = MarkdownRenderer::new(true, true);
        let big = "a".repeat(MAX_BUFFERED_LINE_CHARS + 100);
        let mut out = r.push_chunk(&big);
        out.push_str(&r.flush());
        assert!(r.line_buffer.is_empty());
        assert_eq!(
            out.chars().filter(|&ch| ch == 'a').count(),
            MAX_BUFFERED_LINE_CHARS + 100
        );
    }

    #[test]
    fn markdown_sanitize_strips_bidi_and_controls() {
        assert_eq!(sanitize("x\x1b[31m"), "x?[31m");
        assert_eq!(sanitize("bidi\u{202E}x"), "bidi?x");
        assert_eq!(sanitize("caf\u{00E9}"), "caf\u{00E9}");
    }

    #[test]
    fn markdown_table_aligns_cjk_by_display_width() {
        let got = render_all(
            "| name | value | note |\n| :--- | :---: | ---: |\n| 日本 | x | yz |\n",
            false,
            true,
        );
        assert_eq!(
            got,
            concat!(
                "┌──────┬───────┬──────┐\n",
                "│ name │ value │ note │\n",
                "├──────┼───────┼──────┤\n",
                "│ 日本 │   x   │   yz │\n",
                "└──────┴───────┴──────┘\n",
            )
        );
    }

    #[test]
    fn markdown_table_applies_left_center_and_right_alignment() {
        let got = render_all(
            "| a | b | c |\n| :--- | :---: | ---: |\n| xxx | yyy | zzz |\n",
            false,
            false,
        );
        assert_eq!(
            got,
            concat!(
                "+-----+-----+-----+\n",
                "| a   |  b  |   c |\n",
                "+-----+-----+-----+\n",
                "| xxx | yyy | zzz |\n",
                "+-----+-----+-----+\n",
            )
        );
    }

    #[test]
    fn markdown_invalid_table_is_emitted_literally() {
        let raw = "| a | b |\n| --- | --- |\n| only one |\nafter\n";
        assert_eq!(render_all(raw, false, true), raw);
    }

    #[test]
    fn markdown_invalid_delimiter_is_emitted_literally() {
        let raw = "| a | b |\n| --- |\n";
        assert_eq!(render_all(raw, false, true), raw);
    }

    #[test]
    fn markdown_oversized_table_falls_back_without_growing_table_buffer() {
        let large_cell = "x".repeat(MAX_BUFFERED_LINE_CHARS - 20);
        let header = "| a | b |\n";
        let delimiter = "| --- | --- |\n";
        let row = format!("| {large_cell} | y |\n");
        let raw = format!("{header}{delimiter}{row}");
        let mut renderer = MarkdownRenderer::new(false, true);
        let mut got = renderer.push_chunk(header);
        got.push_str(&renderer.push_chunk(delimiter));
        got.push_str(&renderer.push_chunk(&row));
        got.push_str(&renderer.flush());
        assert_eq!(got, raw);
        assert!(!got.contains('┌'));
    }

    #[test]
    fn markdown_wide_styled_table_keeps_generated_sgr_intact() {
        let wide = "x".repeat(240);
        let raw = format!("| key | value |\n| --- | --- |\n| **bold** | {wide} |\n");
        let got = render_all(&raw, true, true);
        assert!(got.contains(&wide));
        assert!(got.contains(MD_BOLD));
        assert!(got.len() > 240);

        let sgr = Regex::new(r"\x1b\[[0-9;]*m").unwrap();
        assert_eq!(got.matches('\x1b').count(), sgr.find_iter(&got).count());
    }

    #[test]
    fn markdown_tables_render_across_arbitrary_chunks() {
        let raw = "| a | b |\n| --- | ---: |\n| 日本 | 2 |\n";
        let expected = render_all(raw, false, true);
        let mut renderer = MarkdownRenderer::new(false, true);
        let mut got = String::new();
        for chunk in ["| a", " | b |\n| --", "- | ---: |\n| 日", "本 | 2 |\n"] {
            got.push_str(&renderer.push_chunk(chunk));
        }
        got.push_str(&renderer.flush());
        assert_eq!(got, expected);
    }

    #[test]
    fn markdown_nested_lists_accept_two_or_four_space_steps() {
        let two_space = render_all(
            "- root\n  - child\n    - grandchild\n- next\n    1. ordered\n",
            false,
            true,
        );
        assert_eq!(
            two_space,
            "● root\n  ○ child\n    ▪ grandchild\n● next\n  1. ordered\n"
        );

        let four_space = render_all("- root\n    - child\n        - grandchild\n", false, true);
        assert_eq!(four_space, "● root\n  ○ child\n    ▪ grandchild\n");
    }

    #[test]
    fn markdown_ordered_lists_preserve_numbers() {
        assert_eq!(
            render_all("1. first\n2. second\n  10. nested\n", false, false),
            "1. first\n2. second\n  10. nested\n"
        );
    }

    #[test]
    fn markdown_links_use_portable_text_and_url_form() {
        assert_eq!(
            render_line("See [**docs**](https://example.test/path)", false, true),
            "See docs (https://example.test/path)"
        );
        assert_eq!(
            render_line("[bad\x1b[31m](https://x/\x1b[2J)", false, true),
            "bad?[31m (https://x/?[2J)"
        );
    }

    #[test]
    fn markdown_known_code_fence_highlights_keyword_string_and_comment() {
        let got = render_all("```rust\nlet value = \"safe\"; // note\n```\n", true, true);
        assert!(got.contains("\x1b[35mlet"), "{got:?}");
        assert!(got.contains("\x1b[33m\"safe\""), "{got:?}");
        assert!(got.contains("\x1b[2m\x1b[34m// note"), "{got:?}");
    }

    #[test]
    fn markdown_unknown_or_plain_code_fence_keeps_single_color() {
        let unknown = render_all("```wat\nlet x = \"y\"; // note\n```\n", true, true);
        assert_eq!(unknown, "  \x1b[32mlet x = \"y\"; // note\x1b[0m\n");
        let untagged = render_all("```\nplain\n```\n", true, true);
        assert_eq!(untagged, "  \x1b[32mplain\x1b[0m\n");
    }

    #[test]
    fn markdown_code_highlighting_sanitizes_before_adding_sgr() {
        let got = render_all("```python\ndef x(): # \x1b[31m\n```\n", true, true);
        assert!(got.contains("# ?[31m"));
        assert!(!got.contains("# \x1b[31m"));

        let sgr = Regex::new(r"\x1b\[[0-9;]*m").unwrap();
        assert_eq!(got.matches('\x1b').count(), sgr.find_iter(&got).count());
    }

    #[test]
    fn markdown_no_color_keeps_known_code_fence_plain() {
        assert_eq!(
            render_all("```python\ndef x(): # note\n```\n", false, true),
            "  def x(): # note\n"
        );
    }
}
