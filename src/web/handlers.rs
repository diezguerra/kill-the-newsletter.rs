use askama::Template;
use axum::{
    body::Body,
    extract::{Form, Path, State},
    http::{self, HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use tracing::debug;

use crate::database::Pool;
use crate::models::{CreateFeedForm, Entry, Feed, FeedAtomTemplate, NewFeed};
use crate::vars::{email_domain, web_url};
use crate::web::errors::KtnError;

/// Maximum allowed length (in characters) for a feed's `title`, matching
/// the client-side `maxlength` on the create-feed form. Enforced here too
/// since the client-side check is trivially bypassed with a direct POST.
const MAX_TITLE_LEN: usize = 500;

/// Validates a user-supplied feed title. Rejects empty/whitespace-only
/// titles and titles longer than [`MAX_TITLE_LEN`] characters.
fn validate_title(title: &str) -> Result<(), KtnError> {
    if title.trim().is_empty() {
        return Err(KtnError::BadRequest("Title must not be empty".to_owned()));
    }

    if title.chars().count() > MAX_TITLE_LEN {
        return Err(KtnError::BadRequest(format!(
            "Title must not exceed {} characters",
            MAX_TITLE_LEN
        )));
    }

    Ok(())
}

pub async fn create_feed(
    State(pool): State<Pool>,
    form: Form<CreateFeedForm>,
) -> Result<impl IntoResponse, KtnError> {
    debug!("{:?}", form);

    validate_title(&form.title)?;

    let mut form = NewFeed {
        title: form.title.to_owned(),
        reference: None,
    };
    let reference = form.save(&pool).await.map_err(|e| {
        debug!("Couldn't save new feed: {}", e);
        KtnError::InternalServerError
    })?;

    Ok(Redirect::to(&format!("/feeds/{}.html", reference)))
}

pub async fn get_feed(
    Path(reference): Path<String>,
    headers: HeaderMap,
    State(pool): State<Pool>,
) -> Result<impl IntoResponse, KtnError> {
    match reference {
        rr if reference.ends_with(".html") => {
            get_feed_html(Path(rr), State(pool)).await
        }
        rr if reference.ends_with(".xml") => {
            get_feed_xml(Path(rr), headers, State(pool)).await
        }
        _ => Err(KtnError::NotFoundError),
    }
}

/// How long Cloudflare (and any other shared/browser cache) may serve a
/// feed response without checking back with us. Feed content only changes
/// when mail arrives, so a few minutes of staleness is invisible to readers
/// but lets the edge absorb the bulk of feed-reader polling instead of it
/// all hitting the origin machine.
const FEED_CACHE_CONTROL: &str =
    "public, max-age=300, stale-while-revalidate=3600";

/// Cheap identity for a feed's current content, built entirely from data
/// the caller already has in hand (no extra query). `entries` is ordered
/// newest-first, so its length plus the newest row's id changes whenever
/// mail is added or the oldest entries are trimmed.
fn feed_etag(reference: &str, entries: &[Entry]) -> String {
    format!(
        "\"{}-{}-{}\"",
        reference,
        entries.len(),
        entries.first().map(|e| e.id).unwrap_or(0)
    )
}

/// Whether the request's `If-None-Match` already names this ETag, i.e.
/// whether we can answer with a bodyless 304 instead of re-rendering.
fn etag_matches(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get(http::header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|part| part.trim() == etag))
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
    headers: HeaderMap,
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

    let etag = feed_etag(no_ext, &entries);

    if etag_matches(&headers, &etag) {
        return Ok(Response::builder()
            .status(StatusCode::NOT_MODIFIED)
            .header(
                http::header::ETAG,
                http::HeaderValue::from_str(&etag).unwrap(),
            )
            .header(
                http::header::CACHE_CONTROL,
                http::HeaderValue::from_static(FEED_CACHE_CONTROL),
            )
            .body(Body::empty())
            .unwrap());
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
            .header(
                http::header::ETAG,
                http::HeaderValue::from_str(&etag).unwrap(),
            )
            .header(
                http::header::CACHE_CONTROL,
                http::HeaderValue::from_static(FEED_CACHE_CONTROL),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_title_accepts_normal_length_title() {
        assert!(validate_title("My Newsletter").is_ok());
    }

    #[test]
    fn validate_title_rejects_empty_title() {
        let err = validate_title("").unwrap_err();
        assert!(matches!(err, KtnError::BadRequest(_)));
    }

    #[test]
    fn validate_title_rejects_whitespace_only_title() {
        let err = validate_title("   ").unwrap_err();
        assert!(matches!(err, KtnError::BadRequest(_)));
    }

    #[test]
    fn validate_title_accepts_title_of_exactly_max_len() {
        let title = "a".repeat(MAX_TITLE_LEN);
        assert!(validate_title(&title).is_ok());
    }

    #[test]
    fn validate_title_rejects_title_over_max_len() {
        let title = "a".repeat(MAX_TITLE_LEN + 1);
        let err = validate_title(&title).unwrap_err();
        assert!(matches!(err, KtnError::BadRequest(_)));
    }

    fn make_entry(id: i32) -> Entry {
        Entry {
            id,
            created_at: "2024-01-01 00:00:00".to_owned(),
            reference: "ref".to_owned(),
            title: "t".to_owned(),
            author: "a".to_owned(),
            content: "c".to_owned(),
        }
    }

    #[test]
    fn feed_etag_stable_for_same_entries() {
        let entries = vec![make_entry(2), make_entry(1)];
        assert_eq!(feed_etag("ref", &entries), feed_etag("ref", &entries));
    }

    #[test]
    fn feed_etag_changes_when_newest_entry_id_changes() {
        let before = feed_etag("ref", &[make_entry(2), make_entry(1)]);
        let after =
            feed_etag("ref", &[make_entry(3), make_entry(2), make_entry(1)]);
        assert_ne!(before, after);
    }

    #[test]
    fn feed_etag_changes_when_entry_count_changes_without_new_id() {
        // Oldest entry trimmed away: newest id unchanged, count drops.
        let before =
            feed_etag("ref", &[make_entry(3), make_entry(2), make_entry(1)]);
        let after = feed_etag("ref", &[make_entry(3), make_entry(2)]);
        assert_ne!(before, after);
    }

    #[test]
    fn etag_matches_true_when_header_equals_etag() {
        let mut headers = HeaderMap::new();
        headers.insert(
            http::header::IF_NONE_MATCH,
            http::HeaderValue::from_static("\"ref-2-2\""),
        );
        assert!(etag_matches(&headers, "\"ref-2-2\""));
    }

    #[test]
    fn etag_matches_false_when_header_differs() {
        let mut headers = HeaderMap::new();
        headers.insert(
            http::header::IF_NONE_MATCH,
            http::HeaderValue::from_static("\"ref-1-1\""),
        );
        assert!(!etag_matches(&headers, "\"ref-2-2\""));
    }

    #[test]
    fn etag_matches_false_when_header_absent() {
        assert!(!etag_matches(&HeaderMap::new(), "\"ref-2-2\""));
    }

    #[test]
    fn etag_matches_true_when_one_of_several_values_matches() {
        let mut headers = HeaderMap::new();
        headers.insert(
            http::header::IF_NONE_MATCH,
            http::HeaderValue::from_static("\"other-1-1\", \"ref-2-2\""),
        );
        assert!(etag_matches(&headers, "\"ref-2-2\""));
    }
}

/// Exercises the real `get_feed_xml` handler against Postgres. Only the
/// 304 short-circuit is covered here: it's the one branch that runs before
/// `web_url()`/`email_domain()` are read, so it doesn't need WEB_URL /
/// EMAIL_DOMAIN set (unlike the 200 branch — see the note on `insert_feed`
/// in `models::entry`'s tests for why we avoid that dependency).
#[cfg(test)]
mod not_modified_tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

    async fn test_pool() -> Option<Pool> {
        let url = std::env::var("DATABASE_URL").ok()?;
        let pool = PgPoolOptions::new().connect(&url).await.ok()?;
        sqlx::migrate!("./migrations").run(&pool).await.ok()?;
        Some(pool)
    }

    async fn insert_feed(reference: &str, pool: &Pool) {
        sqlx::query(
            r#"INSERT INTO "feeds" ("reference", "title") VALUES ($1, $2)
               ON CONFLICT (reference) DO NOTHING"#,
        )
        .bind(reference)
        .bind("ETag test feed")
        .execute(pool)
        .await
        .expect("test feed insert should succeed");
    }

    #[tokio::test]
    async fn get_feed_xml_returns_304_when_if_none_match_is_current() {
        let Some(pool) = test_pool().await else {
            eprintln!("skipping: no DATABASE_URL / DB unreachable");
            return;
        };

        let reference = format!(
            "etagtest{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        insert_feed(&reference, &pool).await;

        Entry {
            id: 0,
            created_at: "2024-01-01 00:00:00".to_owned(),
            reference: reference.clone(),
            title: "Only entry".to_owned(),
            author: "sender@example.com".to_owned(),
            content: "hello".to_owned(),
        }
        .save(&pool)
        .await
        .expect("entry should save");

        let entries = Entry::find_by_reference(&reference, &pool)
            .await
            .expect("find_by_reference should succeed");
        let etag = feed_etag(&reference, &entries);

        let mut headers = HeaderMap::new();
        headers.insert(
            http::header::IF_NONE_MATCH,
            http::HeaderValue::from_str(&etag).unwrap(),
        );

        let response = get_feed_xml(
            Path(format!("{}.xml", reference)),
            headers,
            State(pool),
        )
        .await
        .expect("handler should succeed");

        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    }
}
