use askama_axum::Template;

use crate::models::Entry;
use crate::time::filters;

#[derive(Template)]
#[template(path = "atom.xml", ext = "xml")]
pub struct FeedAtomTemplate {
    pub web_url: Box<String>,
    pub email_domain: Box<String>,
    pub feed_title: Box<String>,
    pub feed_reference: Box<String>,
    pub entries: Vec<Entry>,
}
