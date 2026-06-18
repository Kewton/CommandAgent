use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    MissingField(String),
    EmptyField(String),
    NoSteps,
    InvalidStepId(String),
    DuplicateStepId(String),
    InvalidYaml(String),
    InvalidEnum { field: String, value: String },
    Io { path: PathBuf, message: String },
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingField(field) => write!(f, "missing required field: {field}"),
            Self::EmptyField(field) => write!(f, "field must not be empty: {field}"),
            Self::NoSteps => write!(f, "step plan must contain at least one step"),
            Self::InvalidStepId(id) => write!(f, "invalid step id: {id}"),
            Self::DuplicateStepId(id) => write!(f, "duplicate step id: {id}"),
            Self::InvalidYaml(message) => write!(f, "invalid plan YAML: {message}"),
            Self::InvalidEnum { field, value } => write!(f, "invalid {field}: {value}"),
            Self::Io { path, message } => write!(f, "{}: {}", path.display(), message),
        }
    }
}

impl std::error::Error for PlanError {}
