use askama::Template;
use axum::{
    self, body,
    extract::{Extension, Path},
    http::{self, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    AddExtensionLayer, Json, Router,
};
use r2d2_sqlite::SqliteConnectionManager;
use std::sync::Arc;

use crate::models::entry::{find_reference, Entry};
use crate::models::feed::{get_title_by_reference, NewFeed};
use crate::time::filters;
use crate::vars::{EMAIL_DOMAIN, WEB_URL};

pub fn build_app(
    pool_arc: Arc<r2d2::Pool<SqliteConnectionManager>>,
) -> axum::routing::IntoMakeService<Router> {
    Router::new()
        .route("/", post(create_feed))
        .route("/:reference", get(get_reference))
        .layer(AddExtensionLayer::new(pool_arc))
        .into_make_service()
}

async fn create_feed(
    item: Json<NewFeed>,
    Extension(pool_arc): Extension<Arc<r2d2::Pool<SqliteConnectionManager>>>,
) -> impl IntoResponse {
    let pool = pool_arc.clone();
    let mut conn = pool.get().expect("Couldn't get database connection");
    println!("{:?}", item);
    item.save(&mut conn)
}

#[derive(Template)]
#[template(path = "atom.xml", ext = "xml")]
struct AtomTemplate {
    web_url: Box<String>,
    email_domain: Box<String>,
    feed_title: Box<String>,
    feed_reference: Box<String>,
    entries: Vec<Entry>,
}

async fn get_reference(
    Path(reference): Path<String>,
    Extension(pool_arc): Extension<Arc<r2d2::Pool<SqliteConnectionManager>>>,
) -> Result<impl IntoResponse, KtnError> {
    // This block needs to be here cause matchit is too fast to be flexible
    let no_ext_ref: String = String::from({
        if reference.ends_with(".xml") {
            &reference[..reference.len() - 4]
        } else {
            &reference
        }
    });

    let pool = pool_arc.clone();
    let mut conn = pool.get().expect("Couldn't get database connection");
    let entries = match find_reference(&no_ext_ref, &mut conn) {
        Ok(entries) => entries,
        Err(_) => return Err(KtnError::InternalServerError),
    };

    if entries.len() == 0 {
        return Err(KtnError::NotFoundError);
    }

    let title = match get_title_by_reference(&reference, &mut conn) {
        Ok(title) => title,
        _ => String::from("No feed title found"),
    };

    let feed = AtomTemplate {
        web_url: Box::new(String::from(WEB_URL)),
        email_domain: Box::new(String::from(EMAIL_DOMAIN)),
        feed_title: Box::new(title),
        feed_reference: Box::new(reference),
        entries: entries,
    }
    .render()
    .unwrap_or(String::from("Couldn't render Atom feed template"));

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("application/atom+xml"),
        )
        .body(body::boxed(body::Full::from(feed)))
        .unwrap())
}

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
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        Response::builder().status(status).body(body).unwrap()
    }
}
