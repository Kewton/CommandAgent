//! Literal display placeholders must never be interpreted as shell syntax.
use std::ffi::CString;
use std::fs::File;
use std::io::Write;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::bash::BashPathConfinementRejection;

pub(crate) const REASON: &str = "placeholder path detected";

pub(crate) fn detected(command: &str) -> bool {
    command.contains("<user>") || command.contains("<redacted>")
}

pub(crate) fn is_placeholder_rejection(tool_name: &str, error: &anyhow::Error) -> bool {
    tool_name == "Bash"
        && error
            .to_string()
            .starts_with("bash_path_confinement_error: placeholder path detected;")
}

pub(crate) fn rejection(command: &str, root: &Path) -> Option<BashPathConfinementRejection> {
    let placeholder = ["<user>", "<redacted>"]
        .into_iter()
        .find(|placeholder| command.contains(placeholder))?;
    let guidance = "Retry with workspace-relative paths; omit the absolute workspace cd prefix. Display placeholders are not executable paths; do not guess a username.";
    let reason = format!("{REASON}; {guidance}");
    Some(BashPathConfinementRejection {
        path: placeholder.to_string(),
        root: root.to_string_lossy().into_owned(),
        nearest_relative: ".".to_string(),
        guidance: guidance.to_string(),
        operation: "placeholder path".to_string(),
        message: format!("bash_path_confinement_error: {reason}"),
        reason,
    })
}

fn command_sha256(command: &str) -> String {
    format!("{:x}", Sha256::digest(command.as_bytes()))
}

pub(crate) fn add_event_fields(event: &mut Value, command: &str) {
    if detected(command) {
        event["placeholder_detected"] = json!(true);
        event["command_sha256"] = json!(command_sha256(command));
    }
}

/// Record the original command before any normalization. Raw bytes are never
/// returned in feedback or added to events. Directory-relative opens prevent
/// workspace-controlled symlinks from redirecting this private write.
pub(crate) fn record_rejection(root: &Path, command: &str) -> std::io::Result<()> {
    let mut directory = File::open(root)?;
    for component in [".commandagent", "evidence", "bash-placeholders"] {
        let name = CString::new(component).expect("constant directory name");
        // SAFETY: directory owns its fd; name is a valid NUL-terminated string.
        let result = unsafe { libc::mkdirat(directory.as_raw_fd(), name.as_ptr(), 0o700) };
        if result != 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::AlreadyExists {
                return Err(error);
            }
        }
        directory = open_at(
            &directory,
            &name,
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )?;
    }
    if directory.metadata()?.permissions().mode() & 0o077 != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "placeholder evidence directory must be owner-private",
        ));
    }
    let name = CString::new(format!("{}.json", uuid::Uuid::now_v7())).expect("generated filename");
    let mut file = open_at(
        &directory,
        &name,
        libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
    )?;
    let bytes = command.as_bytes();
    let prefix = &bytes[..bytes.len().min(64)];
    let evidence = json!({
        "schema_version": "1",
        "command_sha256": command_sha256(command),
        "command_bytes": bytes.len(),
        "command_prefix_bytes": prefix,
        "prefix_encoding": "raw byte array; at most 64 bytes; not redacted",
    });
    file.write_all(serde_json::to_string(&evidence)?.as_bytes())?;
    file.write_all(b"\n")?;
    file.flush()
}

fn open_at(directory: &File, name: &CString, flags: i32) -> std::io::Result<File> {
    // SAFETY: directory owns its fd; name is NUL-terminated. The returned fd is
    // checked and transferred to exactly one File owner.
    let fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags, 0o600) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: openat returned a fresh valid fd owned by this function.
    Ok(unsafe { File::from_raw_fd(fd) })
}
