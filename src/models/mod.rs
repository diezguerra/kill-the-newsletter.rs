//! # Application classes / DAOs
//!
//! Contains classes reprenting the models / DAOs to be used by both the
//! web application and the SMTP server.

mod entry;
mod feed;
mod feed_template;

pub use entry::Entry;
pub use feed::{CreateFeedForm, Feed, NewFeed};
pub use feed_template::FeedAtomTemplate;
