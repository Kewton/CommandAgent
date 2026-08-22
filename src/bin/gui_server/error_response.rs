use axum::Json;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

#[derive(Debug)]
pub struct GuiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    report: Option<serde_json::Value>,
    session_id: Option<String>,
}

impl GuiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            report: None,
            session_id: None,
        }
    }

    /// Attach a diagnostic report to this failure. The `code` and `error` keys
    /// stay byte-identical for every caller that does not use this, so the
    /// additive `report` key never changes an existing response.
    pub fn with_report(mut self, report: serde_json::Value) -> Self {
        self.report = Some(report);
        self
    }

    /// Attach a recoverable session identity without changing the existing
    /// machine-readable code or human-readable error message.
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }
}

impl IntoResponse for GuiError {
    fn into_response(self) -> Response {
        let mut body = serde_json::json!({
            "code": self.code,
            "error": self.message,
        });
        if let Some(report) = self.report
            && let Some(object) = body.as_object_mut()
        {
            object.insert("report".to_string(), report);
        }
        if let Some(session_id) = self.session_id
            && let Some(object) = body.as_object_mut()
        {
            object.insert("session_id".to_string(), session_id.into());
        }
        (
            self.status,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            Json(body),
        )
            .into_response()
    }
}
