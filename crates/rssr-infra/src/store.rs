//! v0.2 library storage on rusqlite: one `library.sqlite3` per library
//! (docs/design/v0.2-architecture.md).
//!
//! `connection`, `migrate` and `error` are target-neutral so the Pages demo can reuse them on
//! wasm32; `executor` owns native connections. Native-only until phase D.

#[cfg(not(target_arch = "wasm32"))]
pub mod backup;
mod codec;
mod connection;
mod documents;
mod entries;
mod error;
#[cfg(not(target_arch = "wasm32"))]
mod executor;
#[cfg(target_arch = "wasm32")]
#[path = "store/wasm_executor.rs"]
mod executor;
mod feeds;
#[cfg(not(target_arch = "wasm32"))]
mod files;
/// Libraries for the browser regression (test tooling only).
#[cfg(not(target_arch = "wasm32"))]
pub mod fixture;
#[cfg(not(target_arch = "wasm32"))]
pub mod import;
#[cfg(not(target_arch = "wasm32"))]
pub mod local;
pub mod migrate;
mod mutations;
mod options;
mod queries;
mod refresh;
mod workspace_edits;
pub use workspace_edits::LibraryWorkspaceEdits;

pub use documents::{LibraryAppState, LibrarySettings};
pub use entries::{LibraryEntries, LocalizedBody, compute_entry_content_hash};
pub use error::{StoreError, StoreResult};
pub use executor::{LibraryStore, StoreOptions};
pub use feeds::LibraryFeeds;
pub use mutations::LibraryMutations;
pub use queries::LibraryQueries;
pub use refresh::LibraryRefreshStore;
