//! cfg(test) transport for the fake dev-server fixtures (Issue #608). A marked
//! workspace makes the fake dev child bind an OS-assigned port (`PORT` is
//! ignored) and announce the real port and generation through a ready file, so
//! a bound number is never freed and re-bound; the logical port in the goal,
//! contract, and evidence is unchanged. The browser readiness probe (a separate
//! path) opts in through its own `dynamic_port` override. `#[cfg(test)]`-only.

use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

const READY_FILE_RELATIVE: &str = ".anvil/evidence/dev-server-ready-port.json";
const MARKER_FILE_RELATIVE: &str = ".anvil/evidence/dev-server-test-transport";
const GENERATION_FILE_RELATIVE: &str = ".anvil/evidence/dev-server-probe-generation";
const BROWSER_COMMAND_RELATIVE: &str = ".anvil/evidence/browser-probe-command.json";
const BROWSER_MOCK_PORT_RELATIVE: &str = ".anvil/evidence/browser-probe-mock-port.txt";

static PROBE_GENERATION: AtomicU64 = AtomicU64::new(0);

fn write_evidence(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, content);
}

/// Mark `root` so every fake dev-server fixture in it uses the OS-assigned
/// transport. Read by the parent probe and the fake child (via its cwd).
pub(crate) fn enable(root: &Path) {
    write_evidence(root, MARKER_FILE_RELATIVE, "1");
}

pub(crate) fn enabled(root: &Path) -> bool {
    root.join(MARKER_FILE_RELATIVE).is_file()
}

/// Opt the browser readiness probe into the same transport: it runs the fake
/// dev child through the workspace package manager with `dynamic_port`, so the
/// child announces its OS-assigned port through the browser probe's ready file
/// instead of re-binding a number the fixture released.
pub(crate) fn enable_browser_probe_transport(root: &Path, port: u16) {
    enable(root);
    let command = json!({
        "program": "npm",
        "args": ["run", "start"],
        "port": port,
        "require_build": true,
        "dynamic_port": true,
        "display": "fake dev server (browser transport)",
    });
    write_evidence(root, BROWSER_COMMAND_RELATIVE, &command.to_string());
}

/// One dev-server probe's transport state. `begin` clears the previous
/// generation's ready file, records the fresh generation where the child reads
/// it, and remembers it for the parent's poll. A stale ready file from an
/// earlier repair/reprobe cycle therefore can never supply the real port.
pub(crate) struct Probe {
    dynamic: bool,
    generation: u64,
}

pub(crate) fn begin(root: &Path, dynamic: bool) -> Probe {
    let generation = if dynamic {
        let generation = PROBE_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
        let _ = std::fs::remove_file(root.join(READY_FILE_RELATIVE));
        write_evidence(root, GENERATION_FILE_RELATIVE, &generation.to_string());
        generation
    } else {
        0
    };
    Probe {
        dynamic,
        generation,
    }
}

impl Probe {
    /// The notified real port when the transport is on and the child has
    /// announced it, otherwise the caller's logical port. The dynamic child
    /// never binds the logical port, so a missing notification keeps the probe
    /// polling until the child announces or the deadline expires.
    pub(crate) fn poll_port(&self, root: &Path, logical: u16) -> u16 {
        if self.dynamic {
            read_ready(root, self.generation).unwrap_or(logical)
        } else {
            logical
        }
    }
}

fn read_generation(root: &Path) -> u64 {
    std::fs::read_to_string(root.join(GENERATION_FILE_RELATIVE))
        .ok()
        .and_then(|text| text.trim().parse::<u64>().ok())
        .unwrap_or(0)
}

fn read_ready(root: &Path, generation: u64) -> Option<u16> {
    let text = std::fs::read_to_string(root.join(READY_FILE_RELATIVE)).ok()?;
    let value = serde_json::from_str::<Value>(text.trim()).ok()?;
    if value.get("generation").and_then(Value::as_u64) != Some(generation) {
        return None;
    }
    let port = value.get("port").and_then(Value::as_u64)?;
    u16::try_from(port).ok().filter(|port| *port != 0)
}

fn publish_ready(root: &Path, generation: u64, port: u16) {
    let ready = json!({"generation": generation, "port": port}).to_string();
    write_evidence(root, READY_FILE_RELATIVE, &ready);
}

/// Bind the fake dev server listener (called from the ignored
/// `fake_dev_server_package_manager_child` test). With the transport enabled it
/// takes an OS-assigned port and announces it, to both the dev-route and the
/// browser readiness probes; otherwise it binds the logical `PORT`.
pub(crate) fn bind_fake_dev_server() -> std::net::TcpListener {
    let root = Path::new(".");
    let dynamic = enabled(root);
    let requested = std::env::var("PORT")
        .expect("PORT")
        .parse::<u16>()
        .expect("PORT number");
    let listener = std::net::TcpListener::bind(("127.0.0.1", if dynamic { 0 } else { requested }))
        .expect("bind fake server");
    if dynamic {
        let actual = listener.local_addr().expect("fake server address").port();
        publish_ready(root, read_generation(root), actual);
        write_evidence(root, BROWSER_MOCK_PORT_RELATIVE, &actual.to_string());
    }
    listener
}

// --- cfg(test) wrappers extracted from acceptance.rs ---

pub(crate) fn run_nextjs_dev_route_probe(config: &Config, evidence_path: &Path) -> Value {
    run_nextjs_dev_route_probe_with_interaction_options(
        config,
        evidence_path,
        BrowserInteractionProbeOptions::default(),
        None,
    )
}

pub(crate) fn inferred_required_capabilities(profile: &str, goal: &str) -> Vec<String> {
    resolve_profile_runtime(profile).required_capabilities(goal)
}

pub(crate) fn inferred_required_evidence(
    profile: &str,
    goal: &str,
    required_capabilities: &[String],
) -> Vec<String> {
    resolve_profile_runtime(profile).required_evidence(goal, required_capabilities)
}

pub(crate) fn inferred_required_obligations(
    profile: &str,
    goal: &str,
    required_capabilities: &[String],
) -> Vec<String> {
    let profile_id = ProfileId::parse(profile);
    ProfileRuntimeRegistry::resolve(&profile_id).required_obligations(
        &profile_id,
        goal,
        required_capabilities,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tempdir under the build target, never under a system prefix
    /// (`/usr`, `/bin`, `/opt`, `/etc`, `/tmp`). `CARGO_TARGET_TMPDIR` is only
    /// defined for integration tests, so a unit-test leaf anchors on the test
    /// binary's own directory, which is always inside the target dir.
    fn evidence_root() -> tempfile::TempDir {
        let parent = std::env::current_exe()
            .expect("test executable path")
            .parent()
            .expect("test executable parent")
            .to_path_buf();
        let dir = tempfile::Builder::new()
            .prefix("issue608-transport-")
            .tempdir_in(parent)
            .expect("tempdir under target");
        std::fs::create_dir_all(dir.path().join(".anvil/evidence")).unwrap();
        dir
    }

    #[test]
    fn ready_port_is_scoped_to_the_announcing_generation() {
        let dir = evidence_root();
        let root = dir.path();
        publish_ready(root, 1, 41_001);
        assert_eq!(read_ready(root, 1), Some(41_001));
        assert_eq!(
            read_ready(root, 2),
            None,
            "stale ready must not leak forward"
        );
        publish_ready(root, 2, 41_002);
        assert_eq!(read_ready(root, 2), Some(41_002));
        assert_eq!(read_ready(root, 1), None, "old port must not be reused");
    }

    #[test]
    fn dynamic_probe_clears_stale_ready_then_observes_its_own_child() {
        let dir = evidence_root();
        let root = dir.path();
        publish_ready(root, 5, 41_005);
        let probe = begin(root, true);
        assert!(
            read_ready(root, 5).is_none(),
            "stale ready cleared before spawn"
        );
        assert_eq!(probe.poll_port(root, 3011), 3011, "no notification yet");
        publish_ready(root, probe.generation, 41_009);
        assert_eq!(probe.poll_port(root, 3011), 41_009);
    }

    #[test]
    fn disabled_transport_keeps_logical_port_and_writes_no_generation() {
        let dir = evidence_root();
        let root = dir.path();
        let probe = begin(root, false);
        assert_eq!(probe.poll_port(root, 3011), 3011);
        assert!(!root.join(GENERATION_FILE_RELATIVE).exists());
        assert!(!enabled(root));
    }

    #[test]
    fn enable_marks_only_its_own_workspace() {
        let dir = evidence_root();
        enable(dir.path());
        assert!(enabled(dir.path()));
        assert!(
            !enabled(evidence_root().path()),
            "marker is workspace-scoped"
        );
    }
}
