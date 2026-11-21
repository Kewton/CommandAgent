use std::any::Any;
use std::io::Write;
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, PanicHookInfo};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::ThreadId;

use serde_json::{Value, json};

use crate::config::Config;

type PanicHook = Box<dyn Fn(&PanicHookInfo<'_>) + Sync + Send + 'static>;

#[derive(Debug, Clone, PartialEq, Eq)]
struct PanicDiagnostic {
    message: String,
    location: String,
}

#[derive(Debug, Clone)]
struct PanicBoundaryContext {
    workspace_root: PathBuf,
    eval_events_path: Option<PathBuf>,
    action: String,
    profile: String,
    prompt_layout: String,
    reproduction_command: String,
}

impl PanicBoundaryContext {
    fn from_config(config: &Config) -> Self {
        Self {
            workspace_root: config.workspace_root.clone(),
            eval_events_path: config.eval_events_path.clone(),
            action: format!("{:?}", config.action),
            profile: config.profile.clone(),
            prompt_layout: config.prompt_layout.as_str().to_string(),
            reproduction_command: current_reproduction_command(),
        }
    }
}

/// Who may replace the process-wide panic hook. One thread at a time owns it;
/// the owner may nest guards, and every other thread waits until it is free.
struct PanicHookOwner {
    thread: Option<ThreadId>,
    depth: usize,
    #[cfg(test)]
    waiting: Vec<ThreadId>,
}

static PANIC_HOOK_OWNER: Mutex<PanicHookOwner> = Mutex::new(PanicHookOwner {
    thread: None,
    depth: 0,
    #[cfg(test)]
    waiting: Vec::new(),
});
static PANIC_HOOK_OWNER_CHANGED: Condvar = Condvar::new();

fn lock_panic_hook_owner() -> MutexGuard<'static, PanicHookOwner> {
    PANIC_HOOK_OWNER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// One level of panic hook ownership. It is not `Send`, so it is released on
/// the thread that took it.
struct PanicHookLease {
    _same_thread: PhantomData<*const ()>,
}

impl PanicHookLease {
    fn acquire() -> Self {
        let me = std::thread::current().id();
        let mut owner = lock_panic_hook_owner();
        if owner.thread != Some(me) {
            #[cfg(test)]
            {
                owner.waiting.push(me);
                PANIC_HOOK_OWNER_CHANGED.notify_all();
            }
            while owner.thread.is_some() {
                owner = PANIC_HOOK_OWNER_CHANGED
                    .wait(owner)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
            #[cfg(test)]
            owner.waiting.retain(|waiting| *waiting != me);
            owner.thread = Some(me);
        }
        owner.depth += 1;
        Self {
            _same_thread: PhantomData,
        }
    }

    #[cfg(test)]
    fn try_acquire() -> Option<Self> {
        let me = std::thread::current().id();
        let mut owner = lock_panic_hook_owner();
        if owner.thread.is_some_and(|thread| thread != me) {
            return None;
        }
        owner.thread = Some(me);
        owner.depth += 1;
        Some(Self {
            _same_thread: PhantomData,
        })
    }
}

impl Drop for PanicHookLease {
    fn drop(&mut self) {
        let mut owner = lock_panic_hook_owner();
        owner.depth -= 1;
        if owner.depth == 0 {
            owner.thread = None;
            PANIC_HOOK_OWNER_CHANGED.notify_all();
        }
    }
}

/// The only path that replaces the process-wide panic hook (Issue #1012).
///
/// `record` sees every panic while the guard is installed. A panic on the
/// installing thread stops there; a panic on any other thread also reaches the
/// hook that was installed before. Guards from different threads never
/// overlap, guards on one thread nest, and dropping a guard puts back the
/// exact hook it replaced.
pub(crate) struct PanicHookGuard {
    previous: Option<Arc<PanicHook>>,
    _lease: PanicHookLease,
}

impl PanicHookGuard {
    pub(crate) fn install(record: impl Fn(&PanicHookInfo<'_>) + Sync + Send + 'static) -> Self {
        Self::replace(PanicHookLease::acquire(), record)
    }

    #[cfg(test)]
    pub(crate) fn try_install(
        record: impl Fn(&PanicHookInfo<'_>) + Sync + Send + 'static,
    ) -> Option<Self> {
        PanicHookLease::try_acquire().map(|lease| Self::replace(lease, record))
    }

    fn replace(
        lease: PanicHookLease,
        record: impl Fn(&PanicHookInfo<'_>) + Sync + Send + 'static,
    ) -> Self {
        let previous = Arc::new(std::panic::take_hook());
        let previous_for_other_threads = Arc::clone(&previous);
        let owner = std::thread::current().id();
        std::panic::set_hook(Box::new(move |info| {
            record(info);
            if std::thread::current().id() != owner {
                previous_for_other_threads(info);
            }
        }));
        Self {
            previous: Some(previous),
            _lease: lease,
        }
    }
}

impl Drop for PanicHookGuard {
    fn drop(&mut self) {
        // Dropping the installed hook releases its handle on the previous hook,
        // so the previous hook itself goes back, not a wrapper around it.
        drop(std::panic::take_hook());
        if let Some(previous) = self.previous.take() {
            let previous: PanicHook = match Arc::try_unwrap(previous) {
                Ok(previous) => previous,
                Err(shared) => Box::new(move |info| shared(info)),
            };
            std::panic::set_hook(previous);
        }
    }
}

#[cfg(test)]
fn panic_hook_address(hook: &PanicHook) -> *const () {
    let hook: &(dyn Fn(&PanicHookInfo<'_>) + Sync + Send + 'static) = &**hook;
    (hook as *const (dyn Fn(&PanicHookInfo<'_>) + Sync + Send + 'static)).cast::<()>()
}

/// Test-only: owns the panic hook for the whole test, installs a sentinel that
/// keeps the message of every panic it sees, and puts the original hook back on
/// drop. Other tests' threads can panic at any time, so a test asks whether the
/// sentinel saw its own message instead of counting calls.
#[cfg(test)]
pub(crate) struct TestPanicHookSentinel {
    seen: Arc<Mutex<Vec<String>>>,
    address: *const (),
    original: Option<PanicHook>,
    _lease: PanicHookLease,
}

#[cfg(test)]
impl TestPanicHookSentinel {
    pub(crate) fn install() -> Self {
        let lease = PanicHookLease::acquire();
        let original = std::panic::take_hook();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        let sentinel: PanicHook = Box::new(move |info| {
            sink.lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(panic_payload_message(info.payload()));
        });
        let address = panic_hook_address(&sentinel);
        std::panic::set_hook(sentinel);
        Self {
            seen,
            address,
            original: Some(original),
            _lease: lease,
        }
    }

    /// How many panics with exactly this message reached the sentinel.
    pub(crate) fn seen(&self, message: &str) -> usize {
        self.seen
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .filter(|seen| *seen == message)
            .count()
    }

    /// True when the installed hook is the sentinel itself, not a wrapper.
    pub(crate) fn is_installed(&self) -> bool {
        let current = std::panic::take_hook();
        let installed = panic_hook_address(&current) == self.address;
        std::panic::set_hook(current);
        installed
    }
}

#[cfg(test)]
impl Drop for TestPanicHookSentinel {
    fn drop(&mut self) {
        // The hook cannot be replaced from a panicking thread; a failed assertion
        // must report as a failure, not abort the test binary.
        if std::thread::panicking() {
            return;
        }
        if let Some(original) = self.original.take() {
            std::panic::set_hook(original);
        }
    }
}

/// Test-only: blocks until `thread` is waiting to own the panic hook.
#[cfg(test)]
pub(crate) fn wait_until_waiting_for_panic_hook(thread: ThreadId) {
    let mut owner = lock_panic_hook_owner();
    while !owner.waiting.contains(&thread) {
        owner = PANIC_HOOK_OWNER_CHANGED
            .wait(owner)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
    }
}

fn install_panic_hook(diagnostics: Arc<Mutex<Vec<PanicDiagnostic>>>) -> PanicHookGuard {
    #[cfg(test)]
    TEST_PANIC_HOOK_INSTALL_COUNT.with(|count| count.set(count.get() + 1));
    PanicHookGuard::install(move |info| {
        // Provider workers can resume their panic on the CLI thread, so retain a bounded
        // set and later match the caught payload instead of assuming one panic thread.
        let mut diagnostics = diagnostics
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if diagnostics.len() == 16 {
            diagnostics.remove(0);
        }
        diagnostics.push(PanicDiagnostic::from_hook(info));
    })
}

impl PanicDiagnostic {
    fn from_hook(info: &PanicHookInfo<'_>) -> Self {
        let location = info
            .location()
            .map(|location| {
                format!(
                    "{}:{}:{}",
                    location.file(),
                    location.line(),
                    location.column()
                )
            })
            .unwrap_or_else(|| "unknown".to_string());
        Self {
            message: panic_payload_message(info.payload()),
            location,
        }
    }

    fn from_payload(payload: &(dyn Any + Send)) -> Self {
        Self {
            message: panic_payload_message(payload),
            location: "unknown".to_string(),
        }
    }
}

#[derive(Debug, Default)]
struct RecoveryNoteOutcome {
    relative_path: String,
    error: String,
}

pub(crate) fn catch_cli_run<F>(config: &Config, operation: F) -> anyhow::Result<()>
where
    F: FnOnce() -> anyhow::Result<()>,
{
    catch_with_context(PanicBoundaryContext::from_config(config), operation)
}

fn catch_with_context<F>(context: PanicBoundaryContext, operation: F) -> anyhow::Result<()>
where
    F: FnOnce() -> anyhow::Result<()>,
{
    let captured = Arc::new(Mutex::new(Vec::new()));
    let hook_guard = install_panic_hook(Arc::clone(&captured));
    let outcome = std::panic::catch_unwind(AssertUnwindSafe(operation));
    if let Err(payload) = std::panic::catch_unwind(AssertUnwindSafe(|| drop(hook_guard))) {
        stderr_line(&format!(
            "internal panic boundary: panic while restoring panic hook: {}",
            panic_payload_message(payload.as_ref())
        ));
    }

    match outcome {
        Ok(result) => result,
        Err(payload) => {
            let payload_message = panic_payload_message(payload.as_ref());
            let diagnostic = captured
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .iter()
                .rev()
                .find(|diagnostic| diagnostic.message == payload_message)
                .cloned()
                .or_else(|| nested_panic_diagnostic(context.eval_events_path.as_deref()))
                .unwrap_or_else(|| PanicDiagnostic::from_payload(payload.as_ref()));
            let recovery = finalize_internal_panic(&context, &diagnostic);
            let recovery_suffix = if recovery.relative_path.is_empty() {
                String::new()
            } else {
                format!("; recovery note: {}", recovery.relative_path)
            };
            Err(anyhow::anyhow!(
                "internal_panic: {} at {}{}",
                diagnostic.message,
                diagnostic.location,
                recovery_suffix
            ))
        }
    }
}

fn finalize_internal_panic(
    context: &PanicBoundaryContext,
    diagnostic: &PanicDiagnostic,
) -> RecoveryNoteOutcome {
    stderr_line(&format!(
        "internal panic caught: {} at {}",
        diagnostic.message, diagnostic.location
    ));

    let recovery = save_recovery_note_safely(context, diagnostic);
    run_finalizer_step("reaping registered server children", || {
        crate::bounded_process::reap_registered_server_children_for_workspace(
            context.eval_events_path.as_deref(),
            "internal_panic",
            &context.workspace_root,
        );
        Ok(())
    });
    run_finalizer_step("writing internal panic terminal event", || {
        crate::eval_events::append_event_failsafe(
            context.eval_events_path.as_deref(),
            internal_panic_stop_event(context, diagnostic, &recovery),
        )
    });
    run_finalizer_step("writing internal panic run summary", || {
        write_internal_panic_summary(context, diagnostic, &recovery);
        Ok(())
    });
    recovery
}

fn save_recovery_note_safely(
    context: &PanicBoundaryContext,
    diagnostic: &PanicDiagnostic,
) -> RecoveryNoteOutcome {
    match std::panic::catch_unwind(AssertUnwindSafe(|| save_recovery_note(context, diagnostic))) {
        Ok(Ok(path)) => RecoveryNoteOutcome {
            relative_path: display_workspace_path(&context.workspace_root, &path),
            error: String::new(),
        },
        Ok(Err(err)) => {
            stderr_line(&format!(
                "internal panic boundary: failed to write recovery note: {err:#}"
            ));
            RecoveryNoteOutcome {
                relative_path: String::new(),
                error: err.to_string(),
            }
        }
        Err(payload) => {
            let error = format!(
                "panic while writing recovery note: {}",
                panic_payload_message(payload.as_ref())
            );
            stderr_line(&format!("internal panic boundary: {error}"));
            RecoveryNoteOutcome {
                relative_path: String::new(),
                error,
            }
        }
    }
}

fn save_recovery_note(
    context: &PanicBoundaryContext,
    diagnostic: &PanicDiagnostic,
) -> anyhow::Result<PathBuf> {
    let dir = crate::runtime_paths::repairs_dir(&context.workspace_root);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("repair-internal-panic-{}.md", uuid::Uuid::now_v7()));
    let message = crate::eval_events::render_stop_reason_text(&diagnostic.message);
    let command_is_redacted = scrub_note(&context.workspace_root, &context.reproduction_command)
        != context.reproduction_command;
    let mut note = format!(
        "# Internal panic recovery note\n\n\
Status: failed\n\
Reason: internal_panic\n\
Panic message: {message}\n\
Panic location: {}\n\n\
Reproduction command:\n\n    {}\n",
        diagnostic.location, context.reproduction_command
    );
    if command_is_redacted {
        note.push_str(
            "\nRedaction note: the reproduction command above was redacted and cannot be re-run as written.\n",
        );
    }
    let note = scrub_note(&context.workspace_root, &note);
    std::fs::write(&path, note)?;
    Ok(path)
}

/// Scrub a persisted panic recovery note with the run scope registered for the
/// workspace, so the panic message, location, and reproduction command never
/// write a registered secret value to disk.
fn scrub_note(root: &Path, text: &str) -> String {
    crate::sensitive_data::active_for(Some(root))
        .map(|context| context.scrub_text(text))
        .unwrap_or_else(|| text.to_string())
}

fn internal_panic_stop_event(
    context: &PanicBoundaryContext,
    diagnostic: &PanicDiagnostic,
    recovery: &RecoveryNoteOutcome,
) -> Value {
    // A direct-command drop may already have emitted its terminal projection during unwind.
    // Mirror those fields so the process-level stop remains compatible while naming the panic.
    let terminal =
        crate::eval_events::latest_tui_command_stop_event(context.eval_events_path.as_deref())
            .filter(|event| event.get("ok").and_then(Value::as_bool) == Some(false));
    let completion_status = terminal_field(&terminal, "completion_status", "failed");
    let task_status = terminal_field(&terminal, "task_status", "failed");
    let assurance_level = terminal_field(&terminal, "assurance_level", "partial");
    let assurance_reason = terminal_field(&terminal, "assurance_reason", "internal_panic");
    let effective_profile = terminal_field(&terminal, "effective_profile", &context.profile);
    let contract_origin = terminal_field(&terminal, "contract_origin", "initial");
    let runtime_acceptance = terminal_field(&terminal, "runtime_acceptance_status", "not_checked");
    let final_acceptance = terminal_field(&terminal, "final_acceptance_status", "not_checked");
    let release_gate = terminal_field(&terminal, "release_gate_status", "not_applicable");
    let release_quality = terminal_field(&terminal, "release_quality_completion", "incomplete");
    let next_action = terminal_field(&terminal, "next_action", "inspect_internal_panic");
    let panic_message = crate::eval_events::body_snippet(&diagnostic.message);

    json!({
        "event": "run_stop",
        "ok": false,
        "lifecycle_stage": "process",
        "action": context.action,
        "reason": "internal_panic",
        "stop_reason": "internal_panic",
        "failure_kind": "internal_panic",
        "status": "failed",
        "completion_status": completion_status,
        "task_status": task_status,
        "profile": context.profile,
        "effective_profile": effective_profile,
        "prompt_layout": context.prompt_layout,
        "contract_origin": contract_origin,
        "assurance_level": assurance_level,
        "assurance_reason": assurance_reason,
        "session_status": "process_exited",
        "repl_status": "not_applicable",
        "process_completion_state": "failed",
        "command_completion_state": "failed",
        "runtime_acceptance_status": runtime_acceptance,
        "final_acceptance_status": final_acceptance,
        "release_gate_status": release_gate,
        "release_quality_completion": release_quality,
        "next_action": next_action,
        "recovery_next_action": "inspect_internal_panic",
        "recovery_prompt_path": recovery.relative_path,
        "recovery_note_path": recovery.relative_path,
        "recovery_note_error": recovery.error,
        "recovery_ultra_plan_path": "",
        "suggested_recovery_command": context.reproduction_command,
        "suggested_recovery_yaml_command": "",
        "panic_message": panic_message,
        "panic_location": diagnostic.location,
        "build_commit": crate::build_info::COMMIT,
        "build_dirty": crate::build_info::dirty(),
        "build_timestamp": crate::build_info::TIMESTAMP,
    })
}

fn write_internal_panic_summary(
    context: &PanicBoundaryContext,
    diagnostic: &PanicDiagnostic,
    recovery: &RecoveryNoteOutcome,
) {
    let recovery_line = if recovery.relative_path.is_empty() {
        format!("Recovery note: unavailable ({})", recovery.error)
    } else {
        format!("Recovery note: {}", recovery.relative_path)
    };
    crate::eval_events::write_run_summary(
        context.eval_events_path.as_deref(),
        &format!(
            "Status: failed\n\
Lifecycle stage: process\n\
Action: {}\n\
Failure kind: internal_panic\n\
Stop reason: internal_panic\n\
Panic message: {}\n\
Panic location: {}\n\
{}\n\
Reproduction command: {}",
            context.action,
            crate::eval_events::body_snippet(&diagnostic.message),
            diagnostic.location,
            recovery_line,
            context.reproduction_command
        ),
    );
}

fn run_finalizer_step(label: &str, operation: impl FnOnce() -> anyhow::Result<()>) {
    match std::panic::catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(())) => {}
        Ok(Err(err)) => stderr_line(&format!(
            "internal panic boundary: failed while {label}: {err:#}"
        )),
        Err(payload) => stderr_line(&format!(
            "internal panic boundary: panic while {label}: {}",
            panic_payload_message(payload.as_ref())
        )),
    }
}

fn terminal_field(terminal: &Option<Value>, key: &str, fallback: &str) -> String {
    terminal
        .as_ref()
        .and_then(|event| event.get(key))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn nested_panic_diagnostic(path: Option<&Path>) -> Option<PanicDiagnostic> {
    let text = std::fs::read_to_string(path?).ok()?;
    text.lines().rev().find_map(|line| {
        let event = serde_json::from_str::<Value>(line).ok()?;
        if event.get("event").and_then(Value::as_str) != Some("panic_caught") {
            return None;
        }
        Some(PanicDiagnostic {
            message: event
                .get("panic_message")
                .or_else(|| event.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("non-string panic payload")
                .to_string(),
            location: event
                .get("panic_location")
                .or_else(|| event.get("location"))
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string(),
        })
    })
}

fn current_reproduction_command() -> String {
    let command = std::env::args_os()
        .map(|argument| shell_quote(&argument.to_string_lossy()))
        .collect::<Vec<_>>()
        .join(" ");
    if command.is_empty() {
        "commandagent".to_string()
    } else {
        command
    }
}

fn shell_quote(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || "_-./:=+,".contains(ch))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }
}

fn display_workspace_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn panic_payload_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

fn stderr_line(message: &str) {
    let _ = writeln!(std::io::stderr().lock(), "{message}");
}

pub(crate) fn report_secondary_panic(label: &str, payload: Box<dyn Any + Send>) {
    stderr_line(&format!(
        "internal panic boundary: panic while {label}: {}",
        panic_payload_message(payload.as_ref())
    ));
}

#[cfg(test)]
thread_local! {
    static TEST_FAULT_REQUESTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static TEST_FINALIZER_FAULT_REQUESTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static TEST_PANIC_HOOK_INSTALL_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_test_panic_hook_install_count() {
    TEST_PANIC_HOOK_INSTALL_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn test_panic_hook_install_count() -> usize {
    TEST_PANIC_HOOK_INSTALL_COUNT.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(crate) fn request_test_fault() {
    TEST_FAULT_REQUESTED.with(|requested| requested.set(true));
}

#[cfg(test)]
pub(crate) fn inject_test_fault_if_requested() {
    let requested = TEST_FAULT_REQUESTED.with(|requested| requested.replace(false));
    if requested {
        panic!("fault injection: CLI run panic boundary");
    }
}

#[cfg(test)]
pub(crate) fn request_test_finalizer_fault() {
    TEST_FINALIZER_FAULT_REQUESTED.with(|requested| requested.set(true));
}

#[cfg(test)]
pub(crate) fn inject_test_finalizer_fault_if_requested() {
    let requested = TEST_FINALIZER_FAULT_REQUESTED.with(|requested| requested.replace(false));
    if requested {
        panic!("fault injection: direct completion finalizer panic");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(root: &Path, events: &Path) -> PanicBoundaryContext {
        PanicBoundaryContext {
            workspace_root: root.to_path_buf(),
            eval_events_path: Some(events.to_path_buf()),
            action: "UltraPlanRun(\"fault injection\")".to_string(),
            profile: "nextjs".to_string(),
            prompt_layout: "legacy".to_string(),
            reproduction_command: "commandagent --ultra-plan-run 'fault injection'".to_string(),
        }
    }

    fn read_events(path: &Path) -> Vec<Value> {
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn fault_injection_writes_terminal_event_and_recovery_note() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join(".anvil/runs/fault/events.jsonl");
        request_test_fault();

        let result = catch_with_context(context(dir.path(), &events), || {
            inject_test_fault_if_requested();
            Ok(())
        });

        let err = result.expect_err("fault injection must fail");
        assert!(err.to_string().contains("internal_panic"), "{err:#}");
        let events = read_events(&events);
        let stop = events.last().unwrap();
        assert_eq!(stop.get("event").and_then(Value::as_str), Some("run_stop"));
        assert_eq!(
            stop.get("reason").and_then(Value::as_str),
            Some("internal_panic")
        );
        assert_eq!(stop.get("ok").and_then(Value::as_bool), Some(false));
        assert_eq!(
            stop.get("panic_message").and_then(Value::as_str),
            Some("fault injection: CLI run panic boundary")
        );
        assert!(
            stop.get("panic_location")
                .and_then(Value::as_str)
                .is_some_and(|location| location.contains("cli_panic_boundary.rs")),
            "{stop:#}"
        );
        let recovery_path = stop
            .get("recovery_note_path")
            .and_then(Value::as_str)
            .unwrap();
        assert!(recovery_path.starts_with(".commandagent/repairs/repair-internal-panic-"));
        let note = std::fs::read_to_string(dir.path().join(recovery_path)).unwrap();
        assert!(note.contains("Reason: internal_panic"), "{note}");
        assert!(
            note.contains("Panic message: fault injection: CLI run panic boundary"),
            "{note}"
        );
        assert!(note.contains("Panic location:"), "{note}");
        assert!(
            note.contains("commandagent --ultra-plan-run 'fault injection'"),
            "{note}"
        );
    }

    #[test]
    fn resumed_worker_panic_keeps_original_location() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join(".anvil/runs/worker-fault/events.jsonl");

        let result = catch_with_context(context(dir.path(), &events), || {
            let payload = std::thread::spawn(|| {
                panic!("fault injection: resumed worker panic");
            })
            .join()
            .expect_err("worker must panic");
            std::panic::resume_unwind(payload)
        });

        assert!(result.unwrap_err().to_string().contains("internal_panic"));
        let events = read_events(&events);
        let stop = events.last().unwrap();
        assert_eq!(
            stop.get("panic_message").and_then(Value::as_str),
            Some("fault injection: resumed worker panic")
        );
        assert!(
            stop.get("panic_location")
                .and_then(Value::as_str)
                .is_some_and(|location| location.contains("cli_panic_boundary.rs")),
            "{stop:#}"
        );
        assert_ne!(
            stop.get("panic_location").and_then(Value::as_str),
            Some("unknown")
        );
    }

    #[test]
    fn normal_result_does_not_add_terminal_event_or_recovery_note() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join(".anvil/runs/normal/events.jsonl");
        let event_path = events.clone();

        let result = catch_with_context(context(dir.path(), &events), || {
            crate::eval_events::emit(
                Some(&event_path),
                json!({"event": "normal_marker", "status": "completed"}),
            );
            Ok(())
        });

        result.unwrap();
        let events = read_events(&events);
        assert_eq!(events.len(), 1, "{events:#?}");
        assert_eq!(
            events[0].get("event").and_then(Value::as_str),
            Some("normal_marker")
        );
        assert!(!dir.path().join(".commandagent/repairs").exists());
    }

    #[test]
    fn recovery_note_and_terminal_event_scrub_a_registered_secret() {
        // The panic boundary module is private to the crate, so the note/model
        // boundary is exercised here rather than from an integration test. Only
        // the thread-local scope is set, so this never disturbs a sibling lib
        // test's process-global scope registry.
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join(".commandagent/runs/secret/events.jsonl");
        let canary = "H01_CANARY_JwtStyle_NonPrefix_29486";
        let mut catalog = crate::sensitive_data::SecretCatalog::new();
        catalog.register(canary);
        crate::sensitive_data::set_current(Some(
            crate::sensitive_data::RedactionContext::from_catalog(catalog),
        ));
        let mut context = context(dir.path(), &events);
        context.reproduction_command = format!("commandagent --ultra-plan-run '{canary}'");

        let result = catch_with_context(context, || {
            panic!("panic carrying {canary}");
        });
        assert!(result.unwrap_err().to_string().contains("internal_panic"));

        let events = read_events(&events);
        let stop = events.last().unwrap();
        let relative = stop
            .get("recovery_note_path")
            .and_then(Value::as_str)
            .unwrap();
        assert!(!relative.is_empty(), "{stop:#}");
        let note = std::fs::read_to_string(dir.path().join(relative)).unwrap();
        assert!(!note.contains(canary), "recovery note leaked: {note}");
        assert!(note.contains("<redacted>"), "{note}");
        assert!(!note.contains("H01_CANARY"), "{note}");
        let rendered = format!("{stop:#}");
        assert!(
            !rendered.contains(canary),
            "terminal event leaked: {rendered}"
        );
        assert!(
            !std::fs::read_to_string(dir.path().join(".commandagent/runs/secret/summary.md"))
                .unwrap_or_default()
                .contains(canary)
        );
        crate::sensitive_data::set_current(None);
    }

    fn read_recovery_note(root: &Path) -> String {
        std::fs::read_dir(root.join(".commandagent/repairs"))
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| std::fs::read_to_string(entry.path()).unwrap())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn r20_panic_note_is_scrubbed_on_a_thread_without_a_current_scope() {
        // The scope registry is process-global, so run this in a child process:
        // a concurrent `reset_scopes_for_tests` in a sibling lib test would
        // otherwise clear this test's registered scope.
        let exe = std::env::current_exe().unwrap();
        let status = std::process::Command::new(exe)
            .args(["--ignored", "--nocapture", "r20_scope_registry_child"])
            .status()
            .unwrap();
        assert!(status.success(), "r20 child exited with {status}");
    }

    #[test]
    #[ignore]
    fn r20_scope_registry_child() {
        // The panicking thread installs no thread-local scope; the note is still
        // scrubbed because the boundary resolves the workspace scope by path.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let events = root.join(".commandagent/runs/thread/events.jsonl");
        let canary = "H01_CANARY_JwtStyle_NonPrefix_29486";
        let mut catalog = crate::sensitive_data::SecretCatalog::new();
        catalog.register(canary);
        crate::sensitive_data::install_scope(catalog, Some(&root), Some(&events));

        let thread_root = root.clone();
        let thread_events = events.clone();
        let handle = std::thread::spawn(move || {
            crate::sensitive_data::set_current(None);
            let mut ctx = context(&thread_root, &thread_events);
            ctx.reproduction_command = format!("commandagent --token {canary}");
            let result = catch_with_context(ctx, || panic!("thread panic {canary}"));
            assert!(result.unwrap_err().to_string().contains("internal_panic"));
            let note = read_recovery_note(&thread_root);
            assert!(!note.contains(canary), "{note}");
        });
        handle.join().unwrap();
    }

    #[test]
    fn r21_multibyte_panic_scrub_does_not_double_panic() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join(".commandagent/runs/mb/events.jsonl");
        let canary = "H09_日本語_Canary_秘密値_5521";
        let mut catalog = crate::sensitive_data::SecretCatalog::new();
        catalog.register(canary);
        crate::sensitive_data::set_current(Some(
            crate::sensitive_data::RedactionContext::from_catalog(catalog),
        ));

        let mut ctx = context(dir.path(), &events);
        ctx.reproduction_command = format!("commandagent --token {canary}");
        let result = catch_with_context(ctx, || panic!("multibyte {canary}"));
        assert!(result.unwrap_err().to_string().contains("internal_panic"));
        let note = read_recovery_note(dir.path());
        assert!(!note.contains(canary), "{note}");
        assert!(!note.contains("日本語"), "{note}");
        crate::sensitive_data::set_current(None);
    }

    #[test]
    fn r22_redacted_reproduction_command_is_marked_not_rerunnable() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join(".commandagent/runs/rerun/events.jsonl");
        let canary = "H01_CANARY_JwtStyle_NonPrefix_29486";
        let mut catalog = crate::sensitive_data::SecretCatalog::new();
        catalog.register(canary);
        crate::sensitive_data::set_current(Some(
            crate::sensitive_data::RedactionContext::from_catalog(catalog),
        ));

        let mut ctx = context(dir.path(), &events);
        ctx.reproduction_command = format!("commandagent --ultra-plan-run '{canary}'");
        let result = catch_with_context(ctx, || panic!("boom"));
        assert!(result.unwrap_err().to_string().contains("internal_panic"));
        let note = read_recovery_note(dir.path());
        assert!(!note.contains(canary), "{note}");
        assert!(
            note.contains("cannot be re-run as written"),
            "the redacted command must be marked not re-runnable: {note}"
        );
        crate::sensitive_data::set_current(None);
    }

    #[test]
    fn recovery_note_io_failure_is_recorded_without_second_unwind() {
        let dir = tempfile::tempdir().unwrap();
        let workspace_file = dir.path().join("workspace-file");
        std::fs::write(&workspace_file, "not a directory").unwrap();
        let events = dir.path().join("events.jsonl");
        request_test_fault();

        let result = catch_with_context(context(&workspace_file, &events), || {
            inject_test_fault_if_requested();
            Ok(())
        });

        assert!(result.unwrap_err().to_string().contains("internal_panic"));
        let events = read_events(&events);
        let stop = events.last().unwrap();
        assert_eq!(
            stop.get("recovery_note_path").and_then(Value::as_str),
            Some("")
        );
        assert!(
            stop.get("recovery_note_error")
                .and_then(Value::as_str)
                .is_some_and(|error| !error.is_empty()),
            "{stop:#}"
        );
    }

    #[test]
    fn overlapping_slash_guard_cannot_take_the_boundary_owner_panic() {
        // Forces the order from Issue #1012: the boundary guard is installed, a slash
        // guard on another thread tries to install while it is held, then the boundary
        // owner thread panics. The boundary must still record the panic location.
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join(".anvil/runs/overlap/events.jsonl");
        let (go_tx, go_rx) = std::sync::mpsc::channel::<()>();
        let (tried_tx, tried_rx) = std::sync::mpsc::channel::<bool>();
        let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
        let slash = std::thread::spawn(move || {
            go_rx.recv().unwrap();
            let guard = crate::tui::slash::try_install_test_panic_hook();
            let overlapped = guard.is_some();
            tried_tx.send(overlapped).unwrap();
            done_rx.recv().unwrap();
            drop(guard);
            overlapped
        });
        request_test_fault();

        let result = catch_with_context(context(dir.path(), &events), || {
            go_tx.send(()).unwrap();
            tried_rx.recv().unwrap();
            inject_test_fault_if_requested();
            Ok(())
        });
        done_tx.send(()).unwrap();
        let overlapped = slash.join().unwrap();

        assert!(result.unwrap_err().to_string().contains("internal_panic"));
        let events = read_events(&events);
        let stop = events.last().unwrap();
        assert_eq!(
            stop.get("panic_message").and_then(Value::as_str),
            Some("fault injection: CLI run panic boundary")
        );
        assert!(
            stop.get("panic_location")
                .and_then(Value::as_str)
                .is_some_and(|location| location.contains("cli_panic_boundary.rs")),
            "{stop:#}"
        );
        assert!(
            !overlapped,
            "a slash panic hook must not be installed while the boundary hook is held"
        );
    }

    #[test]
    fn slash_guard_on_another_thread_waits_until_the_boundary_guard_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join(".anvil/runs/wait/events.jsonl");
        let (go_tx, go_rx) = std::sync::mpsc::channel::<()>();
        let (installed_tx, installed_rx) = std::sync::mpsc::channel::<()>();
        let slash = std::thread::spawn(move || {
            go_rx.recv().unwrap();
            let guard = crate::tui::slash::install_test_panic_hook();
            installed_tx.send(()).unwrap();
            drop(guard);
        });
        let slash_thread = slash.thread().id();
        let mut installed_while_held = None;
        request_test_fault();

        let result = catch_with_context(context(dir.path(), &events), || {
            go_tx.send(()).unwrap();
            wait_until_waiting_for_panic_hook(slash_thread);
            installed_while_held = Some(installed_rx.try_recv());
            inject_test_fault_if_requested();
            Ok(())
        });
        installed_rx.recv().unwrap();
        slash.join().unwrap();

        assert_eq!(
            installed_while_held,
            Some(Err(std::sync::mpsc::TryRecvError::Empty)),
            "the slash guard must not be installed while the boundary guard is held"
        );
        assert!(result.unwrap_err().to_string().contains("internal_panic"));
        let events = read_events(&events);
        let stop = events.last().unwrap();
        assert!(
            stop.get("panic_location")
                .and_then(Value::as_str)
                .is_some_and(|location| location.contains("cli_panic_boundary.rs")),
            "{stop:#}"
        );
    }

    #[test]
    fn nested_guards_restore_the_exact_previous_hook() {
        let sentinel = TestPanicHookSentinel::install();
        let captured = Arc::new(Mutex::new(Vec::new()));

        // Assertions wait until the guards are gone: a guard cannot be removed
        // from a panicking thread.
        let boundary = install_panic_hook(Arc::clone(&captured));
        let replaced_by_boundary = !sentinel.is_installed();
        let slash = crate::tui::slash::install_test_panic_hook();
        let replaced_by_slash = !sentinel.is_installed();
        drop(slash);
        let still_replaced_by_boundary = !sentinel.is_installed();
        drop(boundary);

        assert!(replaced_by_boundary);
        assert!(replaced_by_slash);
        assert!(still_replaced_by_boundary);
        assert!(sentinel.is_installed());
        let _ = std::thread::spawn(|| panic!("nested guards: after the guards are removed")).join();
        assert_eq!(
            sentinel.seen("nested guards: after the guards are removed"),
            1
        );
        assert!(
            !captured
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .iter()
                .any(|diagnostic| diagnostic.message.starts_with("nested guards:"))
        );
    }

    #[test]
    fn other_thread_panic_reaches_the_previous_hook_and_owner_panic_does_not() {
        let sentinel = TestPanicHookSentinel::install();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let guard = install_panic_hook(Arc::clone(&captured));

        let _ = std::thread::spawn(|| panic!("boundary other thread panic")).join();
        let owner = std::panic::catch_unwind(|| panic!("boundary owner thread panic"));
        drop(guard);

        assert!(owner.is_err());
        assert_eq!(sentinel.seen("boundary other thread panic"), 1);
        assert_eq!(sentinel.seen("boundary owner thread panic"), 0);

        // Other tests' threads may panic while the guard is installed; keep only
        // this test's panics.
        let captured = captured
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .filter(|diagnostic| diagnostic.message.starts_with("boundary "))
            .cloned()
            .collect::<Vec<_>>();
        let messages = captured
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            messages,
            ["boundary other thread panic", "boundary owner thread panic"]
        );
        assert!(
            captured
                .iter()
                .all(|diagnostic| diagnostic.location.contains("cli_panic_boundary.rs")),
            "{captured:?}"
        );
        assert!(sentinel.is_installed());
    }

    #[test]
    fn slash_guard_nested_inside_the_boundary_guard_on_one_thread_returns() {
        // The REPL installs the slash guard inside the CLI boundary guard on the
        // same thread (lib.rs catch_cli_run -> repl -> slash::handle_command). A
        // shared lock that cannot be re-entered would deadlock here, so the run is
        // on its own thread and a bounded wait turns a stop into a failure.
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let dir = tempfile::tempdir().unwrap();
            let events = dir.path().join(".anvil/runs/nested/events.jsonl");
            let sentinel = TestPanicHookSentinel::install();
            let mut slash_record = None;
            let mut inner_caught = false;
            let result = catch_with_context(context(dir.path(), &events), || {
                let (slash, read) =
                    crate::tui::slash::install_test_panic_hook_with_record("nested on one thread:");
                let inner =
                    std::panic::catch_unwind(|| panic!("nested on one thread: slash command"));
                inner_caught = inner.is_err();
                drop(slash);
                slash_record = read();
                panic!("nested on one thread: boundary run");
            });
            let stop = read_events(&events).last().cloned().unwrap();
            let restored = sentinel.is_installed();
            let forwarded = sentinel.seen("nested on one thread: slash command")
                + sentinel.seen("nested on one thread: boundary run");
            drop(sentinel);
            done_tx
                .send((
                    result.map_err(|err| err.to_string()),
                    stop,
                    (inner_caught, slash_record),
                    restored,
                    forwarded,
                ))
                .unwrap();
        });

        let (result, stop, (inner_caught, slash_record), restored, forwarded) = done_rx
            .recv_timeout(std::time::Duration::from_secs(60))
            .expect("nesting the slash guard inside the boundary guard must not stop");

        assert!(inner_caught);
        assert!(result.unwrap_err().contains("internal_panic"));
        let (message, location) = slash_record.expect("the slash hook records its panic");
        assert_eq!(message, "nested on one thread: slash command");
        assert!(location.contains("cli_panic_boundary.rs"), "{location}");
        assert_eq!(
            stop.get("panic_message").and_then(Value::as_str),
            Some("nested on one thread: boundary run")
        );
        assert!(
            stop.get("panic_location")
                .and_then(Value::as_str)
                .is_some_and(|location| location.contains("cli_panic_boundary.rs")),
            "{stop:#}"
        );
        assert!(restored, "the hook from before the guards must be back");
        assert_eq!(forwarded, 0, "owner-thread panics stay with the guards");
    }
}
