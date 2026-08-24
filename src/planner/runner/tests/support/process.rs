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
struct ReservedLocalPort {
    listener: std::net::TcpListener,
    port: u16,
}

#[cfg(unix)]
impl ReservedLocalPort {
    fn reserve() -> Self {
        loop {
            let listener =
                std::net::TcpListener::bind(("127.0.0.1", 0)).expect("reserve local test port");
            let port = listener.local_addr().expect("reserved local address").port();
            if port != NEXTJS_DEV_SERVER_DEFAULT_PORT {
                return Self { listener, port };
            }
        }
    }

    fn port(&self) -> u16 {
        self.port
    }

    fn release(self) {
        drop(self.listener);
    }
}

#[cfg(unix)]
fn free_local_port() -> u16 {
    let reservation = ReservedLocalPort::reserve();
    let port = reservation.port();
    reservation.release();
    port
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
