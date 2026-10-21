//! Deterministic start/ready gates for the in-flight TUI interrupt tests
//! (Issue #620). The cancellation is armed by a real product signal — the
//! provider turn starting, or the Bash fixture installing its TERM trap and
//! announcing ready — instead of a fixed wall-clock window anchored before the
//! work began. Every wait is bounded, so a missing signal fails the test rather
//! than hanging it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::ConversationMessage;
use commandagent::tools::registry::ToolSpec;
use commandagent::tui::status::UiStatus;
use commandagent::tui::{InteractionUi, UiGuard};

/// A one-shot signal shared across threads.
pub struct Gate {
    raised: Mutex<bool>,
    signal: Condvar,
}

impl Gate {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            raised: Mutex::new(false),
            signal: Condvar::new(),
        })
    }

    pub fn raise(&self) {
        *self.raised.lock().unwrap() = true;
        self.signal.notify_all();
    }

    pub fn is_raised(&self) -> bool {
        *self.raised.lock().unwrap()
    }

    /// Waits up to `timeout` for the signal. Returns whether it was raised.
    pub fn wait(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut raised = self.raised.lock().unwrap();
        loop {
            if *raised {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            let (guard, _) = self.signal.wait_timeout(raised, deadline - now).unwrap();
            raised = guard;
        }
    }
}

/// A test workspace under the Cargo integration-test temp dir. `tempfile::tempdir`
/// would create it under `/tmp`, which the Bash read guard treats as an allowed
/// system prefix (Issue #604); the integration temp dir is never a system prefix.
pub fn test_tempdir() -> tempfile::TempDir {
    let base = Path::new(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(base).expect("create CARGO_TARGET_TMPDIR");
    tempfile::tempdir_in(base).expect("tempdir under CARGO_TARGET_TMPDIR")
}

/// Provider client that announces its start and then blocks until released, so
/// the caller can abort an in-flight turn and only release the worker after the
/// caller has returned.
#[derive(Clone)]
pub struct StartReleaseClient {
    label: &'static str,
    started: Arc<Gate>,
    release: Arc<Gate>,
    calls: Arc<AtomicUsize>,
}

const RELEASE_TIMEOUT: Duration = Duration::from_secs(30);

impl StartReleaseClient {
    pub fn new(label: &'static str) -> Self {
        Self {
            label,
            started: Gate::new(),
            release: Gate::new(),
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn started_gate(&self) -> Arc<Gate> {
        self.started.clone()
    }

    pub fn release_gate(&self) -> Arc<Gate> {
        self.release.clone()
    }

    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl ChatClient for StartReleaseClient {
    fn label(&self) -> &str {
        self.label
    }

    fn supports_native_tools(&self, _model: &str) -> bool {
        true
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.started.raise();
        let _ = self.release.wait(RELEASE_TIMEOUT);
        Ok(AssistantReply::text("late"))
    }
}

/// Interaction UI whose interrupt is armed only by the provider turn starting.
pub struct ProviderStartInterruptUi {
    events: Mutex<Vec<String>>,
    started: Arc<Gate>,
}

impl ProviderStartInterruptUi {
    pub fn new(started: Arc<Gate>) -> Self {
        Self {
            events: Mutex::new(Vec::new()),
            started,
        }
    }

    pub fn events(&self) -> Vec<String> {
        self.events.lock().unwrap().clone()
    }
}

impl InteractionUi for ProviderStartInterruptUi {
    fn before_model_call(&self, label: &str) -> UiGuard {
        self.events.lock().unwrap().push(format!("model:{label}"));
        UiGuard::noop()
    }

    fn before_tool_call(&self, name: &str) -> UiGuard {
        self.events.lock().unwrap().push(format!("tool:{name}"));
        UiGuard::noop()
    }

    fn publish_status(&self, status: UiStatus) {
        self.events
            .lock()
            .unwrap()
            .push(format!("status:{}:{}", status.provider, status.model));
    }

    fn interrupted(&self) -> bool {
        self.started.is_raised()
    }
}

/// Shell fixture: ignore TERM, announce ready, keep a process-unique marker in
/// the command line, then loop forever. `trap` is installed before `ready`, so
/// the graceful SIGTERM can only arrive once TERM is already ignored.
pub const SHELL_READY_MARKER: &str = "tui-interrupt-ready";

const SHELL_CHILD_MARKER_PREFIX: &str = "command-agent-620-";

pub fn shell_ready_marker(root: &Path) -> PathBuf {
    root.join(SHELL_READY_MARKER)
}

pub fn shell_child_marker() -> String {
    format!("{SHELL_CHILD_MARKER_PREFIX}{}", std::process::id())
}

pub fn shell_interrupt_command() -> String {
    format!(
        "trap '' TERM; echo ready > {SHELL_READY_MARKER}; : {}; while :; do :; done",
        shell_child_marker()
    )
}

/// Whether the shell child started by [`shell_interrupt_command`] is still
/// alive. The command line carries the process-unique marker; the marker sits
/// early so a truncated `ps` listing still contains it.
pub fn shell_child_gone() -> bool {
    let marker = shell_child_marker();
    let output = std::process::Command::new("ps")
        .args(["-A", "-ww", "-o", "command="])
        .output()
        .expect("ps must run to prove the shell child is gone");
    !String::from_utf8_lossy(&output.stdout).contains(&marker)
}

/// Interaction UI whose first interrupt is armed by the Bash fixture's ready
/// marker and whose force is armed by the first interrupt having been
/// delivered. `before_tool_call` is not treated as shell ready.
pub struct ShellReadyInterruptUi {
    events: Mutex<Vec<String>>,
    ready_marker: PathBuf,
    interrupt_after: Duration,
    force_after: Duration,
    ready_at: Mutex<Option<Instant>>,
    interrupt_at: Mutex<Option<Instant>>,
    interrupt_saw_ready: Mutex<Option<bool>>,
}

impl ShellReadyInterruptUi {
    pub fn new(ready_marker: PathBuf, interrupt_after: Duration, force_after: Duration) -> Self {
        Self {
            events: Mutex::new(Vec::new()),
            ready_marker,
            interrupt_after,
            force_after,
            ready_at: Mutex::new(None),
            interrupt_at: Mutex::new(None),
            interrupt_saw_ready: Mutex::new(None),
        }
    }

    /// When the ready marker was first observed, or `None` if it never was.
    pub fn ready_at(&self) -> Option<Instant> {
        *self.ready_at.lock().unwrap()
    }

    /// Whether the ready marker existed when the first interrupt was returned.
    /// False (also before any interrupt) means the interrupt was not gated by
    /// the shell ready signal.
    pub fn interrupt_saw_ready(&self) -> bool {
        self.interrupt_saw_ready.lock().unwrap().unwrap_or(false)
    }

    pub fn events(&self) -> Vec<String> {
        self.events.lock().unwrap().clone()
    }
}

impl InteractionUi for ShellReadyInterruptUi {
    fn before_model_call(&self, label: &str) -> UiGuard {
        self.events.lock().unwrap().push(format!("model:{label}"));
        UiGuard::noop()
    }

    fn before_tool_call(&self, name: &str) -> UiGuard {
        self.events.lock().unwrap().push(format!("tool:{name}"));
        UiGuard::noop()
    }

    fn publish_status(&self, status: UiStatus) {
        self.events
            .lock()
            .unwrap()
            .push(format!("status:{}:{}", status.provider, status.model));
    }

    fn interrupted(&self) -> bool {
        if !self.ready_marker.is_file() {
            return false;
        }
        let now = Instant::now();
        let armed_at = {
            let mut ready_at = self.ready_at.lock().unwrap();
            *ready_at.get_or_insert(now)
        };
        if now.duration_since(armed_at) < self.interrupt_after {
            return false;
        }
        let mut interrupt_at = self.interrupt_at.lock().unwrap();
        if interrupt_at.is_none() {
            *interrupt_at = Some(now);
            self.interrupt_saw_ready
                .lock()
                .unwrap()
                .get_or_insert(self.ready_marker.is_file());
        }
        true
    }

    fn force_interrupted(&self) -> bool {
        let interrupt_at = *self.interrupt_at.lock().unwrap();
        interrupt_at.is_some_and(|at| Instant::now().duration_since(at) >= self.force_after)
    }
}
