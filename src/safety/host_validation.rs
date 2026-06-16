pub fn is_loopback_host(host: &str) -> bool {
    matches!(
        host.trim().to_ascii_lowercase().as_str(),
        "localhost" | "127.0.0.1" | "::1" | "[::1]"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_loopback_hosts() {
        assert!(is_loopback_host("localhost"));
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("::1"));
    }

    #[test]
    fn rejects_remote_hosts() {
        assert!(!is_loopback_host("example.com"));
        assert!(!is_loopback_host("192.168.1.10"));
    }
}
