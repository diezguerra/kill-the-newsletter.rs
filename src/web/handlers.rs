use askama::Template;
use axum::{
    body::Body,
    extract::{Form, Path, State},
    http::{self, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use tracing::debug;

use crate::database::Pool;
use crate::models::{CreateFeedForm, Entry, Feed, FeedAtomTemplate, NewFeed};
use crate::vars::{email_domain, web_url};
use crate::web::errors::KtnError;

pub async fn create_feed(
    State(pool): State<Pool>,
    form: Form<CreateFeedForm>,
) -> impl IntoResponse {
    debug!("{:?}", form);
    let mut form = NewFeed {
        title: form.title.to_owned(),
        reference: None,
    };
    let redir: String = match form.save(&pool).await {
        Ok(reference) => {
            format!("/feeds/{}.html", reference)
        }
        _ => "/500".to_owned(),
    };

    Redirect::to(&redir)
}

pub async fn get_feed(
    Path(reference): Path<String>,
    State(pool): State<Pool>,
) -> Result<impl IntoResponse, KtnError> {
    match reference {
        rr if reference.ends_with(".html") => {
            get_feed_html(Path(rr), State(pool)).await
        }
        rr if reference.ends_with(".xml") => {
            get_feed_xml(Path(rr), State(pool)).await
        }
        _ => Err(KtnError::NotFoundError),
    }
}

pub async fn get_feed_html(
    Path(reference): Path<String>,
    State(pool): State<Pool>,
) -> Result<Response, KtnError> {
    let no_ext: &str = reference.split(".html").next().unwrap();
    let title = match Feed::get_title_given_reference(no_ext, &pool).await {
        Ok(t) => t,
        _ => {
            debug!("No Feed with reference \"{}\" found.", no_ext);
            return Err(KtnError::NotFoundError);
        }
    };

    let feed = NewFeed {
        reference: Some(no_ext.to_owned()),
        title,
    };

    let template = feed.created_template().render();

    match template {
        Ok(template) => Ok(Response::builder()
            .status(StatusCode::OK)
            .header(
                http::header::CONTENT_TYPE,
                http::HeaderValue::from_static("text/html; charset=utf-8"),
            )
            .body(Body::from(template))
            .unwrap()),
        _ => Err(KtnError::InternalServerError),
    }
}

pub async fn get_feed_xml(
    Path(reference): Path<String>,
    State(pool): State<Pool>,
) -> Result<Response, KtnError> {
    let no_ext: &str = reference.split(".xml").next().unwrap();
    let entries = match Entry::find_by_reference(no_ext, &pool).await {
        Ok(entries) => entries,
        Err(_) => return Err(KtnError::NotFoundError),
    };

    // Since we create "Sentinel" entries on Feed creation, this should never
    // be reached, but just in case.
    if entries.is_empty() {
        return Err(KtnError::NotFoundError);
    }

    let title = match Feed::get_title_given_reference(no_ext, &pool).await {
        Ok(title) => title,
        _ => String::from("No feed title found"),
    };

    let template = FeedAtomTemplate {
        web_url: web_url(),
        email_domain: email_domain(),
        feed_title: title,
        feed_reference: no_ext.to_owned(),
        entries,
    }
    .render();

    match template {
        Ok(template) => Ok(Response::builder()
            .status(StatusCode::OK)
            .header(
                http::header::CONTENT_TYPE,
                http::HeaderValue::from_static(
                    "application/atom+xml; charset=utf-8",
                ),
            )
            .body(Body::from(template))
            .unwrap()),
        _ => Err(KtnError::InternalServerError),
    }
}

pub async fn health() -> impl IntoResponse {
    StatusCode::OK
}

pub async fn get_index() -> impl IntoResponse {
    #[derive(Template)]
    #[template(path = "index.html", ext = "html")]
    struct IndexTemplate {
        pub web_url: String,
    }

    let template = IndexTemplate { web_url: web_url() };

    Response::builder()
        .status(StatusCode::OK)
        .header(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("text/html; charset=utf-8"),
        )
        .body(Body::from(template.render().unwrap_or_else(|_| {
            "Couldn't render IndexTemplate".to_owned()
        })))
        .unwrap()
}
