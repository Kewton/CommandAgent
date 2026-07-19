use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::io::Write;

use crate::tui::input_queue::InputQueue;

const POLL_INTERVAL: Duration = Duration::from_millis(100);
const INTERRUPT_NOTICE: &str = "interrupt requested; aborting current operation";
const FORCE_INTERRUPT_NOTICE: &str = "interrupt requested again; force-finalizing immediately";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterruptEnv {
    pub enabled: bool,
}

impl InterruptEnv {
    pub fn detect() -> Self {
        Self::detect_with(
            |key| std::env::var_os(key).map(|value| value.to_string_lossy().into_owned()),
            crate::tui::terminal::stdin_is_tty(),
        )
    }

    pub fn detect_with(get_env: impl Fn(&str) -> Option<String>, stdin_is_tty: bool) -> Self {
        if crate::tui::terminal::env_non_empty_with(get_env, "COMMANDAGENT_NO_INTERRUPT") {
            return Self { enabled: false };
        }
        Self {
            enabled: stdin_is_tty,
        }
    }
}

struct MonitorState {
    stop_requested: bool,
    paused: bool,
    parked: bool,
}

struct Active {
    flag: Arc<AtomicBool>,
    force_flag: Arc<AtomicBool>,
    state: Arc<(Mutex<MonitorState>, Condvar)>,
    input_queue: Option<InputQueue>,
    handle: Option<JoinHandle<()>>,
    raw_mode_active: bool,
}

pub struct InterruptMonitor {
    inner: Option<Active>,
    dummy_flag: Arc<AtomicBool>,
    dummy_force_flag: Arc<AtomicBool>,
}

impl InterruptMonitor {
    pub fn start() -> Self {
        Self::start_with_env_and_input_queue(InterruptEnv::detect(), None)
    }

    pub fn start_with_input_queue(input_queue: InputQueue) -> Self {
        Self::start_with_env_and_input_queue(InterruptEnv::detect(), Some(input_queue))
    }

    pub fn start_with_env(env: InterruptEnv) -> Self {
        Self::start_with_env_and_input_queue(env, None)
    }

    fn start_with_env_and_input_queue(env: InterruptEnv, input_queue: Option<InputQueue>) -> Self {
        let dummy_flag = Arc::new(AtomicBool::new(false));
        let dummy_force_flag = Arc::new(AtomicBool::new(false));
        if !env.enabled || enable_raw_mode().is_err() {
            return Self {
                inner: None,
                dummy_flag,
                dummy_force_flag,
            };
        }
        let flag = Arc::new(AtomicBool::new(false));
        let force_flag = Arc::new(AtomicBool::new(false));
        let state = Arc::new((
            Mutex::new(MonitorState {
                stop_requested: false,
                paused: false,
                parked: false,
            }),
            Condvar::new(),
        ));
        let thread_flag = flag.clone();
        let thread_force_flag = force_flag.clone();
        let thread_state = state.clone();
        let thread_input_queue = input_queue.clone();
        let handle = thread::Builder::new()
            .name("commandagent-interrupt".to_string())
            .spawn(move || {
                monitor_loop(
                    thread_flag,
                    thread_force_flag,
                    thread_state,
                    thread_input_queue,
                )
            })
            .ok();
        match handle {
            Some(handle) => Self {
                inner: Some(Active {
                    flag,
                    force_flag,
                    state,
                    input_queue,
                    handle: Some(handle),
                    raw_mode_active: true,
                }),
                dummy_flag,
                dummy_force_flag,
            },
            None => {
                let _ = disable_raw_mode();
                Self {
                    inner: None,
                    dummy_flag,
                    dummy_force_flag,
                }
            }
        }
    }

    pub fn new_preset(value: bool) -> Self {
        Self {
            inner: None,
            dummy_flag: Arc::new(AtomicBool::new(value)),
            dummy_force_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_active(&self) -> bool {
        self.inner.is_some()
    }

    pub fn interrupted(&self) -> bool {
        match &self.inner {
            Some(active) => active.flag.load(Ordering::SeqCst),
            None => self.dummy_flag.load(Ordering::SeqCst),
        }
    }

    pub fn force_interrupted(&self) -> bool {
        match &self.inner {
            Some(active) => active.force_flag.load(Ordering::SeqCst),
            None => self.dummy_force_flag.load(Ordering::SeqCst),
        }
    }

    pub fn reset(&self) {
        match &self.inner {
            Some(active) => {
                active.flag.store(false, Ordering::SeqCst);
                active.force_flag.store(false, Ordering::SeqCst);
            }
            None => {
                self.dummy_flag.store(false, Ordering::SeqCst);
                self.dummy_force_flag.store(false, Ordering::SeqCst);
            }
        }
    }

    fn pause(&mut self) {
        let Some(active) = self.inner.as_mut() else {
            return;
        };
        if !active.raw_mode_active {
            return;
        }
        let (lock, cvar) = &*active.state;
        if let Ok(mut state) = lock.lock() {
            state.paused = true;
            cvar.notify_all();
            while !state.parked && !state.stop_requested {
                state = cvar.wait(state).unwrap();
            }
        }
        let _ = disable_raw_mode();
        active.raw_mode_active = false;
    }

    fn resume(&mut self) {
        let Some(active) = self.inner.as_mut() else {
            return;
        };
        if active.raw_mode_active {
            return;
        }
        if enable_raw_mode().is_err() {
            if let Some(input_queue) = &active.input_queue {
                input_queue.set_enabled(false);
            }
            return;
        }
        let (lock, cvar) = &*active.state;
        if let Ok(mut state) = lock.lock() {
            state.paused = false;
            state.parked = false;
            cvar.notify_all();
        }
        active.raw_mode_active = true;
    }
}

impl Drop for InterruptMonitor {
    fn drop(&mut self) {
        let Some(mut active) = self.inner.take() else {
            return;
        };
        if let Some(input_queue) = &active.input_queue {
            input_queue.set_enabled(false);
        }
        {
            let (lock, cvar) = &*active.state;
            if let Ok(mut state) = lock.lock() {
                state.stop_requested = true;
                state.paused = false;
                cvar.notify_all();
            }
        }
        if let Some(handle) = active.handle.take() {
            let _ = handle.join();
        }
        if active.raw_mode_active {
            let _ = disable_raw_mode();
        }
    }
}

pub struct PauseGuard {
    monitor: Arc<Mutex<InterruptMonitor>>,
}

impl PauseGuard {
    pub fn new(monitor: Arc<Mutex<InterruptMonitor>>) -> Option<Self> {
        if let Ok(mut monitor_guard) = monitor.lock() {
            monitor_guard.pause();
        }
        Some(Self { monitor })
    }
}

impl Drop for PauseGuard {
    fn drop(&mut self) {
        if let Ok(mut monitor) = self.monitor.lock() {
            monitor.resume();
        }
    }
}

fn monitor_loop(
    flag: Arc<AtomicBool>,
    force_flag: Arc<AtomicBool>,
    state: Arc<(Mutex<MonitorState>, Condvar)>,
    input_queue: Option<InputQueue>,
) {
    let (lock, cvar) = &*state;
    loop {
        {
            let mut shared = lock.lock().unwrap();
            if shared.stop_requested {
                break;
            }
            while shared.paused {
                shared.parked = true;
                cvar.notify_all();
                shared = cvar.wait(shared).unwrap();
                if shared.stop_requested {
                    return;
                }
            }
            shared.parked = false;
        }
        match event::poll(POLL_INTERVAL) {
            Ok(true) => match event::read() {
                Ok(Event::Key(key))
                    if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
                {
                    if route_key_event(key.code, key.modifiers, input_queue.as_ref())
                        == KeyRoute::Interrupt
                    {
                        match register_interrupt_press(&flag, &force_flag) {
                            InterruptPress::First => {
                                emit_interrupt_feedback(INTERRUPT_NOTICE);
                                crate::tui::status_bus::publish_interrupt_requested();
                            }
                            InterruptPress::Force => {
                                emit_interrupt_feedback(FORCE_INTERRUPT_NOTICE);
                                crate::tui::status_bus::publish_force_finalize_requested();
                            }
                        }
                    }
                }
                Ok(_) => {}
                Err(_) => break,
            },
            Ok(false) => {}
            Err(_) => break,
        }
    }
    if let Ok(mut shared) = lock.lock() {
        shared.stop_requested = true;
        shared.paused = false;
        cvar.notify_all();
    }
    if let Some(input_queue) = input_queue {
        input_queue.set_enabled(false);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InterruptPress {
    First,
    Force,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyRoute {
    Ignored,
    Input,
    Interrupt,
}

fn route_key_event(
    code: KeyCode,
    modifiers: KeyModifiers,
    input_queue: Option<&InputQueue>,
) -> KeyRoute {
    if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
        return KeyRoute::Interrupt;
    }
    let Some(input_queue) = input_queue.filter(|queue| queue.is_enabled()) else {
        return if code == KeyCode::Esc {
            KeyRoute::Interrupt
        } else {
            KeyRoute::Ignored
        };
    };
    match code {
        KeyCode::Esc => {
            if input_queue.clear_pending() {
                KeyRoute::Input
            } else {
                KeyRoute::Interrupt
            }
        }
        KeyCode::Backspace => {
            let _ = input_queue.backspace();
            KeyRoute::Input
        }
        KeyCode::Enter => {
            let _ = input_queue.submit();
            KeyRoute::Input
        }
        KeyCode::Char(ch) if !modifiers.contains(KeyModifiers::CONTROL) => {
            let _ = input_queue.push_char(ch);
            KeyRoute::Input
        }
        _ => KeyRoute::Ignored,
    }
}

fn register_interrupt_press(flag: &AtomicBool, force_flag: &AtomicBool) -> InterruptPress {
    if flag.swap(true, Ordering::SeqCst) {
        force_flag.store(true, Ordering::SeqCst);
        InterruptPress::Force
    } else {
        InterruptPress::First
    }
}

fn emit_interrupt_feedback(message: &str) {
    let mut err = std::io::stderr().lock();
    let _ = err.write_all(b"\r\x1b[2K\r\n");
    let _ = err.write_all(message.as_bytes());
    let _ = err.write_all(b"\r\n");
    let _ = err.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interrupt_env_disable() {
        let env = InterruptEnv::detect_with(
            |key| (key == "COMMANDAGENT_NO_INTERRUPT").then(|| "1".to_string()),
            true,
        );
        assert!(!env.enabled);
    }

    #[test]
    fn interrupt_env_non_tty_noop() {
        assert!(!InterruptEnv::detect_with(|_| None, false).enabled);
    }

    #[test]
    fn interrupt_preset_flag() {
        let monitor = InterruptMonitor::new_preset(true);
        assert!(monitor.interrupted());
        assert!(!monitor.force_interrupted());
        monitor.reset();
        assert!(!monitor.interrupted());
        assert!(!monitor.force_interrupted());
    }

    #[test]
    fn interrupt_press_escalates_to_force() {
        let flag = AtomicBool::new(false);
        let force_flag = AtomicBool::new(false);

        assert_eq!(
            register_interrupt_press(&flag, &force_flag),
            InterruptPress::First
        );
        assert!(flag.load(Ordering::SeqCst));
        assert!(!force_flag.load(Ordering::SeqCst));
        assert_eq!(
            register_interrupt_press(&flag, &force_flag),
            InterruptPress::Force
        );
        assert!(force_flag.load(Ordering::SeqCst));
    }

    #[test]
    fn ctrl_c_always_interrupts_and_esc_depends_on_pending_input() {
        let queue = InputQueue::new();
        queue.set_enabled(true);

        assert_eq!(
            route_key_event(KeyCode::Char('c'), KeyModifiers::CONTROL, Some(&queue)),
            KeyRoute::Interrupt
        );
        assert_eq!(
            route_key_event(KeyCode::Esc, KeyModifiers::empty(), Some(&queue)),
            KeyRoute::Interrupt
        );
        assert_eq!(
            route_key_event(KeyCode::Char('x'), KeyModifiers::empty(), Some(&queue)),
            KeyRoute::Input
        );
        assert_eq!(queue.snapshot().pending, "x");
        assert_eq!(
            route_key_event(KeyCode::Esc, KeyModifiers::empty(), Some(&queue)),
            KeyRoute::Input
        );
        assert!(queue.snapshot().pending.is_empty());
    }

    #[test]
    fn input_keys_are_ignored_without_an_enabled_queue() {
        let queue = InputQueue::new();
        assert_eq!(
            route_key_event(KeyCode::Char('x'), KeyModifiers::empty(), Some(&queue)),
            KeyRoute::Ignored
        );
        assert_eq!(
            route_key_event(KeyCode::Esc, KeyModifiers::empty(), Some(&queue)),
            KeyRoute::Interrupt
        );
        assert_eq!(
            route_key_event(KeyCode::Esc, KeyModifiers::empty(), None),
            KeyRoute::Interrupt
        );
    }

    #[test]
    fn enter_queues_multiple_lines_and_backspace_edits() {
        let queue = InputQueue::new();
        queue.set_enabled(true);
        for code in [
            KeyCode::Char('a'),
            KeyCode::Char('b'),
            KeyCode::Backspace,
            KeyCode::Enter,
            KeyCode::Char('c'),
            KeyCode::Enter,
        ] {
            assert_eq!(
                route_key_event(code, KeyModifiers::empty(), Some(&queue)),
                KeyRoute::Input
            );
        }
        assert_eq!(queue.take_queued().as_deref(), Some("a"));
        assert_eq!(queue.take_queued().as_deref(), Some("c"));
    }
}
