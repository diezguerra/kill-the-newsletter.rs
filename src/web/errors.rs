use axum::{
    body::Body,
    http::StatusCode,
    response::{IntoResponse, Response},
};

#[derive(Debug)]
pub enum KtnError {
    NotFoundError,
    InternalServerError,
}

impl std::fmt::Display for KtnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for KtnError {}

impl IntoResponse for KtnError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            KtnError::NotFoundError => {
                (StatusCode::NOT_FOUND, Body::from("Not Found"))
            }
            KtnError::InternalServerError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Body::from("Undetermined error"),
            ),
        };

        Response::builder().status(status).body(body).unwrap()
    }
}
