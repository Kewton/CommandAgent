fn enable_browser_probe_test_override(root: &Path) {
    std::fs::create_dir_all(root.join(".anvil")).unwrap();
    std::fs::write(root.join(".anvil/enable-browser-probe-tests"), "1").unwrap();
}

fn write_browser_probe_mock_command(root: &Path, status: &str) -> u16 {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let dir = root.join(".anvil/evidence");
    std::fs::create_dir_all(&dir).unwrap();
    let exe = std::env::current_exe().unwrap();
    let command = serde_json::json!({
        "program": exe.display().to_string(),
        "args": [
            "--ignored",
            "--exact",
            "minimal_loop::browser_probe::tests::browser_probe_mock_server_child",
            "--nocapture"
        ],
        "env": {
            "COMMANDAGENT_BROWSER_PROBE_MOCK_CHILD": "1",
            "COMMANDAGENT_BROWSER_PROBE_MOCK_PORT": port.to_string(),
            "COMMANDAGENT_BROWSER_PROBE_MOCK_STATUS": status,
            "COMMANDAGENT_BROWSER_PROBE_MOCK_DELAY_MS": "0"
        },
        "port": port,
        "require_build": false,
        "display": "mock browser probe child"
    });
    std::fs::write(
        dir.join("browser-probe-command.json"),
        serde_json::to_string_pretty(&command).unwrap(),
    )
    .unwrap();
    port
}

fn run_ignored_runner_harness(test_name: &str) -> std::process::ExitStatus {
    let exe = std::env::current_exe().unwrap();
    std::process::Command::new(exe)
        .args(["--ignored", "--exact", test_name, "--nocapture"])
        .env("NODE_ENV", "production")
        .status()
        .unwrap()
}

#[cfg(unix)]
fn forced_cleanup_timeout_after_real_cleanup(
    child: Child,
    logs: &DevServerLogPaths,
) -> DevServerCleanup {
    let _ = cleanup_dev_server_child(child, logs);
    DevServerCleanup {
        ok: false,
        failure_kind: Some("dev_server_cleanup_timeout".to_string()),
        output_excerpt: cleanup_timeout_excerpt(
            logs,
            &["forced cleanup timeout for test".to_string()],
        ),
    }
}

#[cfg(unix)]
fn enable_dev_server_probe_test_override(root: &Path) {
    std::fs::create_dir_all(root.join(".anvil")).unwrap();
    std::fs::write(root.join(".anvil/enable-dev-server-probe-tests"), "1").unwrap();
}

#[cfg(unix)]
fn write_fake_nextjs_dev_workspace(root: &Path, port: u16, spawn_grandchild: bool) {
    std::fs::write(
        root.join("package.json"),
        format!(
            r#"{{"scripts":{{"dev":"next dev -p {port}","build":"next build"}},"dependencies":{{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"}}}}"#
        ),
    )
    .unwrap();
    write_fake_nextjs_package_manager(root, spawn_grandchild);
}

#[cfg(unix)]
fn write_fake_nextjs_package_manager(root: &Path, spawn_grandchild: bool) {
    let bin = root.join("node_modules/.bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(root.join("node_modules/next")).unwrap();
    std::fs::create_dir_all(root.join("node_modules/tailwindcss")).unwrap();
    std::fs::create_dir_all(root.join("node_modules/postcss")).unwrap();
    std::fs::create_dir_all(root.join("node_modules/autoprefixer")).unwrap();
    let exe = shell_quote(&std::env::current_exe().unwrap().display().to_string());
    let grandchild = if spawn_grandchild { "1" } else { "0" };
    let script = format!(
        "#!/bin/sh\n\
if [ \"$1\" = \"run\" ] && [ \"$2\" = \"build\" ]; then\n\
  echo \"fake build ok\"\n\
  exit 0\n\
fi\n\
if [ \"$1\" = \"run\" ] && [ \"$2\" = \"dev\" ]; then\n\
  COMMANDAGENT_FAKE_DEV_SERVER_CHILD=1 COMMANDAGENT_FAKE_DEV_SERVER_GRANDCHILD={grandchild} exec {exe} --ignored --exact planner::runner::tests::fake_dev_server_package_manager_child --nocapture\n\
fi\n\
echo \"unexpected fake npm args: $*\" >&2\n\
exit 2\n"
    );
    let path = bin.join("npm");
    std::fs::write(&path, script).unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(&path, permissions).unwrap();
    let next_path = bin.join("next");
    std::fs::write(&next_path, "#!/bin/sh\nexit 0\n").unwrap();
    let mut permissions = std::fs::metadata(&next_path).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(next_path, permissions).unwrap();
}

#[cfg(unix)]
fn write_fake_npm_dependency_installer(root: &Path) {
    let bin = root.join("node_modules/.bin");
    std::fs::create_dir_all(&bin).unwrap();
    let script = r#"#!/bin/sh
set -eu
install_pkg() {
  name="$1"
  if grep -q "\"$name\"" package.json 2>/dev/null; then
mkdir -p "node_modules/$name"
printf '{"name":"%s"}\n' "$name" > "node_modules/$name/package.json"
  fi
}
if [ "$1" = "install" ]; then
  mkdir -p node_modules/.bin
  install_pkg next
  install_pkg react
  install_pkg react-dom
  install_pkg typescript
  install_pkg @types/node
  install_pkg @types/react
  install_pkg @types/react-dom
  install_pkg tailwindcss
  install_pkg postcss
  install_pkg autoprefixer
  if [ -d node_modules/next ]; then
printf '#!/bin/sh\nexit 0\n' > node_modules/.bin/next
chmod +x node_modules/.bin/next
  fi
  printf '{"lockfileVersion":3}\n' > package-lock.json
  exit 0
fi
if [ "$1" = "run" ] && [ "$2" = "build" ]; then
  test -x node_modules/.bin/next || { echo "next missing" >&2; exit 1; }
  if grep -q "\"tailwindcss\"" package.json 2>/dev/null; then
test -d node_modules/tailwindcss || { echo "tailwindcss missing" >&2; exit 1; }
test -d node_modules/postcss || { echo "postcss missing" >&2; exit 1; }
test -d node_modules/autoprefixer || { echo "autoprefixer missing" >&2; exit 1; }
  fi
  echo "fake build ok"
  exit 0
fi
echo "unexpected fake npm args: $*" >&2
exit 2
"#;
    let path = bin.join("npm");
    std::fs::write(&path, script).unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(path, permissions).unwrap();
}

#[cfg(unix)]
fn write_compile_error_fake_npm(root: &Path) {
    let bin = root.join("node_modules/.bin");
    std::fs::create_dir_all(&bin).unwrap();
    let script = r#"#!/bin/sh
set -eu
install_pkg() {
  name="$1"
  if grep -q "\"$name\"" package.json 2>/dev/null; then
mkdir -p "node_modules/$name"
printf '{"name":"%s"}\n' "$name" > "node_modules/$name/package.json"
  fi
}
if [ "$1" = "install" ]; then
  mkdir -p node_modules/.bin
  install_pkg next
  install_pkg react
  install_pkg react-dom
  install_pkg typescript
  install_pkg @types/node
  install_pkg @types/react
  install_pkg @types/react-dom
  install_pkg tailwindcss
  install_pkg postcss
  install_pkg autoprefixer
  if [ -d node_modules/next ]; then
printf '#!/bin/sh\nexit 0\n' > node_modules/.bin/next
chmod +x node_modules/.bin/next
  fi
  printf '{"lockfileVersion":3}\n' > package-lock.json
  exit 0
fi
if [ "$1" = "run" ] && [ "$2" = "build" ]; then
  cat >&2 <<'OUT'
./src/app/page.tsx
Error:
  x the name `player` is defined multiple times

   ,-[./src/app/page.tsx:479:1]
359 |       const player = playerRef.current;
:             ------ previous definition of `player` here
479 |       const player = playerRef.current;
:             ------ `player` redefined here
   `----
> Build failed because of webpack errors
OUT
  exit 1
fi
echo "unexpected fake npm args: $*" >&2
exit 2
"#;
    let path = bin.join("npm");
    std::fs::write(&path, script).unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(path, permissions).unwrap();
}

fn write_nextjs_dual_blocker_workspace(root: &Path) {
    std::fs::create_dir_all(root.join("src/app")).unwrap();
    std::fs::write(
        root.join("package.json"),
        r#"{"scripts":{"build":"next build"},"dependencies":{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"},"devDependencies":{"typescript":"^5.5.0","@types/node":"^20.14.0","@types/react":"^18.3.0","@types/react-dom":"^18.3.0","tailwindcss":"^3.4.19","postcss":"^8.5.15","autoprefixer":"^10.4.20"}}"#,
    )
    .unwrap();
    std::fs::write(root.join("tsconfig.json"), nextjs_tsconfig_json()).unwrap();
    std::fs::write(root.join("postcss.config.js"), nextjs_postcss_config()).unwrap();
    std::fs::write(root.join("tailwind.config.ts"), nextjs_tailwind_config_ts()).unwrap();
    std::fs::write(root.join("src/app/layout.tsx"), nextjs_layout_source()).unwrap();
    std::fs::write(root.join("src/app/globals.css"), nextjs_globals_css()).unwrap();
    std::fs::write(
        root.join("src/app/global.d.ts"),
        "declare module \"*.css\";\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/app/page.tsx"),
        r#""use client";
export default function Page() {
  if (true) {
const player = { lives: 3 };
const enemyBullets = [{ active: true }];
const player = { lives: 2 };
return <main><canvas data-anvil-primary-action />{enemyBullets.length}{player.lives}</main>;
  }
  return <main />;
}
"#,
    )
    .unwrap();
}

#[cfg(not(unix))]
fn write_fake_npm_dependency_installer(root: &Path) {
    let bin = root.join("node_modules/.bin");
    std::fs::create_dir_all(&bin).unwrap();
    let script = r#"@echo off
setlocal
if "%1"=="install" (
  if exist package.json (
findstr /c:"\"next\"" package.json >nul && mkdir node_modules\next 2>nul
findstr /c:"\"tailwindcss\"" package.json >nul && mkdir node_modules\tailwindcss 2>nul
findstr /c:"\"postcss\"" package.json >nul && mkdir node_modules\postcss 2>nul
findstr /c:"\"autoprefixer\"" package.json >nul && mkdir node_modules\autoprefixer 2>nul
if exist node_modules\next (
  echo @echo off> node_modules\.bin\next.cmd
  echo exit /b 0>> node_modules\.bin\next.cmd
  echo {"name":"next"}> node_modules\next\package.json
)
if exist node_modules\tailwindcss echo {"name":"tailwindcss"}> node_modules\tailwindcss\package.json
if exist node_modules\postcss echo {"name":"postcss"}> node_modules\postcss\package.json
if exist node_modules\autoprefixer echo {"name":"autoprefixer"}> node_modules\autoprefixer\package.json
  )
  echo {"lockfileVersion":3}> package-lock.json
  exit /b 0
)
if "%1"=="run" if "%2"=="build" (
  if not exist node_modules\.bin\next.cmd exit /b 1
  if exist node_modules\tailwindcss (
if not exist node_modules\postcss exit /b 1
if not exist node_modules\autoprefixer exit /b 1
  )
  echo fake build ok
  exit /b 0
)
echo unexpected fake npm args: %*
exit /b 2
"#;
    std::fs::write(bin.join("npm.cmd"), script).unwrap();
}

#[cfg(unix)]
fn write_probe_nextjs_workspace(root: &Path, port: u16, page: &str) {
    std::fs::write(
        root.join("package.json"),
        format!(
            r#"{{"scripts":{{"build":"next build","dev":"next dev -p {port}","start":"next start -p {port}"}},"dependencies":{{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"}},"devDependencies":{{"typescript":"^5.5.0","@types/node":"^20.14.0","@types/react":"^18.3.0","@types/react-dom":"^18.3.0","tailwindcss":"^3.4.19","postcss":"^8.5.15","autoprefixer":"^10.4.20"}}}}"#
        ),
    )
    .unwrap();
    write_fake_nextjs_package_manager(root, false);
    std::fs::create_dir_all(root.join("src/app")).unwrap();
    std::fs::write(root.join("tsconfig.json"), nextjs_tsconfig_json()).unwrap();
    std::fs::write(root.join("postcss.config.js"), nextjs_postcss_config()).unwrap();
    std::fs::write(root.join("tailwind.config.ts"), nextjs_tailwind_config_ts()).unwrap();
    std::fs::write(root.join("src/app/page.tsx"), page).unwrap();
    std::fs::write(root.join("src/app/layout.tsx"), nextjs_layout_source()).unwrap();
    std::fs::write(root.join("src/app/globals.css"), nextjs_globals_css()).unwrap();
    std::fs::write(
        root.join("src/app/global.d.ts"),
        "declare module \"*.css\";\n",
    )
    .unwrap();
}

