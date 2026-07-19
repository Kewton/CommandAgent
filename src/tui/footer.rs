use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::config::Config;
use crate::tui::input_queue::{InputQueue, InputSnapshot};
use crate::tui::status::UiStatus;
use crate::tui::status_bus::{
    self, GlobalStatusBusGuard, RuntimeStatus, StatusPublisher, StatusSubscriber, StatusTime,
};

const TICK: Duration = Duration::from_millis(250);
const INPUT_TICK: Duration = Duration::from_millis(50);
const NARROW_FOOTER_COLS: u16 = 100;

pub mod ansi {
    pub fn build_decstbm(rows: u16, footer_rows: u16) -> Option<String> {
        if footer_rows == 0 || rows <= footer_rows {
            None
        } else {
            Some(format!("\x1b[1;{}r", rows - footer_rows))
        }
    }

    pub fn reset_decstbm() -> &'static str {
        "\x1b[r"
    }

    pub fn clear_line() -> &'static str {
        "\r\x1b[2K"
    }

    pub fn save_cursor() -> &'static str {
        "\x1b[s"
    }

    pub fn restore_cursor() -> &'static str {
        "\x1b[u"
    }

    pub fn move_to(row: u16, col: u16) -> String {
        format!("\x1b[{row};{col}H")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FooterEnv {
    pub enabled: bool,
    pub use_color: bool,
}

impl FooterEnv {
    pub fn detect(config: &Config) -> Self {
        Self::detect_with(
            |key| std::env::var_os(key).map(|value| value.to_string_lossy().into_owned()),
            crate::tui::terminal::stdout_is_tty(),
            config.no_footer,
        )
    }

    pub fn detect_with(
        get_env: impl Fn(&str) -> Option<String>,
        stdout_is_tty: bool,
        no_footer: bool,
    ) -> Self {
        if no_footer || crate::tui::terminal::env_non_empty_with(&get_env, "ANVIL_NO_FOOTER") {
            return Self {
                enabled: false,
                use_color: false,
            };
        }
        let no_color = crate::tui::terminal::env_non_empty_with(get_env, "NO_COLOR");
        Self {
            enabled: stdout_is_tty,
            use_color: !no_color,
        }
    }
}

#[derive(Debug)]
struct FooterState {
    status: Mutex<UiStatus>,
    freeze: AtomicBool,
    stop: AtomicBool,
    wake: (Mutex<()>, Condvar),
}

pub struct Footer {
    inner: Option<Active>,
}

struct Active {
    state: Arc<FooterState>,
    _publisher: StatusPublisher,
    _global_guard: GlobalStatusBusGuard,
    handle: Option<JoinHandle<FooterGeometry>>,
    initial_geometry: FooterGeometry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FooterGeometry {
    cols: u16,
    rows: u16,
    footer_rows: u16,
}

impl FooterGeometry {
    fn new(cols: u16, rows: u16) -> Self {
        Self::with_input_row(cols, rows, false)
    }

    fn with_input_row(cols: u16, rows: u16, input_enabled: bool) -> Self {
        Self {
            cols,
            rows,
            footer_rows: footer_rows_for_cols(cols) + u16::from(input_enabled),
        }
    }

    fn decstbm(self) -> Option<String> {
        ansi::build_decstbm(self.rows, self.footer_rows)
    }

    fn is_renderable(self) -> bool {
        self.footer_rows > 0 && self.rows > self.footer_rows
    }
}

impl Footer {
    pub fn start(config: &Config) -> Self {
        Self::start_inner(config, None)
    }

    pub fn start_with_input_queue(config: &Config, input_queue: InputQueue) -> Self {
        Self::start_inner(config, Some(input_queue))
    }

    fn start_inner(config: &Config, input_queue: Option<InputQueue>) -> Self {
        let env = FooterEnv::detect(config);
        if !env.enabled {
            return Self { inner: None };
        }
        let Ok((cols, rows)) = crossterm::terminal::size() else {
            return Self { inner: None };
        };
        let geometry = FooterGeometry::new(cols, rows);
        let Some(decstbm) = geometry.decstbm() else {
            return Self { inner: None };
        };
        {
            let mut stdout = io::stdout().lock();
            if stdout.write_all(decstbm.as_bytes()).is_err() {
                return Self { inner: None };
            }
            let _ = stdout.write_all(b"\x1b[H");
            let _ = stdout.flush();
        }
        let state = Arc::new(FooterState {
            status: Mutex::new(UiStatus::from_config(config)),
            freeze: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            wake: (Mutex::new(()), Condvar::new()),
        });
        let (publisher, subscriber) = status_bus::channel();
        let global_guard = status_bus::install_global(publisher.clone());
        let thread_state = state.clone();
        let handle = thread::Builder::new()
            .name("commandagent-footer".to_string())
            .spawn(move || render_loop(thread_state, subscriber, geometry, env, input_queue))
            .ok();
        match handle {
            Some(handle) => Self {
                inner: Some(Active {
                    state,
                    _publisher: publisher,
                    _global_guard: global_guard,
                    handle: Some(handle),
                    initial_geometry: geometry,
                }),
            },
            None => {
                let _ = io::stdout().write_all(ansi::reset_decstbm().as_bytes());
                Self { inner: None }
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.inner.is_some()
    }

    pub fn publish(&self, status: UiStatus) {
        let Some(active) = &self.inner else {
            return;
        };
        if let Ok(mut current) = active.state.status.lock() {
            if status.prompt_tokens.is_some() || status.completion_tokens.is_some() {
                current.prompt_tokens = add_known(current.prompt_tokens, status.prompt_tokens);
                current.completion_tokens =
                    add_known(current.completion_tokens, status.completion_tokens);
            }
            current.mode = status.mode;
            current.provider = status.provider;
            current.model = status.model;
            current.context_budget = status.context_budget;
            current.yes = status.yes;
        }
        active.state.wake.1.notify_all();
    }

    pub fn freeze(&self) -> Option<FreezeGuard> {
        let state = self.inner.as_ref()?.state.clone();
        state.freeze.store(true, Ordering::SeqCst);
        state.wake.1.notify_all();
        Some(FreezeGuard { state })
    }
}

impl Drop for Footer {
    fn drop(&mut self) {
        let Some(mut active) = self.inner.take() else {
            return;
        };
        active.state.stop.store(true, Ordering::SeqCst);
        active.state.wake.1.notify_all();
        let mut geometry = active.initial_geometry;
        if let Some(handle) = active.handle.take()
            && let Ok(current_geometry) = handle.join()
        {
            geometry = current_geometry;
        }
        let current_geometry = geometry_after_size(geometry, crossterm::terminal::size());
        let mut stdout = io::stdout().lock();
        let _ = stdout
            .write_all(clear_footer_transition_sequence(geometry, current_geometry).as_bytes());
        let _ = stdout.flush();
    }
}

pub struct FreezeGuard {
    state: Arc<FooterState>,
}

impl Drop for FreezeGuard {
    fn drop(&mut self) {
        self.state.freeze.store(false, Ordering::SeqCst);
        self.state.wake.1.notify_all();
    }
}

fn add_known(current: Option<u64>, delta: Option<u64>) -> Option<u64> {
    match (current, delta) {
        (None, None) => None,
        (current, delta) => Some(current.unwrap_or(0) + delta.unwrap_or(0)),
    }
}

fn format_secs(secs: u64) -> String {
    let minutes = secs / 60;
    let seconds = secs % 60;
    if minutes == 0 {
        format!("{seconds}s")
    } else {
        format!("{minutes}m{seconds:02}s")
    }
}

pub fn build_footer_line(status: &UiStatus, use_color: bool) -> String {
    let tokens = match status.token_total() {
        Some(value) => format_token_count(value),
        None => "n/a".to_string(),
    };
    let yes = if status.yes { " [yes]" } else { "" };
    let body = format!(
        "[{}] provider:{} model:{} ctx:{} tokens:{}{}",
        status.mode, status.provider, status.model, status.context_budget, tokens, yes
    );
    if use_color {
        format!("\x1b[2m{body}\x1b[0m")
    } else {
        body
    }
}

pub fn build_live_footer_lines(
    status: &UiStatus,
    runtime: &RuntimeStatus,
    now: StatusTime,
    cols: u16,
    use_color: bool,
) -> Vec<String> {
    if runtime.phase.is_none()
        && runtime.step.is_none()
        && runtime.provider.is_none()
        && runtime.command.is_none()
        && runtime.repair.is_none()
        && runtime.stage.is_none()
        && !runtime.interrupt_requested
        && !runtime.force_finalize_requested
    {
        let line = fit_to_width(&build_footer_line(status, false), cols);
        return vec![if use_color {
            format!("\x1b[2m{line}\x1b[0m")
        } else {
            line
        }];
    }

    let mut primary = Vec::new();
    if let Some(phase) = &runtime.phase {
        let progress = if phase.total == 0 {
            format!("Phase {}", phase.index)
        } else {
            format!("Phase {}/{}", phase.index, phase.total)
        };
        if phase.id.is_empty() {
            primary.push(progress);
        } else {
            primary.push(format!("{progress} {}", phase.id));
        }
    }
    if let Some(step) = &runtime.step {
        let label = if step.kind.is_empty() {
            step.id.clone()
        } else {
            step.kind.clone()
        };
        if !label.is_empty() {
            primary.push(label);
        }
    }

    let mut secondary = Vec::new();
    if let Some(command) = &runtime.command {
        let elapsed = now.elapsed_secs_since(command.started_at);
        secondary.push(format!(
            "cmd {} {}s/{}s",
            command.excerpt, elapsed, command.cap_secs
        ));
    } else if let Some(provider) = &runtime.provider {
        let elapsed = now.elapsed_secs_since(provider.started_at);
        secondary.push(format!(
            "{} {}s/{}s",
            status_bus::provider_scope_label(&provider.scope),
            elapsed,
            provider.deadline_secs
        ));
    } else if let Some(stage) = &runtime.stage {
        secondary.push(stage.clone());
    } else {
        secondary.push(format!(
            "provider:{} model:{}",
            status.provider, status.model
        ));
    }
    if let Some(repair) = &runtime.repair {
        if repair.max == 0 {
            secondary.push(format!("repair {}", repair.attempt));
        } else {
            secondary.push(format!("repair {}/{}", repair.attempt, repair.max));
        }
    }
    if runtime.time_totals.total_secs() > 0 {
        secondary.push(format!(
            "time total:{} provider:{} cmd:{}",
            format_secs(runtime.time_totals.total_secs()),
            format_secs(runtime.time_totals.provider_secs),
            format_secs(runtime.time_totals.command_secs),
        ));
    }
    let prefix = if runtime.force_finalize_requested {
        "[stopping: force-finalizing…] "
    } else if runtime.interrupt_requested {
        "[stopping: aborting current operation…] "
    } else {
        ""
    };
    let joined_primary = primary.join(" · ");
    let joined_secondary = secondary.join(" · ");
    let raw_lines = if cols < NARROW_FOOTER_COLS && !joined_primary.is_empty() {
        vec![format!("{prefix}{joined_primary}"), joined_secondary]
    } else {
        let body = [joined_primary, joined_secondary]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
        vec![format!("{prefix}{body}")]
    };
    raw_lines
        .into_iter()
        .map(|line| {
            let line = fit_to_width(&line, cols);
            if use_color {
                format!("\x1b[2m{line}\x1b[0m")
            } else {
                line
            }
        })
        .collect()
}

pub fn build_input_queue_line(
    snapshot: &InputSnapshot,
    cols: u16,
    use_color: bool,
) -> Option<String> {
    if !snapshot.enabled {
        return None;
    }
    let queue_count = format!("{} queued", snapshot.queued_count);
    let raw = if !snapshot.pending.is_empty() {
        let pending = format!("pending> {}", snapshot.pending);
        if snapshot
            .notice
            .as_deref()
            .is_some_and(|notice| notice.starts_with("input rejected:"))
        {
            format!(
                "{} · {pending} · {queue_count} · Esc clears · Ctrl-C interrupts",
                snapshot.notice.as_deref().unwrap_or_default()
            )
        } else {
            format!("{pending} · {queue_count} · Esc clears · Ctrl-C interrupts")
        }
    } else if let Some(notice) = &snapshot.notice {
        format!("{notice} · {queue_count} · Esc interrupts · Ctrl-C interrupts")
    } else {
        format!("type ahead · {queue_count} · Esc interrupts · Ctrl-C interrupts")
    };
    let line = fit_to_width(&raw, cols);
    Some(if use_color {
        format!("\x1b[36m{line}\x1b[0m")
    } else {
        line
    })
}

fn format_token_count(value: u64) -> String {
    if value < 1_000 {
        value.to_string()
    } else if value.div_euclid(1_000) * 1_000 == value {
        format!("{}k", value / 1_000)
    } else {
        format!("{:.1}k", value as f64 / 1_000.0)
    }
}

fn render_loop(
    state: Arc<FooterState>,
    subscriber: StatusSubscriber,
    mut geometry: FooterGeometry,
    env: FooterEnv,
    input_queue: Option<InputQueue>,
) -> FooterGeometry {
    let mut generation = 0;
    let mut last_rendered: Option<Vec<String>> = None;
    loop {
        if state.stop.load(Ordering::SeqCst) {
            break;
        }
        let input_snapshot = input_queue.as_ref().map(InputQueue::snapshot);
        let input_enabled = input_snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.enabled);
        let previous_geometry = geometry;
        geometry =
            geometry_after_size_with_input(geometry, crossterm::terminal::size(), input_enabled);
        let resized = geometry != previous_geometry;
        if resized || !state.freeze.load(Ordering::SeqCst) {
            let status = match state.status.lock() {
                Ok(status) => status.clone(),
                Err(poisoned) => poisoned.into_inner().clone(),
            };
            let runtime = subscriber.snapshot();
            generation = runtime.generation;
            let mut lines = build_live_footer_lines(
                &status,
                &runtime,
                status_bus::now(),
                geometry.cols,
                env.use_color,
            );
            if let Some(input_line) = input_snapshot
                .as_ref()
                .and_then(|snapshot| build_input_queue_line(snapshot, geometry.cols, env.use_color))
            {
                lines.insert(0, input_line);
            }
            let frame = if resized {
                let frame = resize_footer_region_sequence(previous_geometry, geometry, &lines);
                last_rendered = geometry.is_renderable().then(|| lines.clone());
                Some(frame)
            } else if geometry.is_renderable() && footer_lines_changed(&mut last_rendered, &lines) {
                Some(render_footer_frame_sequence(
                    &lines,
                    geometry.cols,
                    geometry.rows,
                    geometry.footer_rows,
                ))
            } else {
                None
            };
            if let Some(frame) = frame {
                let mut stdout = io::stdout().lock();
                let _ = stdout.write_all(frame.as_bytes());
                let _ = stdout.flush();
            }
        }
        let tick = if input_enabled { INPUT_TICK } else { TICK };
        let _ = subscriber.wait_for_change(generation, tick);
        if let Ok(guard) = state.wake.0.lock() {
            let _ = state.wake.1.wait_timeout(guard, Duration::from_millis(1));
        }
    }
    geometry
}

fn geometry_after_size(current: FooterGeometry, size: io::Result<(u16, u16)>) -> FooterGeometry {
    size.map_or(current, |(cols, rows)| FooterGeometry::new(cols, rows))
}

fn geometry_after_size_with_input(
    current: FooterGeometry,
    size: io::Result<(u16, u16)>,
    input_enabled: bool,
) -> FooterGeometry {
    let (cols, rows) = size.unwrap_or((current.cols, current.rows));
    FooterGeometry::with_input_row(cols, rows, input_enabled)
}

fn footer_lines_changed(last_rendered: &mut Option<Vec<String>>, lines: &[String]) -> bool {
    if last_rendered.as_deref() == Some(lines) {
        return false;
    }
    *last_rendered = Some(lines.to_vec());
    true
}

fn render_footer_frame_sequence(
    lines: &[String],
    cols: u16,
    rows: u16,
    footer_rows: u16,
) -> String {
    let mut out = String::new();
    out.push_str(ansi::save_cursor());
    let first_footer_row = rows.saturating_sub(footer_rows).saturating_add(1);
    for index in 0..footer_rows {
        let row = first_footer_row + index;
        out.push_str(&ansi::move_to(row, 1));
        let line = lines.get(index as usize).map(String::as_str).unwrap_or("");
        out.push_str(&pad_to_width(line, cols));
    }
    out.push_str(ansi::restore_cursor());
    out
}

fn resize_footer_region_sequence(
    old: FooterGeometry,
    new: FooterGeometry,
    lines: &[String],
) -> String {
    let mut out = clear_footer_transition_sequence(old, new);
    let Some(decstbm) = new.decstbm() else {
        return out;
    };
    out.push_str(&decstbm);
    out.push_str(&ansi::move_to(new.rows - new.footer_rows, 1));
    out.push_str(&render_footer_frame_sequence(
        lines,
        new.cols,
        new.rows,
        new.footer_rows,
    ));
    out
}

fn clear_footer_transition_sequence(old: FooterGeometry, new: FooterGeometry) -> String {
    let mut out = String::from(ansi::reset_decstbm());
    for row in stale_footer_rows(old, new) {
        out.push_str(&ansi::move_to(row, 1));
        out.push_str(ansi::clear_line());
    }
    out
}

fn stale_footer_rows(old: FooterGeometry, new: FooterGeometry) -> Vec<u16> {
    let mut rows = Vec::new();
    let old_first = old.rows.saturating_sub(old.footer_rows).saturating_add(1);
    for row in old_first..=old.rows {
        if row <= new.rows {
            push_unique_row(&mut rows, row);
        }
    }
    push_bottom_rows(&mut rows, new.rows, old.footer_rows);
    push_bottom_rows(&mut rows, new.rows, new.footer_rows);
    rows.sort_unstable();
    rows
}

fn push_bottom_rows(rows: &mut Vec<u16>, screen_rows: u16, count: u16) {
    if screen_rows == 0 || count == 0 {
        return;
    }
    let first = screen_rows.saturating_sub(count).saturating_add(1);
    for row in first..=screen_rows {
        push_unique_row(rows, row);
    }
}

fn push_unique_row(rows: &mut Vec<u16>, row: u16) {
    if row > 0 && !rows.contains(&row) {
        rows.push(row);
    }
}

fn footer_rows_for_cols(cols: u16) -> u16 {
    if cols < NARROW_FOOTER_COLS { 2 } else { 1 }
}

pub fn clear_footer_region_sequence(rows: u16, footer_rows: u16) -> String {
    let mut out = String::from(ansi::reset_decstbm());
    if footer_rows == 0 || rows == 0 || rows <= footer_rows {
        return out;
    }
    let first_footer_row = rows.saturating_sub(footer_rows).saturating_add(1);
    for row in first_footer_row..=rows {
        out.push_str(&ansi::move_to(row, 1));
        out.push_str(ansi::clear_line());
    }
    out
}

pub fn fit_to_width(value: &str, cols: u16) -> String {
    let max_width = cols as usize;
    if max_width == 0 {
        return String::new();
    }
    if display_width(value) <= max_width {
        return value.to_string();
    }
    let marker = "...";
    let marker_width = display_width(marker);
    if max_width <= marker_width {
        return crate::util::truncate_at_char_boundary(value, max_width).to_string();
    }
    let target = max_width - marker_width;
    let mut width = 0usize;
    let mut end = 0usize;
    for (index, ch) in value.char_indices() {
        let next = width + char_display_width(ch);
        if next > target {
            break;
        }
        width = next;
        end = index + ch.len_utf8();
    }
    let mut out = crate::util::truncate_at_char_boundary(value, end)
        .trim_end()
        .to_string();
    out.push_str(marker);
    out
}

fn display_width(value: &str) -> usize {
    value.chars().map(char_display_width).sum()
}

fn pad_to_width(value: &str, cols: u16) -> String {
    let width = display_width_ansi(value);
    let mut out = value.to_string();
    if width < cols as usize {
        out.push_str(&" ".repeat(cols as usize - width));
    }
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
            continue;
        }
        width += char_display_width(ch);
    }
    width
}

fn char_display_width(ch: char) -> usize {
    let cp = ch as u32;
    if cp < 0x20 || (0x7f..=0x9f).contains(&cp) {
        0
    } else if matches!(
        cp,
        0x1100..=0x115f
            | 0x2329..=0x232a
            | 0x2e80..=0xa4cf
            | 0xac00..=0xd7a3
            | 0xf900..=0xfaff
            | 0xfe10..=0xfe19
            | 0xfe30..=0xfe6f
            | 0xff00..=0xff60
            | 0xffe0..=0xffe6
    ) {
        2
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn status(tokens: Option<u64>) -> UiStatus {
        UiStatus {
            mode: "act".to_string(),
            provider: "ollama".to_string(),
            model: "m".to_string(),
            context_budget: 65536,
            yes: true,
            prompt_tokens: tokens,
            completion_tokens: None,
        }
    }

    #[test]
    fn footer_env_disable_by_flag() {
        assert!(!FooterEnv::detect_with(|_| None, true, true).enabled);
    }

    #[test]
    fn footer_env_disable_by_footer_off_cli_mode() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        let config = crate::config::Config::from_cli(crate::cli::Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--footer",
            "off",
        ]))
        .unwrap();

        assert!(config.no_footer);
        assert_eq!(config.field_sources.footer, "flag");
        assert!(!FooterEnv::detect_with(|_| None, true, config.no_footer).enabled);
    }

    #[test]
    fn footer_env_disable_by_env() {
        let env = FooterEnv::detect_with(
            |key| (key == "ANVIL_NO_FOOTER").then(|| "1".to_string()),
            true,
            false,
        );
        assert!(!env.enabled);
    }

    #[test]
    fn footer_env_non_tty_noop() {
        assert!(!FooterEnv::detect_with(|_| None, false, false).enabled);
    }

    #[test]
    fn footer_decstbm_rows_under_two_self_disable() {
        assert_eq!(ansi::build_decstbm(1, 1), None);
        assert_eq!(ansi::build_decstbm(2, 1).as_deref(), Some("\x1b[1;1r"));
        assert_eq!(ansi::build_decstbm(4, 2).as_deref(), Some("\x1b[1;2r"));
    }

    #[test]
    fn footer_restore_sequence_clears_reserved_rows_after_panic_unwind() {
        let sequence = clear_footer_region_sequence(24, 2);

        assert!(sequence.starts_with(ansi::reset_decstbm()));
        assert!(sequence.contains("\x1b[23;1H\r\x1b[2K"));
        assert!(sequence.contains("\x1b[24;1H\r\x1b[2K"));
    }

    #[test]
    fn footer_geometry_keeps_last_size_when_terminal_query_fails() {
        let current = FooterGeometry::new(120, 24);
        let failed = Err(io::Error::other("terminal unavailable"));

        assert_eq!(geometry_after_size(current, failed), current);
    }

    #[test]
    fn footer_resize_grow_clears_old_and_new_bottom_rows_without_scrolling() {
        let old = FooterGeometry::new(80, 24);
        let new = FooterGeometry::new(80, 30);
        let sequence = resize_footer_region_sequence(
            old,
            new,
            &["new top".to_string(), "new bottom".to_string()],
        );

        assert!(sequence.starts_with(ansi::reset_decstbm()));
        for row in [23, 24, 29, 30] {
            assert!(
                sequence.contains(&format!("{}{}", ansi::move_to(row, 1), ansi::clear_line())),
                "missing cleanup for row {row}: {sequence:?}"
            );
        }
        assert!(sequence.contains("\x1b[1;28r"), "{sequence:?}");
        assert!(!sequence.contains('\n'), "{sequence:?}");
    }

    #[test]
    fn footer_resize_shrink_uses_only_visible_rows_and_clamps_body_cursor() {
        let old = FooterGeometry::new(80, 30);
        let new = FooterGeometry::new(80, 20);
        let sequence = resize_footer_region_sequence(
            old,
            new,
            &["new top".to_string(), "new bottom".to_string()],
        );

        assert!(!sequence.contains("\x1b[29;1H"), "{sequence:?}");
        assert!(!sequence.contains("\x1b[30;1H"), "{sequence:?}");
        for row in [19, 20] {
            assert!(
                sequence.contains(&format!("{}{}", ansi::move_to(row, 1), ansi::clear_line())),
                "missing cleanup for row {row}: {sequence:?}"
            );
        }
        let region = sequence.find("\x1b[1;18r").unwrap();
        let body_cursor = sequence[region..].find("\x1b[18;1H").unwrap() + region;
        let footer_frame = sequence.rfind("\x1b[19;1H").unwrap();
        assert!(
            region < body_cursor && body_cursor < footer_frame,
            "{sequence:?}"
        );
    }

    #[test]
    fn footer_resize_across_width_threshold_switches_one_and_two_rows() {
        let runtime = RuntimeStatus {
            phase: Some(status_bus::PhaseStatus {
                index: 1,
                total: 3,
                id: "implementation".to_string(),
            }),
            ..RuntimeStatus::default()
        };
        let wide = FooterGeometry::new(100, 24);
        let narrow = FooterGeometry::new(99, 24);
        let wide_lines =
            build_live_footer_lines(&status(None), &runtime, StatusTime::ZERO, wide.cols, false);
        let narrow_lines = build_live_footer_lines(
            &status(None),
            &runtime,
            StatusTime::ZERO,
            narrow.cols,
            false,
        );

        assert_eq!(wide.footer_rows, 1);
        assert_eq!(wide_lines.len(), 1);
        assert_eq!(narrow.footer_rows, 2);
        assert_eq!(narrow_lines.len(), 2);
        let sequence = resize_footer_region_sequence(wide, narrow, &narrow_lines);
        assert!(sequence.contains("\x1b[1;22r"), "{sequence:?}");
        assert!(sequence.contains("\x1b[23;1H"), "{sequence:?}");
        assert!(sequence.contains("\x1b[24;1H"), "{sequence:?}");
        assert!(
            narrow_lines
                .iter()
                .all(|line| display_width_ansi(line) <= narrow.cols as usize)
        );
        let back_to_wide = resize_footer_region_sequence(narrow, wide, &wide_lines);
        assert!(
            back_to_wide.contains("\x1b[23;1H\r\x1b[2K"),
            "{back_to_wide:?}"
        );
        assert!(back_to_wide.contains("\x1b[1;23r"), "{back_to_wide:?}");
    }

    #[test]
    fn footer_shutdown_cleanup_uses_resized_geometry() {
        let rendered = FooterGeometry::new(80, 24);
        let resized = FooterGeometry::new(80, 30);
        let sequence = clear_footer_transition_sequence(rendered, resized);

        assert!(sequence.starts_with(ansi::reset_decstbm()));
        assert!(sequence.contains("\x1b[23;1H\r\x1b[2K"));
        assert!(sequence.contains("\x1b[24;1H\r\x1b[2K"));
        assert!(sequence.contains("\x1b[29;1H\r\x1b[2K"));
        assert!(sequence.contains("\x1b[30;1H\r\x1b[2K"));
    }

    #[test]
    fn footer_tiny_resize_releases_region_until_height_recovers() {
        let old = FooterGeometry::new(80, 24);
        let tiny = FooterGeometry::new(80, 2);
        let sequence = resize_footer_region_sequence(old, tiny, &[]);

        assert!(!tiny.is_renderable());
        assert!(sequence.starts_with(ansi::reset_decstbm()));
        assert!(sequence.contains("\x1b[1;1H\r\x1b[2K"));
        assert!(sequence.contains("\x1b[2;1H\r\x1b[2K"));
        assert_eq!(
            clear_footer_region_sequence(tiny.rows, tiny.footer_rows),
            "\x1b[r"
        );
    }

    #[test]
    fn queued_input_adds_one_resize_safe_footer_row() {
        let base = FooterGeometry::new(120, 24);
        let with_input = FooterGeometry::with_input_row(120, 24, true);
        let narrow_with_input = FooterGeometry::with_input_row(80, 24, true);

        assert_eq!(base.footer_rows, 1);
        assert_eq!(with_input.footer_rows, 2);
        assert_eq!(narrow_with_input.footer_rows, 3);
        let sequence = resize_footer_region_sequence(
            base,
            with_input,
            &["type ahead".to_string(), "status".to_string()],
        );
        assert!(sequence.contains("\x1b[1;22r"), "{sequence:?}");
        assert!(sequence.contains("\x1b[23;1H"), "{sequence:?}");
        assert!(sequence.contains("\x1b[24;1H"), "{sequence:?}");
    }

    #[test]
    fn pending_input_footer_echo_explains_esc_and_ctrl_c() {
        let snapshot = InputSnapshot {
            enabled: true,
            pending: "run the 日本語 check".to_string(),
            queued_count: 2,
            notice: None,
        };

        let line = build_input_queue_line(&snapshot, 160, false).unwrap();

        assert!(line.contains("pending> run the 日本語 check"), "{line}");
        assert!(line.contains("2 queued"), "{line}");
        assert!(line.contains("Esc clears"), "{line}");
        assert!(line.contains("Ctrl-C interrupts"), "{line}");
    }

    #[test]
    fn empty_pending_footer_shows_confirmation_and_interrupt_semantics() {
        let snapshot = InputSnapshot {
            enabled: true,
            pending: String::new(),
            queued_count: 1,
            notice: Some("queued: follow up".to_string()),
        };

        let line = build_input_queue_line(&snapshot, 160, false).unwrap();

        assert!(line.starts_with("queued: follow up"), "{line}");
        assert!(line.contains("Esc interrupts"), "{line}");
        assert!(line.contains("Ctrl-C interrupts"), "{line}");
    }

    #[test]
    fn disabled_input_queue_does_not_claim_a_footer_line() {
        let snapshot = InputSnapshot {
            enabled: false,
            pending: String::new(),
            queued_count: 0,
            notice: None,
        };

        assert_eq!(build_input_queue_line(&snapshot, 120, false), None);
    }

    #[test]
    fn footer_unknown_tokens_are_na() {
        let line = build_footer_line(&status(None), false);
        assert!(line.contains("tokens:n/a"));
    }

    #[test]
    fn footer_known_tokens_are_formatted() {
        let line = build_footer_line(&status(Some(1500)), false);
        assert!(line.contains("tokens:1.5k"));
    }

    #[test]
    fn live_footer_long_planner_turn_snapshot() {
        let runtime = RuntimeStatus {
            phase: Some(status_bus::PhaseStatus {
                index: 2,
                total: 5,
                id: "core-logic".to_string(),
            }),
            step: Some(status_bus::StepStatus {
                id: "write".to_string(),
                kind: "implement".to_string(),
            }),
            provider: Some(status_bus::ProviderTurnStatus {
                scope: "planner_step".to_string(),
                deadline_secs: 600,
                started_at: StatusTime::from_secs(10),
            }),
            ..RuntimeStatus::default()
        };

        let lines = build_live_footer_lines(
            &status(None),
            &runtime,
            StatusTime::from_secs(57),
            140,
            false,
        );

        assert_eq!(
            lines,
            vec!["Phase 2/5 core-logic · implement · planning steps 47s/600s".to_string()]
        );
    }

    #[test]
    fn live_footer_command_execution_replaces_provider_segment() {
        let runtime = RuntimeStatus {
            provider: Some(status_bus::ProviderTurnStatus {
                scope: "executor".to_string(),
                deadline_secs: 600,
                started_at: StatusTime::ZERO,
            }),
            command: Some(status_bus::CommandStatus {
                excerpt: "\"npm\" \"run\" \"build\"".to_string(),
                cap_secs: 30,
                started_at: StatusTime::from_secs(5),
            }),
            ..RuntimeStatus::default()
        };

        let lines = build_live_footer_lines(
            &status(None),
            &runtime,
            StatusTime::from_secs(12),
            140,
            false,
        );

        assert_eq!(lines, vec!["cmd \"npm\" \"run\" \"build\" 7s/30s"]);
    }

    #[test]
    fn live_footer_repair_and_interrupt_prefix_snapshot() {
        let runtime = RuntimeStatus {
            phase: Some(status_bus::PhaseStatus {
                index: 1,
                total: 2,
                id: "検証".to_string(),
            }),
            step: Some(status_bus::StepStatus {
                id: "修復".to_string(),
                kind: "verify".to_string(),
            }),
            repair: Some(status_bus::RepairStatus {
                attempt: 1,
                max: 2,
                kind: "final_acceptance".to_string(),
            }),
            last_event: Some("tool_args_path_normalized".to_string()),
            interrupt_requested: true,
            ..RuntimeStatus::default()
        };

        let lines =
            build_live_footer_lines(&status(None), &runtime, StatusTime::from_secs(0), 80, false);

        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("[stopping: aborting current operation…] Phase 1/2"));
        assert!(lines[1].contains("repair 1/2"));
        assert!(!lines.join("\n").contains("last:"), "{lines:?}");
    }

    #[test]
    fn live_footer_ignores_last_event_ticker() {
        let runtime = RuntimeStatus {
            last_event: Some("tool_args_path_normalized".to_string()),
            ..RuntimeStatus::default()
        };

        let lines = build_live_footer_lines(
            &status(None),
            &runtime,
            StatusTime::from_secs(0),
            120,
            false,
        );

        assert_eq!(
            lines,
            vec!["[act] provider:ollama model:m ctx:65536 tokens:n/a [yes]"]
        );
        assert!(!lines.join("\n").contains("last:"), "{lines:?}");
    }

    #[test]
    fn live_footer_force_finalize_prefix_snapshot() {
        let runtime = RuntimeStatus {
            command: Some(status_bus::CommandStatus {
                excerpt: "sleep 600".to_string(),
                cap_secs: 600,
                started_at: StatusTime::from_secs(10),
            }),
            interrupt_requested: true,
            force_finalize_requested: true,
            ..RuntimeStatus::default()
        };

        let lines = build_live_footer_lines(
            &status(None),
            &runtime,
            StatusTime::from_secs(12),
            120,
            false,
        );

        assert_eq!(
            lines,
            vec!["[stopping: force-finalizing…] cmd sleep 600 2s/600s"]
        );
    }

    #[test]
    fn live_footer_fitting_is_multibyte_safe_for_japanese_paths() {
        let long = format!(
            "Phase 1/3 実装 · cmd .anvil/repairs/{} 123s/600s",
            "日本語ディレクトリ/".repeat(24)
        );
        for cols in 1..120 {
            let fitted = fit_to_width(&long, cols);
            assert!(fitted.is_char_boundary(fitted.len()));
            assert!(display_width(&fitted) <= cols as usize || cols <= 3);
        }
    }

    #[test]
    fn footer_skips_render_until_composed_line_changes() {
        let runtime = RuntimeStatus {
            provider: Some(status_bus::ProviderTurnStatus {
                scope: "planner_step".to_string(),
                deadline_secs: 600,
                started_at: StatusTime::ZERO,
            }),
            ..RuntimeStatus::default()
        };
        let mut last = None;
        let first = build_live_footer_lines(
            &status(None),
            &runtime,
            StatusTime::from_millis(0),
            120,
            false,
        );
        let same = build_live_footer_lines(
            &status(None),
            &runtime,
            StatusTime::from_millis(250),
            120,
            false,
        );
        let next_second = build_live_footer_lines(
            &status(None),
            &runtime,
            StatusTime::from_millis(1_000),
            120,
            false,
        );

        assert!(footer_lines_changed(&mut last, &first));
        assert!(!footer_lines_changed(&mut last, &same));
        assert!(footer_lines_changed(&mut last, &next_second));
    }

    #[test]
    fn footer_frame_overwrites_without_clear_line_between_frames() {
        let frame = render_footer_frame_sequence(&["short".to_string()], 12, 24, 1);

        assert!(!frame.contains(ansi::clear_line()), "{frame:?}");
        assert!(frame.contains("\x1b[24;1Hshort       "), "{frame:?}");
    }

    #[test]
    fn footer_frame_pads_empty_rows_to_clear_stale_tails() {
        let frame = render_footer_frame_sequence(&["p1".to_string()], 10, 24, 2);

        assert!(frame.contains("\x1b[23;1Hp1        "), "{frame:?}");
        assert!(frame.contains("\x1b[24;1H          "), "{frame:?}");
    }

    #[test]
    fn footer_padding_ignores_sgr_width() {
        let padded = pad_to_width("\x1b[2mabc\x1b[0m", 5);

        assert_eq!(display_width_ansi(&padded), 5);
        assert!(padded.ends_with("  "));
    }
}
