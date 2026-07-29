#[cfg(unix)]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(unix)]
static DEV_SERVER_PROBE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(unix)]
fn dev_server_probe_test_guard() -> std::sync::MutexGuard<'static, ()> {
    DEV_SERVER_PROBE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(unix)]
static TEST_DEV_SERVER_PORT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(34_011);

#[cfg(unix)]
fn free_local_port() -> u16 {
    for _ in 0..2_000 {
        let port = TEST_DEV_SERVER_PORT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if port > u16::MAX as usize {
            break;
        }
        let port = port as u16;
        if port == NEXTJS_DEV_SERVER_DEFAULT_PORT {
            continue;
        }
        if test_dev_server_port_is_available(port) {
            return port;
        }
    }
    loop {
        match std::net::TcpListener::bind(("127.0.0.1", 0)) {
            Ok(listener) => {
                let port = listener.local_addr().unwrap().port();
                drop(listener);
                if port != NEXTJS_DEV_SERVER_DEFAULT_PORT
                    && test_dev_server_port_is_available(port)
                {
                    return port;
                }
            }
            Err(_) => return NEXTJS_DEV_SERVER_DEFAULT_PORT + 1,
        }
    }
}

#[cfg(unix)]
fn test_dev_server_port_is_available(port: u16) -> bool {
    let Ok(listener) = std::net::TcpListener::bind(("127.0.0.1", port)) else {
        return false;
    };
    drop(listener);
    !localhost_port_accepts_connection(port)
}

fn read_jsonl_events(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect()
}

fn dev_server_stage_names(events: &[Value]) -> Vec<&str> {
    events
        .iter()
        .filter(|event| {
            event.get("event").and_then(Value::as_str) == Some("dev_server_lifecycle")
        })
        .filter_map(|event| event.get("stage").and_then(Value::as_str))
        .collect()
}

#[cfg(unix)]
fn wait_until_process_group_gone(pgid: u32, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !process_group_exists(pgid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    !process_group_exists(pgid)
}

#[cfg(unix)]
fn process_group_exists(pgid: u32) -> bool {
    let Ok(pgid) = i32::try_from(pgid) else {
        return false;
    };
    // SAFETY: signal 0 performs existence/permission checking only, using
    // a process-group id originally emitted from a spawned child pid.
    let rc = unsafe { libc::kill(-pgid, 0) };
    if rc == 0 {
        return true;
    }
    let err = std::io::Error::last_os_error();
    err.raw_os_error() != Some(libc::ESRCH)
}
