use crate::tui::progress::{sanitize_for_progress, truncate_chars};
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const UTF8_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const ASCII_FRAMES: &[&str] = &["|", "/", "-", "\\"];
const TICK_INTERVAL: Duration = Duration::from_millis(80);
const MAX_LABEL_CHARS: usize = 96;
const CLEAR_LINE: &str = "\r\x1b[2K";
const SPINNER_COLOR: &str = "\x1b[36m";
const RESET: &str = "\x1b[0m";

#[derive(Debug, Clone, Copy)]
pub struct WaitSpinnerConfig {
    pub enabled: bool,
    pub color_enabled: bool,
    pub utf8: bool,
}

#[derive(Debug)]
pub struct WaitSpinner {
    config: WaitSpinnerConfig,
    write_to_terminal: bool,
    active: Option<ActiveWait>,
}

#[derive(Debug)]
struct ActiveWait {
    stop: Arc<AtomicBool>,
    wake: Arc<(Mutex<()>, Condvar)>,
    handle: Option<JoinHandle<()>>,
}

impl WaitSpinner {
    pub fn new(config: WaitSpinnerConfig) -> Self {
        Self {
            config,
            write_to_terminal: true,
            active: None,
        }
    }

    pub fn disabled() -> Self {
        Self::new(WaitSpinnerConfig {
            enabled: false,
            color_enabled: false,
            utf8: false,
        })
    }

    #[cfg(test)]
    fn test_without_terminal_output(config: WaitSpinnerConfig) -> Self {
        Self {
            config,
            write_to_terminal: false,
            active: None,
        }
    }

    pub fn start(&mut self, label: impl Into<String>) {
        self.stop();
        if !self.config.enabled {
            return;
        }

        let label = spinner_label(&label.into());
        let config = self.config;
        let write_to_terminal = self.write_to_terminal;
        let stop = Arc::new(AtomicBool::new(false));
        let wake = Arc::new((Mutex::new(()), Condvar::new()));
        let worker_stop = Arc::clone(&stop);
        let worker_wake = Arc::clone(&wake);
        let handle = match thread::Builder::new()
            .name("commandagent-spinner".into())
            .spawn(move || {
                render_loop(label, config, write_to_terminal, worker_stop, worker_wake);
            }) {
            Ok(handle) => handle,
            Err(_) => return,
        };

        self.active = Some(ActiveWait {
            stop,
            wake,
            handle: Some(handle),
        });
    }

    pub fn stop(&mut self) {
        let Some(mut active) = self.active.take() else {
            return;
        };
        active.stop.store(true, Ordering::SeqCst);
        let (_, cvar) = &*active.wake;
        cvar.notify_all();
        if let Some(handle) = active.handle.take() {
            let _ = handle.join();
        }
        if self.write_to_terminal {
            clear_line();
        }
    }
}

impl Drop for WaitSpinner {
    fn drop(&mut self) {
        self.stop();
    }
}

fn render_loop(
    label: String,
    config: WaitSpinnerConfig,
    write_to_terminal: bool,
    stop: Arc<AtomicBool>,
    wake: Arc<(Mutex<()>, Condvar)>,
) {
    let started = Instant::now();
    let frames = spinner_frames(config.utf8);
    let mut index = 0usize;
    let (lock, cvar) = &*wake;

    while !stop.load(Ordering::SeqCst) {
        let frame = frames[index % frames.len()];
        let line = render_spinner_line(
            frame,
            &label,
            started.elapsed().as_secs(),
            config.color_enabled,
        );
        if write_to_terminal {
            let mut stderr = io::stderr().lock();
            if stderr.write_all(line.as_bytes()).is_err() {
                stop.store(true, Ordering::SeqCst);
                return;
            }
            let _ = stderr.flush();
        }

        index = index.wrapping_add(1);
        let guard = lock.lock().unwrap();
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let (_guard, _) = cvar.wait_timeout(guard, TICK_INTERVAL).unwrap();
    }
}

fn clear_line() {
    let mut stderr = io::stderr().lock();
    let _ = stderr.write_all(clear_line_sequence().as_bytes());
    let _ = stderr.flush();
}

pub(crate) fn spinner_frames(utf8: bool) -> &'static [&'static str] {
    if utf8 { UTF8_FRAMES } else { ASCII_FRAMES }
}

pub(crate) fn render_spinner_line(
    frame: &str,
    label: &str,
    elapsed_secs: u64,
    color_enabled: bool,
) -> String {
    if color_enabled {
        format!("{CLEAR_LINE}{SPINNER_COLOR}{frame}{RESET} {label} {elapsed_secs}s")
    } else {
        format!("{CLEAR_LINE}{frame} {label} {elapsed_secs}s")
    }
}

pub(crate) fn clear_line_sequence() -> &'static str {
    CLEAR_LINE
}

pub(crate) fn spinner_label(label: &str) -> String {
    truncate_chars(&sanitize_for_progress(label), MAX_LABEL_CHARS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_spinner_can_start_and_stop_without_output_thread() {
        let mut spinner = WaitSpinner::disabled();

        spinner.start("model");
        assert!(spinner.active.is_none());

        spinner.stop();
        assert!(spinner.active.is_none());
    }

    #[test]
    fn active_spinner_stop_is_idempotent() {
        let mut spinner = WaitSpinner::test_without_terminal_output(WaitSpinnerConfig {
            enabled: true,
            color_enabled: false,
            utf8: false,
        });

        spinner.start("model");
        spinner.stop();
        spinner.stop();

        assert!(spinner.active.is_none());
    }

    #[test]
    fn spinner_label_sanitizes_control_characters_and_truncates() {
        let label = spinner_label(&format!("model\n{}", "x".repeat(200)));

        assert!(!label.contains('\n'));
        assert!(label.len() < 140);
    }

    #[test]
    fn frame_selection_uses_utf8_locale() {
        assert_eq!(spinner_frames(true)[0], "⠋");
        assert_eq!(spinner_frames(false)[0], "|");
    }

    #[test]
    fn renders_spinner_line_with_clear_prefix_and_optional_color() {
        assert_eq!(
            render_spinner_line("|", "model", 2, false),
            "\r\x1b[2K| model 2s"
        );
        assert_eq!(
            render_spinner_line("⠋", "model", 2, true),
            "\r\x1b[2K\x1b[36m⠋\x1b[0m model 2s"
        );
    }

    #[test]
    fn exposes_clear_line_sequence() {
        assert_eq!(clear_line_sequence(), "\r\x1b[2K");
    }
}
