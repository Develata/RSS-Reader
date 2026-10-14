//! v0.2 library storage on rusqlite: one `library.sqlite3` per library
//! (docs/design/v0.2-architecture.md).
//!
//! `connection`, `migrate` and `error` are target-neutral so the Pages demo can reuse them on
//! wasm32; `executor` owns native connections. Native-only until phase D.

mod codec;
mod connection;
mod documents;
mod entries;
mod error;
mod executor;
mod feeds;
mod files;
pub mod import;
pub mod local;
pub mod migrate;
mod mutations;
mod refresh;

pub use documents::{LibraryAppState, LibrarySettings};
pub use entries::{LibraryEntries, LocalizedBody, compute_entry_content_hash};
pub use error::{StoreError, StoreResult};
pub use executor::{LibraryStore, StoreOptions};
pub use feeds::LibraryFeeds;
pub use mutations::LibraryMutations;
pub use refresh::LibraryRefreshStore;
