use std::io::Read;

use anyhow::{Context, bail};

const READ_BUFFER_BYTES: usize = 8 * 1024;
const MAX_STREAM_LINE_BYTES: usize = 8 * 1024 * 1024;
const AFTER_FIRST_CHUNK_PREFIX: &str = "provider stream failed after partial output:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamControl {
    Continue,
    Stop,
}

pub fn after_first_chunk(err: anyhow::Error) -> anyhow::Error {
    anyhow::anyhow!("{AFTER_FIRST_CHUNK_PREFIX} {err:#}")
}

pub fn is_after_first_chunk_error(message: &str) -> bool {
    message.contains(AFTER_FIRST_CHUNK_PREFIX)
}

pub fn retry_allowed(attempt: usize, retries: usize, delivered_chunk: bool) -> bool {
    !delivered_chunk && attempt < retries
}

pub fn read_ndjson<R, F>(reader: R, mut on_line: F) -> anyhow::Result<()>
where
    R: Read,
    F: FnMut(&str) -> anyhow::Result<StreamControl>,
{
    read_utf8_lines(reader, |line| {
        if line.trim().is_empty() {
            return Ok(StreamControl::Continue);
        }
        on_line(line)
    })
}

pub fn read_sse<R, F>(reader: R, mut on_data: F) -> anyhow::Result<()>
where
    R: Read,
    F: FnMut(&str) -> anyhow::Result<StreamControl>,
{
    let mut data_lines = Vec::new();
    let mut stopped = false;
    read_utf8_lines(reader, |line| {
        if stopped {
            return Ok(StreamControl::Stop);
        }
        if line.is_empty() {
            stopped = emit_sse_event(&mut data_lines, &mut on_data)? == StreamControl::Stop;
            return Ok(if stopped {
                StreamControl::Stop
            } else {
                StreamControl::Continue
            });
        }
        if line.starts_with(':') {
            return Ok(StreamControl::Continue);
        }
        if let Some(data) = line.strip_prefix("data:") {
            data_lines.push(data.strip_prefix(' ').unwrap_or(data).to_string());
        }
        Ok(StreamControl::Continue)
    })?;
    if !stopped && !data_lines.is_empty() {
        let _ = emit_sse_event(&mut data_lines, &mut on_data)?;
    }
    Ok(())
}

fn emit_sse_event<F>(data_lines: &mut Vec<String>, on_data: &mut F) -> anyhow::Result<StreamControl>
where
    F: FnMut(&str) -> anyhow::Result<StreamControl>,
{
    if data_lines.is_empty() {
        return Ok(StreamControl::Continue);
    }
    let data = data_lines.join("\n");
    data_lines.clear();
    if data.trim() == "[DONE]" {
        return Ok(StreamControl::Stop);
    }
    on_data(&data)
}

fn read_utf8_lines<R, F>(mut reader: R, mut on_line: F) -> anyhow::Result<()>
where
    R: Read,
    F: FnMut(&str) -> anyhow::Result<StreamControl>,
{
    let mut pending = Vec::new();
    let mut read_buffer = [0_u8; READ_BUFFER_BYTES];
    loop {
        let read = reader
            .read(&mut read_buffer)
            .context("failed to read provider stream")?;
        if read == 0 {
            break;
        }
        pending.extend_from_slice(&read_buffer[..read]);
        while let Some(newline) = pending.iter().position(|byte| *byte == b'\n') {
            let mut line = pending.drain(..=newline).collect::<Vec<_>>();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            let line =
                std::str::from_utf8(&line).context("provider stream contains invalid UTF-8")?;
            if on_line(line)? == StreamControl::Stop {
                return Ok(());
            }
        }
        if pending.len() > MAX_STREAM_LINE_BYTES {
            bail!("provider stream line exceeds {MAX_STREAM_LINE_BYTES} bytes");
        }
    }
    if !pending.is_empty() {
        let line = std::str::from_utf8(&pending)
            .context("provider stream contains incomplete or invalid UTF-8")?;
        let _ = on_line(line)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Cursor};

    struct OneByteReader<R> {
        inner: R,
    }

    impl<R: Read> Read for OneByteReader<R> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            self.inner.read(&mut buffer[..1])
        }
    }

    #[test]
    fn ndjson_preserves_utf8_split_across_reads() {
        let source = "{\"text\":\"日🙂\"}\n";
        let mut lines = Vec::new();
        read_ndjson(
            OneByteReader {
                inner: Cursor::new(source.as_bytes()),
            },
            |line| {
                lines.push(line.to_string());
                Ok(StreamControl::Continue)
            },
        )
        .unwrap();
        assert_eq!(lines, vec![r#"{"text":"日🙂"}"#]);
    }

    #[test]
    fn sse_combines_data_lines_and_stops_at_done() {
        let source = "event: x\r\ndata: one\r\ndata: two\r\n\r\ndata: [DONE]\n\ndata: ignored\n\n";
        let mut events = Vec::new();
        read_sse(Cursor::new(source), |data| {
            events.push(data.to_string());
            Ok(StreamControl::Continue)
        })
        .unwrap();
        assert_eq!(events, vec!["one\ntwo"]);
    }

    #[test]
    fn invalid_utf8_is_a_clear_error() {
        let err = read_ndjson(Cursor::new(vec![b'{', 0xff, b'}', b'\n']), |_| {
            Ok(StreamControl::Continue)
        })
        .unwrap_err()
        .to_string();
        assert!(err.contains("invalid UTF-8"), "{err}");
    }

    #[test]
    fn retry_is_allowed_only_before_first_chunk_and_with_budget() {
        assert!(retry_allowed(0, 1, false));
        assert!(!retry_allowed(0, 1, true));
        assert!(!retry_allowed(1, 1, false));
    }
}
