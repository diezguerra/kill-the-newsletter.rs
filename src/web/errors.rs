use axum::{
    body,
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
        let body = match self {
            KtnError::NotFoundError => {
                body::boxed(body::Full::from("Not Found"))
            }
            _ => body::boxed(body::Full::from("Undertermined error")),
        };

        let status = match self {
            KtnError::NotFoundError => StatusCode::NOT_FOUND,
            KtnError::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
        };

        Response::builder().status(status).body(body).unwrap()
    }
}
