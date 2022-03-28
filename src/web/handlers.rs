use askama::Template;
use axum::{
    body,
    extract::{Extension, Form, Path},
    http::{self, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use tracing::debug;

use crate::database::Pool;
use crate::models::{Entry, Feed, FeedAtomTemplate, NewFeed};
use crate::vars::{EMAIL_DOMAIN, WEB_URL};
use crate::web::errors::KtnError;

pub async fn create_feed(
    form: Form<NewFeed>,
    Extension(pool): Extension<Pool>,
) -> impl IntoResponse {
    println!("{:?}", form);
    let mut form = NewFeed {
        title: form.title.to_owned(),
        reference: form.reference.to_owned(),
    };
    let redir: String = match form.save(&pool).await {
        Ok(reference) => {
            format!("/feeds/{}.html", reference)
        }
        _ => "/500".to_owned(),
    };

    Redirect::to(redir.parse().unwrap())
}

pub async fn get_feed(
    Path(reference): Path<String>,
    Extension(pool): Extension<Pool>,
) -> Result<impl IntoResponse, KtnError> {
    match reference {
        rr if reference.ends_with(".html") => {
            get_feed_html(Path(rr), Extension(pool)).await
        }
        rr if reference.ends_with(".xml") => {
            get_feed_xml(Path(rr), Extension(pool)).await
        }
        _ => Err(KtnError::NotFoundError),
    }
}

pub async fn get_feed_html(
    Path(reference): Path<String>,
    Extension(pool): Extension<Pool>,
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
            .body(body::boxed(body::Full::from(template)))
            .unwrap()),
        _ => Err(KtnError::InternalServerError),
    }
}

pub async fn get_feed_xml(
    Path(reference): Path<String>,
    Extension(pool): Extension<Pool>,
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
        web_url: String::from(WEB_URL),
        email_domain: String::from(EMAIL_DOMAIN),
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
            .body(body::boxed(body::Full::from(template)))
            .unwrap()),
        _ => Err(KtnError::InternalServerError),
    }
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
