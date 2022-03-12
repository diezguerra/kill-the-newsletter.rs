use askama::Template;
use axum::{
    body,
    extract::{Extension, Form, Path},
    http::{self, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use r2d2_sqlite::SqliteConnectionManager;
use std::sync::Arc;
use tracing::error;

use crate::models::{Entry, Feed, FeedAtomTemplate, NewFeed};
use crate::vars::{EMAIL_DOMAIN, WEB_URL};
use crate::web::errors::KtnError;

pub async fn create_feed(
    form: Form<NewFeed>,
    Extension(pool_arc): Extension<Arc<r2d2::Pool<SqliteConnectionManager>>>,
) -> impl IntoResponse {
    let mut conn = pool_arc.get().expect("Couldn't get database connection");
    println!("{:?}", form);
    let mut form = NewFeed {
        title: form.title.to_owned(),
        reference: form.reference.to_owned(),
    };
    let reference: String = form.save(&mut conn);

    Redirect::to(format!("/{}", reference).parse().unwrap())
}

pub async fn get_feed_created(
    Path(reference): Path<String>,
    Extension(pool_arc): Extension<Arc<r2d2::Pool<SqliteConnectionManager>>>,
) -> Result<impl IntoResponse, KtnError> {
    let mut conn = pool_arc.get().expect("Couldn't get database connection");

    let title = Feed::get_title_given_reference(&reference, &mut conn);
    if let Err(rusqlite::Error::QueryReturnedNoRows) = title {
        error!(
            concat!(
                "web::handlers::get_feed_created couldn't ",
                "get_title_given_reference  \"{}\""
            ),
            &reference
        );
        return Err(KtnError::NotFoundError);
    }

    let feed = NewFeed {
        reference: Some(reference),
        title: title.unwrap(),
    };

    let template = feed
        .created_template()
        .render()
        .unwrap_or_else(|_| String::from("Couldn't render Atom feed template"));

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("text/html; charset=utf-8"),
        )
        .body(body::boxed(body::Full::from(template)))
        .unwrap())
}

pub async fn get_feed(
    Path(reference): Path<String>,
    Extension(pool_arc): Extension<Arc<r2d2::Pool<SqliteConnectionManager>>>,
) -> Result<impl IntoResponse, KtnError> {
    let mut conn = pool_arc.get().expect("Couldn't get database connection");

    let no_ext: &str = &reference[..reference.len() - 4];
    let entries = match Entry::find_by_reference(no_ext, &mut conn) {
        Ok(entries) => entries,
        Err(_) => return Err(KtnError::InternalServerError),
    };

    if entries.is_empty() {
        return Err(KtnError::NotFoundError);
    }

    let title = match Feed::get_title_given_reference(no_ext, &mut conn) {
        Ok(title) => title,
        _ => String::from("No feed title found"),
    };

    let template = FeedAtomTemplate {
        web_url: String::from(WEB_URL),
        email_domain: String::from(EMAIL_DOMAIN),
        feed_title: title,
        feed_reference: no_ext.to_owned(),
        entries,
    }
    .render()
    .unwrap_or_else(|_| String::from("Couldn't render created feed template"));

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static(
                "application/atom+xml; charset=utf-8",
            ),
        )
        .body(body::boxed(body::Full::from(template)))
        .unwrap())
}

pub async fn get_index() -> impl IntoResponse {
    #[derive(Template)]
    #[template(path = "index.html", ext = "html")]
    struct IndexTemplate {
        pub web_url: String,
    }

    IndexTemplate {
        web_url: String::from(WEB_URL),
    }
}
