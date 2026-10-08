// Test-only transport adapter for the issue475 observer. The frozen observer
// bytes stay authoritative: `build` and `start` still run `observer.mjs`, so the
// store import, GET/POST and hash observations keep going through the original
// observer. Only the transport changes: `start` binds an OS-assigned port and
// announces the real port through the browser probe's ready file, so the logical
// goal port is never freed and re-bound. Mirrors the issue448/#537 transport.
//
// The ready-file literal must match browser_probe::test_support.

#[cfg(unix)]
const ISSUE475_READY_RELATIVE: &str = ".anvil/evidence/browser-probe-mock-port.txt";

#[cfg(unix)]
const ISSUE475_HOOK_RELATIVE: &str = "node_modules/.issue475/transport-ready.cjs";

#[cfg(unix)]
fn issue475_set_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// Install the transport adapter into a disposable workspace before the Recovery
/// snapshot: a workspace `npm` shim that keeps the original observer
/// authoritative, the readiness hook, and the browser-probe command override.
#[cfg(unix)]
fn install_issue475_observer_transport(root: &Path, logical_port: u16) {
    let bin = root.join("node_modules/.bin");
    std::fs::create_dir_all(&bin).unwrap();
    let npm = bin.join("npm");
    std::fs::write(&npm, issue475_mock_npm_script(logical_port)).unwrap();
    issue475_set_executable(&npm);

    let hook = root.join(ISSUE475_HOOK_RELATIVE);
    std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
    std::fs::write(&hook, issue475_ready_hook_script()).unwrap();

    let command = root.join(".anvil/evidence/browser-probe-command.json");
    std::fs::create_dir_all(command.parent().unwrap()).unwrap();
    std::fs::write(
        &command,
        serde_json::to_string_pretty(&json!({
            "program": "npm",
            "args": ["run", "start"],
            "env": {},
            "display": "npm run start",
            "port": logical_port,
            "require_build": true,
            "dynamic_port": true,
        }))
        .unwrap(),
    )
    .unwrap();
}

#[cfg(unix)]
fn issue475_mock_npm_script(logical_port: u16) -> String {
    format!(
        r#"#!/bin/sh
# Test-only npm transport: the original observer stays authoritative, and
# `start` binds an OS-assigned port announced through the ready file.
set -eu
[ "$#" -eq 2 ] && [ "$1" = "run" ] || exit 90
case "$2" in
  build)
    exec node observer.mjs build
    ;;
  start)
    [ "$PORT" = "{logical_port}" ] || exit 92
    rm -f {ready}
    exec env PORT=0 node --require ./{hook} observer.mjs start
    ;;
  *) exit 93 ;;
esac
"#,
        logical_port = logical_port,
        ready = ISSUE475_READY_RELATIVE,
        hook = ISSUE475_HOOK_RELATIVE,
    )
}

#[cfg(unix)]
fn issue475_ready_hook_script() -> &'static str {
    r#"// Announce the OS-assigned port the real observer binds through the ready file
// the browser probe polls in dynamic_port mode. Nothing is mocked or re-routed.
const fs = require('node:fs');
const path = require('node:path');
const net = require('node:net');
const ready = path.join(process.cwd(), '.anvil/evidence/browser-probe-mock-port.txt');
const listen = net.Server.prototype.listen;
net.Server.prototype.listen = function (...args) {
  this.once('listening', () => {
    const address = this.address();
    const port = address && typeof address === 'object' ? address.port : 0;
    if (port) {
      fs.mkdirSync(path.dirname(ready), { recursive: true });
      fs.writeFileSync(ready, String(port));
    }
  });
  return listen.apply(this, args);
};
"#
}

/// The actual port the observer announced in its observation workspace.
#[cfg(unix)]
fn issue475_announced_port(observation: &Path) -> u16 {
    std::fs::read_to_string(observation.join(ISSUE475_READY_RELATIVE))
        .unwrap_or_else(|error| panic!("observer announced no port: {error}"))
        .trim()
        .parse()
        .expect("announced port is a u16")
}
