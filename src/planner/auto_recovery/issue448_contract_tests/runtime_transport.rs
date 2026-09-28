// Test-only npm transport for the original issue448 contract. The frozen build
// transport stays authoritative for `build`; `start` binds an OS-assigned port
// and publishes the real port through a ready file so two concurrent test
// processes never contend for the logical port 60302.
//
// The ready file literal must match browser_probe::test_support.

fn install_issue448_mock_npm_transport(root: &Path, frozen_build_transport: &Path) {
    let bin = root.join("node_modules/.bin");
    std::fs::create_dir_all(&bin).unwrap();
    let transport = bin.join("npm-build-transport");
    std::fs::copy(frozen_build_transport, &transport).unwrap();
    issue448_set_executable(&transport);
    let npm = bin.join("npm");
    std::fs::write(&npm, issue448_mock_npm_script()).unwrap();
    issue448_set_executable(&npm);
    let command_path = root.join(".anvil/evidence/browser-probe-command.json");
    if let Some(parent) = command_path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(
        &command_path,
        serde_json::to_string_pretty(&json!({
            "program": "npm",
            "args": ["run", "start"],
            "env": {},
            "display": "npm run start",
            "port": 60302,
            "require_build": true,
            "dynamic_port": true,
        }))
        .unwrap(),
    )
    .unwrap();
}

fn issue448_set_executable(path: &Path) {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn issue448_mock_npm_script() -> String {
    r#"#!/bin/sh
# Test-only npm transport: frozen build transport + OS-assigned start port.
set -eu
bin=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
runtime=$(CDPATH= cd -- "$bin/../.issue448" && pwd)
[ "$#" -eq 2 ] && [ "$1" = "run" ] || exit 90
case "$2" in
  build)
    exec sh "$bin/npm-build-transport" "$@"
    ;;
  start)
    [ "$PORT" = "60302" ] || exit 92
    mkdir -p .commandagent/evidence .anvil/evidence
    printf '%s\n' 'npm run start' >> .commandagent/evidence/issue448-commands.txt
    rm -f .anvil/evidence/browser-probe-mock-port.txt
    test_exe=$(cat "$runtime/test-exe.txt")
    status=$(cat "$runtime/http-status.txt")
    exec env COMMANDAGENT_BROWSER_PROBE_MOCK_CHILD=1 \
      COMMANDAGENT_BROWSER_PROBE_MOCK_PORT=0 \
      COMMANDAGENT_BROWSER_PROBE_MOCK_STATUS="$status" \
      COMMANDAGENT_BROWSER_PROBE_MOCK_DELAY_MS=0 \
      "$test_exe" --ignored --exact \
      minimal_loop::browser_probe::tests::browser_probe_mock_server_child --nocapture
    ;;
  *) exit 93 ;;
esac
"#
    .to_string()
}
