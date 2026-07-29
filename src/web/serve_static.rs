//! # Static file serving handler
//!
//! Usage:
//! Setting `STATIC_FOLDER` in OS env or `.env` file such as
//! `STATIC_FOLDER=name_of_folder`
//! ```
//! let app = Router::new().nest("/static", get(static))
//! ```
//!
//! This was yanked from <https://github.com/tokio-rs/axum/discussions/446>

use axum::{
    body::Body,
    http::{
        uri::{PathAndQuery, Uri},
        Request, Response, StatusCode,
    },
};
use tower::ServiceExt;
use tower_http::services::ServeDir;

use crate::vars::static_folder;

pub async fn handler(uri: Uri) -> Result<Response<Body>, (StatusCode, String)> {
    let res = get_static_file(uri.clone()).await?;

    if res.status() == StatusCode::NOT_FOUND {
        // try with `.html`, appended to the path only so any query string
        // isn't dragged along into the filename we look up on disk.
        let html_path_and_query = match uri.query() {
            Some(query) => format!("{}.html?{}", uri.path(), query),
            None => format!("{}.html", uri.path()),
        };

        match html_path_and_query.parse::<PathAndQuery>() {
            Ok(path_and_query) => {
                let mut parts = uri.into_parts();
                parts.path_and_query = Some(path_and_query);
                match Uri::from_parts(parts) {
                    Ok(uri_html) => get_static_file(uri_html).await,
                    Err(_) => Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Invalid URI".to_string(),
                    )),
                }
            }
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
) -> Result<Response<Body>, (StatusCode, String)> {
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();

    // `ServeDir` implements `tower::Service` so we can call it with
    // `tower::ServiceExt::oneshot`
    match ServeDir::new(static_folder()).oneshot(req).await {
        Ok(res) => Ok(res.map(Body::new)),
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", err),
        )),
    }
}
