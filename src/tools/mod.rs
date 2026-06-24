pub mod bash;
pub mod edit;
pub mod glob;
pub mod grep;
pub mod read;
pub mod registry;
pub mod test_output;
pub mod write;

use crate::safety::path_guard::PathGuardError;
use bash::CommandClass;
use std::path::PathBuf;

pub type ToolResult<T> = Result<T, ToolError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolError {
    Path(PathGuardError),
    Io {
        path: PathBuf,
        message: String,
    },
    BinaryFile {
        path: PathBuf,
    },
    EditMatchNotFound,
    EditMatchAmbiguous {
        count: usize,
    },
    BashBlocked {
        class: CommandClass,
        command: String,
        message: String,
    },
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Path(err) => write!(f, "{}", err),
            Self::Io { path, message } => write!(f, "{}: {}", path.display(), message),
            Self::BinaryFile { path } => {
                write!(f, "refusing to read binary file: {}", path.display())
            }
            Self::EditMatchNotFound => write!(f, "edit target was not found"),
            Self::EditMatchAmbiguous { count } => {
                write!(
                    f,
                    "edit target matched {} times; refusing ambiguous edit",
                    count
                )
            }
            Self::BashBlocked {
                class,
                command,
                message,
            } => {
                write!(
                    f,
                    "bash command blocked as {:?}: {}; command={}",
                    class, message, command
                )
            }
        }
    }
}

impl std::error::Error for ToolError {}

impl From<PathGuardError> for ToolError {
    fn from(value: PathGuardError) -> Self {
        Self::Path(value)
    }
}
