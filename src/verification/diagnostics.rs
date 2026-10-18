use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompileError {
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub excerpt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_bound: Option<bool>,
}

impl CompileError {
    pub fn location(&self) -> String {
        if self.line > 0 && self.column > 0 {
            format!("{}:{}:{}", self.path, self.line, self.column)
        } else if self.line > 0 {
            format!("{}:{}", self.path, self.line)
        } else {
            self.path.clone()
        }
    }

    pub fn summary(&self) -> String {
        format!("{} {}", self.location(), self.message)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildVerifierStatus {
    PolicyRejected,
    DependencyMissing,
    Blocked,
    Passed,
    Failed,
}

impl BuildVerifierStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PolicyRejected => "policy_rejected",
            Self::DependencyMissing => "dependency_missing",
            Self::Blocked => "blocked",
            Self::Passed => "passed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PythonTracebackFrame {
    pub file: String,
    pub line: usize,
    pub function: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PythonTraceback {
    pub final_frame: PythonTracebackFrame,
    pub leading_frames: Vec<PythonTracebackFrame>,
    pub exception_type: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub target_path: Option<String>,
}

impl PythonTraceback {
    pub(crate) fn signature(&self) -> String {
        format!(
            "traceback:{}:{}:{}:{}",
            self.target_path
                .as_deref()
                .unwrap_or(self.final_frame.file.as_str()),
            self.final_frame.line,
            self.exception_type,
            self.message
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::minimal_loop::build_verifier::{BuildVerifierStatus, CompileError};
    use crate::minimal_loop::python_traceback::{PythonTraceback, PythonTracebackFrame};

    fn compile_error(line: usize, column: usize) -> CompileError {
        CompileError {
            path: "src/a.ts".to_string(),
            line,
            column,
            message: "bad type".to_string(),
            excerpt: String::new(),
            symbol: None,
            route_bound: None,
        }
    }

    #[test]
    fn compile_error_location_and_summary_cover_every_position_shape() {
        let table = [
            (3, 5, "src/a.ts:3:5"),
            (3, 0, "src/a.ts:3"),
            (0, 0, "src/a.ts"),
            (0, 5, "src/a.ts"),
        ];
        for (line, column, location) in table {
            let error = compile_error(line, column);
            assert_eq!(error.location(), location);
            assert_eq!(error.summary(), format!("{location} bad type"));
        }
    }

    #[test]
    fn compile_error_serializes_optional_fields_only_when_present() {
        let bare = compile_error(3, 5);
        assert_eq!(
            serde_json::to_string(&bare).unwrap(),
            r#"{"path":"src/a.ts","line":3,"column":5,"message":"bad type"}"#
        );
        let full = CompileError {
            excerpt: "const x = 1;".into(),
            symbol: Some("Widget".into()),
            route_bound: Some(true),
            ..compile_error(3, 5)
        };
        assert_eq!(
            serde_json::to_string(&full).unwrap(),
            r#"{"path":"src/a.ts","line":3,"column":5,"message":"bad type","excerpt":"const x = 1;","symbol":"Widget","route_bound":true}"#
        );
    }

    #[test]
    fn build_verifier_status_wording_and_serialization_match() {
        let table = [
            (BuildVerifierStatus::PolicyRejected, "policy_rejected"),
            (BuildVerifierStatus::DependencyMissing, "dependency_missing"),
            (BuildVerifierStatus::Blocked, "blocked"),
            (BuildVerifierStatus::Passed, "passed"),
            (BuildVerifierStatus::Failed, "failed"),
        ];
        for (status, text) in table {
            assert_eq!(status.as_str(), text);
            assert_eq!(
                serde_json::to_string(&status).unwrap(),
                format!("\"{text}\"")
            );
        }
    }

    fn traceback(target: Option<&str>) -> PythonTraceback {
        PythonTraceback {
            final_frame: PythonTracebackFrame {
                file: "/abs/app.py".to_string(),
                line: 7,
                function: "main".to_string(),
            },
            leading_frames: vec![PythonTracebackFrame {
                file: "/abs/app.py".to_string(),
                line: 2,
                function: "<module>".to_string(),
            }],
            exception_type: "ValueError".to_string(),
            message: "bad value".to_string(),
            target_path: target.map(str::to_string),
        }
    }

    #[test]
    fn python_traceback_signature_prefers_target_path() {
        assert_eq!(
            traceback(Some("app.py")).signature(),
            "traceback:app.py:7:ValueError:bad value"
        );
        assert_eq!(
            traceback(None).signature(),
            "traceback:/abs/app.py:7:ValueError:bad value"
        );
    }

    #[test]
    fn python_traceback_serialization_is_stable_and_round_trips() {
        let mapped = traceback(Some("app.py"));
        let json = serde_json::to_string(&mapped).unwrap();
        assert_eq!(
            json,
            r#"{"final_frame":{"file":"/abs/app.py","line":7,"function":"main"},"leading_frames":[{"file":"/abs/app.py","line":2,"function":"<module>"}],"exception_type":"ValueError","message":"bad value","target_path":"app.py"}"#
        );
        assert_eq!(
            serde_json::from_str::<PythonTraceback>(&json).unwrap(),
            mapped
        );
        let unmapped = traceback(None);
        let json = serde_json::to_string(&unmapped).unwrap();
        assert!(!json.contains("target_path"), "{json}");
        assert_eq!(
            serde_json::from_str::<PythonTraceback>(&json).unwrap(),
            unmapped
        );
    }
}
