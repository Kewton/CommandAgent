use crate::tui::progress::{sanitize_for_progress, truncate_chars};
use std::io::{self, Write};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const TICK_INTERVAL: Duration = Duration::from_secs(2);
const MAX_LABEL_CHARS: usize = 96;

#[derive(Debug)]
pub struct WaitSpinner {
    enabled: bool,
    active: Option<ActiveWait>,
}

#[derive(Debug)]
struct ActiveWait {
    stop: Sender<()>,
    handle: Option<JoinHandle<()>>,
}

impl WaitSpinner {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            active: None,
        }
    }

    pub fn start(&mut self, label: impl Into<String>) {
        self.stop();
        if !self.enabled {
            return;
        }

        let label = spinner_label(&label.into());
        let (stop, stop_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let started = Instant::now();
            loop {
                match stop_rx.recv_timeout(TICK_INTERVAL) {
                    Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
                    Err(RecvTimeoutError::Timeout) => {}
                }

                let elapsed = started.elapsed().as_secs();
                let mut stderr = io::stderr().lock();
                let _ = writeln!(stderr, "waiting: {label} {elapsed}s");
                let _ = stderr.flush();
            }
        });

        self.active = Some(ActiveWait {
            stop,
            handle: Some(handle),
        });
    }

    pub fn stop(&mut self) {
        let Some(mut active) = self.active.take() else {
            return;
        };
        let _ = active.stop.send(());
        if let Some(handle) = active.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for WaitSpinner {
    fn drop(&mut self) {
        self.stop();
    }
}

fn spinner_label(label: &str) -> String {
    truncate_chars(&sanitize_for_progress(label), MAX_LABEL_CHARS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_spinner_can_start_and_stop_without_output_thread() {
        let mut spinner = WaitSpinner::new(false);

        spinner.start("model");
        assert!(spinner.active.is_none());

        spinner.stop();
        assert!(spinner.active.is_none());
    }

    #[test]
    fn spinner_label_sanitizes_control_characters_and_truncates() {
        let label = spinner_label(&format!("model\n{}", "x".repeat(200)));

        assert!(!label.contains('\n'));
        assert!(label.len() < 140);
    }
}
