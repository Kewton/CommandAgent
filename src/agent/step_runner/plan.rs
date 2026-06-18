use super::PlanError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepPlan {
    pub goal: String,
    pub profile: String,
    pub style: String,
    pub intent: WorkIntent,
    pub required_artifacts: Vec<String>,
    pub steps: Vec<StepPlanStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepPlanStep {
    pub id: String,
    pub kind: StepKind,
    pub instruction: String,
    pub expected_result: ExpectedResult,
    pub expected_paths: Vec<String>,
    pub verify: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkIntent {
    New,
    Modify,
    Investigate,
    Document,
    Data,
    Unknown,
}

impl WorkIntent {
    pub fn parse(value: &str) -> Result<Self, PlanError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "new" | "create" => Ok(Self::New),
            "modify" | "fix" | "enhance" | "refactor" => Ok(Self::Modify),
            "investigate" | "triage" | "debug" => Ok(Self::Investigate),
            "document" | "docs" => Ok(Self::Document),
            "data" | "data-analysis" | "data-pipeline" => Ok(Self::Data),
            "unknown" | "" => Ok(Self::Unknown),
            other => Err(PlanError::InvalidEnum {
                field: "intent".to_string(),
                value: other.to_string(),
            }),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Modify => "modify",
            Self::Investigate => "investigate",
            Self::Document => "document",
            Self::Data => "data",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    Inspect,
    Create,
    Edit,
    Setup,
    Verify,
    Repair,
    Report,
}

impl StepKind {
    pub fn parse(value: &str) -> Result<Self, PlanError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "inspect" | "read" | "analyze" | "analyse" => Ok(Self::Inspect),
            "create" => Ok(Self::Create),
            "edit" | "modify" | "update" => Ok(Self::Edit),
            "setup" | "install" | "configure" => Ok(Self::Setup),
            "verify" | "check" | "test" | "shell" | "command" | "run" => Ok(Self::Verify),
            "repair" | "fix" => Ok(Self::Repair),
            "report" | "summarize" | "summarise" => Ok(Self::Report),
            other => Err(PlanError::InvalidEnum {
                field: "step.kind".to_string(),
                value: other.to_string(),
            }),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inspect => "inspect",
            Self::Create => "create",
            Self::Edit => "edit",
            Self::Setup => "setup",
            Self::Verify => "verify",
            Self::Repair => "repair",
            Self::Report => "report",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedResult {
    Pass,
    Fail,
    Unavailable,
}

impl ExpectedResult {
    pub fn parse(value: &str) -> Result<Self, PlanError> {
        let normalized = value.trim().to_ascii_lowercase().replace('_', "-");
        match normalized.as_str() {
            "pass" | "passed" | "success" | "successful" | "ok" | "available" => Ok(Self::Pass),
            "fail" | "failed" | "failure" | "expected-failure" => Ok(Self::Fail),
            "unavailable" | "not-available" | "not-available-yet" => Ok(Self::Unavailable),
            other => Err(PlanError::InvalidEnum {
                field: "step.expected_result".to_string(),
                value: other.to_string(),
            }),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Unavailable => "unavailable",
        }
    }
}
