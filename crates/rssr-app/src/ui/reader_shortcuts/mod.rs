//! Shared Reader input rules and their current host projection.
//! Page session/router assembly stays in `pages::reader_page::shortcuts`.
mod policy;
pub(crate) mod webview;

pub(crate) use policy::{ReaderShortcut, resolve};
