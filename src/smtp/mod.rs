//! # SMTP server module
//!
//! Couldn't figure out how to Build a small server with Samotop or others,
//! so in an effort to reduce complexity and learn more about Tokio I ended
//! up building a small server myself off the TcpStream using a state machine
//! build with Enums and matching. Mercy, I implore.

pub mod app;
mod parse;
pub mod state_machine;
