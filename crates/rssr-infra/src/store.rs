//! v0.2 library storage on rusqlite: one `library.sqlite3` per library
//! (docs/design/v0.2-architecture.md).
//!
//! `connection`, `migrate` and `error` are target-neutral so the Pages demo can reuse them on
//! wasm32; `executor` owns native connections. Native-only until phase D.

mod connection;
mod error;
mod executor;
pub mod migrate;

pub use error::{StoreError, StoreResult};
pub use executor::{LibraryStore, StoreOptions};
