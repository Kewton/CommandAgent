use std::io::Read;
use std::thread::JoinHandle;

use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamCapture {
    pub text: String,
    pub captured_bytes: usize,
    pub total_bytes: u64,
    pub truncated: bool,
    /// Internal telemetry used to surface invalid UTF-8 replacement without
    /// changing the serialized stream shape.
    #[serde(skip)]
    pub invalid_utf8_replaced: bool,
}

pub(crate) fn capture_stream<R>(
    mut reader: R,
    max_bytes: usize,
) -> JoinHandle<std::io::Result<StreamCapture>>
where
    R: Read + Send + 'static,
{
    std::thread::spawn(move || {
        let mut captured = Vec::with_capacity(max_bytes.min(8192));
        let mut total_bytes = 0u64;
        let mut buffer = [0u8; 8192];
        loop {
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            total_bytes = total_bytes.saturating_add(read as u64);
            let remaining = max_bytes.saturating_sub(captured.len());
            captured.extend_from_slice(&buffer[..read.min(remaining)]);
        }
        let captured_bytes = captured.len();
        let (text, invalid_utf8_replaced) = match String::from_utf8(captured) {
            Ok(text) => (text, false),
            Err(error) => (String::from_utf8_lossy(error.as_bytes()).to_string(), true),
        };
        Ok(StreamCapture {
            text,
            captured_bytes,
            total_bytes,
            truncated: total_bytes > captured_bytes as u64,
            invalid_utf8_replaced,
        })
    })
}

pub(crate) fn join_capture(
    handle: JoinHandle<std::io::Result<StreamCapture>>,
    stream: &str,
) -> anyhow::Result<StreamCapture> {
    handle
        .join()
        .map_err(|_| anyhow::anyhow!("pipeline {stream} capture thread panicked"))?
        .with_context(|| format!("failed to capture pipeline {stream}"))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn invalid_utf8_is_replaced_and_marked() {
        let capture = capture_stream(Cursor::new(vec![b'a', 0xff, b'b']), 64)
            .join()
            .expect("capture thread")
            .expect("capture result");
        assert_eq!(capture.text, "a\u{fffd}b");
        assert!(capture.invalid_utf8_replaced);
    }
}
