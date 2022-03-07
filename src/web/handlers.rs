use askama::Template;
use axum::{
    body,
    extract::{Extension, Path},
    http::{self, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use r2d2_sqlite::SqliteConnectionManager;
use std::sync::Arc;

use crate::models::{Entry, Feed, FeedAtomTemplate, NewFeed};
use crate::vars::{EMAIL_DOMAIN, WEB_URL};
use crate::web::errors::KtnError;

pub async fn create_feed(
    item: Json<NewFeed>,
    Extension(pool_arc): Extension<Arc<r2d2::Pool<SqliteConnectionManager>>>,
) -> impl IntoResponse {
    let pool = pool_arc.clone();
    let mut conn = pool.get().expect("Couldn't get database connection");
    println!("{:?}", item);
    item.save(&mut conn)
}

pub async fn get_feed(
    Path(reference): Path<String>,
    Extension(pool_arc): Extension<Arc<r2d2::Pool<SqliteConnectionManager>>>,
) -> Result<impl IntoResponse, KtnError> {
    // This block needs to be here cause axum's use of the `matchit` crate
    // is too fast to be flexible
    let no_ext_ref: String = String::from({
        if reference.ends_with(".xml") {
            &reference[..reference.len() - 4]
        } else {
            &reference
        }
    });

    let pool = pool_arc.clone();
    let mut conn = pool.get().expect("Couldn't get database connection");
    let entries = match Entry::find_by_reference(&no_ext_ref, &mut conn) {
        Ok(entries) => entries,
        Err(_) => return Err(KtnError::InternalServerError),
    };

    if entries.len() == 0 {
        return Err(KtnError::NotFoundError);
    }

    let title = match Feed::get_title_given_reference(&reference, &mut conn) {
        Ok(title) => title,
        _ => String::from("No feed title found"),
    };

    let feed = FeedAtomTemplate {
        web_url: Box::new(String::from(WEB_URL)),
        email_domain: Box::new(String::from(EMAIL_DOMAIN)),
        feed_title: Box::new(title),
        feed_reference: Box::new(reference),
        entries: entries,
    }
    // Rendering and building a response manually because it's the only
    // half sane way to inject a content-type header
    // -- askama_axum + ext on template is not enough (yields text/xml)
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
