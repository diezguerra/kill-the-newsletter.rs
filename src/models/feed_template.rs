//! Representation of the Atom XML template to be rendered

use askama_axum::Template;

use crate::models::Entry;
use crate::time::filters;

#[derive(Template)]
#[template(path = "atom.xml", ext = "xml")]
pub struct FeedAtomTemplate {
    pub web_url: String,
    pub email_domain: String,
    pub feed_title: String,
    pub feed_reference: String,
    pub entries: Vec<Entry>,
}
