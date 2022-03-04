use askama::Template;
use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    AddExtensionLayer, Json, Router,
};
use r2d2_sqlite::SqliteConnectionManager;
use std::sync::Arc;

use crate::models::entry::{find_reference, Entry};
use crate::models::feed::{get_title_by_reference, NewFeed};
use crate::time::filters;
use crate::vars::{EMAIL_DOMAIN, WEB_URL};

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
) -> (StatusCode, impl IntoResponse) {
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
        Err(_) => return (StatusCode::NOT_FOUND, String::from("Not found")),
    };

    if entries.len() == 0 {
        return (StatusCode::NOT_FOUND, String::from("Not found"));
    }

    let title = match get_title_by_reference(&reference, &mut conn) {
        Ok(title) => title,
        _ => String::from("No feed title found"),
    };

    (
        StatusCode::OK,
        AtomTemplate {
            web_url: Box::new(String::from(WEB_URL)),
            email_domain: Box::new(String::from(EMAIL_DOMAIN)),
            feed_title: Box::new(title),
            feed_reference: Box::new(reference),
            entries: entries,
        }
        .render()
        .expect("Failed to render Atom"),
    )
}

pub fn build_app(
    pool_arc: Arc<r2d2::Pool<SqliteConnectionManager>>,
) -> axum::routing::IntoMakeService<Router> {
    Router::new()
        .route("/", post(create_feed))
        .route("/:reference", get(get_reference))
        .layer(AddExtensionLayer::new(pool_arc))
        .into_make_service()
}
