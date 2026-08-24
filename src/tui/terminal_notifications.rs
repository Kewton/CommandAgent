use std::io::{self, Write};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::Value;

const BELL_THRESHOLD: Duration = Duration::from_secs(10);
const TITLE_MAX_BYTES: usize = 120;
const OSC_TITLE_PREFIX: &[u8] = b"\x1b]2;";
const OSC_TERMINATOR: u8 = b'\x07';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalNotificationEnv {
    pub title_enabled: bool,
    pub bell_enabled: bool,
}

impl TerminalNotificationEnv {
    pub fn detect() -> Self {
        Self::detect_with(
            |key| std::env::var_os(key).map(|value| value.to_string_lossy().into_owned()),
            crate::tui::terminal::stdout_is_tty(),
        )
    }

    pub fn detect_with(get_env: impl Fn(&str) -> Option<String>, stdout_is_tty: bool) -> Self {
        if !stdout_is_tty {
            return Self {
                title_enabled: false,
                bell_enabled: false,
            };
        }
        Self {
            title_enabled: !crate::tui::terminal::env_non_empty_with(
                &get_env,
                "COMMANDAGENT_NO_TERMINAL_TITLE",
            ),
            bell_enabled: !crate::tui::terminal::env_non_empty_with(
                get_env,
                "COMMANDAGENT_NO_BELL",
            ),
        }
    }
}

#[derive(Debug)]
struct TerminalNotifications {
    env: TerminalNotificationEnv,
    command_started_at: Option<Duration>,
    title_active: bool,
    finished: bool,
}

impl TerminalNotifications {
    fn new(env: TerminalNotificationEnv) -> Self {
        Self {
            env,
            command_started_at: None,
            title_active: false,
            finished: false,
        }
    }

    fn command_started_at(&mut self, now: Duration) {
        if !self.finished {
            self.command_started_at = Some(now);
        }
    }

    fn project_event_at(&mut self, event: &Value, now: Duration) -> Vec<u8> {
        if self.finished {
            return Vec::new();
        }
        match event.get("event").and_then(Value::as_str) {
            Some("tui_command_start") => {
                self.command_started_at(now);
                Vec::new()
            }
            Some("tui_command_stop") => self.command_finished_at(now),
            Some("ultra_phase_start") if self.env.title_enabled => {
                self.title_active = true;
                title_sequence(&phase_title(event))
            }
            _ => Vec::new(),
        }
    }

    fn command_finished_at(&mut self, now: Duration) -> Vec<u8> {
        let Some(started_at) = self.command_started_at.take() else {
            return Vec::new();
        };
        if self.env.bell_enabled && now.saturating_sub(started_at) >= BELL_THRESHOLD {
            vec![OSC_TERMINATOR]
        } else {
            Vec::new()
        }
    }

    fn finish(&mut self) -> Vec<u8> {
        if self.finished {
            return Vec::new();
        }
        self.finished = true;
        self.command_started_at = None;
        if self.env.title_enabled && self.title_active {
            self.title_active = false;
            title_sequence("")
        } else {
            Vec::new()
        }
    }
}

#[derive(Debug)]
struct ActiveNotifications {
    notifications: TerminalNotifications,
    started_at: Instant,
}

static ACTIVE: OnceLock<Mutex<Option<ActiveNotifications>>> = OnceLock::new();

fn active() -> &'static Mutex<Option<ActiveNotifications>> {
    ACTIVE.get_or_init(|| Mutex::new(None))
}

pub struct TerminalNotificationGuard {
    previous: Option<ActiveNotifications>,
}

pub fn install() -> TerminalNotificationGuard {
    let mut current = active()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let previous = current.take();
    *current = Some(ActiveNotifications {
        notifications: TerminalNotifications::new(TerminalNotificationEnv::detect()),
        started_at: Instant::now(),
    });
    TerminalNotificationGuard { previous }
}

pub fn command_started() {
    let mut current = active()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(active) = current.as_mut() else {
        return;
    };
    let now = active.started_at.elapsed();
    active.notifications.command_started_at(now);
}

pub fn project_event(event: &Value) {
    let bytes = {
        let mut current = active()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(active) = current.as_mut() else {
            return;
        };
        let now = active.started_at.elapsed();
        active.notifications.project_event_at(event, now)
    };
    write_stdout(&bytes);
}

pub fn finish_process() {
    let bytes = {
        let mut current = active()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        current
            .as_mut()
            .map(|active| active.notifications.finish())
            .unwrap_or_default()
    };
    write_stdout(&bytes);
}

impl Drop for TerminalNotificationGuard {
    fn drop(&mut self) {
        let bytes = {
            let mut current = active()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let bytes = current
                .as_mut()
                .map(|active| active.notifications.finish())
                .unwrap_or_default();
            *current = self.previous.take();
            bytes
        };
        write_stdout(&bytes);
    }
}

fn write_stdout(bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    let mut stdout = io::stdout().lock();
    let _ = stdout.write_all(bytes);
    let _ = stdout.flush();
}

fn phase_title(event: &Value) -> String {
    let index = event
        .get("phase_index")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total = event
        .get("total_phases")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let id = event
        .get("phase_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown");
    sanitize_title(&format!("CommandAgent — Phase {index}/{total}: {id}"))
}

fn sanitize_title(value: &str) -> String {
    let flattened = value.replace(['\n', '\r', '\t'], " ");
    let sanitized = crate::tui::markdown::sanitize(&flattened);
    crate::util::truncate_at_char_boundary(&sanitized, TITLE_MAX_BYTES).to_string()
}

fn title_sequence(title: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(OSC_TITLE_PREFIX.len() + title.len() + 1);
    bytes.extend_from_slice(OSC_TITLE_PREFIX);
    bytes.extend_from_slice(title.as_bytes());
    bytes.push(OSC_TERMINATOR);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn env(title_enabled: bool, bell_enabled: bool) -> TerminalNotificationEnv {
        TerminalNotificationEnv {
            title_enabled,
            bell_enabled,
        }
    }

    #[test]
    fn notification_env_honors_tty_current_and_legacy_disable_names() {
        assert_eq!(
            TerminalNotificationEnv::detect_with(|_| None, true),
            env(true, true)
        );
        assert_eq!(
            TerminalNotificationEnv::detect_with(
                |key| (key == "COMMANDAGENT_NO_TERMINAL_TITLE").then(|| "1".to_string()),
                true,
            ),
            env(false, true)
        );
        assert_eq!(
            TerminalNotificationEnv::detect_with(
                |key| (key == "ANVIL_NO_TERMINAL_TITLE").then(|| "1".to_string()),
                true,
            ),
            env(false, true)
        );
        assert_eq!(
            TerminalNotificationEnv::detect_with(
                |key| (key == "COMMANDAGENT_NO_BELL").then(|| "1".to_string()),
                true,
            ),
            env(true, false)
        );
        assert_eq!(
            TerminalNotificationEnv::detect_with(
                |key| (key == "ANVIL_NO_BELL").then(|| "1".to_string()),
                true,
            ),
            env(true, false)
        );
        assert_eq!(
            TerminalNotificationEnv::detect_with(|_| None, false),
            env(false, false)
        );
    }

    #[test]
    fn phase_start_emits_exact_osc_2_title_and_finish_clears_it_once() {
        let mut notifications = TerminalNotifications::new(env(true, true));
        let event = json!({
            "event": "ultra_phase_start",
            "phase_index": 2,
            "total_phases": 5,
            "phase_id": "core-logic",
        });

        assert_eq!(
            notifications.project_event_at(&event, Duration::ZERO),
            "\x1b]2;CommandAgent — Phase 2/5: core-logic\x07".as_bytes()
        );
        assert_eq!(notifications.finish(), b"\x1b]2;\x07");
        assert!(notifications.finish().is_empty());
    }

    #[test]
    fn phase_title_sanitizes_controls_and_bidi_and_caps_utf8_bytes() {
        let mut notifications = TerminalNotifications::new(env(true, false));
        let event = json!({
            "event": "ultra_phase_start",
            "phase_index": 1,
            "total_phases": 2,
            "phase_id": format!("safe\x1b]2;bad\x07\u{202e}{}", "日".repeat(80)),
        });

        let sequence = notifications.project_event_at(&event, Duration::ZERO);
        assert!(sequence.starts_with(OSC_TITLE_PREFIX));
        assert_eq!(sequence.last(), Some(&OSC_TERMINATOR));
        let payload = std::str::from_utf8(&sequence[OSC_TITLE_PREFIX.len()..sequence.len() - 1])
            .expect("title payload must remain valid UTF-8");
        assert!(payload.len() <= TITLE_MAX_BYTES, "{payload:?}");
        assert!(!payload.contains('\x1b'), "{payload:?}");
        assert!(!payload.contains('\x07'), "{payload:?}");
        assert!(!payload.contains('\u{202e}'), "{payload:?}");
        assert!(payload.contains("safe?]2;bad??"), "{payload:?}");
    }

    #[test]
    fn bell_uses_injected_time_threshold_and_emits_once_per_command() {
        let mut notifications = TerminalNotifications::new(env(false, true));
        let start = json!({"event": "tui_command_start"});
        let stop = json!({"event": "tui_command_stop"});

        assert!(
            notifications
                .project_event_at(&start, Duration::from_secs(5))
                .is_empty()
        );
        assert!(
            notifications
                .project_event_at(&stop, Duration::from_millis(14_999))
                .is_empty()
        );
        assert!(
            notifications
                .project_event_at(&start, Duration::from_secs(20))
                .is_empty()
        );
        assert_eq!(
            notifications.project_event_at(&stop, Duration::from_secs(30)),
            b"\x07"
        );
        assert!(
            notifications
                .project_event_at(&stop, Duration::from_secs(40))
                .is_empty()
        );

        let mut direct_notifications = TerminalNotifications::new(env(false, true));
        direct_notifications.command_started_at(Duration::from_secs(50));
        assert_eq!(
            direct_notifications.project_event_at(&stop, Duration::from_secs(60)),
            b"\x07"
        );
    }

    #[test]
    fn disabled_features_emit_nothing() {
        let mut notifications = TerminalNotifications::new(env(false, false));
        notifications.command_started_at(Duration::ZERO);
        assert!(
            notifications
                .project_event_at(
                    &json!({
                        "event": "ultra_phase_start",
                        "phase_index": 1,
                        "total_phases": 1,
                        "phase_id": "safe",
                    }),
                    Duration::ZERO,
                )
                .is_empty()
        );
        assert!(
            notifications
                .project_event_at(
                    &json!({"event": "tui_command_stop"}),
                    Duration::from_secs(10),
                )
                .is_empty()
        );
        assert!(notifications.finish().is_empty());
    }
}
