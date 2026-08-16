use axum::Json;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

#[derive(Debug)]
pub struct GuiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl GuiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
}

impl IntoResponse for GuiError {
    fn into_response(self) -> Response {
        (
            self.status,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            Json(serde_json::json!({
                "code": self.code,
                "error": self.message,
            })),
        )
            .into_response()
    }
}
