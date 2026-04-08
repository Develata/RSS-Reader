#[cfg(target_arch = "wasm32")]
pub mod browser;
#[cfg(not(target_arch = "wasm32"))]
mod non_refresh;
#[cfg(not(target_arch = "wasm32"))]
mod refresh;

#[cfg(not(target_arch = "wasm32"))]
pub use non_refresh::{InfraOpmlCodec, SqliteAppStateAdapter};
#[cfg(not(target_arch = "wasm32"))]
pub use refresh::{InfraFeedRefreshSource, SqliteRefreshStore};
