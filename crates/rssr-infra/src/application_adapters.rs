#[cfg(all(target_arch = "wasm32", feature = "browser-store"))]
pub mod browser;
#[cfg(not(target_arch = "wasm32"))]
mod non_refresh;
#[cfg(not(target_arch = "wasm32"))]
mod refresh;

#[cfg(not(target_arch = "wasm32"))]
pub use non_refresh::InfraOpmlCodec;
#[cfg(not(target_arch = "wasm32"))]
pub use refresh::InfraFeedRefreshSource;
