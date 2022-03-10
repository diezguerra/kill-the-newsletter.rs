//! Usage:
//! Setting `STATIC_FOLDER` in OS env or `.env` file such as
//! `STATIC_FOLDER=name_of_folder`
//! ```
//! let app = Router::new().nest("/static", get(static))
//! ```
//! 
//! This was yanked from <https://github.com/tokio-rs/axum/discussions/446>

use axum::{
    body::{boxed, Body, BoxBody},
    http::{Request, Response, StatusCode, Uri},
};
use tower::ServiceExt;
use tower_http::services::ServeDir;

use crate::vars::STATIC_FOLDER;

pub async fn handler(
    uri: Uri,
) -> Result<Response<BoxBody>, (StatusCode, String)> {
    let res = get_static_file(uri.clone()).await?;

    if res.status() == StatusCode::NOT_FOUND {
        // try with `.html`
        // TODO: handle if the Uri has query parameters
        match format!("{}.html", uri).parse() {
            Ok(uri_html) => get_static_file(uri_html).await,
            Err(_) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Invalid URI".to_string(),
            )),
        }
    } else {
        Ok(res)
    }
}

async fn get_static_file(
    uri: Uri,
) -> Result<Response<BoxBody>, (StatusCode, String)> {
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();

    // `ServeDir` implements `tower::Service` so,
    // we can call it with `tower::ServiceExt::oneshot`
    match ServeDir::new(STATIC_FOLDER).oneshot(req).await {
        Ok(res) => Ok(res.map(boxed)),
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", err),
        )),
    }
}
