use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

pub struct ServerError {
    pub message: String,
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let body = Json(json!({
            "error": self.message,
        }));

        (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
    }
}

pub fn to_server_error<T: ToString>(err: T) -> ServerError {
    ServerError {
        message: err.to_string(),
    }
}
