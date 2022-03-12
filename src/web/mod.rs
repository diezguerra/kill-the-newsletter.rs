//! # Web application module
//!
//! Contains the application, and the different handlers for the main requests:
//! * Homepage
//! * Create feed
//! * Render feed in XML
//! * Serve static files (favicons, for now)

mod app;
mod errors;
mod handlers;
pub mod serve_static;

pub use app::build_app;
